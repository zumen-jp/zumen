//! IDS の XML を読む。XSD は同梱せず、IDS 1.0 の文書が書く要素と属性だけを読む

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Element, Node};

use crate::pattern::Pattern;
use crate::{
    Cardinality, Constraint, Facet, Ids, Occurs, Relation, Restriction, Specification, Value,
};

/// IDS の名前空間
pub(crate) const NAMESPACE: &str = "http://standards.buildingsmart.org/IDS";
/// XML Schema の名前空間（`xs:restriction`）
const XS: &str = "http://www.w3.org/2001/XMLSchema";

/// 読めない場所。要素の範囲と、診断の値
struct Bad {
    span: core::ops::Range<usize>,
    near: String,
}

impl Ids {
    /// IDS を読む。XML として読めないか、IDS の形でなければ `ids.file.invalid` を報告して失敗する。
    /// 知らない facet と制約は、`ids.facet.unsupported`・`ids.restriction.unsupported` を報告して、
    /// その facet を確かめられないものとして残す
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<Self, Error> {
        let doc = zumen_xml::read(src, cx)?;
        let mut reader = Reader { doc: &doc, cx };
        match reader.ids() {
            Ok(Ok(ids)) => Ok(ids),
            Ok(Err(bad)) => {
                reader.cx.report(
                    Diagnostic::new(codes::IDS_FILE_INVALID)
                        .with("near", bad.near)
                        .at(Ref::at_bytes(bad.span.start as u64, bad.span.end as u64)),
                );
                Err(Error::Invalid)
            }
            Err(error) => Err(error),
        }
    }
}

struct Reader<'d, 'c, 'b> {
    doc: &'d Document,
    cx: &'c mut Cx<'b>,
}

type Read<T> = Result<Result<T, Bad>, Error>;

impl<'d> Reader<'d, '_, '_> {
    fn local(&self, element: &Element) -> &'d [u8] {
        self.doc.local_name(element.name())
    }

    fn is_ids(&self, element: &Element, local: &[u8]) -> bool {
        self.doc.is(element.name(), Some(NAMESPACE), local)
    }

    fn bad(&self, element: &Element) -> Bad {
        Bad {
            span: element.span(),
            near: String::from_utf8_lossy(self.local(element)).into_owned(),
        }
    }

    /// 子の要素
    fn children(&self, element: &Element) -> Vec<&'d Element> {
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Element(index) => self.doc.element(*index),
                _ => None,
            })
            .collect()
    }

    /// 要素の中の文字。文字と CDATA を、書いた順につなぐ
    fn text(&self, element: &Element) -> String {
        let mut out = String::new();
        for node in element.children() {
            match node {
                Node::Text(range) => out.push_str(&self.doc.text(range.clone())),
                Node::CData(range) => out.push_str(&self.doc.cdata(range.clone())),
                Node::Element(_) => {}
            }
        }
        out
    }

    fn attribute(&self, element: &Element, local: &[u8]) -> Option<String> {
        self.doc
            .attribute(element, None, local)
            .map(|attribute| self.doc.value(attribute))
    }

    fn work(&mut self, element: &Element) -> Result<(), Error> {
        self.cx.check(Kind::Work, 1).map_err(|stop| {
            let span = element.span();
            self.cx.report(
                stop.to_diagnostic()
                    .at(Ref::at_bytes(span.start as u64, span.end as u64)),
            );
            Error::Stopped(stop)
        })
    }

    fn ids(&mut self) -> Read<Ids> {
        let Some(root) = self.doc.element(self.doc.root()) else {
            return Ok(Err(Bad {
                span: 0..0,
                near: String::new(),
            }));
        };
        if !self.is_ids(root, b"ids") {
            return Ok(Err(self.bad(root)));
        }
        let mut ids = Ids {
            title: None,
            specifications: Vec::new(),
        };
        let mut found = false;
        for child in self.children(root) {
            if self.is_ids(child, b"info") {
                ids.title = self
                    .children(child)
                    .into_iter()
                    .find(|e| self.is_ids(e, b"title"))
                    .map(|e| self.text(e));
            } else if self.is_ids(child, b"specifications") {
                found = true;
                for spec in self.children(child) {
                    if !self.is_ids(spec, b"specification") {
                        return Ok(Err(self.bad(spec)));
                    }
                    match self.specification(spec)? {
                        Ok(spec) => ids.specifications.push(spec),
                        Err(bad) => return Ok(Err(bad)),
                    }
                }
            } else {
                return Ok(Err(self.bad(child)));
            }
        }
        if !found || ids.specifications.is_empty() {
            return Ok(Err(self.bad(root)));
        }
        Ok(Ok(ids))
    }

    fn specification(&mut self, element: &Element) -> Read<Specification> {
        self.work(element)?;
        let Some(name) = self.attribute(element, b"name") else {
            return Ok(Err(self.bad(element)));
        };
        let versions: Vec<String> = self
            .attribute(element, b"ifcVersion")
            .unwrap_or_default()
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect();
        if versions.is_empty() {
            return Ok(Err(self.bad(element)));
        }
        let mut spec = Specification {
            name,
            identifier: self.attribute(element, b"identifier"),
            versions,
            occurs: Occurs::default(),
            applicability: Vec::new(),
            requirements: Vec::new(),
            span: element.span(),
        };
        let mut seen_applicability = false;
        for child in self.children(element) {
            let applicability = self.is_ids(child, b"applicability");
            if !applicability && !self.is_ids(child, b"requirements") {
                return Ok(Err(self.bad(child)));
            }
            if applicability {
                if seen_applicability {
                    return Ok(Err(self.bad(child)));
                }
                seen_applicability = true;
                spec.occurs = match self.occurs(child) {
                    Some(occurs) => occurs,
                    None => return Ok(Err(self.bad(child))),
                };
            }
            for facet in self.children(child) {
                let facet = match self.facet(facet)? {
                    Ok(facet) => facet,
                    Err(bad) => return Ok(Err(bad)),
                };
                if applicability {
                    spec.applicability.push(facet);
                } else {
                    spec.requirements.push(facet);
                }
            }
        }
        // 禁じる仕様は、要件を持たない
        if !seen_applicability || (spec.occurs.max == Some(0) && !spec.requirements.is_empty()) {
            return Ok(Err(self.bad(element)));
        }
        Ok(Ok(spec))
    }

    /// `minOccurs`・`maxOccurs`。書かなければ 1 と `unbounded`
    fn occurs(&self, element: &Element) -> Option<Occurs> {
        let min = match self.attribute(element, b"minOccurs") {
            None => 1,
            Some(text) => text.trim().parse().ok()?,
        };
        let max = match self.attribute(element, b"maxOccurs") {
            None => None,
            Some(text) if text.trim() == "unbounded" => None,
            Some(text) => Some(text.trim().parse().ok()?),
        };
        if max.is_some_and(|max| max < min) {
            return None;
        }
        Some(Occurs { min, max })
    }

    /// facet。知らない facet は、報告して [`Facet::Unsupported`] にする
    fn facet(&mut self, element: &Element) -> Read<Facet> {
        self.work(element)?;
        let cardinality = match self.attribute(element, b"cardinality").as_deref() {
            None => Cardinality::Required,
            Some("required") => Cardinality::Required,
            Some("optional") => Cardinality::Optional,
            Some("prohibited") => Cardinality::Prohibited,
            Some(_) => return Ok(Err(self.bad(element))),
        };
        let local = self.local(element);
        if !self
            .doc
            .namespace(element.name())
            .is_some_and(|n| n == NAMESPACE)
        {
            return Ok(Err(self.bad(element)));
        }
        let mut values = Vec::new();
        for child in self.children(element) {
            let name = String::from_utf8_lossy(self.local(child)).into_owned();
            values.push((name, child));
        }
        let mut take = |name: &str| -> Option<&'d Element> {
            let at = values.iter().position(|(n, _)| n == name)?;
            Some(values.remove(at).1)
        };
        let facet = match local {
            b"entity" => {
                let (Some(name), predefined) = (take("name"), take("predefinedType")) else {
                    return Ok(Err(self.bad(element)));
                };
                let name = match self.value(name)? {
                    Ok(value) => value,
                    Err(bad) => return Ok(Err(bad)),
                };
                let predefined = match predefined.map(|e| self.value(e)).transpose()? {
                    Some(Err(bad)) => return Ok(Err(bad)),
                    Some(Ok(value)) => Some(value),
                    None => None,
                };
                Facet::Entity { name, predefined }
            }
            b"attribute" => {
                let (Some(name), value) = (take("name"), take("value")) else {
                    return Ok(Err(self.bad(element)));
                };
                let name = match self.value(name)? {
                    Ok(value) => value,
                    Err(bad) => return Ok(Err(bad)),
                };
                let value = match value.map(|e| self.value(e)).transpose()? {
                    Some(Err(bad)) => return Ok(Err(bad)),
                    Some(Ok(value)) => Some(value),
                    None => None,
                };
                Facet::Attribute {
                    name,
                    value,
                    cardinality,
                }
            }
            b"classification" => {
                let (value, Some(system)) = (take("value"), take("system")) else {
                    return Ok(Err(self.bad(element)));
                };
                let value = match value.map(|e| self.value(e)).transpose()? {
                    Some(Err(bad)) => return Ok(Err(bad)),
                    Some(Ok(value)) => Some(value),
                    None => None,
                };
                let system = match self.value(system)? {
                    Ok(value) => Some(value),
                    Err(bad) => return Ok(Err(bad)),
                };
                Facet::Classification {
                    value,
                    system,
                    cardinality,
                }
            }
            b"property" => {
                let (Some(set), Some(name), value) =
                    (take("propertySet"), take("baseName"), take("value"))
                else {
                    return Ok(Err(self.bad(element)));
                };
                let set = match self.value(set)? {
                    Ok(value) => value,
                    Err(bad) => return Ok(Err(bad)),
                };
                let name = match self.value(name)? {
                    Ok(value) => value,
                    Err(bad) => return Ok(Err(bad)),
                };
                let value = match value.map(|e| self.value(e)).transpose()? {
                    Some(Err(bad)) => return Ok(Err(bad)),
                    Some(Ok(value)) => Some(value),
                    None => None,
                };
                let data_type = self
                    .attribute(element, b"dataType")
                    .map(|t| t.trim().to_ascii_uppercase());
                // 値の決まりは、型を要る
                if value.is_some() && data_type.is_none() {
                    return Ok(Err(self.bad(element)));
                }
                Facet::Property {
                    set,
                    name,
                    data_type,
                    value,
                    cardinality,
                }
            }
            b"material" => {
                let value = match take("value").map(|e| self.value(e)).transpose()? {
                    Some(Err(bad)) => return Ok(Err(bad)),
                    Some(Ok(value)) => Some(value),
                    None => None,
                };
                Facet::Material { value, cardinality }
            }
            b"partOf" => {
                let Some(entity) = take("entity") else {
                    return Ok(Err(self.bad(element)));
                };
                let relation = match self.attribute(element, b"relation").as_deref() {
                    None => None,
                    Some(text) => match Relation::of(text.trim()) {
                        Some(relation) => Some(relation),
                        None => return Ok(Err(self.bad(element))),
                    },
                };
                let entity = match self.facet(entity)? {
                    Ok(facet @ Facet::Entity { .. }) => facet,
                    Ok(_) => return Ok(Err(self.bad(entity))),
                    Err(bad) => return Ok(Err(bad)),
                };
                Facet::PartOf {
                    entity: Box::new(entity),
                    relation,
                    cardinality,
                }
            }
            _ => {
                let kind = String::from_utf8_lossy(local).into_owned();
                let span = element.span();
                self.cx.report(
                    Diagnostic::new(codes::IDS_FACET_UNSUPPORTED)
                        .with("kind", kind.as_str())
                        .at(Ref::at_bytes(span.start as u64, span.end as u64)),
                );
                return Ok(Ok(Facet::Unsupported { kind, cardinality }));
            }
        };
        // 知らない子の要素（`uri` は属性。子は値だけ）
        if let Some((_, child)) = values.first() {
            return Ok(Err(self.bad(child)));
        }
        Ok(Ok(facet))
    }

    /// 値（`simpleValue` か `xs:restriction`）
    fn value(&mut self, element: &Element) -> Read<Value> {
        let children = self.children(element);
        let [only] = children.as_slice() else {
            return Ok(Err(self.bad(element)));
        };
        if self.is_ids(only, b"simpleValue") {
            return Ok(Ok(Value::Simple(self.text(only))));
        }
        if !self.doc.is(only.name(), Some(XS), b"restriction") {
            return Ok(Err(self.bad(only)));
        }
        let base = self.attribute(only, b"base").map(|base| {
            let base = base.trim();
            base.rsplit(':').next().unwrap_or(base).to_owned()
        });
        let mut restriction = Restriction {
            base,
            constraints: Vec::new(),
        };
        for constraint in self.children(only) {
            self.work(constraint)?;
            if self.doc.namespace(constraint.name()).as_deref() != Some(XS) {
                return Ok(Err(self.bad(constraint)));
            }
            if self.local(constraint) == b"annotation" {
                continue;
            }
            let Some(value) = self.attribute(constraint, b"value") else {
                return Ok(Err(self.bad(constraint)));
            };
            let number = || value.trim().parse::<f64>().ok().filter(|v| v.is_finite());
            let length = || value.trim().parse::<u64>().ok();
            let parsed = match self.local(constraint) {
                b"enumeration" => Some(Constraint::Enumeration(value.clone())),
                b"pattern" => Some(match Pattern::new(&value) {
                    Some(pattern) => Constraint::Pattern(Box::new(pattern)),
                    None => Constraint::Unsupported(format!("pattern {value}")),
                }),
                b"minInclusive" => number().map(Constraint::MinInclusive),
                b"maxInclusive" => number().map(Constraint::MaxInclusive),
                b"minExclusive" => number().map(Constraint::MinExclusive),
                b"maxExclusive" => number().map(Constraint::MaxExclusive),
                b"length" => length().map(Constraint::Length),
                b"minLength" => length().map(Constraint::MinLength),
                b"maxLength" => length().map(Constraint::MaxLength),
                other => Some(Constraint::Unsupported(
                    String::from_utf8_lossy(other).into_owned(),
                )),
            };
            let Some(parsed) = parsed else {
                return Ok(Err(self.bad(constraint)));
            };
            if let Constraint::Unsupported(kind) = &parsed {
                let span = constraint.span();
                self.cx.report(
                    Diagnostic::new(codes::IDS_RESTRICTION_UNSUPPORTED)
                        .with("kind", kind.as_str())
                        .at(Ref::at_bytes(span.start as u64, span.end as u64)),
                );
            }
            restriction.constraints.push(parsed);
        }
        Ok(Ok(Value::Restriction(restriction)))
    }
}

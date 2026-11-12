//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcProduct（5.1）、IfcRepresentationResource（8.18/8.9）、
//! IfcMappedItem（IfcGeometryResource）の属性をたどる L1 のビュー。幾何の値への変換は行わない。

use std::collections::{BTreeMap, BTreeSet};

use super::{Category, Version, arg, category, decode, reference};
use crate::{Cx, Diagnostic, Error, IfcFile, Kind, Record, Value, at_bytes, codes, stopped};

/// 取得した ProductShape の nodes 内だけの参照。STEP 番号・要素の ID・意味値ではない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeRef(pub u32);

/// 要素の形をたどる L1 のビュー。同じ IfcFile のレコードを借用する。
///
/// 配置・写像・文脈・単位の適用や幾何の検査を済ませた領域モデルではない。source 固有の参照を含むため、
/// 意味値の比較・要素の ID・ハッシュには使わない。Building のコンポーネントと Hash v1 は変更しない。
#[derive(Clone, Debug)]
pub struct ProductShape<'f> {
    /// 呼び出しで指定した要素の L1 レコード
    pub product: &'f Record,
    /// ObjectPlacement。値と親の配置は元のレコードから読める。適用・正規化しない
    pub placement: Option<&'f Record>,
    /// ProductDefinitionShape。省略なら None と ifc.shape.missing
    pub definition: Option<ShapeRef>,
    /// 初めに参照した順。同じ L1 インスタンスは 1 度だけ持つ。所有権は再帰しない
    pub nodes: Vec<ShapeNode<'f>>,
}

impl<'f> ProductShape<'f> {
    /// このビュー内の参照から項目を引く。範囲外は None
    pub fn node(&self, at: ShapeRef) -> Option<&ShapeNode<'f>> {
        self.nodes.get(at.0 as usize)
    }
}

/// source とその参照関係。L2 の形の意味値とは分ける
#[derive(Clone, Debug)]
pub struct ShapeNode<'f> {
    /// 元の L1 レコード。id と span から出所へ戻れる
    pub source: &'f Record,
    /// 形状表現をたどる関係。元のレコード内の並びを保つ
    pub links: ShapeLinks<'f>,
}

/// L1 の参照関係。STEP 番号・グラフの並びを含むため、意味値の等価性や hash を定義しない。
/// 文字列も表現を選ぶためのラベルであり、幾何を評価した結果ではない
#[derive(Clone, Debug)]
pub enum ShapeLinks<'f> {
    /// IfcProductDefinitionShape の Representations
    Definition {
        /// Representations の参照を元の並びで保持する
        representations: Vec<ShapeRef>,
    },
    /// IfcShapeRepresentation。文脈の配置・精度も元のレコードに残す
    Representation {
        /// ContextOfItems。配置・精度・親の文脈は未適用
        context: &'f Record,
        /// RepresentationIdentifier。Body・Axis・FootPrint などの元のラベル
        identifier: Option<String>,
        /// RepresentationType。元のラベルであり、内容の意味検査を済ませたことは示さない
        representation_type: Option<String>,
        /// Items の参照。元の並びを保持する
        items: Vec<ShapeRef>,
    },
    /// IfcMappedItem。写像元を共有し、MappingTarget の未適用の値をレコードで持つ
    Mapped {
        /// 共有する MappingSource
        source: ShapeRef,
        /// MappingTarget。軸・原点・尺度の値と省略は元のレコードに保持する
        target: &'f Record,
    },
    /// IfcRepresentationMap。MappingOrigin は未適用。元の表現は共有できる
    Map {
        /// MappingOrigin の元の配置
        origin: &'f Record,
        /// MappedRepresentation
        representation: ShapeRef,
    },
    /// IfcGeometricSet / IfcGeometricCurveSet。曲線以外の要素も Unsupported と診断で残す
    Set {
        /// Elements の参照。元の並びを保持する
        elements: Vec<ShapeRef>,
    },
    /// source.id() を IfcFile::curve に渡せる。曲線の値・単位・診断はその取得時に読む
    Curve,
    /// このビューでは展開しない項目。元の値・参照は source に残し、ifc.shape.unsupported を返す
    Unsupported,
}

#[derive(Clone, Copy)]
enum Role {
    Definition,
    Representation,
    Map,
    Item,
}
enum Frame {
    Enter(u64),
    Leave(u64),
}

impl IfcFile {
    /// 建物の要素の L1 番号から、形状表現・共有写像・曲線への参照を必要時に取得する（feature building）。
    ///
    /// Building::lift が知る IfcProduct の実体（element / spatial / grid）が対象。IfcTypeProduct は扱わない。
    /// 元のレコードを借用する L1 ビューであり、配置・写像・単位を適用しない。曲線は source.id() を curve() に渡す。
    /// 未対応の葉は出所と診断を残す。省略した配置・表現・表現ラベルは None と診断、不正な構造と循環は Invalid。
    /// 深さ・手間・確保前の展開量と中断は呼び出しの Cx で数え、停止・不正時に部分結果は返さない。
    pub fn product_shape<'f>(
        &'f self,
        instance: u64,
        cx: &mut Cx,
    ) -> Result<ProductShape<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            if Version::of(self.schema()).is_none() {
                for (kind, count) in [
                    (Kind::Work, self.schema().len() as u64),
                    (
                        Kind::DecodedBytes,
                        (self.schema().len() as u64).saturating_mul(3),
                    ),
                ] {
                    cx.check(kind, count).map_err(|stop| {
                        stopped(cx, stop, self.instance(instance).map(Record::span))
                    })?;
                }
                cx.report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                    "schema",
                    String::from_utf8_lossy(self.schema()).into_owned(),
                ));
                return Err(Error::Unsupported);
            }
            Reader {
                file: self,
                cx,
                current: instance,
                nodes: Vec::new(),
                index: BTreeMap::new(),
                active: BTreeSet::new(),
                children: Vec::new(),
            }
            .run(instance)
        })
    }
}

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: u64,
    nodes: Vec<Option<ShapeNode<'f>>>,
    index: BTreeMap<u64, ShapeRef>,
    active: BTreeSet<u64>,
    children: Vec<u64>,
}

impl<'f> Reader<'f, '_, '_> {
    fn check(&mut self, kind: Kind, count: u64) -> Result<(), Error> {
        self.cx.check(kind, count).map_err(|stop| {
            stopped(
                self.cx,
                stop,
                self.file.instance(self.current).map(Record::span),
            )
        })
    }
    fn alloc(&mut self, count: usize, bytes: u64) -> Result<(), Error> {
        self.check(Kind::DecodedBytes, (count as u64).saturating_mul(bytes))
    }
    fn invalid(&mut self, reason: &'static str) -> Error {
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_INVALID)
                .with("reason", reason)
                .at(self.place()),
        );
        Error::Invalid
    }
    fn place(&self) -> zumen_base::Ref {
        self.file
            .instance(self.current)
            .map_or_else(zumen_base::Ref::default, |r| at_bytes(r.span()))
            .with_id(format!("i{}", self.current))
    }
    fn missing(&mut self, field: &'static str) {
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_MISSING)
                .with("field", field)
                .at(self.place()),
        );
    }
    fn unsupported(&mut self, record: &Record) -> Result<(), Error> {
        let kind = self.file.kind(record);
        self.check(Kind::Work, kind.len() as u64)?;
        self.alloc(kind.len(), 3)?;
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_UNSUPPORTED)
                .with("kind", String::from_utf8_lossy(kind).into_owned())
                .at(self.place()),
        );
        Ok(())
    }
    fn record(&mut self, id: u64) -> Result<&'f Record, Error> {
        self.check(Kind::Work, 1)?;
        let r = self
            .file
            .instance(id)
            .ok_or_else(|| self.invalid("reference"))?;
        if r.parts().len() != 1 {
            return Err(self.invalid("type"));
        }
        Ok(r)
    }
    fn is(&self, r: &Record, kind: &[u8]) -> bool {
        self.file.kind(r).eq_ignore_ascii_case(kind)
    }
    fn required(&mut self, r: &Record, at: usize) -> Result<u64, Error> {
        reference(arg(r, at)).ok_or_else(|| self.invalid("reference"))
    }
    fn optional(
        &mut self,
        r: &Record,
        at: usize,
        field: &'static str,
    ) -> Result<Option<&'f Record>, Error> {
        if matches!(arg(r, at), Some(Value::Null)) {
            self.missing(field);
            return Ok(None);
        }
        let id = self.required(r, at)?;
        self.record(id).map(Some)
    }
    fn text(
        &mut self,
        r: &Record,
        at: usize,
        field: &'static str,
    ) -> Result<Option<String>, Error> {
        let raw = match arg(r, at) {
            Some(Value::Null) => {
                self.missing(field);
                return Ok(None);
            }
            Some(Value::Text { raw, .. }) => raw.clone(),
            _ => return Err(self.invalid("text")),
        };
        self.check(Kind::Work, raw.len() as u64)?;
        // 置換の文字とエスケープの展開を含め、decode の確保より先に予約する
        self.alloc(raw.len(), 12)?;
        let (text, invalid) = decode(self.file.bytes(raw));
        if invalid {
            self.cx
                .report(Diagnostic::new(codes::IFC_TEXT_INVALID).at(self.place()));
        }
        Ok(Some(text))
    }
    fn link(&mut self, id: u64, role: Role) -> Result<ShapeRef, Error> {
        let r = self.record(id)?;
        let valid = match role {
            Role::Definition => {
                self.is(r, b"IFCPRODUCTDEFINITIONSHAPE") || self.is(r, b"IFCPRODUCTREPRESENTATION")
            }
            Role::Representation => [
                b"IFCSHAPEREPRESENTATION".as_slice(),
                b"IFCTOPOLOGYREPRESENTATION",
                b"IFCSTYLEDREPRESENTATION",
            ]
            .iter()
            .any(|kind| self.is(r, kind)),
            Role::Map => self.is(r, b"IFCREPRESENTATIONMAP"),
            Role::Item => ![
                b"IFCPRODUCTDEFINITIONSHAPE".as_slice(),
                b"IFCSHAPEREPRESENTATION",
                b"IFCREPRESENTATIONMAP",
            ]
            .iter()
            .any(|k| self.is(r, k)),
        };
        if !valid {
            return Err(self.invalid("type"));
        }
        let reference = if let Some(&at) = self.index.get(&id) {
            at
        } else {
            // node・BTreeMap の索引・active の領域。64 bit と wasm32 で同じ予約量
            self.alloc(1, 576)?;
            let at = ShapeRef(u32::try_from(self.nodes.len()).map_err(|_| self.invalid("count"))?);
            self.nodes.push(None);
            self.index.insert(id, at);
            at
        };
        self.alloc(1, 8)?;
        self.children.push(id);
        Ok(reference)
    }
    fn list(&mut self, r: &Record, at: usize, role: Role) -> Result<Vec<ShapeRef>, Error> {
        let Some(Value::List(values)) = arg(r, at) else {
            return Err(self.invalid("list"));
        };
        if values.is_empty() {
            return Err(self.invalid("count"));
        }
        self.check(Kind::Work, values.len() as u64)?;
        self.alloc(values.len(), 8)?;
        let mut found = Vec::with_capacity(values.len());
        for value in values {
            let id = reference(Some(value)).ok_or_else(|| self.invalid("reference"))?;
            found.push(self.link(id, role)?);
        }
        Ok(found)
    }
    fn typed(&mut self, r: &Record, at: usize, kinds: &[&[u8]]) -> Result<&'f Record, Error> {
        let id = self.required(r, at)?;
        let target = self.record(id)?;
        if kinds.iter().any(|kind| self.is(target, kind)) {
            Ok(target)
        } else {
            Err(self.invalid("type"))
        }
    }
    fn parse(&mut self, r: &'f Record) -> Result<ShapeLinks<'f>, Error> {
        Ok(if self.is(r, b"IFCPRODUCTDEFINITIONSHAPE") {
            ShapeLinks::Definition {
                representations: self.list(r, 3, Role::Representation)?,
            }
        } else if self.is(r, b"IFCSHAPEREPRESENTATION") {
            let context = self.typed(
                r,
                1,
                &[
                    b"IFCGEOMETRICREPRESENTATIONCONTEXT",
                    b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
                ],
            )?;
            ShapeLinks::Representation {
                context,
                identifier: self.text(r, 2, "RepresentationIdentifier")?,
                representation_type: self.text(r, 3, "RepresentationType")?,
                items: self.list(r, 4, Role::Item)?,
            }
        } else if self.is(r, b"IFCMAPPEDITEM") {
            let id = self.required(r, 1)?;
            ShapeLinks::Mapped {
                source: self.link(id, Role::Map)?,
                target: self.typed(
                    r,
                    2,
                    &[
                        b"IFCCARTESIANTRANSFORMATIONOPERATOR2D",
                        b"IFCCARTESIANTRANSFORMATIONOPERATOR2DNONUNIFORM",
                        b"IFCCARTESIANTRANSFORMATIONOPERATOR3D",
                        b"IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM",
                    ],
                )?,
            }
        } else if self.is(r, b"IFCREPRESENTATIONMAP") {
            let origin = self.typed(r, 1, &[b"IFCAXIS2PLACEMENT2D", b"IFCAXIS2PLACEMENT3D"])?;
            let id = self.required(r, 2)?;
            ShapeLinks::Map {
                origin,
                representation: self.link(id, Role::Representation)?,
            }
        } else if self.is(r, b"IFCGEOMETRICCURVESET") || self.is(r, b"IFCGEOMETRICSET") {
            ShapeLinks::Set {
                elements: self.list(r, 1, Role::Item)?,
            }
        } else if [
            b"IFCPOLYLINE".as_slice(),
            b"IFCLINE",
            b"IFCCIRCLE",
            b"IFCCOMPOSITECURVE",
            b"IFCTRIMMEDCURVE",
        ]
        .iter()
        .any(|kind| self.is(r, kind))
            || (self.is(r, b"IFCINDEXEDPOLYCURVE")
                && Version::of(self.file.schema()) != Some(Version::Ifc2x3))
        {
            ShapeLinks::Curve
        } else {
            self.unsupported(r)?;
            ShapeLinks::Unsupported
        })
    }
    fn run(mut self, id: u64) -> Result<ProductShape<'f>, Error> {
        let product = self.record(id)?;
        if !matches!(
            category(self.file.kind(product), self.file.schema()),
            Some(Category::Element | Category::Spatial | Category::Grid)
        ) {
            return Err(self.invalid("product"));
        }
        let placement = self.optional(product, 6, "ObjectPlacement")?;
        if placement.is_some_and(|record| {
            ![
                b"IFCLOCALPLACEMENT".as_slice(),
                b"IFCGRIDPLACEMENT",
                b"IFCLINEARPLACEMENT",
            ]
            .iter()
            .any(|kind| self.is(record, kind))
        }) {
            return Err(self.invalid("type"));
        }
        let definition = self.optional(product, 7, "Representation")?;
        let root = definition
            .map(|r| self.link(r.id().unwrap_or(0), Role::Definition))
            .transpose()?;
        self.children.clear();
        self.alloc(1, 24)?;
        let mut stack: Vec<_> = definition
            .into_iter()
            .map(|r| Frame::Enter(r.id().unwrap_or(0)))
            .collect();
        let mut entered = 0u64;
        let result = (|| {
            while let Some(frame) = stack.pop() {
                self.check(Kind::Work, 1)?;
                match frame {
                    Frame::Leave(id) => {
                        self.active.remove(&id);
                        self.cx.leave();
                        entered -= 1;
                    }
                    Frame::Enter(id) => {
                        self.current = id;
                        if self.active.contains(&id) {
                            return Err(self.invalid("cycle"));
                        }
                        let at = self
                            .index
                            .get(&id)
                            .copied()
                            .ok_or_else(|| self.invalid("reference"))?;
                        if self.nodes.get(at.0 as usize).is_some_and(Option::is_some) {
                            continue;
                        }
                        self.cx.enter().map_err(|stop| {
                            stopped(self.cx, stop, self.file.instance(id).map(Record::span))
                        })?;
                        entered += 1;
                        self.active.insert(id);
                        let source = self.record(id)?;
                        self.children = Vec::new();
                        let links = self.parse(source)?;
                        let Some(slot) = self.nodes.get_mut(at.0 as usize) else {
                            return Err(self.invalid("reference"));
                        };
                        *slot = Some(ShapeNode { source, links });
                        self.alloc(1 + self.children.len(), 24)?;
                        stack.push(Frame::Leave(id));
                        stack.extend(
                            core::mem::take(&mut self.children)
                                .into_iter()
                                .rev()
                                .map(Frame::Enter),
                        );
                    }
                }
            }
            self.alloc(self.nodes.len(), 320)?;
            let nodes = core::mem::take(&mut self.nodes)
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| self.invalid("reference"))?;
            Ok(ProductShape {
                product,
                placement,
                definition: root,
                nodes,
            })
        })();
        for _ in 0..entered {
            self.cx.leave();
        }
        result
    }
}

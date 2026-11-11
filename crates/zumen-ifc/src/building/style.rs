//! IFC2X3 TC1 / IFC4 ADD2 TC1 / IFC4.3 ADD2 の IfcStyledItem・IfcSurfaceStyle と
//! IfcSurfaceStyleShading / Rendering の元属性を読む。表示・材質・レイヤ・写像による継承は評価しない。

use std::collections::BTreeSet;

use zumen_building::{
    ColourOrFactor, ReflectanceMethod, SpecularHighlight, SurfaceColour, SurfaceRendering,
    SurfaceShading, SurfaceSide, SurfaceStyle,
};

use super::{Version, arg, decode, reference};
use crate::{
    Cx, Diagnostic, Error, IfcFile, Kind, Record, ShapeNode, Value, at_bytes, codes, stopped,
};

/// 形の項目に直接付いた面の表示。出所を含む全体の PartialEq / Hash は定義しない
#[derive(Clone, Debug)]
pub struct ItemSurfaceStyles<'f> {
    /// StyledItem の元の名前。割り当てがなければ None
    pub name: Option<String>,
    /// 元の集合の並びで保持する。優先順位や面への適用は行わない
    pub styles: Vec<SurfaceStyleBinding<'f>>,
    /// 割り当ての元レコード。意味の値から分ける
    pub source: ItemStyleSource<'f>,
}

/// 直接の表示の割り当ての出所
#[derive(Clone, Debug)]
pub struct ItemStyleSource<'f> {
    /// 呼び出しで指定した形の項目
    pub item: &'f Record,
    /// 直接 Item に結びつく StyledItem。なければ None
    pub styled_item: Option<&'f Record>,
    /// 旧版の PresentationStyleAssignment。経路の並びを保つ
    pub assignments: Vec<&'f Record>,
}

/// 面の表示の定義と元レコード。意味の比較は definition に対して行う
#[derive(Clone, Debug)]
pub struct SurfaceStyleBinding<'f> {
    /// L1 に依存しない元の値。最終的な表示ではない
    pub definition: SurfaceStyle,
    /// 元のレコード
    pub source: SurfaceStyleSource<'f>,
}

/// 面の表示の出所。同じ IfcFile のレコードを借用する
#[derive(Clone, Debug)]
pub struct SurfaceStyleSource<'f> {
    /// SurfaceStyle
    pub style: &'f Record,
    /// Shading または Rendering
    pub shading: &'f Record,
    /// 読んだ色のレコード。属性を読んだ順で、同じ色の再使用も保つ
    pub colours: Vec<&'f Record>,
}

impl IfcFile {
    /// 既存 ProductShape の項目に直接付いた面の表示を必要時に取得する。
    ///
    /// 同じ IfcFile から取得した ShapeNode を渡す。Shading / Rendering の全属性を元の値のまま持ち、
    /// 色の係数・反射式・既定値は適用しない。材質・レイヤ・写像からの継承や上書きも行わない。
    /// 直接の StyledItem がなければ空の成功。ほかの表示の種類が混じれば Unsupported、使う値や参照の不正は Invalid。
    /// 予算と中断は Cx を共有し、エラー時に部分結果を返さない。Building と Hash v1 は変更しない。
    pub fn surface_styles<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<ItemSurfaceStyles<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                colours: Vec::new(),
            };
            reader.check(Kind::Work, 1)?;
            let item = self
                .instance(reader.current)
                .ok_or_else(|| reader.invalid("source"))?;
            if !std::ptr::eq(item, node.source) {
                return Err(reader.invalid("source"));
            }
            let Some(version) = Version::of(self.schema()) else {
                reader.check(Kind::Work, self.schema().len() as u64)?;
                reader.alloc(self.schema().len(), 3)?;
                reader
                    .cx
                    .report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ));
                return Err(Error::Unsupported);
            };
            reader.within(reader.current, |reader, item| reader.run(item, version))
        })
    }
}

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: u64,
    colours: Vec<&'f Record>,
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
    fn alloc(&mut self, count: usize, size: u64) -> Result<(), Error> {
        self.check(Kind::DecodedBytes, (count as u64).saturating_mul(size))
    }
    fn invalid(&mut self, reason: &'static str) -> Error {
        let place = self
            .file
            .instance(self.current)
            .map_or_else(zumen_base::Ref::default, |r| at_bytes(r.span()))
            .with_id(format!("i{}", self.current));
        self.cx.report(
            Diagnostic::new(codes::IFC_STYLE_INVALID)
                .with("reason", reason)
                .at(place),
        );
        Error::Invalid
    }
    fn unsupported(&mut self, record: &Record) -> Error {
        let kind = self.file.kind(record);
        self.unsupported_kind(record, kind)
    }
    fn unsupported_kind(&mut self, record: &Record, kind: &[u8]) -> Error {
        if let Err(error) = self.check(Kind::Work, kind.len() as u64) {
            return error;
        }
        if let Err(error) = self.alloc(kind.len(), 6) {
            return error;
        }
        self.cx.report(
            Diagnostic::new(codes::IFC_STYLE_UNSUPPORTED)
                .with("kind", String::from_utf8_lossy(kind).into_owned())
                .at(at_bytes(record.span())),
        );
        Error::Unsupported
    }
    fn is(&self, record: &Record, name: &[u8]) -> bool {
        self.file.kind(record).eq_ignore_ascii_case(name)
    }
    fn within<T>(
        &mut self,
        id: u64,
        f: impl FnOnce(&mut Self, &'f Record) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let previous = self.current;
        self.current = id;
        let result = (|| {
            self.check(Kind::Work, 1)?;
            let record = self
                .file
                .instance(id)
                .ok_or_else(|| self.invalid("reference"))?;
            if record.parts().len() != 1 {
                return Err(self.invalid("type"));
            }
            self.cx
                .enter()
                .map_err(|stop| stopped(self.cx, stop, Some(record.span())))?;
            let result = f(self, record);
            self.cx.leave();
            result
        })();
        self.current = previous;
        result
    }
    fn arity(&mut self, record: &Record, count: usize) -> Result<(), Error> {
        if record
            .parts()
            .first()
            .is_some_and(|p| p.args().len() == count)
        {
            Ok(())
        } else {
            Err(self.invalid("arity"))
        }
    }
    fn target(&mut self, value: Option<&Value>) -> Result<u64, Error> {
        reference(value).ok_or_else(|| self.invalid("reference"))
    }
    fn list(&mut self, value: Option<&'f Value>, max: usize) -> Result<&'f [Value], Error> {
        let Some(Value::List(values)) = value else {
            return Err(self.invalid("list"));
        };
        if values.is_empty() || values.len() > max {
            return Err(self.invalid("count"));
        }
        self.check(Kind::Work, values.len() as u64)?;
        // 集合の重複を調べる表を、確保の前に環境によらない量で数える。
        self.alloc(1, 256)?;
        self.alloc(values.len(), 64)?;
        let mut seen = BTreeSet::new();
        for value in values {
            let id = self.target(Some(value))?;
            if !seen.insert(id) {
                return Err(self.invalid("duplicate"));
            }
        }
        Ok(values)
    }
    fn text(&mut self, value: Option<&Value>, record: &Record) -> Result<Option<String>, Error> {
        match value {
            Some(Value::Null) => Ok(None),
            Some(Value::Text { raw, .. }) => {
                self.check(Kind::Work, raw.len() as u64)?;
                self.alloc(raw.len(), 12)?;
                self.alloc(1, 32)?;
                let (name, invalid) = decode(self.file.bytes(raw.clone()));
                if invalid {
                    self.cx.report(
                        Diagnostic::new(codes::IFC_TEXT_INVALID).at(at_bytes(record.span())),
                    );
                }
                Ok(Some(name))
            }
            _ => Err(self.invalid("text")),
        }
    }
    fn number(&mut self, value: Option<&Value>) -> Result<f64, Error> {
        let Some(Value::Number(raw)) = value else {
            return Err(self.invalid("number"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        std::str::from_utf8(self.file.bytes(raw.clone()))
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .ok_or_else(|| self.invalid("number"))
    }
    fn ratio(&mut self, value: Option<&Value>) -> Result<f64, Error> {
        let value = self.number(value)?;
        if (0.0..=1.0).contains(&value) {
            Ok(value)
        } else {
            Err(self.invalid("range"))
        }
    }
    fn optional_ratio(&mut self, value: Option<&Value>) -> Result<Option<f64>, Error> {
        match value {
            Some(Value::Null) => Ok(None),
            _ => self.ratio(value).map(Some),
        }
    }
    fn enumeration(&mut self, value: Option<&'f Value>) -> Result<&'f [u8], Error> {
        let Some(Value::Enum(raw)) = value else {
            return Err(self.invalid("enumeration"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        Ok(self.file.bytes(raw.clone()))
    }
    fn colour(&mut self, id: u64) -> Result<SurfaceColour, Error> {
        self.within(id, |reader, record| {
            if !reader.is(record, b"IFCCOLOURRGB") {
                return Err(reader.invalid("type"));
            }
            reader.arity(record, 4)?;
            let name = reader.text(arg(record, 1), record)?;
            let rgb = [
                reader.ratio(arg(record, 2))?,
                reader.ratio(arg(record, 3))?,
                reader.ratio(arg(record, 4))?,
            ];
            reader.alloc(1, 32)?;
            reader.colours.push(record);
            Ok(SurfaceColour { name, rgb })
        })
    }
    fn colour_or_factor(&mut self, value: Option<&Value>) -> Result<Option<ColourOrFactor>, Error> {
        match value {
            Some(Value::Null) => Ok(None),
            Some(Value::Ref(id)) => self.colour(*id).map(|v| Some(ColourOrFactor::Colour(v))),
            Some(Value::Typed(part))
                if self
                    .file
                    .bytes(part.name())
                    .eq_ignore_ascii_case(b"IFCNORMALISEDRATIOMEASURE")
                    && part.args().len() == 1 =>
            {
                self.ratio(part.args().first())
                    .map(|v| Some(ColourOrFactor::Factor(v)))
            }
            _ => Err(self.invalid("select")),
        }
    }
    fn highlight(&mut self, value: Option<&Value>) -> Result<Option<SpecularHighlight>, Error> {
        match value {
            Some(Value::Null) => Ok(None),
            Some(Value::Typed(part)) if part.args().len() == 1 => {
                let kind = self.file.bytes(part.name());
                if kind.eq_ignore_ascii_case(b"IFCSPECULAREXPONENT") {
                    self.number(part.args().first())
                        .map(|v| Some(SpecularHighlight::Exponent(v)))
                } else if kind.eq_ignore_ascii_case(b"IFCSPECULARROUGHNESS") {
                    self.ratio(part.args().first())
                        .map(|v| Some(SpecularHighlight::Roughness(v)))
                } else {
                    Err(self.invalid("select"))
                }
            }
            _ => Err(self.invalid("select")),
        }
    }
    fn method(
        &mut self,
        value: Option<&'f Value>,
        version: Version,
    ) -> Result<ReflectanceMethod, Error> {
        let value = self.enumeration(value)?;
        let methods = [
            (b"BLINN".as_slice(), ReflectanceMethod::Blinn),
            (b"FLAT".as_slice(), ReflectanceMethod::Flat),
            (b"GLASS".as_slice(), ReflectanceMethod::Glass),
            (b"MATT".as_slice(), ReflectanceMethod::Matt),
            (b"METAL".as_slice(), ReflectanceMethod::Metal),
            (b"MIRROR".as_slice(), ReflectanceMethod::Mirror),
            (b"PHONG".as_slice(), ReflectanceMethod::Phong),
            (b"PLASTIC".as_slice(), ReflectanceMethod::Plastic),
            (b"STRAUSS".as_slice(), ReflectanceMethod::Strauss),
            (b"NOTDEFINED".as_slice(), ReflectanceMethod::NotDefined),
        ];
        if value.eq_ignore_ascii_case(b"PHYSICAL") && version == Version::Ifc4x3 {
            return Ok(ReflectanceMethod::Physical);
        }
        methods
            .iter()
            .find(|(name, _)| value.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
            .ok_or_else(|| self.invalid("enumeration"))
    }
    fn shading(&mut self, record: &'f Record, version: Version) -> Result<SurfaceShading, Error> {
        let rendering = self.is(record, b"IFCSURFACESTYLERENDERING");
        let shading = self.is(record, b"IFCSURFACESTYLESHADING");
        if !rendering && !shading {
            return Err(self.unsupported(record));
        }
        self.arity(
            record,
            if rendering {
                9
            } else if version == Version::Ifc2x3 {
                1
            } else {
                2
            },
        )?;
        let id = self.target(arg(record, 1))?;
        let colour = self.colour(id)?;
        let transparency = if rendering || version != Version::Ifc2x3 {
            self.optional_ratio(arg(record, 2))?
        } else {
            None
        };
        let rendering = if rendering {
            Some(SurfaceRendering {
                diffuse: self.colour_or_factor(arg(record, 3))?,
                transmission: self.colour_or_factor(arg(record, 4))?,
                diffuse_transmission: self.colour_or_factor(arg(record, 5))?,
                reflection: self.colour_or_factor(arg(record, 6))?,
                specular: self.colour_or_factor(arg(record, 7))?,
                highlight: self.highlight(arg(record, 8))?,
                method: self.method(arg(record, 9), version)?,
            })
        } else {
            None
        };
        Ok(SurfaceShading {
            colour,
            transparency,
            rendering,
        })
    }
    fn surface(&mut self, id: u64, version: Version) -> Result<SurfaceStyleBinding<'f>, Error> {
        self.within(id, |reader, record| reader.surface_record(record, version))
    }
    fn surface_record(
        &mut self,
        record: &'f Record,
        version: Version,
    ) -> Result<SurfaceStyleBinding<'f>, Error> {
        let reader = self;
        if !reader.is(record, b"IFCSURFACESTYLE") {
            if [
                b"IFCCURVESTYLE".as_slice(),
                b"IFCFILLAREASTYLE".as_slice(),
                b"IFCTEXTSTYLE".as_slice(),
            ]
            .iter()
            .any(|kind| reader.is(record, kind))
                || (version == Version::Ifc2x3 && reader.is(record, b"IFCSYMBOLSTYLE"))
            {
                return Err(reader.unsupported(record));
            }
            return Err(reader.invalid("type"));
        }
        reader.arity(record, 3)?;
        let name = reader.text(arg(record, 1), record)?;
        let side = reader.enumeration(arg(record, 2))?;
        let side = if side.eq_ignore_ascii_case(b"POSITIVE") {
            SurfaceSide::Positive
        } else if side.eq_ignore_ascii_case(b"NEGATIVE") {
            SurfaceSide::Negative
        } else if side.eq_ignore_ascii_case(b"BOTH") {
            SurfaceSide::Both
        } else {
            return Err(reader.invalid("side"));
        };
        let values = reader.list(arg(record, 3), 5)?;
        let mut found = None;
        for value in values {
            let shading_id = reader.target(Some(value))?;
            let value = reader.within(shading_id, |reader, r| {
                if !reader.is(r, b"IFCSURFACESTYLESHADING")
                    && !reader.is(r, b"IFCSURFACESTYLERENDERING")
                {
                    if [
                        b"IFCSURFACESTYLELIGHTING".as_slice(),
                        b"IFCSURFACESTYLEWITHTEXTURES".as_slice(),
                        b"IFCSURFACESTYLEREFRACTION".as_slice(),
                        b"IFCEXTERNALLYDEFINEDSURFACESTYLE".as_slice(),
                    ]
                    .iter()
                    .any(|kind| reader.is(r, kind))
                    {
                        return Err(reader.unsupported(r));
                    }
                    return Err(reader.invalid("type"));
                }
                if found.is_some() {
                    return Err(reader.invalid("shading-count"));
                }
                reader.shading(r, version).map(|v| (v, r))
            })?;
            found = Some(value);
        }
        let (shading, source) = found.ok_or_else(|| reader.invalid("shading-count"))?;
        Ok(SurfaceStyleBinding {
            definition: SurfaceStyle {
                name,
                side,
                shading,
            },
            source: SurfaceStyleSource {
                style: record,
                shading: source,
                colours: std::mem::take(&mut reader.colours),
            },
        })
    }
    fn presentation(
        &mut self,
        id: u64,
        version: Version,
        result: &mut ItemSurfaceStyles<'f>,
    ) -> Result<(), Error> {
        self.within(id, |reader, record| {
            if reader.is(record, b"IFCPRESENTATIONSTYLEASSIGNMENT") {
                if version == Version::Ifc4x3 {
                    return Err(reader.invalid("assignment-version"));
                }
                reader.arity(record, 1)?;
                if let Some(Value::List(values)) = arg(record, 1) {
                    reader.check(Kind::Work, values.len() as u64)?;
                    if values.iter().any(|v| match v {
                        Value::Enum(raw) => {
                            reader.file.bytes(raw.clone()).eq_ignore_ascii_case(b"NULL")
                        }
                        Value::Typed(p) => reader
                            .file
                            .bytes(p.name())
                            .eq_ignore_ascii_case(b"IFCNULLSTYLE"),
                        _ => false,
                    }) {
                        return Err(reader.unsupported_kind(record, b"IFCNULLSTYLE"));
                    }
                }
                let values = reader.list(arg(record, 1), usize::MAX)?;
                reader.alloc(1, 32)?;
                result.source.assignments.push(record);
                for value in values {
                    let id = reader.target(Some(value))?;
                    reader.reserve_style(result)?;
                    result.styles.push(reader.surface(id, version)?);
                }
            } else {
                if version == Version::Ifc2x3 {
                    return Err(reader.invalid("assignment-version"));
                }
                reader.reserve_style(result)?;
                result.styles.push(reader.surface_record(record, version)?);
            }
            Ok(())
        })
    }
    fn reserve_style(&mut self, result: &mut ItemSurfaceStyles<'f>) -> Result<(), Error> {
        let old = result.styles.capacity();
        if result.styles.len() == old {
            // 32 bit の Vec の上限に合わせ、予算の固定サイズでも isize の範囲を超えない。
            let capacity = old.saturating_mul(2).max(1).min(i32::MAX as usize / 1024);
            if capacity <= old {
                return Err(self.invalid("capacity"));
            }
            self.check(Kind::Work, result.styles.len() as u64)?;
            // 予算は確保する容量で数える。小さい Vec が暗黙に複数項目を確保することを避ける。
            self.alloc(capacity.saturating_sub(old), 1024)?;
            result
                .styles
                .reserve_exact(capacity.saturating_sub(result.styles.len()));
        }
        Ok(())
    }
    fn run(&mut self, item: &'f Record, version: Version) -> Result<ItemSurfaceStyles<'f>, Error> {
        if self.is(item, b"IFCSTYLEDITEM") {
            return Err(self.invalid("item"));
        }
        let mut styled = None;
        for record in self.file.records() {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| stopped(self.cx, stop, Some(record.span())))?;
            if self.is(record, b"IFCSTYLEDITEM")
                && reference(arg(record, 1)) == item.id()
                && styled.replace(record).is_some()
            {
                self.current = record.id().unwrap_or(self.current);
                return Err(self.invalid("styled-count"));
            }
        }
        let mut result = ItemSurfaceStyles {
            name: None,
            styles: Vec::new(),
            source: ItemStyleSource {
                item,
                styled_item: styled,
                assignments: Vec::new(),
            },
        };
        let Some(styled) = styled else {
            return Ok(result);
        };
        let styled_id = styled.id().ok_or_else(|| self.invalid("source"))?;
        self.within(styled_id, |reader, record| {
            reader.arity(record, 3)?;
            result.name = reader.text(arg(record, 3), record)?;
            let values = reader.list(
                arg(record, 2),
                if version == Version::Ifc2x3 {
                    1
                } else {
                    usize::MAX
                },
            )?;
            for value in values {
                let id = reader.target(Some(value))?;
                reader.presentation(id, version, &mut result)?;
            }
            Ok(())
        })?;
        Ok(result)
    }
}

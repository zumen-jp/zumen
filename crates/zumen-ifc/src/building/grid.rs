//! IFC2X3 TC1 / IFC4 ADD2 TC1 / IFC4.3 ADD2 の IfcGrid と IfcGridAxis の属性、
//! unique list、AxisCurve.Dim = 2 と単一の grid への所属を読む。配置・向き・幾何は評価しない。
use super::{Version, arg, decode, reference};
use crate::{Cx, Diagnostic, Error, IfcFile, Kind, Record, Value, at_bytes, codes, stopped};
use std::collections::BTreeSet;
use zumen_building::{CurveGraph, CurveKind, CurveTrim, GridAxisDefinition, GridLayout, GridSet};

/// 通り芯から取得した軸と曲線。出所付きの全体に意味の等価性や Hash を定義しない
#[derive(Clone, Debug)]
pub struct GridAxes<'f> {
    /// U・V・W の順で、各列の元の順序を保つ
    pub axes: Vec<GridAxisCurve<'f>>,
    /// 元の配置の種類の指定。省略した場合と IFC2X3 では None
    pub layout: Option<GridLayout>,
    /// 元のレコード。配置は未解決・未適用の参照
    pub source: GridSource<'f>,
}
/// 軸の指定と、その軸の曲線のグラフ。共有の範囲はこの curve の中だけ
#[derive(Clone, Debug)]
pub struct GridAxisCurve<'f> {
    /// 所属・名前・向き。元のレコードの番号を含めない
    pub definition: GridAxisDefinition,
    /// 2D の元の曲線。単位と曲線内の出所も保持する
    pub curve: CurveGraph,
    /// IfcGridAxis
    pub source: &'f Record,
}
/// 通り芯の出所。同じ IfcFile のレコードを借用する
#[derive(Clone, Debug)]
pub struct GridSource<'f> {
    /// IfcGrid
    pub grid: &'f Record,
    /// ObjectPlacement。4.3 で許された省略は None。親の配置や属性は展開しない
    pub placement: Option<&'f Record>,
    /// 任意の Representation。軸の曲線はここからではなく U/V/W から得る
    pub representation: Option<&'f Record>,
}
// 曲線や文字列の中身は別に予約する。固定の軸の容量は両ポインター幅で上から数える。
const AXIS_BYTES: usize = 1024;
const _: () = assert!(size_of::<GridAxisCurve<'static>>() <= AXIS_BYTES);
impl IfcFile {
    /// 通り芯の L1 番号から U/V/W の軸と曲線をまとめて必要時に取得する。
    ///
    /// AxisTag・SameSense と各 CurveGraph を保持し、配置・反転・既定値・幾何を評価しない。
    /// 共有は各 CurveGraph 内だけ。同じ曲線を指す複数の軸も、出所を保って個別に取得する。
    /// 不正・未対応・予算・中断では部分結果を返さない。Building の成分・ID・Hash は変更しない。
    pub fn grid_axes(&self, instance: u64, cx: &mut Cx) -> Result<GridAxes<'_>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let source = self.instance(instance);
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, source.map(Record::span)))?;
            let Some(source) = source else {
                cx.report(Diagnostic::new(codes::IFC_GRID_INVALID).with("reason", "reference"));
                return Err(Error::Invalid);
            };
            let mut reader = Reader {
                file: self,
                cx,
                current: source,
            };
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
            reader.within(instance, |reader, record| reader.run(record, version))
        })
    }
}
struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: &'f Record,
}
impl<'f> Reader<'f, '_, '_> {
    fn check(&mut self, kind: Kind, amount: u64) -> Result<(), Error> {
        self.cx
            .check(kind, amount)
            .map_err(|stop| stopped(self.cx, stop, Some(self.current.span())))
    }
    fn alloc(&mut self, count: usize, size: u64) -> Result<(), Error> {
        self.check(Kind::DecodedBytes, (count as u64).saturating_mul(size))
    }
    fn invalid(&mut self, reason: &'static str) -> Error {
        self.cx.report(
            Diagnostic::new(codes::IFC_GRID_INVALID)
                .with("reason", reason)
                .at(at_bytes(self.current.span())),
        );
        Error::Invalid
    }
    fn within<T>(
        &mut self,
        id: u64,
        f: impl FnOnce(&mut Self, &'f Record) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let record = self
            .file
            .instance(id)
            .ok_or_else(|| self.invalid("reference"))?;
        let previous = self.current;
        self.current = record;
        let result = (|| {
            self.check(Kind::Work, 1)?;
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
    fn is(&self, record: &Record, name: &[u8]) -> bool {
        self.file.kind(record).eq_ignore_ascii_case(name)
    }
    fn arity(&mut self, record: &Record, count: usize) -> Result<(), Error> {
        if record
            .parts()
            .first()
            .is_some_and(|part| part.args().len() == count)
        {
            Ok(())
        } else {
            Err(self.invalid("arity"))
        }
    }
    fn target(&mut self, value: Option<&Value>) -> Result<u64, Error> {
        reference(value).ok_or_else(|| self.invalid("reference"))
    }
    fn text(&mut self, value: Option<&Value>) -> Result<Option<String>, Error> {
        match value {
            Some(Value::Null) => Ok(None),
            Some(Value::Text { raw, .. }) => {
                self.check(Kind::Work, raw.len() as u64)?;
                self.alloc(raw.len(), 12)?;
                self.alloc(1, 32)?;
                let (text, invalid) = decode(self.file.bytes(raw.clone()));
                if invalid {
                    self.cx.report(
                        Diagnostic::new(codes::IFC_TEXT_INVALID).at(at_bytes(self.current.span())),
                    );
                }
                Ok(Some(text))
            }
            _ => Err(self.invalid("text")),
        }
    }
    fn enumeration(&mut self, value: Option<&'f Value>) -> Result<&'f [u8], Error> {
        let Some(Value::Enum(raw)) = value else {
            return Err(self.invalid("enum"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        Ok(self.file.bytes(raw.clone()))
    }
    fn layout(&mut self, grid: &'f Record, version: Version) -> Result<Option<GridLayout>, Error> {
        if version == Version::Ifc2x3 || matches!(arg(grid, 11), Some(Value::Null)) {
            return Ok(None);
        }
        let value = self.enumeration(arg(grid, 11))?;
        for (name, layout) in [
            (b"RECTANGULAR".as_slice(), GridLayout::Rectangular),
            (b"RADIAL", GridLayout::Radial),
            (b"TRIANGULAR", GridLayout::Triangular),
            (b"IRREGULAR", GridLayout::Irregular),
            (b"USERDEFINED", GridLayout::UserDefined),
            (b"NOTDEFINED", GridLayout::NotDefined),
        ] {
            if value.eq_ignore_ascii_case(name) {
                return Ok(Some(layout));
            }
        }
        Err(self.invalid("enum"))
    }
    fn list(&mut self, value: Option<&'f Value>, optional: bool) -> Result<&'f [Value], Error> {
        if optional && matches!(value, Some(Value::Null)) {
            return Ok(&[]);
        }
        let Some(Value::List(values)) = value else {
            return Err(self.invalid("list"));
        };
        if values.is_empty() {
            return Err(self.invalid("count"));
        }
        self.check(Kind::Work, values.len() as u64)?;
        self.alloc(1, 256)?;
        self.alloc(values.len(), 64)?;
        Ok(values)
    }
    fn owners(&mut self, grid: &Record, selected: &BTreeSet<u64>) -> Result<(), Error> {
        for record in self.file.records() {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| stopped(self.cx, stop, Some(record.span())))?;
            if std::ptr::eq(record, grid) || !self.is(record, b"IFCGRID") {
                continue;
            }
            for at in 8..=10 {
                if let Some(Value::List(values)) = arg(record, at) {
                    self.cx
                        .check(Kind::Work, values.len() as u64)
                        .map_err(|stop| stopped(self.cx, stop, Some(record.span())))?;
                    if values.iter().any(|value| {
                        reference(Some(value)).is_some_and(|id| selected.contains(&id))
                    }) {
                        let previous = self.current;
                        self.current = record;
                        let error = self.invalid("owner");
                        self.current = previous;
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }
    fn two_dimensions(&mut self, graph: &CurveGraph) -> Result<(), Error> {
        for item in &graph.items {
            let previous = self.current;
            self.current = item
                .source
                .instance
                .and_then(|id| self.file.instance(id))
                .unwrap_or(previous);
            let result = (|| {
                self.check(Kind::Work, 1)?;
                let two = match &item.kind {
                    CurveKind::Polyline(points) | CurveKind::Indexed { points, .. } => {
                        self.check(Kind::Work, points.len() as u64)?;
                        points.iter().all(|point| point.dimension() == 2)
                    }
                    CurveKind::Line {
                        point, direction, ..
                    } => point.dimension() == 2 && direction.dimension() == 2,
                    CurveKind::Circle { position, .. } => position.location.dimension() == 2,
                    CurveKind::Trimmed { trim1, trim2, .. } => {
                        self.check(
                            Kind::Work,
                            (trim1.len() as u64).saturating_add(trim2.len() as u64),
                        )?;
                        trim1.iter().chain(trim2).all(
                        |value| !matches!(value, CurveTrim::Point(point) if point.dimension() != 2),
                    )
                    }
                    CurveKind::Composite { .. } => true,
                    CurveKind::Unsupported(kind) => {
                        self.check(Kind::Work, kind.len() as u64)?;
                        self.alloc(kind.len(), 6)?;
                        let record = item
                            .source
                            .instance
                            .and_then(|id| self.file.instance(id))
                            .unwrap_or(self.current);
                        self.cx.report(
                            Diagnostic::new(codes::IFC_GRID_UNSUPPORTED)
                                .with("kind", kind.as_str())
                                .at(at_bytes(record.span())),
                        );
                        return Err(Error::Unsupported);
                    }
                };
                if !two {
                    return Err(self.invalid("dimension"));
                }
                Ok(())
            })();
            self.current = previous;
            result?;
        }
        Ok(())
    }
    fn axis(&mut self, id: u64, set: GridSet) -> Result<GridAxisCurve<'f>, Error> {
        self.within(id, |reader, record| {
            if !reader.is(record, b"IFCGRIDAXIS") {
                return Err(reader.invalid("type"));
            }
            reader.arity(record, 3)?;
            let tag = reader.text(arg(record, 1))?;
            let same_sense = match reader.enumeration(arg(record, 3))? {
                value if value.eq_ignore_ascii_case(b"T") => true,
                value if value.eq_ignore_ascii_case(b"F") => false,
                _ => return Err(reader.invalid("sense")),
            };
            let curve_id = reader.target(arg(record, 2))?;
            let curve = reader.file.curve(curve_id, reader.cx)?;
            reader.two_dimensions(&curve)?;
            Ok(GridAxisCurve {
                definition: GridAxisDefinition {
                    set,
                    tag,
                    same_sense,
                },
                curve,
                source: record,
            })
        })
    }
    fn run(&mut self, grid: &'f Record, version: Version) -> Result<GridAxes<'f>, Error> {
        if !self.is(grid, b"IFCGRID") {
            return Err(self.invalid("type"));
        }
        self.arity(grid, if version == Version::Ifc2x3 { 10 } else { 11 })?;
        let placement = if matches!(arg(grid, 6), Some(Value::Null)) {
            None
        } else {
            let placement = self.target(arg(grid, 6))?;
            Some(self.within(placement, |reader, record| {
                if reader.is(record, b"IFCLOCALPLACEMENT")
                    || reader.is(record, b"IFCGRIDPLACEMENT")
                    || (version == Version::Ifc4x3 && reader.is(record, b"IFCLINEARPLACEMENT"))
                {
                    Ok(record)
                } else {
                    Err(reader.invalid("placement"))
                }
            })?)
        };
        let representation = if matches!(arg(grid, 7), Some(Value::Null)) {
            None
        } else {
            let id = self.target(arg(grid, 7))?;
            Some(self.within(id, |reader, record| {
                if reader.is(record, b"IFCPRODUCTDEFINITIONSHAPE")
                    || reader.is(record, b"IFCPRODUCTREPRESENTATION")
                    || reader.is(record, b"IFCMATERIALDEFINITIONREPRESENTATION")
                {
                    Ok(record)
                } else {
                    Err(reader.invalid("representation"))
                }
            })?)
        };
        if placement.is_none() {
            // Grid.HasPlacement は 4.1 で削除。4.3 は Product の条件付き規則に従う。
            if version != Version::Ifc4x3 {
                return Err(self.invalid("placement"));
            }
            if let Some(representation) = representation {
                let id = representation
                    .id()
                    .ok_or_else(|| self.invalid("reference"))?;
                let needs_placement = self.within(id, |reader, record| {
                    let Some(Value::List(values)) = arg(record, 3) else {
                        return Err(reader.invalid("list"));
                    };
                    if values.is_empty() {
                        return Err(reader.invalid("count"));
                    }
                    reader.check(Kind::Work, values.len() as u64)?;
                    let mut shape = false;
                    for value in values {
                        let id = reader.target(Some(value))?;
                        shape |= reader.within(id, |reader, record| {
                            if reader.is(record, b"IFCSHAPEREPRESENTATION") {
                                Ok(true)
                            } else if reader.is(record, b"IFCSTYLEDREPRESENTATION")
                                || reader.is(record, b"IFCTOPOLOGYREPRESENTATION")
                            {
                                Ok(false)
                            } else {
                                Err(reader.invalid("representation"))
                            }
                        })?;
                    }
                    Ok(shape)
                })?;
                if needs_placement {
                    return Err(self.invalid("placement"));
                }
            }
        }
        let layout = self.layout(grid, version)?;
        let lists = [
            self.list(arg(grid, 8), false)?,
            self.list(arg(grid, 9), false)?,
            self.list(arg(grid, 10), true)?,
        ];
        let count = lists
            .iter()
            .fold(0usize, |n, values| n.saturating_add(values.len()));
        if count > i32::MAX as usize / AXIS_BYTES {
            return Err(self.invalid("capacity"));
        }
        let mut selected = BTreeSet::new();
        for value in lists.iter().flat_map(|values| values.iter()) {
            let id = self.target(Some(value))?;
            if !selected.insert(id) {
                return Err(self.invalid("duplicate"));
            }
        }
        self.owners(grid, &selected)?;
        self.alloc(count, AXIS_BYTES as u64)?;
        let mut axes = Vec::with_capacity(count);
        for (values, set) in lists.into_iter().zip([GridSet::U, GridSet::V, GridSet::W]) {
            for value in values {
                let id = self.target(Some(value))?;
                axes.push(self.axis(id, set)?);
            }
        }
        Ok(GridAxes {
            axes,
            layout,
            source: GridSource {
                grid,
                placement,
                representation,
            },
        })
    }
}

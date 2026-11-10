//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC 4.3 ADD2 の IfcGeometryResource の各実体の「属性」に従う。
//! 曲線を評価せず、元の次元・単位・向き・媒介変数を持つ。所有権と探索をともに平らな列にし、再帰しない。

use zumen_base::Ref;
use zumen_building::{
    Coordinates, CurveGraph, CurveItem, CurveKind, CurvePosition, CurveRef, CurveSegment,
    CurveSource, CurveTransition, CurveTrim, IndexedSegment, TrimmingPreference, Unit,
};

use super::{ENTITIES, Lifter, Version, arg, reference};
use crate::{Cx, Diagnostic, Error, IfcFile, Kind, Record, Value, at_bytes, codes, stopped};
use std::collections::{BTreeMap, BTreeSet};

impl IfcFile {
    /// 1 つの曲線を必要なときに建物モデルの型へ写す（feature `building`）。
    ///
    /// Polyline・Line・Circle・IndexedPolyCurve（IFC4 以降）・CompositeCurve・TrimmedCurve を保持する。
    /// 座標と長さは文書の単位と曲線の座標空間のまま、円の媒介変数は文書の平面角の単位のまま返す。配置・写像・
    /// 要素の変換は適用しない。ほかの種類は診断と `Unsupported`、壊れた値や循環は診断と `Error::Invalid` を返す。
    ///
    /// 共有する基の曲線は 1 度だけ写す。この呼び出し内だけで有効な `CurveRef` と、元のインスタンスを分けて返す。
    /// `Building::lift`・要素の ID・ハッシュは変えない。予算と中断で止めたときは部分結果を返さない。
    pub fn curve(&self, instance: u64, cx: &mut Cx) -> Result<CurveGraph, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let Some(version) = Version::of(self.schema()) else {
                cx.report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                    "schema",
                    String::from_utf8_lossy(self.schema()).into_owned(),
                ));
                return Err(Error::Unsupported);
            };
            Reader::new(self, version, cx).run(instance)
        })
    }
}

enum Frame {
    Enter(u64),
    Leave(u64),
}

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    version: Version,
    cx: &'c mut Cx<'b>,
    current: u64,
    sources: Vec<u64>,
    nodes: Vec<Option<CurveItem>>,
    references: BTreeMap<u64, CurveRef>,
    active: BTreeSet<u64>,
    children: Vec<u64>,
    angle_used: bool,
}

impl<'f, 'c, 'b> Reader<'f, 'c, 'b> {
    fn new(file: &'f IfcFile, version: Version, cx: &'c mut Cx<'b>) -> Self {
        Self {
            file,
            version,
            cx,
            current: 0,
            sources: Vec::new(),
            nodes: Vec::new(),
            references: BTreeMap::new(),
            active: BTreeSet::new(),
            children: Vec::new(),
            angle_used: false,
        }
    }

    fn check(&mut self, kind: Kind, amount: u64) -> Result<(), Error> {
        self.cx.check(kind, amount).map_err(|stop| {
            stopped(
                self.cx,
                stop,
                self.file.instance(self.current).map(Record::span),
            )
        })
    }

    // 各項目の予約量は 64 bit と wasm32 で共通にする。size_of は使わない
    fn alloc(&mut self, count: usize, slot: u64) -> Result<(), Error> {
        self.check(Kind::DecodedBytes, slot.saturating_mul(count as u64))
    }

    fn invalid(&mut self, reason: &'static str) -> Error {
        let place = self
            .file
            .instance(self.current)
            .map_or_else(Ref::default, |r| at_bytes(r.span()));
        self.cx.report(
            Diagnostic::new(codes::IFC_CURVE_INVALID)
                .with("reason", reason)
                .at(place.with_id(format!("i{}", self.current))),
        );
        Error::Invalid
    }

    fn record(&mut self, id: u64) -> Result<&'f Record, Error> {
        self.check(Kind::Work, 1)?;
        let record = self
            .file
            .instance(id)
            .ok_or_else(|| self.invalid("reference"))?;
        if id != self.current {
            self.alloc(1, 8)?;
            self.sources.push(id);
        }
        Ok(record)
    }

    fn is(&self, record: &Record, kind: &[u8]) -> bool {
        self.file.kind(record).eq_ignore_ascii_case(kind)
    }

    fn of(&mut self, id: u64, kind: &[u8]) -> Result<&'f Record, Error> {
        let record = self.record(id)?;
        if self.is(record, kind) {
            Ok(record)
        } else {
            Err(self.invalid("type"))
        }
    }

    fn target(&mut self, record: &Record, at: usize) -> Result<u64, Error> {
        reference(arg(record, at)).ok_or_else(|| self.invalid("reference"))
    }

    fn list(&mut self, value: Option<&'f Value>) -> Result<&'f [Value], Error> {
        match value {
            Some(Value::List(values)) => {
                self.check(Kind::Work, values.len() as u64)?;
                Ok(values)
            }
            _ => Err(self.invalid("list")),
        }
    }

    fn number(&mut self, value: Option<&Value>) -> Result<f64, Error> {
        let range = match value {
            Some(Value::Number(range)) => range,
            _ => return Err(self.invalid("number")),
        };
        self.check(Kind::Work, range.len() as u64)?;
        core::str::from_utf8(self.file.bytes(range.clone()))
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| self.invalid("number"))
    }

    fn enumeration(&mut self, value: Option<&Value>) -> Result<&'f [u8], Error> {
        match value {
            Some(Value::Enum(range)) => {
                self.check(Kind::Work, range.len() as u64)?;
                Ok(self.file.bytes(range.clone()))
            }
            _ => Err(self.invalid("enumeration")),
        }
    }

    fn flag(&mut self, value: Option<&Value>, unknown: bool) -> Result<Option<bool>, Error> {
        match self.enumeration(value)? {
            b"T" => Ok(Some(true)),
            b"F" => Ok(Some(false)),
            b"U" if unknown => Ok(None),
            _ => Err(self.invalid("boolean")),
        }
    }

    fn coordinates(&mut self, value: Option<&'f Value>) -> Result<Coordinates, Error> {
        let values = self.list(value)?;
        match values {
            [x, y] => Ok(Coordinates::Two([
                self.number(Some(x))?,
                self.number(Some(y))?,
            ])),
            [x, y, z] => Ok(Coordinates::Three([
                self.number(Some(x))?,
                self.number(Some(y))?,
                self.number(Some(z))?,
            ])),
            _ => Err(self.invalid("dimension")),
        }
    }

    fn point(&mut self, id: u64) -> Result<Coordinates, Error> {
        let record = self.of(id, b"IFCCARTESIANPOINT")?;
        self.coordinates(arg(record, 1))
    }

    fn direction(&mut self, id: u64) -> Result<Coordinates, Error> {
        let record = self.of(id, b"IFCDIRECTION")?;
        let direction = self.coordinates(arg(record, 1))?;
        let nonzero = match direction {
            Coordinates::Two(v) => v.iter().any(|&x| x != 0.0),
            Coordinates::Three(v) => v.iter().any(|&x| x != 0.0),
        };
        if nonzero {
            Ok(direction)
        } else {
            Err(self.invalid("direction"))
        }
    }

    fn reference(&mut self, id: u64) -> Result<CurveRef, Error> {
        self.check(Kind::Work, 1)?;
        if let Some(&reference) = self.references.get(&id) {
            return Ok(reference);
        }
        self.alloc(1, 256)?;
        // 木の索引と探索の状態のための領域も、確保する前に数える。幅の差で環境ごとに量を変えない
        self.check(Kind::DecodedBytes, 256)?;
        let reference =
            CurveRef(u32::try_from(self.nodes.len()).map_err(|_| self.invalid("count"))?);
        self.nodes.push(None);
        self.references.insert(id, reference);
        Ok(reference)
    }

    fn child(&mut self, id: u64) -> Result<CurveRef, Error> {
        let reference = self.reference(id)?;
        self.alloc(1, 8)?;
        self.children.push(id);
        Ok(reference)
    }

    fn run(mut self, id: u64) -> Result<CurveGraph, Error> {
        self.current = id;
        let root = self.reference(id)?;
        self.alloc(1, 24)?;
        let mut stack = vec![Frame::Enter(id)];
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
                        let reference = self
                            .references
                            .get(&id)
                            .copied()
                            .ok_or_else(|| self.invalid("reference"))?;
                        if self
                            .nodes
                            .get(reference.0 as usize)
                            .is_some_and(Option::is_some)
                        {
                            continue;
                        }
                        self.cx.enter().map_err(|stop| {
                            stopped(self.cx, stop, self.file.instance(id).map(Record::span))
                        })?;
                        entered += 1;
                        self.active.insert(id);
                        self.sources = Vec::new();
                        self.children = Vec::new();
                        let kind = self.parse(id)?;
                        let item = CurveItem {
                            kind,
                            source: CurveSource {
                                instance: Some(id),
                                records: core::mem::take(&mut self.sources),
                            },
                        };
                        if let Some(slot) = self.nodes.get_mut(reference.0 as usize) {
                            *slot = Some(item);
                        }
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
            let (length_unit, angle_unit) = self.units()?;
            if self.angle_used && angle_unit.is_none() {
                self.cx.report(
                    Diagnostic::new(codes::IFC_UNIT_MISSING)
                        .with("type", "IfcPlaneAngleMeasure")
                        .at(at_bytes(
                            self.file.instance(id).map(Record::span).unwrap_or(0..0),
                        )),
                );
            }
            self.alloc(self.nodes.len(), 256)?;
            let items = core::mem::take(&mut self.nodes)
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| self.invalid("reference"))?;
            Ok(CurveGraph {
                root,
                items,
                length_unit,
                angle_unit,
            })
        })();
        for _ in 0..entered {
            self.cx.leave();
        }
        result
    }

    fn parse(&mut self, id: u64) -> Result<CurveKind, Error> {
        let record = self.record(id)?;
        let kind = self.file.kind(record);
        if kind.eq_ignore_ascii_case(b"IFCPOLYLINE") {
            let values = self.list(arg(record, 1))?;
            if values.len() < 2 {
                return Err(self.invalid("count"));
            }
            self.alloc(values.len(), 32)?;
            let mut points = Vec::with_capacity(values.len());
            for value in values {
                let id = reference(Some(value)).ok_or_else(|| self.invalid("reference"))?;
                let point = self.point(id)?;
                if points
                    .first()
                    .is_some_and(|p: &Coordinates| p.dimension() != point.dimension())
                {
                    return Err(self.invalid("dimension"));
                }
                points.push(point);
            }
            Ok(CurveKind::Polyline(points))
        } else if kind.eq_ignore_ascii_case(b"IFCLINE") {
            let point_id = self.target(record, 1)?;
            let point = self.point(point_id)?;
            let vector_id = self.target(record, 2)?;
            let vector = self.of(vector_id, b"IFCVECTOR")?;
            let direction_id = self.target(vector, 1)?;
            let direction = self.direction(direction_id)?;
            let magnitude = self.number(arg(vector, 2))?;
            if point.dimension() != direction.dimension() || magnitude <= 0.0 {
                return Err(self.invalid("direction"));
            }
            Ok(CurveKind::Line {
                point,
                direction,
                magnitude,
            })
        } else if kind.eq_ignore_ascii_case(b"IFCCIRCLE") {
            self.angle_used = true;
            let position_id = self.target(record, 1)?;
            let position = self.position(position_id)?;
            let radius = self.number(arg(record, 2))?;
            if radius <= 0.0 {
                return Err(self.invalid("radius"));
            }
            Ok(CurveKind::Circle { position, radius })
        } else if kind.eq_ignore_ascii_case(b"IFCINDEXEDPOLYCURVE")
            && self.version != Version::Ifc2x3
        {
            self.indexed(record)
        } else if kind.eq_ignore_ascii_case(b"IFCCOMPOSITECURVE") {
            self.composite(record)
        } else if kind.eq_ignore_ascii_case(b"IFCTRIMMEDCURVE") {
            let basis_id = self.target(record, 1)?;
            let basis = self.child(basis_id)?;
            let trim1 = self.trims(arg(record, 2))?;
            let trim2 = self.trims(arg(record, 3))?;
            let sense_agreement = self.flag(arg(record, 4), false)?.unwrap_or(false);
            let preference = match self.enumeration(arg(record, 5))? {
                b"CARTESIAN" => TrimmingPreference::Cartesian,
                b"PARAMETER" => TrimmingPreference::Parameter,
                b"UNSPECIFIED" => TrimmingPreference::Unspecified,
                _ => return Err(self.invalid("enumeration")),
            };
            Ok(CurveKind::Trimmed {
                basis,
                trim1,
                trim2,
                sense_agreement,
                preference,
            })
        } else {
            self.check(Kind::Work, kind.len() as u64)?;
            self.check(Kind::DecodedBytes, kind.len() as u64 * 6)?;
            let name = String::from_utf8_lossy(kind).into_owned();
            self.cx.report(
                Diagnostic::new(codes::IFC_CURVE_UNSUPPORTED)
                    .with("kind", name.as_str())
                    .at(at_bytes(record.span())),
            );
            Ok(CurveKind::Unsupported(name))
        }
    }

    fn position(&mut self, id: u64) -> Result<CurvePosition, Error> {
        let record = self.record(id)?;
        let three = self.is(record, b"IFCAXIS2PLACEMENT3D");
        if !three && !self.is(record, b"IFCAXIS2PLACEMENT2D") {
            return Err(self.invalid("placement"));
        }
        let point_id = self.target(record, 1)?;
        let location = self.point(point_id)?;
        if location.dimension() != if three { 3 } else { 2 } {
            return Err(self.invalid("dimension"));
        }
        let axis = if three && !matches!(arg(record, 2), Some(Value::Null)) {
            let direction_id = self.target(record, 2)?;
            match self.direction(direction_id)? {
                Coordinates::Three(v) => Some(v),
                _ => return Err(self.invalid("dimension")),
            }
        } else {
            None
        };
        let at = if three { 3 } else { 2 };
        let reference_direction = if matches!(arg(record, at), Some(Value::Null)) {
            None
        } else {
            let direction_id = self.target(record, at)?;
            let direction = self.direction(direction_id)?;
            if direction.dimension() != location.dimension() {
                return Err(self.invalid("dimension"));
            }
            Some(direction)
        };
        Ok(CurvePosition {
            location,
            axis,
            reference_direction,
        })
    }

    fn indexed(&mut self, record: &'f Record) -> Result<CurveKind, Error> {
        let points_id = self.target(record, 1)?;
        let list = self.record(points_id)?;
        let dimension = if self.is(list, b"IFCCARTESIANPOINTLIST2D") {
            2
        } else if self.is(list, b"IFCCARTESIANPOINTLIST3D") {
            3
        } else {
            return Err(self.invalid("type"));
        };
        let values = self.list(arg(list, 1))?;
        if values.len() < 2 {
            return Err(self.invalid("count"));
        }
        self.alloc(values.len(), 32)?;
        let mut points = Vec::with_capacity(values.len());
        for value in values {
            let point = self.coordinates(Some(value))?;
            if point.dimension() != dimension {
                return Err(self.invalid("dimension"));
            }
            points.push(point);
        }
        let segments = if matches!(arg(record, 2), Some(Value::Null)) {
            None
        } else {
            let values = self.list(arg(record, 2))?;
            if values.is_empty() {
                return Err(self.invalid("count"));
            }
            self.alloc(values.len(), 32)?;
            let mut segments = Vec::with_capacity(values.len());
            let mut previous = None;
            for value in values {
                let Value::Typed(part) = value else {
                    return Err(self.invalid("indices"));
                };
                if part.args().len() != 1 {
                    return Err(self.invalid("indices"));
                }
                let indices = self.list(part.args().first())?;
                let arc = self
                    .file
                    .bytes(part.name())
                    .eq_ignore_ascii_case(b"IFCARCINDEX");
                if !arc
                    && !self
                        .file
                        .bytes(part.name())
                        .eq_ignore_ascii_case(b"IFCLINEINDEX")
                {
                    return Err(self.invalid("indices"));
                }
                if indices.len() < 2 || (arc && indices.len() != 3) {
                    return Err(self.invalid("indices"));
                }
                self.alloc(indices.len(), 4)?;
                let mut found = Vec::with_capacity(indices.len());
                for value in indices {
                    let Value::Number(range) = value else {
                        return Err(self.invalid("indices"));
                    };
                    self.check(Kind::Work, range.len() as u64)?;
                    let index = core::str::from_utf8(self.file.bytes(range.clone()))
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                        .filter(|&n| n > 0 && u64::from(n) <= points.len() as u64)
                        .ok_or_else(|| self.invalid("indices"))?;
                    found.push(index);
                }
                if previous.is_some() && previous != found.first().copied() {
                    return Err(self.invalid("connectivity"));
                }
                previous = found.last().copied();
                if arc {
                    let indices: [u32; 3] =
                        found.try_into().map_err(|_| self.invalid("indices"))?;
                    segments.push(IndexedSegment::Arc(indices));
                } else {
                    segments.push(IndexedSegment::Line(found));
                }
            }
            Some(segments)
        };
        let self_intersect = if matches!(arg(record, 3), Some(Value::Null)) {
            None
        } else {
            self.flag(arg(record, 3), false)?
        };
        Ok(CurveKind::Indexed {
            points,
            segments,
            self_intersect,
        })
    }

    fn composite(&mut self, record: &'f Record) -> Result<CurveKind, Error> {
        let values = self.list(arg(record, 1))?;
        if values.is_empty() {
            return Err(self.invalid("count"));
        }
        self.alloc(values.len(), 32)?;
        let mut segments = Vec::with_capacity(values.len());
        for value in values {
            let id = reference(Some(value)).ok_or_else(|| self.invalid("reference"))?;
            let segment = self.of(id, b"IFCCOMPOSITECURVESEGMENT")?;
            let transition = match self.enumeration(arg(segment, 1))? {
                b"DISCONTINUOUS" => CurveTransition::Discontinuous,
                b"CONTINUOUS" => CurveTransition::Continuous,
                b"CONTSAMEGRADIENT" => CurveTransition::SameGradient,
                b"CONTSAMEGRADIENTSAMECURVATURE" => CurveTransition::SameGradientSameCurvature,
                _ => return Err(self.invalid("enumeration")),
            };
            let same_sense = self.flag(arg(segment, 2), false)?.unwrap_or(false);
            let target = self.target(segment, 3)?;
            let curve = self.child(target)?;
            segments.push(CurveSegment {
                curve,
                same_sense,
                transition,
                instance: Some(id),
            });
        }
        let self_intersect = self.flag(arg(record, 2), true)?;
        Ok(CurveKind::Composite {
            segments,
            self_intersect,
        })
    }

    fn trims(&mut self, value: Option<&'f Value>) -> Result<Vec<CurveTrim>, Error> {
        let values = self.list(value)?;
        if values.is_empty() || values.len() > 2 {
            return Err(self.invalid("trim"));
        }
        self.alloc(values.len(), 48)?;
        let mut trims = Vec::with_capacity(values.len());
        for value in values {
            let trim = match value {
                Value::Ref(id) => CurveTrim::Point(self.point(*id)?),
                Value::Typed(part)
                    if self
                        .file
                        .bytes(part.name())
                        .eq_ignore_ascii_case(b"IFCPARAMETERVALUE")
                        && part.args().len() == 1 =>
                {
                    CurveTrim::Parameter(self.number(part.args().first())?)
                }
                _ => return Err(self.invalid("trim")),
            };
            if trims.iter().any(|v| {
                matches!(
                    (v, trim),
                    (CurveTrim::Point(_), CurveTrim::Point(_))
                        | (CurveTrim::Parameter(_), CurveTrim::Parameter(_))
                )
            }) {
                return Err(self.invalid("trim"));
            }
            trims.push(trim);
        }
        Ok(trims)
    }

    fn units(&mut self) -> Result<(Option<Unit>, Option<Unit>), Error> {
        let mut project = None;
        for id in self.file.ids() {
            self.check(Kind::Work, 1)?;
            if self
                .file
                .instance(id)
                .is_some_and(|r| self.is(r, b"IFCPROJECT"))
            {
                project = Some(id);
                break;
            }
        }
        let assignment = project
            .and_then(|id| self.file.instance(id))
            .and_then(|r| reference(arg(r, 9)));
        let Some(assignment) = assignment else {
            return Ok((None, None));
        };
        let assignment = self.of(assignment, b"IFCUNITASSIGNMENT")?;
        let values = self.list(arg(assignment, 1))?;
        // 既存の単位の読み取りを共有する。種類の表・索引の領域も、作る前に数える
        self.check(Kind::DecodedBytes, ENTITIES.len() as u64 * 256)?;
        let mut lifter = Lifter::new(self.file, self.version);
        let (mut length, mut angle) = (None, None);
        for value in values {
            let id = reference(Some(value)).ok_or_else(|| self.invalid("reference"))?;
            let record = self.record(id)?;
            // 単位の文字と、換算の係数の先の SI 単位を写す前に大きさを数える
            self.check(Kind::Work, record.span().len() as u64)?;
            self.check(Kind::DecodedBytes, 256 + record.span().len() as u64 * 4)?;
            if self.is(record, b"IFCCONVERSIONBASEDUNIT") {
                let factor_id = self.target(record, 4)?;
                let factor = self.record(factor_id)?;
                if !self.is(factor, b"IFCMEASUREWITHUNIT") {
                    return Err(self.invalid("unit"));
                }
                // 既存の数の読み取りへ、任意の深さの型付きの値を渡さない
                let value = match arg(factor, 1) {
                    Some(Value::Typed(part)) if part.args().len() == 1 => part.args().first(),
                    value => value,
                };
                if self.number(value)? <= 0.0 {
                    return Err(self.invalid("unit"));
                }
                let base_id = self.target(factor, 2)?;
                let base = self.record(base_id)?;
                let bytes = (factor.span().len() as u64).saturating_add(base.span().len() as u64);
                self.check(Kind::Work, bytes)?;
                self.check(Kind::DecodedBytes, 512 + bytes * 4)?;
            }
            if let Some(unit) = lifter.unit(id, self.cx) {
                if unit
                    .scale
                    .is_some_and(|scale| !scale.is_finite() || scale <= 0.0)
                {
                    return Err(self.invalid("unit"));
                }
                match unit.kind.as_str() {
                    "LENGTHUNIT" if length.is_none() => length = Some(unit),
                    "PLANEANGLEUNIT" if angle.is_none() => angle = Some(unit),
                    _ => {}
                }
            }
            lifter.halted(self.cx, Some(record.span()))?;
        }
        Ok((length, angle))
    }
}

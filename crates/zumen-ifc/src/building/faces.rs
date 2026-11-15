//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcFacetedBrep、IfcFaceBasedSurfaceModel、
//! IfcShellBasedSurfaceModel と、TopologyResource の ConnectedFaceSet / Face / FaceBound /
//! FaceOuterBound / PolyLoop の属性と自然言語の説明に従う。EXPRESS は使わない。
//! PolyLoop は重複しない 3D の点を 3 個以上持ち、閉じる点は末尾に含めない。
//! 平面性、辺の接続、ループの交差や内包、閉殻・立体の有効性は評価しない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_building::{
    ConnectedFaces, FaceBoundRef, FaceBoundary, FaceGraph, FaceLoopRef, FaceModelKind,
    FacePointRef, FacePolygon, FaceRef, FaceSetRef, PolygonFace, Unit,
};

use super::{Version, arg, extrusion::face_length_unit, reference};
use crate::{
    Cx, Diagnostic, Error, Form, IfcFile, Kind, Record, ShapeNode, Value, at_bytes, codes, stopped,
};

/// 面集合の元の値。配置・写像・単位換算・三角形化は未適用
#[derive(Clone, Debug)]
pub struct FacetedFaces<'f> {
    /// IFC と出所に依存しない面集合の定義
    pub definition: FaceGraph,
    /// 点の元の長さの単位。なければ None と診断を返す
    pub length_unit: Option<Unit>,
    /// 各表と同じ順の元のレコード
    pub source: FaceGraphSource<'f>,
}

/// 定義とは分けて持つ借用出所。各列は FaceGraph の同名の列に対応する
#[derive(Clone, Debug)]
pub struct FaceGraphSource<'f> {
    /// 形の葉
    pub item: &'f Record,
    /// 面集合
    pub sets: Vec<&'f Record>,
    /// 面
    pub faces: Vec<&'f Record>,
    /// 境界
    pub bounds: Vec<&'f Record>,
    /// 多角形のループ
    pub loops: Vec<&'f Record>,
    /// 点
    pub points: Vec<&'f Record>,
    /// 単位を読むためにたどったレコード
    pub units: Vec<&'f Record>,
}

impl IfcFile {
    /// ProductShape の葉から、多角形の面集合の元値と共有を必要時に取得する。
    ///
    /// FacetedBrep、ShellBasedSurfaceModel、FaceBasedSurfaceModel を扱う。
    /// 外輪の指定と向きは元のまま保つ。閉殻の申告を検証済みとはみなさない。
    /// 未対応の曲面・辺のループ・添字の面は種類と場所を診断する。
    /// 同じ IfcFile の ShapeNode を要求し、不正・未対応・停止で部分結果を返さない。
    pub fn faceted_faces<'f>(
        &'f self,
        node: &ShapeNode<'f>,
        cx: &mut Cx,
    ) -> Result<FacetedFaces<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                index: BTreeMap::new(),
                graph: FaceGraph {
                    kind: FaceModelKind::FacetedSolid,
                    roots: Vec::new(),
                    sets: Vec::new(),
                    faces: Vec::new(),
                    bounds: Vec::new(),
                    loops: Vec::new(),
                    points: Vec::new(),
                },
                source: FaceGraphSource {
                    item: node.source,
                    sets: Vec::new(),
                    faces: Vec::new(),
                    bounds: Vec::new(),
                    loops: Vec::new(),
                    points: Vec::new(),
                    units: Vec::new(),
                },
            };
            reader.check(Kind::Work, 1)?;
            let root = self
                .instance(reader.current)
                .filter(|r| std::ptr::eq(*r, node.source))
                .ok_or_else(|| reader.invalid("face-source"))?;
            reader.source.item = root;
            reader.check(Kind::Work, self.schema().len() as u64)?;
            let Some(version) = Version::of(self.schema()) else {
                reader.alloc(self.schema().len(), 3)?;
                reader
                    .cx
                    .report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ));
                return Err(Error::Unsupported);
            };
            reader.depth(|r| r.run(root, version))
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Set,
    Face,
    Bound,
    Loop,
    Point,
}

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: u64,
    index: BTreeMap<u64, (Role, u32)>,
    graph: FaceGraph,
    source: FaceGraphSource<'f>,
}

impl<'f> Reader<'f, '_, '_> {
    fn run(&mut self, root: &'f Record, version: Version) -> Result<FacetedFaces<'f>, Error> {
        self.record(root.id().unwrap_or(0))?;
        self.alloc(1, 1024)?;
        let kind = self.file.kind(root);
        self.graph.kind = if kind.eq_ignore_ascii_case(b"IFCFACETEDBREP") {
            FaceModelKind::FacetedSolid
        } else if kind.eq_ignore_ascii_case(b"IFCSHELLBASEDSURFACEMODEL") {
            FaceModelKind::ShellSurface
        } else if kind.eq_ignore_ascii_case(b"IFCFACEBASEDSURFACEMODEL") {
            FaceModelKind::FaceSurface
        } else {
            return Err(self.unsupported(root));
        };
        self.attributes(root, 1)?;
        let roots = if self.graph.kind == FaceModelKind::FacetedSolid {
            self.alloc(1, 64)?;
            vec![self.target(arg(root, 1))?]
        } else {
            self.references(root, 1, 1)?
        };
        for id in roots {
            let at = self.node(id, Role::Set)?;
            self.current = id;
            let closed = self
                .graph
                .sets
                .get(at as usize)
                .map(|set| set.closed)
                .ok_or_else(|| self.invalid("index"))?;
            if (self.graph.kind == FaceModelKind::FacetedSolid && closed != Some(true))
                || (self.graph.kind == FaceModelKind::ShellSurface && closed.is_none())
            {
                return Err(self.invalid("shell-kind"));
            }
            self.graph.roots.push(FaceSetRef(at));
        }
        let (length_unit, units) = face_length_unit(self.file, root, version, self.cx)?;
        self.source.units = units;
        if length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(root.span())),
            );
        }
        // 結果へ所有権を移す。読み取り用の索引は返さない。
        let definition = FaceGraph {
            kind: self.graph.kind,
            roots: core::mem::take(&mut self.graph.roots),
            sets: core::mem::take(&mut self.graph.sets),
            faces: core::mem::take(&mut self.graph.faces),
            bounds: core::mem::take(&mut self.graph.bounds),
            loops: core::mem::take(&mut self.graph.loops),
            points: core::mem::take(&mut self.graph.points),
        };
        let source = FaceGraphSource {
            item: root,
            sets: core::mem::take(&mut self.source.sets),
            faces: core::mem::take(&mut self.source.faces),
            bounds: core::mem::take(&mut self.source.bounds),
            loops: core::mem::take(&mut self.source.loops),
            points: core::mem::take(&mut self.source.points),
            units: core::mem::take(&mut self.source.units),
        };
        Ok(FacetedFaces {
            definition,
            length_unit,
            source,
        })
    }
    fn node(&mut self, id: u64, role: Role) -> Result<u32, Error> {
        self.current = id;
        self.check(Kind::Work, 1)?;
        if let Some(&(found, at)) = self.index.get(&id) {
            return if found == role {
                Ok(at)
            } else {
                Err(self.invalid("reference-type"))
            };
        }
        self.depth(|r| {
            let record = r.record(id)?;
            // 表・借用出所・索引の伸長を、プラットフォームによらない上限で先に数える。
            r.alloc(1, 256)?;
            let at = r.item(record, role)?;
            r.index.insert(id, (role, at));
            Ok(at)
        })
    }
    fn item(&mut self, record: &'f Record, role: Role) -> Result<u32, Error> {
        let kind = self.file.kind(record);
        let id = record.id().unwrap_or(0);
        match role {
            Role::Set => {
                let closed = if kind.eq_ignore_ascii_case(b"IFCCLOSEDSHELL") {
                    Some(true)
                } else if kind.eq_ignore_ascii_case(b"IFCOPENSHELL") {
                    Some(false)
                } else if kind.eq_ignore_ascii_case(b"IFCCONNECTEDFACESET") {
                    None
                } else {
                    return Err(self.invalid("face-set-type"));
                };
                self.attributes(record, 1)?;
                let ids = self.references(record, 1, 1)?;
                let mut faces = Vec::with_capacity(ids.len());
                for child in ids {
                    faces.push(FaceRef(self.node(child, Role::Face)?));
                }
                let at = self.slot(self.graph.sets.len())?;
                self.graph.sets.push(ConnectedFaces { faces, closed });
                self.source.sets.push(record);
                Ok(at)
            }
            Role::Face => {
                if !kind.eq_ignore_ascii_case(b"IFCFACE") {
                    return Err(self.unsupported(record));
                }
                self.attributes(record, 1)?;
                let ids = self.references(record, 1, 1)?;
                let mut bounds = Vec::with_capacity(ids.len());
                let mut outer = 0;
                for child in ids {
                    let at = self.node(child, Role::Bound)?;
                    outer += u32::from(
                        self.graph
                            .bounds
                            .get(at as usize)
                            .map(|bound| bound.outer)
                            .ok_or_else(|| self.invalid("index"))?,
                    );
                    if outer > 1 {
                        self.current = id;
                        return Err(self.invalid("outer-bound"));
                    }
                    bounds.push(FaceBoundRef(at));
                }
                let at = self.slot(self.graph.faces.len())?;
                self.graph.faces.push(PolygonFace { bounds });
                self.source.faces.push(record);
                Ok(at)
            }
            Role::Bound => {
                let outer = if kind.eq_ignore_ascii_case(b"IFCFACEOUTERBOUND") {
                    true
                } else if kind.eq_ignore_ascii_case(b"IFCFACEBOUND") {
                    false
                } else {
                    return Err(self.invalid("face-bound-type"));
                };
                self.attributes(record, 2)?;
                let orientation = match arg(record, 2) {
                    Some(Value::Enum(raw))
                        if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"T") =>
                    {
                        true
                    }
                    Some(Value::Enum(raw))
                        if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"F") =>
                    {
                        false
                    }
                    _ => return Err(self.invalid("orientation")),
                };
                let child = self.target(arg(record, 1))?;
                let polygon = FaceLoopRef(self.node(child, Role::Loop)?);
                let at = self.slot(self.graph.bounds.len())?;
                self.graph.bounds.push(FaceBoundary {
                    polygon,
                    orientation,
                    outer,
                });
                self.source.bounds.push(record);
                Ok(at)
            }
            Role::Loop => {
                if !kind.eq_ignore_ascii_case(b"IFCPOLYLOOP") {
                    return Err(self.unsupported(record));
                }
                self.attributes(record, 1)?;
                let ids = self.references(record, 1, 3)?;
                self.alloc(ids.len(), 128)?;
                let mut points = Vec::with_capacity(ids.len());
                let mut coordinates = BTreeSet::new();
                for child in ids {
                    let at = self.node(child, Role::Point)?;
                    let key = self
                        .graph
                        .points
                        .get(at as usize)
                        .copied()
                        .ok_or_else(|| self.invalid("index"))?
                        .map(|v| if v == 0. { 0 } else { v.to_bits() });
                    if !coordinates.insert(key) {
                        self.current = id;
                        return Err(self.invalid("repeated-point"));
                    }
                    points.push(FacePointRef(at));
                }
                let at = self.slot(self.graph.loops.len())?;
                self.graph.loops.push(FacePolygon { points });
                self.source.loops.push(record);
                Ok(at)
            }
            Role::Point => {
                if !kind.eq_ignore_ascii_case(b"IFCCARTESIANPOINT") {
                    return Err(self.invalid("face-point-type"));
                }
                self.attributes(record, 1)?;
                let Some(Value::List(values)) = arg(record, 1) else {
                    return Err(self.invalid("coordinates"));
                };
                if values.len() != 3 {
                    return Err(self.invalid("dimension"));
                }
                let mut point = [0.; 3];
                for (slot, value) in point.iter_mut().zip(values) {
                    let Value::Number(raw) = value else {
                        return Err(self.invalid("number"));
                    };
                    self.check(Kind::Work, raw.len() as u64)?;
                    *slot = std::str::from_utf8(self.file.bytes(raw.clone()))
                        .ok()
                        .and_then(|s| s.parse::<f64>().ok())
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| self.invalid("number"))?;
                }
                let at = self.slot(self.graph.points.len())?;
                self.graph.points.push(point);
                self.source.points.push(record);
                Ok(at)
            }
        }
    }
    fn references(&mut self, record: &Record, at: usize, min: usize) -> Result<Vec<u64>, Error> {
        let Some(Value::List(values)) = arg(record, at) else {
            return Err(self.invalid("list"));
        };
        if values.len() < min {
            return Err(self.invalid("list-length"));
        }
        self.alloc(values.len(), 128)?;
        let mut result = Vec::with_capacity(values.len());
        let mut seen = BTreeSet::new();
        for value in values {
            self.check(Kind::Work, 1)?;
            let id = self.target(Some(value))?;
            if !seen.insert(id) {
                return Err(self.invalid("duplicate-reference"));
            }
            result.push(id);
        }
        Ok(result)
    }
    fn record(&mut self, id: u64) -> Result<&'f Record, Error> {
        self.current = id;
        self.check(Kind::Work, 1)?;
        let record = self
            .file
            .instance(id)
            .ok_or_else(|| self.invalid("reference"))?;
        self.check(Kind::Work, self.file.kind(record).len() as u64)?;
        if record.form() != Form::Entity || record.parts().len() != 1 {
            return Err(self.unsupported(record));
        }
        self.check(Kind::Elements, 1)?;
        Ok(record)
    }
    fn attributes(&mut self, record: &Record, count: usize) -> Result<(), Error> {
        if record.parts().first().map_or(0, |p| p.args().len()) != count {
            return Err(self.invalid("attributes"));
        }
        Ok(())
    }
    fn target(&mut self, value: Option<&Value>) -> Result<u64, Error> {
        reference(value).ok_or_else(|| self.invalid("reference"))
    }
    fn slot(&mut self, len: usize) -> Result<u32, Error> {
        u32::try_from(len).map_err(|_| self.invalid("index"))
    }
    fn depth<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        self.cx.enter().map_err(|s| {
            stopped(
                self.cx,
                s,
                self.file.instance(self.current).map(Record::span),
            )
        })?;
        let result = f(self);
        self.cx.leave();
        result
    }
    fn check(&mut self, kind: Kind, count: u64) -> Result<(), Error> {
        self.cx.check(kind, count).map_err(|s| {
            stopped(
                self.cx,
                s,
                self.file.instance(self.current).map(Record::span),
            )
        })
    }
    fn alloc(&mut self, count: usize, bytes: u64) -> Result<(), Error> {
        self.check(Kind::DecodedBytes, (count as u64).saturating_mul(bytes))
    }
    fn place(&self) -> zumen_base::Ref {
        self.file
            .instance(self.current)
            .map_or_else(zumen_base::Ref::default, |r| at_bytes(r.span()))
            .with_id(format!("i{}", self.current))
    }
    fn invalid(&mut self, reason: &'static str) -> Error {
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_INVALID)
                .with("reason", reason)
                .at(self.place()),
        );
        Error::Invalid
    }
    fn unsupported(&mut self, record: &Record) -> Error {
        self.current = record.id().unwrap_or(0);
        let kind = self.file.kind(record);
        if let Err(error) = self.alloc(kind.len(), 3) {
            return error;
        }
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_UNSUPPORTED)
                .with("kind", String::from_utf8_lossy(kind).into_owned())
                .at(self.place()),
        );
        Error::Unsupported
    }
}

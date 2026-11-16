//! IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcTriangulatedFaceSet / IfcPolygonalFaceSet、
//! IfcIndexedPolygonalFace / WithVoids、IfcCartesianPointList3D の自然言語と属性の説明に従う。
//! CoordIndex は座標には直接または PnIndex を通して、Normals には直接対応する。
//! IFC4.3 の点のタグも保持する。平面性・縮退・法線の向き・閉殻の有効性は評価しない。

use std::collections::BTreeMap;

use zumen_building::{IndexedFaceSet, IndexedFaces, IndexedPolygon, IndexedPolygonRef, Unit};

use super::{Version, arg, extrusion::face_length_unit, reference};
use crate::{
    Cx, Diagnostic, Error, Form, IfcFile, Kind, Record, ShapeNode, Value, at_bytes, codes, stopped,
};

/// 添字付きの面集合。座標・法線・点の対応・穴・タグの元値を持つ
#[derive(Clone, Debug)]
pub struct TessellatedFaces<'f> {
    /// IFC や出所に依存しない定義
    pub definition: IndexedFaceSet,
    /// 点の元の長さの単位。換算は未適用
    pub length_unit: Option<Unit>,
    /// 元のレコードの借用出所
    pub source: IndexedFaceSource<'f>,
}

/// 添字付き面集合の出所。意味の値に STEP 番号を含めない
#[derive(Clone, Debug)]
pub struct IndexedFaceSource<'f> {
    /// 形の葉
    pub item: &'f Record,
    /// 点の列とタグ
    pub coordinates: &'f Record,
    /// 多角形の面。IndexedFaces::Polygons::faces と同じ順。三角形では空
    pub faces: Vec<&'f Record>,
    /// 単位を読むためにたどった元のレコード
    pub units: Vec<&'f Record>,
}

impl IfcFile {
    /// ProductShape の葉から、IFC4 以降の三角形・多角形の面集合を必要時に取得する。
    ///
    /// PnIndex は座標に適用せず、1 から始まる元の添字を保持する。法線は CoordIndex と
    /// 直接対応する。点のタグ、穴、面の共有、Closed の省略を保ち、形は評価しない。
    /// 不正・未対応・停止で部分結果を返さず、共有 Cx の予算と中断を検査する。
    pub fn tessellated_faces<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<TessellatedFaces<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
            };
            reader.check(Kind::Work, 1)?;
            let root = self
                .instance(reader.current)
                .filter(|r| std::ptr::eq(*r, node.source))
                .ok_or_else(|| reader.invalid("face-source"))?;
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

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: u64,
}
struct PointList<'f> {
    source: &'f Record,
    values: Vec<[f64; 3]>,
    tags: Option<Vec<String>>,
}
impl<'f> Reader<'f, '_, '_> {
    fn run(&mut self, root: &'f Record, version: Version) -> Result<TessellatedFaces<'f>, Error> {
        self.record(root.id().unwrap_or(0))?;
        let triangles = if self.is(root, b"IFCTRIANGULATEDFACESET") {
            true
        } else if self.is(root, b"IFCPOLYGONALFACESET") {
            false
        } else {
            return Err(self.unsupported(root));
        };
        if version == Version::Ifc2x3 {
            return Err(self.unsupported(root));
        }
        self.attributes(root, if triangles { 5 } else { 4 })?;
        self.alloc(1, 512)?;
        let coordinates_id = self.target(arg(root, 1))?;
        self.current = coordinates_id;
        let PointList {
            source: coordinates,
            values: points,
            tags,
        } = self.depth(|r| r.points(coordinates_id, version))?;
        self.current = root.id().unwrap_or(0);
        let point_index = match arg(root, if triangles { 5 } else { 4 }) {
            Some(Value::Null) => None,
            value => Some(self.indices(value, 1, points.len())?),
        };
        let count = point_index.as_ref().map_or(points.len(), Vec::len);
        let closed = match arg(root, if triangles { 3 } else { 2 }) {
            Some(Value::Null) => None,
            Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"T") => {
                Some(true)
            }
            Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"F") => {
                Some(false)
            }
            _ => return Err(self.invalid("closed")),
        };
        let (faces, sources) = if triangles {
            let normals = match arg(root, 2) {
                Some(Value::Null) => None,
                value => Some(self.vectors(value)?),
            };
            let values = self.list(arg(root, 4), 1)?;
            self.alloc(values.len(), 48)?;
            let mut indices = Vec::with_capacity(values.len());
            for value in values {
                self.check(Kind::Work, 1)?;
                let triple = self.list(Some(value), 3)?;
                if triple.len() != 3 {
                    return Err(self.invalid("triangle"));
                }
                let mut row = [0; 3];
                for (slot, value) in row.iter_mut().zip(triple) {
                    *slot = self.index(value, count)?;
                    if normals.as_ref().is_some_and(|ns| *slot as usize > ns.len()) {
                        return Err(self.invalid("normal-index"));
                    }
                }
                indices.push(row);
            }
            (IndexedFaces::Triangles { indices, normals }, Vec::new())
        } else {
            self.polygons(root, count)?
        };
        let (length_unit, units) = face_length_unit(self.file, root, version, self.cx)?;
        if length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(root.span())),
            );
        }
        Ok(TessellatedFaces {
            definition: IndexedFaceSet {
                points,
                tags,
                point_index,
                closed,
                faces,
            },
            length_unit,
            source: IndexedFaceSource {
                item: root,
                coordinates,
                faces: sources,
                units,
            },
        })
    }
    fn polygons(
        &mut self,
        root: &'f Record,
        count: usize,
    ) -> Result<(IndexedFaces, Vec<&'f Record>), Error> {
        let values = self.list(arg(root, 3), 1)?;
        // 参照列・面の表・借用出所と共有の索引を、確保前にまとめて予約する。
        self.alloc(values.len(), 256)?;
        let mut indices = Vec::with_capacity(values.len());
        let mut faces = Vec::new();
        let mut sources = Vec::new();
        let mut known = BTreeMap::new();
        for value in values {
            self.current = root.id().unwrap_or(0);
            self.check(Kind::Work, 1)?;
            let id = self.target(Some(value))?;
            let at = if let Some(&at) = known.get(&id) {
                at
            } else {
                self.current = id;
                let (face, source) = self.depth(|r| r.polygon(id, count))?;
                let at = u32::try_from(faces.len()).map_err(|_| self.invalid("index"))?;
                faces.push(face);
                sources.push(source);
                known.insert(id, at);
                at
            };
            indices.push(IndexedPolygonRef(at));
        }
        Ok((IndexedFaces::Polygons { indices, faces }, sources))
    }
    fn polygon(&mut self, id: u64, count: usize) -> Result<(IndexedPolygon, &'f Record), Error> {
        let record = self.record(id)?;
        let holes = if self.is(record, b"IFCINDEXEDPOLYGONALFACEWITHVOIDS") {
            true
        } else if self.is(record, b"IFCINDEXEDPOLYGONALFACE") {
            false
        } else {
            return Err(self.unsupported(record));
        };
        self.attributes(record, if holes { 2 } else { 1 })?;
        let boundary = self.indices(arg(record, 1), 3, count)?;
        let mut inner = Vec::new();
        if holes {
            let rows = self.list(arg(record, 2), 1)?;
            self.alloc(rows.len(), 48)?;
            inner.reserve(rows.len());
            for row in rows {
                self.check(Kind::Work, 1)?;
                inner.push(self.indices(Some(row), 3, count)?);
            }
        }
        Ok((
            IndexedPolygon {
                boundary,
                holes: inner,
            },
            record,
        ))
    }
    fn points(&mut self, id: u64, version: Version) -> Result<PointList<'f>, Error> {
        let record = self.record(id)?;
        if !self.is(record, b"IFCCARTESIANPOINTLIST3D") {
            return Err(self.invalid("point-list-type"));
        }
        self.attributes(record, if version == Version::Ifc4x3 { 2 } else { 1 })?;
        let points = self.vectors(arg(record, 1))?;
        let tags = if version == Version::Ifc4x3 {
            match arg(record, 2) {
                Some(Value::Null) => None,
                value => {
                    let values = self.list(value, 1)?;
                    if values.len() != points.len() {
                        return Err(self.invalid("point-tags"));
                    }
                    self.alloc(values.len(), 48)?;
                    let mut tags = Vec::with_capacity(values.len());
                    for value in values {
                        let Value::Text { raw, .. } = value else {
                            return Err(self.invalid("text"));
                        };
                        self.check(Kind::Work, raw.len() as u64)?;
                        self.alloc(raw.len(), 12)?;
                        let (text, invalid) = self.file.decode_text(self.file.bytes(raw.clone()));
                        if invalid {
                            self.cx.report(
                                Diagnostic::new(codes::IFC_TEXT_INVALID)
                                    .at(at_bytes(record.span())),
                            );
                        }
                        tags.push(text);
                    }
                    Some(tags)
                }
            }
        } else {
            None
        };
        Ok(PointList {
            source: record,
            values: points,
            tags,
        })
    }
    fn vectors(&mut self, value: Option<&Value>) -> Result<Vec<[f64; 3]>, Error> {
        let values = self.list(value, 1)?;
        self.alloc(values.len(), 48)?;
        let mut result = Vec::with_capacity(values.len());
        for row in values {
            self.check(Kind::Work, 1)?;
            let triple = self.list(Some(row), 3)?;
            if triple.len() != 3 {
                return Err(self.invalid("dimension"));
            }
            let mut vector = [0.; 3];
            for (slot, value) in vector.iter_mut().zip(triple) {
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
            result.push(vector);
        }
        Ok(result)
    }
    fn indices(
        &mut self,
        value: Option<&Value>,
        min: usize,
        count: usize,
    ) -> Result<Vec<u32>, Error> {
        let values = self.list(value, min)?;
        self.alloc(values.len(), 8)?;
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            result.push(self.index(value, count)?);
        }
        Ok(result)
    }
    fn index(&mut self, value: &Value, count: usize) -> Result<u32, Error> {
        let Value::Number(raw) = value else {
            return Err(self.invalid("index"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        std::str::from_utf8(self.file.bytes(raw.clone()))
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&i| i > 0 && i as usize <= count)
            .ok_or_else(|| self.invalid("index"))
    }
    fn list<'a>(&mut self, value: Option<&'a Value>, min: usize) -> Result<&'a [Value], Error> {
        match value {
            Some(Value::List(values)) if values.len() >= min => Ok(values),
            _ => Err(self.invalid("list")),
        }
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
    fn is(&self, record: &Record, kind: &[u8]) -> bool {
        self.file.kind(record).eq_ignore_ascii_case(kind)
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

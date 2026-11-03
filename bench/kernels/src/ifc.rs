//! IFC の形状表現を `zumen_geom3d::Shape` に写す試作（#5）。カーネルの候補を試料で測るためで、コアには入れない。
//!
//! L1 のインスタンスを直に読む。写すのは、公開サンプル（`samples/ifc`）の Body の形状表現に現れる項目だけにする:
//! 押し出し（矩形・円・任意の閉じた断面・穴のある断面）、Brep、面の模型、多角形と三角形の面の集合、ブーリアンと
//! 半空間、写像。曲線は、折れ線・添字の折れ線（線と弧）・合成の曲線・円と線を切った曲線・円。弧は弦の誤差で
//! 折れ線にする（試作なので。#419 は曲線を種類のまま持つ）。属性の位置は、IFC2X3 TC1・IFC4 ADD2 TC1・
//! IFC 4.3 ADD2 の実体の定義による（1 から数える）。

use zumen_geom3d::{BooleanOp, Curve, Face, Plane, Profile, Shape, Transform, Xy, Xyz, cross, dot};
use zumen_ifc::{IfcFile, Value};

/// 写せなかった理由
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Miss {
    /// 写さない項目の実体（大文字の名前）
    Item(String),
    /// 値が読めない（参照の先がない・数が足りないなど）
    Invalid(&'static str),
}

type Result<T> = core::result::Result<T, Miss>;

/// 形を写す
pub struct Model<'f> {
    file: &'f IfcFile,
    /// 弧を折れ線にするときの弦の誤差（ファイルの長さの単位）
    chord: f64,
    /// 多角形で区切った半空間の柱を押し出す長さ（ファイルの長さの単位）
    far: f64,
}

/// 長さを 1 にする。長さが 0 か有限でなければ `None`
fn unit(v: Xyz) -> Option<Xyz> {
    let length = libm::sqrt(dot(v, v));
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    Some([v[0] / length, v[1] / length, v[2] / length])
}

/// `v` から、単位のベクトル `axis` の向きの成分を除く
fn reject(v: Xyz, axis: Xyz) -> Xyz {
    let along = dot(v, axis);
    [
        v[0] - along * axis[0],
        v[1] - along * axis[1],
        v[2] - along * axis[2],
    ]
}

fn columns(x: Xyz, y: Xyz, z: Xyz, origin: Xyz) -> Transform {
    Transform([
        [x[0], y[0], z[0], origin[0]],
        [x[1], y[1], z[1], origin[1]],
        [x[2], y[2], z[2], origin[2]],
    ])
}

/// IFC の関数 `IfcFirstProjAxis`: `reference` を `z` に直交させた X。`reference` がなければ X 軸（`z` が X 軸なら
/// Y 軸）を使う
fn first_axis(z: Xyz, reference: Option<Xyz>) -> Option<Xyz> {
    let v = match reference {
        Some(r) => r,
        None if z == [1.0, 0.0, 0.0] => [0.0, 1.0, 0.0],
        None => [1.0, 0.0, 0.0],
    };
    unit(reject(v, z))
}

/// 逆の変換。軸の行列式が 0 なら `None`
pub fn inverse(t: &Transform) -> Option<Transform> {
    let m = &t.0;
    let det = t.determinant();
    if !det.is_finite() || det.abs() <= f64::EPSILON {
        return None;
    }
    let a = |r: usize, c: usize| m[r][c];
    // 余因子の転置を行列式で割る
    let mut inv = [[0.0; 4]; 3];
    for (r, row) in inv.iter_mut().enumerate() {
        for (c, value) in row.iter_mut().take(3).enumerate() {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            *value = (a(r1, c1) * a(r2, c2) - a(r1, c2) * a(r2, c1)) / det;
        }
    }
    let o = t.column(3);
    for row in &mut inv {
        row[3] = -(row[0] * o[0] + row[1] * o[1] + row[2] * o[2]);
    }
    Some(Transform(inv))
}

impl<'f> Model<'f> {
    /// `metre` は、1 m をファイルの長さの単位で表した値（mm なら 1000）
    pub fn new(file: &'f IfcFile, metre: f64) -> Self {
        Self {
            file,
            chord: 0.001 * metre,
            far: 1000.0 * metre,
        }
    }

    /// 実体の名前（大文字）。インスタンスがなければ空
    pub fn kind(&self, id: u64) -> String {
        self.file
            .instance(id)
            .map(|r| String::from_utf8_lossy(self.file.kind(r)).to_ascii_uppercase())
            .unwrap_or_default()
    }

    fn args(&self, id: u64) -> Result<&'f [Value]> {
        let record = self.file.instance(id).ok_or(Miss::Invalid("reference"))?;
        Ok(record.parts().first().map(|p| p.args()).unwrap_or(&[]))
    }

    /// 位置 `at`（1 から）の属性。ないときは `$` と同じに扱う
    fn arg(&self, id: u64, at: usize) -> Result<&'f Value> {
        Ok(self.args(id)?.get(at - 1).unwrap_or(&Value::Null))
    }

    fn number<T: core::str::FromStr>(&self, value: &Value) -> Option<T> {
        match value {
            Value::Number(range) => core::str::from_utf8(self.file.bytes(range.clone()))
                .ok()?
                .parse()
                .ok(),
            Value::Typed(part) => match part.args() {
                [only] => self.number(only),
                _ => None,
            },
            _ => None,
        }
    }

    fn real(&self, value: &Value) -> Option<f64> {
        self.number(value)
    }

    fn reals(&self, value: &Value) -> Option<Vec<f64>> {
        match value {
            Value::List(values) => values.iter().map(|v| self.real(v)).collect(),
            _ => None,
        }
    }

    fn integers(&self, value: &Value) -> Option<Vec<i64>> {
        match value {
            Value::List(values) => values.iter().map(|v| self.number(v)).collect(),
            _ => None,
        }
    }

    fn enumeration(&self, value: &Value) -> Option<String> {
        match value {
            Value::Enum(range) => {
                Some(String::from_utf8_lossy(self.file.bytes(range.clone())).to_ascii_uppercase())
            }
            _ => None,
        }
    }

    /// 真理値。`.F.` だけを偽とし、ほか（`.T.`・`$`）は `default`
    fn flag(&self, value: &Value, default: bool) -> bool {
        match self.enumeration(value).as_deref() {
            Some("F") => false,
            Some("T") => true,
            _ => default,
        }
    }

    fn text(&self, value: &Value) -> Option<String> {
        match value {
            Value::Text { raw, .. } => {
                Some(String::from_utf8_lossy(self.file.bytes(raw.clone())).into_owned())
            }
            _ => None,
        }
    }

    fn reference(value: &Value) -> Option<u64> {
        match value {
            Value::Ref(id) => Some(*id),
            _ => None,
        }
    }

    fn refs(value: &Value) -> Vec<u64> {
        match value {
            Value::Ref(id) => vec![*id],
            Value::List(values) => values.iter().filter_map(Self::reference).collect(),
            _ => Vec::new(),
        }
    }

    fn target(&self, id: u64, at: usize, what: &'static str) -> Result<u64> {
        Self::reference(self.arg(id, at)?).ok_or(Miss::Invalid(what))
    }

    fn triple(values: &[f64], what: &'static str) -> Result<Xyz> {
        match values {
            [x, y] => Ok([*x, *y, 0.0]),
            [x, y, z] => Ok([*x, *y, *z]),
            _ => Err(Miss::Invalid(what)),
        }
    }

    /// `IfcCartesianPoint`（1 Coordinates）。2 次元の点は Z を 0 にする
    fn point(&self, id: u64) -> Result<Xyz> {
        let c = self.reals(self.arg(id, 1)?).ok_or(Miss::Invalid("point"))?;
        Self::triple(&c, "point")
    }

    /// `IfcDirection`（1 DirectionRatios）
    fn direction(&self, id: u64) -> Result<Xyz> {
        let c = self
            .reals(self.arg(id, 1)?)
            .ok_or(Miss::Invalid("direction"))?;
        Self::triple(&c, "direction")
    }

    fn optional_direction(&self, value: &Value) -> Result<Option<Xyz>> {
        Self::reference(value)
            .map(|d| self.direction(d))
            .transpose()
    }

    /// `IfcAxis2Placement3D`（1 Location・2 Axis・3 RefDirection）と `IfcAxis2Placement2D`（1 Location・
    /// 2 RefDirection）。軸は IFC の関数 `IfcBuildAxes`・`IfcFirstProjAxis` で作る
    pub fn placement(&self, id: u64) -> Result<Transform> {
        let location = self.point(self.target(id, 1, "location")?)?;
        let (axis, reference) = match self.kind(id).as_str() {
            "IFCAXIS2PLACEMENT3D" => (
                self.optional_direction(self.arg(id, 2)?)?,
                self.optional_direction(self.arg(id, 3)?)?,
            ),
            "IFCAXIS2PLACEMENT2D" => (None, self.optional_direction(self.arg(id, 2)?)?),
            other => return Err(Miss::Item(other.to_owned())),
        };
        let z = match axis {
            Some(a) => unit(a).ok_or(Miss::Invalid("axis"))?,
            None => [0.0, 0.0, 1.0],
        };
        let x = first_axis(z, reference).ok_or(Miss::Invalid("axis"))?;
        Ok(columns(x, cross(z, x), z, location))
    }

    fn optional_placement(&self, value: &Value) -> Result<Transform> {
        match Self::reference(value) {
            Some(id) => self.placement(id),
            None => Ok(Transform::IDENTITY),
        }
    }

    /// `IfcCartesianTransformationOperator3D`（1 Axis1・2 Axis2・3 LocalOrigin・4 Scale・5 Axis3）と
    /// `…3DnonUniform`（6 Scale2・7 Scale3）。軸は IFC の関数 `IfcBaseAxis` で作る。Axis2 が逆を向けば鏡に写す
    fn operator(&self, id: u64) -> Result<Transform> {
        let kind = self.kind(id);
        if !kind.starts_with("IFCCARTESIANTRANSFORMATIONOPERATOR3D") {
            return Err(Miss::Item(kind));
        }
        let axis1 = self.optional_direction(self.arg(id, 1)?)?;
        let axis2 = self.optional_direction(self.arg(id, 2)?)?;
        let origin = self.point(self.target(id, 3, "origin")?)?;
        let s = self.real(self.arg(id, 4)?).unwrap_or(1.0);
        let axis3 = self.optional_direction(self.arg(id, 5)?)?;
        let z = match axis3 {
            Some(a) => unit(a).ok_or(Miss::Invalid("axis3"))?,
            None => [0.0, 0.0, 1.0],
        };
        let x = first_axis(z, axis1).ok_or(Miss::Invalid("axis1"))?;
        // IfcSecondProjAxis: Axis2（なければ Y 軸）を Z と X に直交させる
        let y = unit(reject(reject(axis2.unwrap_or([0.0, 1.0, 0.0]), z), x))
            .ok_or(Miss::Invalid("axis2"))?;
        let (s2, s3) = if kind == "IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM" {
            (
                self.real(self.arg(id, 6)?).unwrap_or(s),
                self.real(self.arg(id, 7)?).unwrap_or(s),
            )
        } else {
            (s, s)
        };
        let scaled = |v: Xyz, k: f64| [v[0] * k, v[1] * k, v[2] * k];
        Ok(columns(scaled(x, s), scaled(y, s2), scaled(z, s3), origin))
    }

    // 曲線

    /// `IfcCartesianPointList2D`・`…3D`（1 CoordList）
    fn point_list(&self, id: u64) -> Result<Vec<Xyz>> {
        let Value::List(rows) = self.arg(id, 1)? else {
            return Err(Miss::Invalid("point list"));
        };
        rows.iter()
            .map(|row| {
                Self::triple(
                    &self.reals(row).ok_or(Miss::Invalid("point list"))?,
                    "point list",
                )
            })
            .collect()
    }

    /// 3 点を通る弧を折れ線にし、2 つめからの点を `out` に足す。3 点が一直線なら、線分にする
    fn arc_through(&self, a: Xy, b: Xy, c: Xy, out: &mut Vec<Xy>) {
        let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
        if !d.is_finite() || d.abs() <= f64::EPSILON {
            out.push(b);
            out.push(c);
            return;
        }
        let sq = |p: Xy| p[0] * p[0] + p[1] * p[1];
        let ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
        let uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
        let radius = libm::hypot(a[0] - ux, a[1] - uy);
        let angle = |p: Xy| libm::atan2(p[1] - uy, p[0] - ux).to_degrees();
        let ccw = |from: f64, to: f64| (to - from).rem_euclid(360.0);
        let (start, mid, end) = (angle(a), angle(b), angle(c));
        // 中の点を通る向きに回す
        let sweep = if ccw(start, mid) <= ccw(start, end) {
            ccw(start, end)
        } else {
            ccw(start, end) - 360.0
        };
        let mut points = Vec::new();
        zumen_geom2d::arc([ux, uy], radius, start, sweep, self.chord, &mut points);
        // 端は元の点にする
        points.pop();
        out.extend(points.into_iter().skip(1));
        out.push(c);
    }

    /// 2D の曲線を折れ線にする
    fn curve(&self, id: u64) -> Result<Vec<Xy>> {
        let kind = self.kind(id);
        let flat = |p: Xyz| [p[0], p[1]];
        match kind.as_str() {
            // IfcPolyline: 1 Points
            "IFCPOLYLINE" => Self::refs(self.arg(id, 1)?)
                .into_iter()
                .map(|p| self.point(p).map(flat))
                .collect(),
            // IfcIndexedPolyCurve: 1 Points・2 Segments（IfcLineIndex・IfcArcIndex）
            "IFCINDEXEDPOLYCURVE" => {
                let points: Vec<Xy> = self
                    .point_list(self.target(id, 1, "points")?)?
                    .into_iter()
                    .map(flat)
                    .collect();
                let at = |i: i64| {
                    usize::try_from(i - 1)
                        .ok()
                        .and_then(|i| points.get(i).copied())
                        .ok_or(Miss::Invalid("index"))
                };
                let Value::List(segments) = self.arg(id, 2)? else {
                    // 区分がなければ、点を順につなぐ
                    return Ok(points);
                };
                let mut out: Vec<Xy> = Vec::new();
                for segment in segments {
                    let Value::Typed(part) = segment else {
                        return Err(Miss::Invalid("segment"));
                    };
                    let name =
                        String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
                    let indices = match part.args() {
                        [list] => self.integers(list).ok_or(Miss::Invalid("segment"))?,
                        _ => return Err(Miss::Invalid("segment")),
                    };
                    let first = at(*indices.first().ok_or(Miss::Invalid("segment"))?)?;
                    if out.last() != Some(&first) {
                        out.push(first);
                    }
                    match (name.as_str(), indices.as_slice()) {
                        ("IFCLINEINDEX", _) => {
                            for i in &indices[1..] {
                                out.push(at(*i)?);
                            }
                        }
                        ("IFCARCINDEX", [_, b, c]) => {
                            self.arc_through(first, at(*b)?, at(*c)?, &mut out)
                        }
                        _ => return Err(Miss::Item(name)),
                    }
                }
                Ok(out)
            }
            // IfcCompositeCurve: 1 Segments。IfcCompositeCurveSegment: 1 Transition・2 SameSense・3 ParentCurve
            "IFCCOMPOSITECURVE" => {
                let mut out: Vec<Xy> = Vec::new();
                for segment in Self::refs(self.arg(id, 1)?) {
                    let same = self.flag(self.arg(segment, 2)?, true);
                    let mut points = self.curve(self.target(segment, 3, "segment")?)?;
                    if !same {
                        points.reverse();
                    }
                    for p in points {
                        if out.last() != Some(&p) {
                            out.push(p);
                        }
                    }
                }
                Ok(out)
            }
            "IFCTRIMMEDCURVE" => self.trimmed(id),
            // IfcCircle: 1 Position・2 Radius
            "IFCCIRCLE" => {
                let frame = self.optional_placement(self.arg(id, 1)?)?;
                let radius = self.real(self.arg(id, 2)?).ok_or(Miss::Invalid("radius"))?;
                let mut points = Vec::new();
                zumen_geom2d::arc([0.0, 0.0], radius, 0.0, 360.0, self.chord, &mut points);
                points.pop();
                Ok(points
                    .into_iter()
                    .map(|p| flat(frame.point([p[0], p[1], 0.0])))
                    .collect())
            }
            _ => Err(Miss::Item(kind)),
        }
    }

    /// `IfcTrimmedCurve`（1 BasisCurve・2 Trim1・3 Trim2・4 SenseAgreement・5 MasterRepresentation）。基の曲線は
    /// `IfcLine`（1 Pnt・2 Dir）と `IfcCircle`。端は点か媒介変数で、両方あれば点を使う。円の媒介変数は角度とし、
    /// 単位は度とする（試料の平面の角度の単位）
    fn trimmed(&self, id: u64) -> Result<Vec<Xy>> {
        let basis = self.target(id, 1, "basis")?;
        let sense = self.flag(self.arg(id, 4)?, true);
        let trim = |at: usize| -> Result<(Option<Xy>, Option<f64>)> {
            let Value::List(values) = self.arg(id, at)? else {
                return Err(Miss::Invalid("trim"));
            };
            let mut point = None;
            let mut parameter = None;
            for v in values {
                match v {
                    Value::Ref(p) => point = Some(self.point(*p).map(|p| [p[0], p[1]])?),
                    other => parameter = parameter.or(self.real(other)),
                }
            }
            Ok((point, parameter))
        };
        let ((p1, t1), (p2, t2)) = (trim(2)?, trim(3)?);
        match self.kind(basis).as_str() {
            "IFCLINE" => {
                let (a, b) = match (p1, p2) {
                    (Some(a), Some(b)) => (a, b),
                    _ => {
                        // IfcVector: 1 Orientation・2 Magnitude
                        let origin = self.point(self.target(basis, 1, "line")?)?;
                        let vector = self.target(basis, 2, "line")?;
                        let dir = self.direction(self.target(vector, 1, "line")?)?;
                        let magnitude = self.real(self.arg(vector, 2)?).unwrap_or(1.0);
                        let at = |t: f64| {
                            [
                                origin[0] + dir[0] * magnitude * t,
                                origin[1] + dir[1] * magnitude * t,
                            ]
                        };
                        let (t1, t2) = (
                            t1.ok_or(Miss::Invalid("trim"))?,
                            t2.ok_or(Miss::Invalid("trim"))?,
                        );
                        (at(t1), at(t2))
                    }
                };
                Ok(if sense { vec![a, b] } else { vec![b, a] })
            }
            "IFCCIRCLE" => {
                let frame = self.optional_placement(self.arg(basis, 1)?)?;
                let radius = self
                    .real(self.arg(basis, 2)?)
                    .ok_or(Miss::Invalid("radius"))?;
                let angle = |p: Xy| {
                    let local = Plane::project(&frame, [p[0], p[1], 0.0]);
                    libm::atan2(local[1], local[0]).to_degrees()
                };
                let a1 = p1.map(angle).or(t1).ok_or(Miss::Invalid("trim"))?;
                let a2 = p2.map(angle).or(t2).ok_or(Miss::Invalid("trim"))?;
                let mut sweep = if sense {
                    (a2 - a1).rem_euclid(360.0)
                } else {
                    -(a1 - a2).rem_euclid(360.0)
                };
                if sweep == 0.0 {
                    sweep = if sense { 360.0 } else { -360.0 };
                }
                let mut points = Vec::new();
                zumen_geom2d::arc([0.0, 0.0], radius, a1, sweep, self.chord, &mut points);
                let mut out: Vec<Xy> = points
                    .into_iter()
                    .map(|p| {
                        let q = frame.point([p[0], p[1], 0.0]);
                        [q[0], q[1]]
                    })
                    .collect();
                // 端は元の点にする
                if let (Some(first), Some(a)) = (out.first_mut(), p1) {
                    *first = a;
                }
                if let (Some(last), Some(b)) = (out.last_mut(), p2) {
                    *last = b;
                }
                Ok(out)
            }
            other => Err(Miss::Item(other.to_owned())),
        }
    }

    /// 閉じた輪。終わりの点が始めの点と同じなら除く
    fn closed(&self, id: u64) -> Result<Curve> {
        let mut points = self.curve(id)?;
        while points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        Ok(Curve::Polygon(points))
    }

    // 断面

    /// 断面。`IfcRectangleProfileDef`（3 Position・4 XDim・5 YDim）・`IfcCircleProfileDef`（3 Position・
    /// 4 Radius）・`IfcArbitraryClosedProfileDef`（3 OuterCurve）・`IfcArbitraryProfileDefWithVoids`（4 InnerCurves）
    fn profile(&self, id: u64) -> Result<Profile> {
        let kind = self.kind(id);
        match kind.as_str() {
            "IFCRECTANGLEPROFILEDEF" => {
                let frame = self.optional_placement(self.arg(id, 3)?)?;
                let x = self.real(self.arg(id, 4)?).ok_or(Miss::Invalid("xdim"))? / 2.0;
                let y = self.real(self.arg(id, 5)?).ok_or(Miss::Invalid("ydim"))? / 2.0;
                let points = [[-x, -y], [x, -y], [x, y], [-x, y]]
                    .map(|p| {
                        let q = frame.point([p[0], p[1], 0.0]);
                        [q[0], q[1]]
                    })
                    .to_vec();
                Ok(Profile {
                    outer: Curve::Polygon(points),
                    holes: Vec::new(),
                })
            }
            "IFCCIRCLEPROFILEDEF" => {
                let frame = self.optional_placement(self.arg(id, 3)?)?;
                let radius = self.real(self.arg(id, 4)?).ok_or(Miss::Invalid("radius"))?;
                let c = frame.column(3);
                Ok(Profile {
                    outer: Curve::Circle {
                        center: [c[0], c[1]],
                        radius,
                    },
                    holes: Vec::new(),
                })
            }
            "IFCARBITRARYCLOSEDPROFILEDEF" | "IFCARBITRARYPROFILEDEFWITHVOIDS" => {
                let outer = self.closed(self.target(id, 3, "outer")?)?;
                let holes = if kind == "IFCARBITRARYPROFILEDEFWITHVOIDS" {
                    Self::refs(self.arg(id, 4)?)
                        .into_iter()
                        .map(|c| self.closed(c))
                        .collect::<Result<Vec<_>>>()?
                } else {
                    Vec::new()
                };
                Ok(Profile { outer, holes })
            }
            _ => Err(Miss::Item(kind)),
        }
    }

    // 面

    /// `IfcFace`（1 Bounds）。`IfcFaceOuterBound` を外の輪にし、ほかを穴にする。`IfcFaceBound`（1 Bound・
    /// 2 Orientation）の輪は `IfcPolyLoop`（1 Polygon）
    fn face(&self, id: u64) -> Result<Face> {
        let mut outer = None;
        let mut holes = Vec::new();
        for bound in Self::refs(self.arg(id, 1)?) {
            let ring_id = self.target(bound, 1, "bound")?;
            if self.kind(ring_id) != "IFCPOLYLOOP" {
                return Err(Miss::Item(self.kind(ring_id)));
            }
            let mut ring: Vec<Xyz> = Self::refs(self.arg(ring_id, 1)?)
                .into_iter()
                .map(|p| self.point(p))
                .collect::<Result<_>>()?;
            if !self.flag(self.arg(bound, 2)?, true) {
                ring.reverse();
            }
            if self.kind(bound) == "IFCFACEOUTERBOUND" && outer.is_none() {
                outer = Some(ring);
            } else {
                holes.push(ring);
            }
        }
        // 外の輪の印がなければ、最初の輪を外にする
        let outer = match outer {
            Some(o) => o,
            None if !holes.is_empty() => holes.remove(0),
            None => return Err(Miss::Invalid("face")),
        };
        Ok(Face { outer, holes })
    }

    /// `IfcConnectedFaceSet`・`IfcClosedShell`・`IfcOpenShell`（1 CfsFaces）
    fn faces(&self, set: u64) -> Result<Vec<Face>> {
        Self::refs(self.arg(set, 1)?)
            .into_iter()
            .map(|f| self.face(f))
            .collect()
    }

    /// 添字の位置。`PnIndex` があれば、それを通す（1 から）
    fn index(i: i64, pn: Option<&[i64]>, len: usize) -> Result<usize> {
        let i = match pn {
            Some(pn) => *usize::try_from(i - 1)
                .ok()
                .and_then(|i| pn.get(i))
                .ok_or(Miss::Invalid("pn"))?,
            None => i,
        };
        usize::try_from(i - 1)
            .ok()
            .filter(|&i| i < len)
            .ok_or(Miss::Invalid("index"))
    }

    /// `IfcPolygonalFaceSet`（1 Coordinates・2 Closed・3 Faces・4 PnIndex）。面は `IfcIndexedPolygonalFace`
    /// （1 CoordIndex）と `IfcIndexedPolygonalFaceWithVoids`（2 InnerCoordIndices）
    fn polygonal(&self, id: u64) -> Result<Shape> {
        let points = self.point_list(self.target(id, 1, "points")?)?;
        let closed = self.flag(self.arg(id, 2)?, false);
        let pn = self.integers(self.arg(id, 4)?);
        let ring = |value: &Value| -> Result<Vec<Xyz>> {
            self.integers(value)
                .ok_or(Miss::Invalid("ring"))?
                .into_iter()
                .map(|i| Ok(points[Self::index(i, pn.as_deref(), points.len())?]))
                .collect()
        };
        let mut faces = Vec::new();
        for face in Self::refs(self.arg(id, 3)?) {
            let outer = ring(self.arg(face, 1)?)?;
            let holes = match (self.kind(face).as_str(), self.arg(face, 2)?) {
                ("IFCINDEXEDPOLYGONALFACEWITHVOIDS", Value::List(inner)) => {
                    inner.iter().map(&ring).collect::<Result<_>>()?
                }
                _ => Vec::new(),
            };
            faces.push(Face { outer, holes });
        }
        Ok(Shape::FaceSet { faces, closed })
    }

    /// `IfcTriangulatedFaceSet`（1 Coordinates・2 Normals・3 Closed・4 CoordIndex・5 PnIndex）
    fn triangulated(&self, id: u64) -> Result<Shape> {
        let positions = self.point_list(self.target(id, 1, "points")?)?;
        let closed = self.flag(self.arg(id, 3)?, false);
        let Value::List(rows) = self.arg(id, 4)? else {
            return Err(Miss::Invalid("triangles"));
        };
        let pn = self.integers(self.arg(id, 5)?);
        let mut triangles = Vec::with_capacity(rows.len());
        for row in rows {
            let [a, b, c] = self.integers(row).ok_or(Miss::Invalid("triangle"))?[..] else {
                return Err(Miss::Invalid("triangle"));
            };
            let at = |i| Self::index(i, pn.as_deref(), positions.len()).map(|i| i as u32);
            triangles.push([at(a)?, at(b)?, at(c)?]);
        }
        Ok(Shape::Triangles {
            positions,
            triangles,
            closed,
        })
    }

    // 形の項目

    /// 半空間。`IfcHalfSpaceSolid`（1 BaseSurface・2 AgreementFlag）は、`IfcPlane`（1 Position）の Z を法線とし、
    /// AgreementFlag が真なら法線の裏が中身（IFC の定義。`Shape::HalfSpace` と同じ向き）。`IfcBoxedHalfSpace` の
    /// 箱は、計算を速める囲みなので使わない。`IfcPolygonalBoundedHalfSpace`（3 Position・4 PolygonalBoundary）は、
    /// 境の多角形を Position の Z の向きに遠くまで押し出した柱の中の、半空間の部分にする
    fn half_space(&self, id: u64) -> Result<Shape> {
        let kind = self.kind(id);
        let surface = self.target(id, 1, "surface")?;
        if self.kind(surface) != "IFCPLANE" {
            return Err(Miss::Item(self.kind(surface)));
        }
        let frame = self.placement(self.target(surface, 1, "plane")?)?;
        let z = frame.column(2);
        let normal = if self.flag(self.arg(id, 2)?, true) {
            z
        } else {
            [-z[0], -z[1], -z[2]]
        };
        let plane = Plane {
            origin: frame.column(3),
            normal,
        };
        match kind.as_str() {
            "IFCHALFSPACESOLID" | "IFCBOXEDHALFSPACE" => Ok(Shape::HalfSpace { plane }),
            "IFCPOLYGONALBOUNDEDHALFSPACE" => {
                let position = self.placement(self.target(id, 3, "position")?)?;
                let boundary = self.closed(self.target(id, 4, "boundary")?)?;
                let prism = Shape::Extrusion {
                    profile: Profile {
                        outer: boundary,
                        holes: Vec::new(),
                    },
                    position: Transform::translation([0.0, 0.0, -self.far]).then(&position),
                    direction: [0.0, 0.0, 1.0],
                    depth: 2.0 * self.far,
                };
                Ok(Shape::Boolean {
                    op: BooleanOp::Intersection,
                    first: Box::new(prism),
                    second: Box::new(Shape::HalfSpace { plane }),
                })
            }
            _ => Err(Miss::Item(kind)),
        }
    }

    /// 形の項目を写す
    pub fn item(&self, id: u64) -> Result<Shape> {
        let kind = self.kind(id);
        match kind.as_str() {
            // 1 SweptArea・2 Position・3 ExtrudedDirection・4 Depth
            "IFCEXTRUDEDAREASOLID" => Ok(Shape::Extrusion {
                profile: self.profile(self.target(id, 1, "area")?)?,
                position: self.optional_placement(self.arg(id, 2)?)?,
                direction: self.direction(self.target(id, 3, "direction")?)?,
                depth: self.real(self.arg(id, 4)?).ok_or(Miss::Invalid("depth"))?,
            }),
            // 1 Outer
            "IFCFACETEDBREP" => Ok(Shape::FaceSet {
                faces: self.faces(self.target(id, 1, "shell")?)?,
                closed: true,
            }),
            // 1 SbsmBoundary・1 FbsmFaces
            "IFCSHELLBASEDSURFACEMODEL" | "IFCFACEBASEDSURFACEMODEL" => {
                let mut faces = Vec::new();
                let mut closed = true;
                for shell in Self::refs(self.arg(id, 1)?) {
                    closed &= self.kind(shell) == "IFCCLOSEDSHELL";
                    faces.extend(self.faces(shell)?);
                }
                Ok(Shape::FaceSet { faces, closed })
            }
            "IFCPOLYGONALFACESET" => self.polygonal(id),
            "IFCTRIANGULATEDFACESET" => self.triangulated(id),
            // 1 Operator・2 FirstOperand・3 SecondOperand
            "IFCBOOLEANRESULT" | "IFCBOOLEANCLIPPINGRESULT" => {
                let op = match self.enumeration(self.arg(id, 1)?).as_deref() {
                    Some("DIFFERENCE") => BooleanOp::Difference,
                    Some("UNION") => BooleanOp::Union,
                    Some("INTERSECTION") => BooleanOp::Intersection,
                    _ => return Err(Miss::Invalid("operator")),
                };
                Ok(Shape::Boolean {
                    op,
                    first: Box::new(self.operand(self.target(id, 2, "operand")?)?),
                    second: Box::new(self.operand(self.target(id, 3, "operand")?)?),
                })
            }
            // 1 MappingSource（IfcRepresentationMap: 1 MappingOrigin・2 MappedRepresentation）・2 MappingTarget。
            // 写像の元の配置のあとに、写す先の変換を当てる
            "IFCMAPPEDITEM" => {
                let map = self.target(id, 1, "map")?;
                let origin = self.placement(self.target(map, 1, "origin")?)?;
                let shape = self.representation(self.target(map, 2, "representation")?)?;
                let target = self.operator(self.target(id, 2, "target")?)?;
                Ok(Shape::Mapped {
                    transform: origin.then(&target),
                    shape: Box::new(shape),
                })
            }
            _ => Err(Miss::Item(kind)),
        }
    }

    fn operand(&self, id: u64) -> Result<Shape> {
        match self.kind(id).as_str() {
            "IFCHALFSPACESOLID" | "IFCBOXEDHALFSPACE" | "IFCPOLYGONALBOUNDEDHALFSPACE" => {
                self.half_space(id)
            }
            _ => self.item(id),
        }
    }

    /// `IfcShapeRepresentation`（4 Items）の項目。複数なら組にする
    pub fn representation(&self, id: u64) -> Result<Shape> {
        let mut shapes = Self::refs(self.arg(id, 4)?)
            .into_iter()
            .map(|i| self.item(i))
            .collect::<Result<Vec<_>>>()?;
        Ok(if shapes.len() == 1 {
            shapes.remove(0)
        } else {
            Shape::Group(shapes)
        })
    }

    /// 製品（7 Representation）の形状表現のうち、識別子（`IfcShapeRepresentation` の 2 RepresentationIdentifier）
    /// が `identifiers` の前にあるもの。`IfcProductDefinitionShape` は 3 Representations。なければ `None`
    pub fn shape(&self, product: u64, identifiers: &[&str]) -> Result<Option<Shape>> {
        let Some(definition) = Self::reference(self.arg(product, 7)?) else {
            return Ok(None);
        };
        let representations = Self::refs(self.arg(definition, 3)?);
        for wanted in identifiers {
            for representation in &representations {
                if self.text(self.arg(*representation, 2)?).as_deref() == Some(*wanted) {
                    return self.representation(*representation).map(Some);
                }
            }
        }
        Ok(None)
    }

    /// 製品の `Body` の形状表現
    pub fn body(&self, product: u64) -> Result<Option<Shape>> {
        self.shape(product, &["Body"])
    }

    /// 開口の形。`Body` がなければ `Reference`（Archicad 28 の書き出しは、開口の形を Body の文脈の `Reference` に
    /// 書く）
    pub fn opening(&self, product: u64) -> Result<Option<Shape>> {
        self.shape(product, &["Body", "Reference"])
    }
}

/// 形の中の項目の種類を、出る順に名前で並べる（組と写像は中を見る）
pub fn kinds(shape: &Shape, out: &mut Vec<&'static str>) {
    match shape {
        Shape::Boolean { op, first, second } => {
            out.push(match op {
                BooleanOp::Difference => "difference",
                BooleanOp::Union => "union",
                BooleanOp::Intersection => "intersection",
            });
            kinds(first, out);
            kinds(second, out);
        }
        Shape::Mapped { shape, .. } => {
            out.push("mapped");
            kinds(shape, out);
        }
        Shape::Group(shapes) => shapes.iter().for_each(|s| kinds(s, out)),
        other => out.push(other.kind()),
    }
}

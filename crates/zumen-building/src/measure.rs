//! 建物の形からの計測。測り方・単位・丸めを呼び出し側が明示する。

mod analytic;
mod area;
mod boundary;
mod curves;
mod decimal;
mod mesh;
mod selection;
mod unit;
mod walls;

pub use decimal::{Decimal, Rounding, RoundingMode, RoundingStage};
pub use unit::{LengthUnit, Unit};
pub use walls::{WallBasis, WallCenter};

use crate::{Building, ElementRef, GeometryRef, Id};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_geom3d::{Failure, Tolerance};

/// 測り方を決めたプロファイルの識別。値を省略してプロファイルを推定しない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile<'a> {
    /// プロファイルの識別名。
    pub id: &'a str,
    /// 適用した版。
    pub version: &'a str,
    /// 適用した内容のハッシュ。
    pub hash: [u8; 32],
}

/// 結果が保持するプロファイルの識別。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileIdentity {
    /// プロファイルの識別名。
    pub id: String,
    /// 適用した版。
    pub version: String,
    /// 適用した内容のハッシュ。
    pub hash: [u8; 32],
}

/// 共通の計測の指定。長さの許容差と弦の誤差はメートル。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options<'a> {
    /// 次元を含む出力単位。
    pub unit: Unit,
    /// 十進の丸め方・桁・段。
    pub rounding: Rounding,
    /// 位相の計算と曲線の近似に使う許容差。
    pub tolerance: Tolerance,
    /// 測り方をプロファイルから得た場合の識別。
    pub profile: Option<Profile<'a>>,
}

/// 面積を測る境界。指定を省略して法令や用途を推定しない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// 元の領域の形を使う。
    Shape,
    /// 周囲の壁の中心まで測る。
    WallCenter,
    /// 周囲の壁の内側まで測る。
    Inside,
}

/// 元の領域の境界の解釈。明示した値は自動判定に優先する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputBoundary {
    /// 頂点と壁芯の距離から推定する。
    Auto,
    /// 元の形は壁芯の境界である。
    WallCenter,
    /// 元の形は内法の境界である。
    Inside,
}

/// 水平断面の測り方。原点は文書単位のプロジェクト座標。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area<'a> {
    /// 切る高さと、座標の差を取る原点。
    pub origin: zumen_crs::ProjectPoint,
    /// 求める面積の境界。
    pub boundary: Boundary,
    /// 元の領域の解釈。
    pub input: InputBoundary,
    /// 壁芯を求める壁。呼び出し側が階と対象を選ぶ。
    pub walls: &'a [ElementRef],
}

/// 結果に残す計測の操作と条件。
#[derive(Clone, Debug, PartialEq)]
pub enum Method {
    /// 名指した表現の有限の曲線長。
    Length {
        /// 指定した表現の名前。
        representation: String,
    },
    /// 水平断面の面積。
    Area {
        /// 文書単位のプロジェクト座標。
        origin: zumen_crs::ProjectPoint,
        /// 求めた境界。
        boundary: Boundary,
        /// 元の領域の解釈の指定。
        input: InputBoundary,
        /// 壁芯の候補として指定された壁の ID。失敗した壁も消さない。
        walls: Vec<Id>,
    },
    /// 閉じた形の体積。
    Volume {
        /// 開口の差を適用したか。
        openings: bool,
    },
}

/// 元の境界の解釈の出所。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryEvidence {
    /// 形のまま測り、壁芯か内法かを判定していない。
    Shape,
    /// 呼び出し側の明示指定。
    Explicit,
    /// 頂点の距離による推定。
    Inferred,
}

/// 面積の境界を解釈・換算した根拠。
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryDecision {
    /// 元の領域に採用した解釈。
    pub source: Boundary,
    /// 指定か推定か。
    pub evidence: BoundaryEvidence,
    /// 判定した頂点の数。一直線上の中間点は除く。
    pub vertices: u64,
    /// 壁芯から 1 mm 以内の頂点数。
    pub center_vertices: u64,
    /// 壁の半厚から 1 mm 以内の頂点数。
    pub inside_vertices: u64,
    /// 判定・換算に使った壁芯。計測原点を引いたメートルで返す。
    pub walls: Vec<WallCenter>,
}

/// 値の計算に使った元の形の項目。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// 元の要素が所有する形の参照。
    pub item: GeometryRef,
    /// 元の形の項目のインスタンス番号。
    pub instance: Option<u64>,
    /// 元の形の評価に使ったメートルの許容差。計測で指定した許容差と分ける。
    pub tolerance: Tolerance,
    /// 元の寸法を使ったか、評価した形を使ったか。
    pub basis: SourceBasis,
}

/// 値を計算した形の出所。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceBasis {
    /// 元の寸法・曲線の定義。浮動小数点の計算自体の誤差は含む。
    Definition,
    /// 分割・格子・集合演算を経た形。
    Evaluated,
}

/// 1 要素の計算値。ファイルに書かれた数量は参照しない。
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    /// 対象の要素。
    pub element: ElementRef,
    /// 対象の永続 ID。
    pub id: Id,
    /// 出力単位に換算した丸め前の値。計測できなければ None。
    pub raw: Option<f64>,
    /// 要素ごとに丸める指定の場合だけ、十進の計算値を持つ。
    pub rounded: Option<Decimal>,
    /// 計算に使った項目と形状評価の許容差。
    pub sources: Vec<Source>,
    /// 面積の境界の解釈と根拠。長さ・体積・失敗した面積では None。
    pub boundary: Option<BoundaryDecision>,
}

/// 計算値と計測条件。記録済みの数量や表示用の文字列は混ぜない。
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// 計測の操作と指定した条件。
    pub method: Method,
    /// 要求した要素の順の値。重複する対象は要求時に拒否する。
    pub measurements: Vec<Measurement>,
    /// すべての値を計算できたときの、丸め前の和。
    pub raw_total: Option<f64>,
    /// 指定した段で丸めた和。1 要素でも計算できなければ None。
    pub rounded_total: Option<Decimal>,
    /// 明示された出力単位。
    pub unit: Unit,
    /// 適用した丸め方・桁・段。
    pub rounding: Rounding,
    /// 計測のために指定された許容差。
    pub tolerance: Tolerance,
    /// 測り方を決めたプロファイルの識別。
    pub profile: Option<ProfileIdentity>,
}

struct Calculated {
    si_value: f64,
    sources: Vec<Source>,
    boundary: Option<BoundaryDecision>,
}

/// 名前を明示した表現の曲線の長さを求める。無限の直線を有限の線とみなさない。
pub fn lengths(
    building: &Building,
    elements: &[ElementRef],
    representation: &str,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Report, Failure> {
    validate(building, elements, 1, options, cx)?;
    if representation.is_empty() || representation.len() > 1024 {
        return Err(Failure::Invalid("measure_representation"));
    }
    cx.check(Kind::Work, representation.len() as u64)?;
    cx.check(Kind::DecodedBytes, representation.len() as u64)?;
    collect(
        building,
        elements,
        options,
        Method::Length {
            representation: representation.into(),
        },
        |element, cx| curves::length(building, element, representation, options, cx),
        cx,
    )
}

/// 指定した高さの水平断面を、領域の形のまま計測する。origin は文書単位のプロジェクト座標。
/// 断面外の部分を足さず、穴と同じ要素の部品の重なりを閉領域の演算で扱う。
pub fn section_areas(
    building: &Building,
    elements: &[ElementRef],
    origin: zumen_crs::ProjectPoint,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Report, Failure> {
    areas(
        building,
        elements,
        Area {
            origin,
            boundary: Boundary::Shape,
            input: InputBoundary::Auto,
            walls: &[],
        },
        options,
        cx,
    )
}

/// 水平断面の面積を指定した境界で求める。壁芯の根拠が不足した場合は推測で補わない。
pub fn areas(
    building: &Building,
    elements: &[ElementRef],
    area: Area<'_>,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Report, Failure> {
    validate(building, elements, 2, options, cx)?;
    if !area.walls.is_empty() {
        validate(building, area.walls, 2, options, cx)?;
    }
    let factor = curves::document_scale(building, cx)?;
    let origin = [
        area.origin.x * factor,
        area.origin.y * factor,
        area.origin.z * factor,
    ];
    if !origin.into_iter().all(f64::is_finite) {
        return Err(Failure::Invalid("measure_plane"));
    }
    let meshes = selection::evaluate(building, elements, true, options, cx)?;
    let original = area.boundary == Boundary::Shape && analytic::no_openings(building, true, cx)?;
    let centers = if area.boundary != Boundary::Shape && !area.walls.is_empty() {
        let gross = selection::evaluate(building, area.walls, false, options, cx)?;
        walls::gather(building, area.walls, &gross, origin, options, cx)?
    } else {
        Vec::new()
    };
    let mut ids = Vec::new();
    reserve(&mut ids, area.walls.len(), 16, cx)?;
    for &wall in area.walls {
        cx.check(Kind::Work, 1)?;
        ids.push(
            building
                .id(wall)
                .ok_or(Failure::Invalid("measure_wall_element"))?,
        );
    }
    collect(
        building,
        elements,
        options,
        Method::Area {
            origin: area.origin,
            boundary: area.boundary,
            input: area.input,
            walls: ids,
        },
        |element, cx| {
            let mesh = meshes
                .get(element.0 as usize)
                .ok_or(Failure::Invalid("measure_element"))?;
            if original && let Some(si_value) = analytic::area(building, mesh, origin, cx)? {
                return Ok(Calculated {
                    si_value,
                    sources: mesh_sources(building, mesh, SourceBasis::Definition, cx)?,
                    boundary: Some(BoundaryDecision {
                        source: Boundary::Shape,
                        evidence: BoundaryEvidence::Shape,
                        vertices: 0,
                        center_vertices: 0,
                        inside_vertices: 0,
                        walls: Vec::new(),
                    }),
                });
            }
            let section = area::section(building, mesh, origin, options, cx)?;
            boundary::measure(building, element, section, &centers, area, cx)
        },
        cx,
    )
}

fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    slot: u64,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    let size = values
        .len()
        .checked_add(additional)
        .ok_or(Failure::Invalid("measure_size"))?;
    let limit = i32::MAX as usize / slot.max(1) as usize;
    if size > limit {
        return Err(Failure::Invalid("measure_size"));
    }
    if size > values.capacity() {
        let capacity = size
            .max(values.capacity().saturating_mul(2))
            .clamp(4, limit);
        cx.check(
            Kind::DecodedBytes,
            (capacity - values.capacity()) as u64 * slot,
        )?;
        values
            .try_reserve_exact(capacity - values.len())
            .map_err(|_| Failure::Failed("allocation".into()))?;
    }
    Ok(())
}

fn validate(
    building: &Building,
    elements: &[ElementRef],
    dimension: u8,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    cx.check(Kind::Work, 1)?;
    if elements.is_empty()
        || options.unit.dimension() != dimension
        || !options.tolerance.length.is_finite()
        || options.tolerance.length <= 0.0
        || !options.tolerance.chord.is_finite()
        || options.tolerance.chord <= 0.0
        || !(-18..=18).contains(&options.rounding.digits)
        || options
            .profile
            .is_some_and(|p| p.id.is_empty() || p.version.is_empty())
    {
        return Err(Failure::Invalid("measure_options"));
    }
    let n = elements.len() as u64;
    let bytes = n
        .checked_mul(64)
        .filter(|v| *v <= i32::MAX as u64)
        .ok_or(Failure::Invalid("measure_size"))?;
    cx.check(Kind::DecodedBytes, bytes)?;
    cx.check(Kind::Work, n.saturating_mul(65 - n.leading_zeros() as u64))?;
    let mut seen = std::collections::BTreeSet::new();
    for &element in elements {
        cx.check(Kind::Work, 0)?;
        if building.element(element).is_none() || !seen.insert(element) {
            return Err(Failure::Invalid("measure_elements"));
        }
    }
    Ok(())
}

fn collect(
    building: &Building,
    elements: &[ElementRef],
    options: Options<'_>,
    method: Method,
    mut calculate: impl FnMut(ElementRef, &mut Cx<'_>) -> Result<Calculated, Failure>,
    cx: &mut Cx<'_>,
) -> Result<Report, Failure> {
    let profile = if let Some(profile) = options.profile {
        let bytes = (profile.id.len() as u64)
            .checked_add(profile.version.len() as u64)
            .filter(|v| *v <= i32::MAX as u64)
            .ok_or(Failure::Invalid("measure_size"))?;
        cx.check(Kind::Work, bytes)?;
        cx.check(Kind::DecodedBytes, bytes)?;
        Some(ProfileIdentity {
            id: profile.id.into(),
            version: profile.version.into(),
            hash: profile.hash,
        })
    } else {
        None
    };
    let mut measurements = Vec::new();
    reserve(&mut measurements, elements.len(), 224, cx)?;
    let mut raw_total = 0.0;
    let mut rounded_total = options.rounding.apply(0.0, cx)?;
    let mut complete = true;
    for &element in elements {
        cx.check(Kind::Work, 1)?;
        let calculation = calculate(element, cx).and_then(|calculated| {
            let raw = calculated.si_value / options.unit.si_scale();
            if !raw.is_finite() || raw < 0.0 {
                return Err(Failure::Invalid("measure_value"));
            }
            let rounded = if options.rounding.stage == RoundingStage::Element {
                Some(options.rounding.apply(raw, cx)?)
            } else {
                None
            };
            Ok((raw, rounded, calculated.sources, calculated.boundary))
        });
        let (raw, rounded, sources, boundary) = match calculation {
            Ok((raw, rounded, sources, boundary)) => {
                raw_total += raw;
                if !raw_total.is_finite() {
                    return Err(Failure::Invalid("measure_total"));
                }
                if let Some(value) = rounded {
                    rounded_total = rounded_total
                        .checked_add(value)
                        .ok_or(Failure::Invalid("measure_total"))?;
                }
                (Some(raw), rounded, sources, boundary)
            }
            Err(Failure::Stopped(stop)) => return Err(Failure::Stopped(stop)),
            Err(error) => {
                complete = false;
                let reason = match &error {
                    Failure::Invalid(s) | Failure::Unsupported(s) => *s,
                    Failure::Failed(s) => s.as_str(),
                    Failure::Stopped(_) => "stopped",
                };
                cx.report(
                    Diagnostic::new(codes::BUILDING_MEASURE_FAILED)
                        .with("reason", reason)
                        .at(building.place(element)),
                );
                cx.check(Kind::Work, 0)?;
                (None, None, Vec::new(), None)
            }
        };
        measurements.push(Measurement {
            element,
            id: building
                .id(element)
                .ok_or(Failure::Invalid("measure_element"))?,
            raw,
            rounded,
            sources,
            boundary,
        });
    }
    if complete && options.rounding.stage == RoundingStage::Total {
        rounded_total = options.rounding.apply(raw_total, cx)?;
    }
    cx.check(Kind::Work, 0)?;
    Ok(Report {
        method,
        measurements,
        raw_total: complete.then_some(raw_total),
        rounded_total: complete.then_some(rounded_total),
        unit: options.unit,
        rounding: options.rounding,
        tolerance: options.tolerance,
        profile,
    })
}

/// 要素の体積を計算する。開口を引くかを明示し、同じ要素内の重なる部品は和にする。
pub fn volumes(
    building: &Building,
    elements: &[ElementRef],
    openings: bool,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Report, Failure> {
    validate(building, elements, 3, options, cx)?;
    let meshes = selection::evaluate(building, elements, openings, options, cx)?;
    let original = analytic::no_openings(building, openings, cx)?;
    collect(
        building,
        elements,
        options,
        Method::Volume { openings },
        |element, cx| {
            let mesh = meshes
                .get(element.0 as usize)
                .ok_or(Failure::Invalid("measure_element"))?;
            let exact = if original {
                analytic::volume(building, mesh, cx)?
            } else {
                None
            };
            let (si_value, basis) = if let Some(value) = exact {
                (value, SourceBasis::Definition)
            } else {
                (
                    mesh::volume(mesh, options.tolerance, &building.place(element), cx)?,
                    SourceBasis::Evaluated,
                )
            };
            let sources = mesh_sources(building, mesh, basis, cx)?;
            Ok(Calculated {
                si_value,
                sources,
                boundary: None,
            })
        },
        cx,
    )
}

fn mesh_sources(
    building: &Building,
    mesh: &crate::evaluation::ElementMesh,
    basis: SourceBasis,
    cx: &mut Cx<'_>,
) -> Result<Vec<Source>, Failure> {
    let mut sources = Vec::new();
    reserve(&mut sources, mesh.parts.len(), 64, cx)?;
    for part in &mesh.parts {
        cx.check(Kind::Work, 1)?;
        sources.push(Source {
            item: part.item,
            instance: building
                .geometry(mesh.element)
                .and_then(|g| g.data().nodes.get(part.item.0 as usize))
                .and_then(|n| n.source.instance),
            tolerance: part.tolerance,
            basis,
        });
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    #[test]
    fn logical_reservations_cover_output_layouts() {
        assert!(size_of::<super::Measurement>() <= 224);
        assert!(size_of::<super::Source>() <= 64);
        assert!(size_of::<super::curves::Curve<'_>>() <= 160);
        assert!(size_of::<super::WallCenter>() <= 112);
    }
}

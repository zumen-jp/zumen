//! 開いた図面を新しい DXF に書く（決定 0139・0206・0281）。対応する R2004 DWG では共有の表を再符号化する。
//!
//! 図面モデルから書く処理（lower）は `zumen-acad` の [`zumen_acad::lower_to_dxf_with`] にある。ここでは、要素の形を
//! API の形の値（`add` が受けるもの）を通して書く先に移す方法（[`Geometric`]）を渡し、公開の操作の `convert` と
//! `civil_drawing` が同じ値を書くようにする。

use zumen_acad::{Dxf, DxfFile, Relocate};
use zumen_base::Cx;
use zumen_drawing::{Drawing2D, ModelPoint, PaperPoint, Shape, Symbol};
use zumen_sdk::Codec;

use crate::geometry;
use crate::session::Failed;

/// 書いたファイルと、足した要素の数
pub(crate) struct Converted {
    pub(crate) file: DxfFile,
    pub(crate) bytes: Vec<u8>,
    pub(crate) elements: u64,
    pub(crate) report: zumen_acad::ConversionReport,
}

/// R2004 DWG は共有の表を一度だけ読む。再符号化の範囲外なら同じ表から図面モデルを作る。
pub(crate) fn from_r2004(
    source: &zumen_acad::DwgFile,
    limit: u64,
    cx: &mut Cx<'_>,
) -> Result<Converted, Failed> {
    match source
        .reencode_r2004_dxf_with_report(limit, cx)
        .map_err(|_| Failed)?
    {
        zumen_acad::dxf::R2004Conversion::Dxf(encoded) => {
            let file = Dxf::read(&encoded.bytes, cx).map_err(|_| Failed)?;
            Ok(Converted {
                file,
                bytes: encoded.bytes,
                elements: encoded.elements,
                report: encoded.report,
            })
        }
        zumen_acad::dxf::R2004Conversion::DrawingSource(db) => {
            let drawing = zumen_acad::lift_drawing(&db, cx).map_err(|_| Failed)?;
            to_dxf_with_report(&drawing, limit, cx)
        }
    }
}

/// 図面モデルを、新しい DXF（R2004）に書く。診断は `zumen-acad` が報告する
pub(crate) fn to_dxf(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Converted, Failed> {
    to_dxf_with_report(drawing, 0, cx)
}

/// 実変換と同じ書き手から、上限付きの明細も受け取る
pub(crate) fn to_dxf_with_report(
    drawing: &Drawing2D,
    limit: u64,
    cx: &mut Cx<'_>,
) -> Result<Converted, Failed> {
    let lowered =
        zumen_acad::lower_to_dxf_with_report(drawing, &Geometric, limit, cx).map_err(|_| Failed)?;
    Ok(Converted {
        file: lowered.file,
        bytes: lowered.bytes,
        elements: lowered.elements,
        report: lowered.report,
    })
}

/// 形を API の形の値に直し、書く先の原点で形に戻す。形の値で表せない形と、戻せない形は移さない
struct Geometric;

impl Relocate for Geometric {
    fn fidelity_model(
        &self,
        source: &Shape<ModelPoint>,
        _output: &Shape<ModelPoint>,
    ) -> zumen_acad::ConversionFidelity {
        geometric_fidelity(source)
    }

    fn fidelity_paper(
        &self,
        source: &Shape<PaperPoint>,
        _output: &Shape<PaperPoint>,
    ) -> zumen_acad::ConversionFidelity {
        geometric_fidelity(source)
    }

    fn accepts_model_symbol(&self, symbol: &Symbol<ModelPoint>) -> bool {
        geometry::symbol_grid(symbol).is_some()
    }

    fn accepts_paper_symbol(&self, symbol: &Symbol<PaperPoint>) -> bool {
        geometry::symbol_grid(symbol).is_some()
    }

    fn model(&self, shape: &Shape<ModelPoint>, origin: ModelPoint) -> Option<Shape<ModelPoint>> {
        geometry::shape_of(&geometry::geometry_of(shape)?, None, origin).ok()
    }

    fn paper(&self, shape: &Shape<PaperPoint>, origin: PaperPoint) -> Option<Shape<PaperPoint>> {
        geometry::shape_of(&geometry::geometry_of(shape)?, None, origin).ok()
    }
}

/// 基本図形は同じ意味の形へ移す。文字・注記・記号は API の形の値にない意味があるため、見た目だけとする
fn geometric_fidelity<P>(shape: &Shape<P>) -> zumen_acad::ConversionFidelity {
    match shape {
        Shape::Curve(zumen_drawing::Edge::Spline(spline))
            if spline.fit_tolerance.is_some() || spline.knot_parameterization.is_some() =>
        {
            zumen_acad::ConversionFidelity::Drawn
        }
        Shape::Line { .. }
        | Shape::Circle { .. }
        | Shape::Curve(_)
        | Shape::Polyline(_)
        | Shape::Point(_) => zumen_acad::ConversionFidelity::Equivalent,
        _ => zumen_acad::ConversionFidelity::Drawn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limits, Ref};
    use zumen_civil::{Alignment, CivilModel, Horizontal, HorizontalKind, Vertical, VerticalKind};
    use zumen_crs::MapPoint;

    fn drawing(cx: &mut Cx<'_>) -> Drawing2D {
        let model = CivilModel {
            alignments: vec![Alignment {
                id: "本線".to_string(),
                horizontal: vec![Horizontal {
                    start: MapPoint {
                        northing: 0.0,
                        easting: 0.0,
                    },
                    direction: 0.0,
                    length: 220.0,
                    kind: HorizontalKind::Line,
                    source: Ref::default(),
                }],
                vertical: vec![Vertical {
                    start_distance: 0.0,
                    start_height: 50.0,
                    start_gradient: 0.02,
                    length: 220.0,
                    kind: VerticalKind::ConstantGradient,
                    source: Ref::default(),
                }],
                ..Alignment::default()
            }],
            ..CivilModel::default()
        };
        zumen_civil_drawing::drawing(
            &model,
            &model.alignments[0],
            &zumen_civil_drawing::Options::default(),
            cx,
        )
        .unwrap()
    }

    /// DXF の組（グループコードと値）から、TEXT の位置合わせの第 2 点（11・21・31）と位置合わせ（72・73）を除く
    fn without_alignment(bytes: &[u8]) -> Vec<(String, String)> {
        let text = String::from_utf8_lossy(bytes);
        let lines: Vec<&str> = text.lines().collect();
        let mut entity = String::new();
        let mut pairs = Vec::new();
        for pair in lines.chunks(2) {
            let (code, value) = (pair[0].trim(), pair[1]);
            if code == "0" {
                entity = value.to_string();
            }
            if entity == "TEXT" && matches!(code, "11" | "21" | "31" | "72" | "73") {
                continue;
            }
            pairs.push((code.to_string(), value.to_string()));
        }
        pairs
    }

    /// `convert` と `civil_drawing` が書く経路（[`to_dxf`]）は、`zumen-acad` の lower に形の移し方を渡すだけで、
    /// 土木の図面では Rust から直接呼んだ lower と同じ要素の数・診断になる。バイト列は、API の形の値が文字の
    /// 位置合わせを持たないため、文字の位置合わせ（11・21・31・72・73）だけが違う（#763）
    #[test]
    #[allow(clippy::disallowed_methods)] // 外部照合の出力先だけを受け取る
    fn the_api_writes_what_the_rust_lower_writes() {
        let run = |api: bool| {
            let mut budget = Budget::new(Limits::default());
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let drawing = drawing(&mut cx);
            let (bytes, elements) = if api {
                let Ok(converted) = to_dxf(&drawing, &mut cx) else {
                    panic!("書けない");
                };
                (converted.bytes, converted.elements)
            } else {
                let Ok(lowered) = zumen_acad::lower_to_dxf(&drawing, &mut cx) else {
                    panic!("書けない");
                };
                (lowered.bytes, lowered.elements)
            };
            (bytes, elements, diag.into_vec())
        };
        let (api, rust) = (run(true), run(false));
        assert!(api.1 > 0);
        assert_eq!(api.1, rust.1);
        assert_eq!(api.2, rust.2);
        assert!(
            without_alignment(&api.0) == without_alignment(&rust.0),
            "位置合わせのほかの値が違う"
        );
        if let Some(folder) = std::env::var_os("ZUMEN_DXF_LOWER_OUT") {
            let folder = std::path::PathBuf::from(folder);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("api.dxf"), api.0).unwrap();
            std::fs::write(folder.join("rust.dxf"), rust.0).unwrap();
        }
    }

    /// API の移し方でも、符号化できない文字を分けて当て直したあと、用紙の後続の線をすべて残す
    #[test]
    fn the_api_keeps_paper_lines_after_rejected_text() {
        use zumen_sdk::Lift;

        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut drawing = drawing(&mut cx);
        let mut profile = drawing.sheets.remove(1);
        profile.views.clear();
        profile.order.clear();
        let mut rejected = Vec::new();
        for (index, element) in profile.elements.iter_mut().enumerate() {
            if let Shape::Text(text) = &mut element.shape {
                // R2004 の ANSI_932 に戻せないが、API の形の値にはできる文字
                text.content = "🙂".into();
                element.source = Ref::at_record(1000 + index as u64);
                rejected.push(element.source.clone());
            }
        }
        let lines = profile.elements.len() - rejected.len();
        assert!(!rejected.is_empty() && lines > 2);
        profile
            .elements
            .sort_by_key(|element| !matches!(element.shape, Shape::Text(_)));
        drawing.sheets = vec![profile];
        let converted = to_dxf(&drawing, &mut cx).ok().unwrap();
        let read = converted.file.lift(&mut cx).unwrap();
        assert_eq!(read.sheets[0].elements.len(), lines);
        let dropped: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code() == zumen_base::codes::CONVERT_ELEMENT_UNSUPPORTED
            })
            .collect();
        assert_eq!(
            dropped
                .iter()
                .map(|diagnostic| diagnostic.count())
                .sum::<u64>(),
            rejected.len() as u64
        );
        assert_eq!(
            dropped
                .iter()
                .flat_map(|diagnostic| diagnostic.places())
                .collect::<Vec<_>>(),
            rejected
                .iter()
                .take(Diagnostics::DEFAULT_MAX_PLACES)
                .collect::<Vec<_>>()
        );
    }

    /// API の形の値を通すとスプラインの許容差とノットの取り方が失われるため、等価と予告しない
    #[test]
    fn the_estimate_exposes_spline_parameters_that_the_api_does_not_write() {
        use zumen_drawing::{Edge, KnotParameterization, Spline};
        use zumen_sdk::Lift;

        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut drawing = drawing(&mut cx);
        let space = drawing.spaces.first_mut().unwrap();
        let mut element = space.elements.first().unwrap().clone();
        element.source = Ref::at_record(987);
        element.shape = Shape::Curve(Edge::Spline(Spline {
            degree: 3,
            knots: Vec::new(),
            control: Vec::new(),
            weights: Vec::new(),
            fit: vec![
                ModelPoint {
                    space: space.id,
                    x: 0.0,
                    y: 0.0,
                },
                ModelPoint {
                    space: space.id,
                    x: 10.0,
                    y: 3.0,
                },
                ModelPoint {
                    space: space.id,
                    x: 20.0,
                    y: 0.0,
                },
            ],
            start_tangent: None,
            end_tangent: None,
            closed: false,
            periodic: false,
            fit_tolerance: Some(0.25),
            knot_parameterization: Some(KnotParameterization::Uniform),
        }));
        space.elements.push(element);
        let converted = to_dxf_with_report(&drawing, 1000, &mut cx).ok().unwrap();
        let detail = converted
            .report
            .details
            .iter()
            .find(|row| row.kind == "curve" && row.source.record() == Some(987))
            .unwrap();
        assert_eq!(detail.fidelity, zumen_acad::ConversionFidelity::Drawn);
        assert_eq!(detail.output_elements, 1);
        assert!(
            detail
                .reasons
                .iter()
                .any(|reason| reason == "relocated_shape")
        );
        assert!(
            detail
                .reasons
                .iter()
                .any(|reason| reason == "spline_fit_tolerance")
        );
        assert!(
            detail
                .reasons
                .iter()
                .any(|reason| reason == "spline_knot_parameterization")
        );
        let read = converted.file.lift(&mut cx).unwrap();
        let written = read
            .spaces
            .iter()
            .flat_map(|space| &space.elements)
            .find_map(|element| match &element.shape {
                Shape::Curve(Edge::Spline(spline)) => Some(spline),
                _ => None,
            })
            .unwrap();
        assert_ne!(written.fit_tolerance, Some(0.25));
        assert_ne!(
            written.knot_parameterization,
            Some(KnotParameterization::Uniform)
        );
    }
}

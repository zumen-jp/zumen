//! 合成の線形から平面図・縦断図・横断図の図面モデルを作り、シーンに描く（決定 0231）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, Ref, codes};
use zumen_civil::{
    Alignment, CivilModel, CrossSection, CrossSectionLine, Horizontal, HorizontalKind, SurfaceRole,
    Tin, TransitionCurve, Vertical, VerticalKind,
};
use zumen_civil_drawing::{Options, drawing};
use zumen_crs::MapPoint;
use zumen_drawing::{Drawing2D, Edge, Length, Origin, Shape, SheetId, SpaceId};
use zumen_scene::{Chunking, Palette, build};

/// 北へ 100 m の直線、半径 200 m へ右に曲がるクロソイド 50 m、円弧 40 m、直線へ戻るブロス曲線 30 m
fn alignment() -> Alignment {
    let parts: [(f64, HorizontalKind, [f64; 2], Option<zumen_geom2d::Ramp>); 4] = [
        (100.0, HorizontalKind::Line, [0.0, 0.0], None),
        (
            50.0,
            HorizontalKind::Transition {
                start_radius: None,
                end_radius: Some(200.0),
                curve: TransitionCurve::Clothoid,
            },
            [0.0, 1.0 / 200.0],
            Some(zumen_geom2d::Ramp::Clothoid),
        ),
        (
            40.0,
            HorizontalKind::Arc { radius: 200.0 },
            [1.0 / 200.0; 2],
            None,
        ),
        (
            30.0,
            HorizontalKind::Transition {
                start_radius: Some(200.0),
                end_radius: None,
                curve: TransitionCurve::Bloss,
            },
            [1.0 / 200.0, 0.0],
            Some(zumen_geom2d::Ramp::Bloss),
        ),
    ];
    let (mut point, mut direction) = ([0.0, 0.0], 0.0);
    let mut horizontal = Vec::new();
    for (length, kind, curvatures, ramp) in parts {
        horizontal.push(Horizontal {
            start: MapPoint {
                northing: point[0],
                easting: point[1],
            },
            direction,
            length,
            kind,
            source: Ref::default(),
        });
        (point, direction) =
            zumen_geom2d::along(point, direction, curvatures, length, ramp, length);
    }
    let segment =
        |start_distance: f64, start_height: f64, start_gradient: f64, length: f64, kind| Vertical {
            start_distance,
            start_height,
            start_gradient,
            length,
            kind,
            source: Ref::default(),
        };
    let section = |distance: f64, ground: f64| CrossSection {
        distance,
        name: None,
        lines: vec![
            CrossSectionLine {
                name: None,
                role: SurfaceRole::Existing,
                points: vec![[-10.0, ground - 0.5], [10.0, ground + 0.5]],
            },
            CrossSectionLine {
                name: None,
                role: SurfaceRole::Design,
                points: vec![
                    [-3.5, ground + 0.93],
                    [0.0, ground + 1.0],
                    [3.5, ground + 0.93],
                ],
            },
        ],
        source: Ref::default(),
    };
    Alignment {
        id: "本線".to_string(),
        horizontal,
        vertical: vec![
            segment(0.0, 50.0, 0.02, 100.0, VerticalKind::ConstantGradient),
            segment(
                100.0,
                52.0,
                0.02,
                40.0,
                VerticalKind::Parabola {
                    end_gradient: -0.01,
                },
            ),
            segment(140.0, 52.4, -0.01, 80.0, VerticalKind::ConstantGradient),
        ],
        cross_sections: vec![section(20.0, 49.0), section(120.0, 51.0)],
        ..Alignment::default()
    }
}

fn surface() -> Tin {
    let at = |northing: f64, easting: f64| MapPoint { northing, easting };
    Tin {
        id: "計画面".to_string(),
        name: None,
        role: SurfaceRole::Design,
        points: vec![
            (at(0.0, -20.0), 50.0),
            (at(0.0, 20.0), 50.0),
            (at(60.0, 20.0), 51.0),
            (at(60.0, -20.0), 51.0),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3], [0, 2, 9]],
        boundaries: Vec::new(),
        breaklines: Vec::new(),
        source: Ref::default(),
    }
}

#[cfg(test)]
fn make() -> (Drawing2D, Vec<Code>) {
    let model = CivilModel {
        alignments: vec![alignment()],
        surfaces: vec![surface()],
        ..CivilModel::default()
    };
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = drawing(&model, &model.alignments[0], &Options::default(), &mut cx).unwrap();
    (out, diag.iter().map(|d| d.code()).collect())
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
}

#[test]
fn makes_a_plan_a_profile_and_cross_sections() {
    let (out, codes) = make();
    assert_eq!(out.spaces.len(), 4);
    let names: Vec<_> = out
        .sheets
        .iter()
        .map(|sheet| sheet.name.clone().unwrap())
        .collect();
    assert_eq!(
        names,
        ["平面図", "縦断図", "横断図 20.000", "横断図 120.000"]
    );
    // ブロス曲線は折れ線にし、範囲の外の点の三角形は除いて知らせる
    assert_eq!(
        codes,
        [
            codes::CIVIL_CURVE_APPROXIMATED,
            codes::CIVIL_TRIANGLE_INVALID
        ]
    );
    // 縦断図のビューは、横 1/1000・縦 1/100
    let m = out.sheets[1].views[0].transform.m;
    assert!((m[0] - 1.0).abs() < 1e-12 && (m[4] - 10.0).abs() < 1e-12);
    assert!((out.sheets[0].views[0].transform.m[0] - 1.0).abs() < 1e-12);
}

#[test]
fn the_clothoid_becomes_the_origin_form_and_meets_the_arc() {
    let (out, _) = make();
    let plan = &out.spaces[0];
    let edges: Vec<_> = plan
        .elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Curve(edge) => Some(edge),
            _ => None,
        })
        .collect();
    let Edge::Clothoid {
        origin,
        parameter,
        angle,
        counterclockwise,
        start,
        end,
    } = edges[0]
    else {
        panic!("{:?}", edges[0]);
    };
    let length = |value: &Length| match value {
        Length::Model(length) => length.value,
        Length::Paper(_) => f64::NAN,
    };
    let points = zumen_geom2d::clothoid(
        [origin.x, origin.y],
        length(parameter),
        *angle,
        *counterclockwise,
        length(start),
        length(end),
        1e-4,
    );
    // 右へ曲がるので時計回り。A² = 50 × 200
    assert!(!counterclockwise);
    assert!((length(parameter) - 100.0).abs() < 1e-9);
    // 区間の初め（北へ 100 m）から、円弧の初めへ着く
    let arc = &alignment().horizontal[2];
    assert!(near(points[0], [0.0, 100.0]));
    assert!(near(
        *points.last().unwrap(),
        [arc.start.easting, arc.start.northing]
    ));
    let Edge::Arc {
        center,
        radius,
        counterclockwise,
        ..
    } = edges[1]
    else {
        panic!("{:?}", edges[1]);
    };
    assert!(!counterclockwise);
    assert!((length(radius) - 200.0).abs() < 1e-9);
    let gap = libm::hypot(center.x - arc.start.easting, center.y - arc.start.northing);
    assert!((gap - 200.0).abs() < 1e-9);
}

#[test]
fn every_sheet_draws_in_the_scene() {
    let (out, _) = make();
    for sheet in &out.sheets {
        let options = zumen_scene::Options {
            origin: Origin::Sheet(sheet.id),
            tolerance: Some(0.01),
            print: false,
            region: None,
            palette: Palette::default(),
            chunking: Chunking::PROVISIONAL,
        };
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let scene = build(
            &out,
            &options,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .map_err(|(stop, _)| stop)
        .unwrap();
        assert!(scene.counts.lines > 0, "{:?}", sheet.name);
        assert!(scene.counts.texts > 0, "{:?}", sheet.name);
    }
    // 平面図の空間には、TIN の 2 つの三角形と測点（No.0〜No.11）の線と文字がある
    let plan = &out.spaces[0];
    let texts = plan
        .elements
        .iter()
        .filter(|e| matches!(e.shape, Shape::Text(_)))
        .count();
    assert_eq!(texts, 12);
    assert_eq!(out.sheets[1].id, SheetId(1));
    assert_eq!(out.spaces[1].id, SpaceId(1));
}

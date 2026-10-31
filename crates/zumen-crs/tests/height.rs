//! 標高の成果と基準、空間の間の変換を確かめる（決定 0019）。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref, codes};
use zumen_crs::{
    Axes, Height, HeightDatum, MapConversion, MapPoint, MapPosition, ProjectPoint, Realization,
    VerticalCrs,
};

fn with_cx<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let value = f(&mut cx);
    (value, diag)
}

fn height(value: f64, datum: HeightDatum, realization: Realization) -> Height {
    Height {
        value,
        datum,
        realization,
    }
}

#[test]
fn heights_of_different_realizations_are_reported() {
    let a = height(12.5, HeightDatum::TokyoBay, Realization::Survey2024);
    let b = height(10.0, HeightDatum::TokyoBay, Realization::Survey2024);
    let (same, diag) = with_cx(|cx| a.difference(&b, Ref::at_record(0), cx));
    assert_eq!(same, Some(2.5));
    assert!(diag.is_empty());

    let old = height(10.0, HeightDatum::TokyoBay, Realization::Survey2011);
    let (mixed, diag) = with_cx(|cx| a.difference(&old, Ref::at_record(3), cx));
    assert_eq!(mixed, Some(2.5));
    let found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(found, [codes::CRS_HEIGHT_REALIZATION_MIXED]);

    let ellipsoid = height(50.0, HeightDatum::Ellipsoid, Realization::Survey2024);
    let (none, diag) = with_cx(|cx| a.difference(&ellipsoid, Ref::at_record(4), cx));
    assert_eq!(none, None);
    let found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(found, [codes::CRS_HEIGHT_DATUM_UNSUPPORTED]);
}

#[test]
fn unwritten_realizations_are_assumed_with_a_diagnostic() {
    let (written, diag) = with_cx(|cx| {
        Realization::or_assume(
            Some(Realization::Survey2011),
            Realization::Survey2024,
            Ref::at_record(0),
            cx,
        )
    });
    assert_eq!(written, Realization::Survey2011);
    assert!(diag.is_empty());
    let (assumed, diag) =
        with_cx(|cx| Realization::or_assume(None, Realization::Survey2024, Ref::at_record(0), cx));
    assert_eq!(assumed, Realization::Survey2024);
    let found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(found, [codes::CRS_HEIGHT_REALIZATION_ASSUMED]);
}

#[test]
fn vertical_systems_carry_their_realization() {
    assert_eq!(VerticalCrs::from_epsg(6695), Some(VerticalCrs::Jgd2011));
    assert_eq!(VerticalCrs::from_epsg(11317), Some(VerticalCrs::Jgd2024));
    assert_eq!(
        VerticalCrs::from_name("EPSG:11317").map(VerticalCrs::realization),
        Some(Realization::Survey2024)
    );
    assert_eq!(
        VerticalCrs::from_name("JGD2011 (vertical) height").map(VerticalCrs::realization),
        Some(Realization::Survey2011)
    );
    assert_eq!(VerticalCrs::Jgd2024.epsg(), 11317);
}

/// IfcMapConversion: 尺度を掛け、反時計回りに回し、動かす。逆の変換で戻る
#[test]
fn map_conversions_scale_rotate_and_translate() {
    let conversion = MapConversion {
        eastings: -20_000.0,
        northings: 35_000.0,
        orthogonal_height: 3.5,
        x_axis_abscissa: 0.0,
        x_axis_ordinate: 2.0,
        scale: 0.001,
    };
    // X 軸が北を向くので、X の 1000 mm は北へ 1 m
    let position = conversion
        .to_map(ProjectPoint {
            x: 1000.0,
            y: 2000.0,
            z: 500.0,
        })
        .unwrap();
    assert_eq!(
        position,
        MapPosition {
            point: MapPoint {
                northing: 35_001.0,
                easting: -20_002.0
            },
            height: 4.0
        }
    );
    let back = conversion.to_project(position).unwrap();
    assert!(
        (back.x - 1000.0).abs() < 1e-6
            && (back.y - 2000.0).abs() < 1e-6
            && (back.z - 500.0).abs() < 1e-9
    );
    let broken = MapConversion {
        x_axis_abscissa: 0.0,
        x_axis_ordinate: 0.0,
        ..conversion
    };
    assert_eq!(
        broken.to_map(ProjectPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }),
        None
    );
    let negative = MapConversion {
        scale: -1.0,
        ..conversion
    };
    assert_eq!(negative.to_project(position), None);
}

/// 測量の向き（X が北）の点は、東・北の順に直すと成分が入れ替わる
#[test]
fn surveying_axes_swap_into_east_and_north() {
    assert_eq!(Axes::NorthEast.east_north([3.0, 4.0]), [4.0, 3.0]);
    assert_eq!(Axes::EastNorth.east_north([3.0, 4.0]), [3.0, 4.0]);
    assert_eq!(
        Axes::NorthEast.map_point([3000.0, 4000.0], 0.001),
        MapPoint {
            northing: 3.0,
            easting: 4.0
        }
    );
    let [a, b, c, d, e, f] = Axes::NorthEast.to_east_north();
    let (x, y) = (3.0, 4.0);
    assert_eq!([a * x + b * y + c, d * x + e * y + f], [4.0, 3.0]);
}

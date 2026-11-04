//! テストの中で作った拡張 DM（数値地形図データ）を読み、地理の領域モデルに移す（決定 0233、#378）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_crs::{Crs, MapPoint, Zone};
use zumen_extdm::ExtDm;
use zumen_geo::{Form, GeoModel, Location, Position, Value};
use zumen_geom3d::Shape;
use zumen_sdk::{Codec, FileModel, Lift};

/// フィールドの値。整数型は右に、文字型は左に詰める
enum F<'a> {
    I(i64),
    A(&'a str),
}

use F::{A, I};

/// 84 バイトのレコードと区切りの CR LF。`fields` はカラム（0 から）・桁数・値
#[cfg(test)]
fn rec(kind: &str, fields: &[(usize, usize, F)], sjis: bool) -> Vec<u8> {
    let mut record = vec![b' '; 84];
    record[..kind.len()].copy_from_slice(kind.as_bytes());
    for (start, width, value) in fields {
        let bytes = match value {
            I(value) => format!("{value:>width$}").into_bytes(),
            A(text) if sjis => encoding_rs::SHIFT_JIS.encode(text).0.into_owned(),
            A(text) => text.as_bytes().to_vec(),
        };
        assert!(bytes.len() <= *width, "{start}");
        record[*start..*start + bytes.len()].copy_from_slice(&bytes);
    }
    record.extend_from_slice(b"\r\n");
    record
}

/// I7 の値を並べたレコード（座標・グリッド・不整三角網）
fn numbers(values: &[i64]) -> Vec<u8> {
    let fields: Vec<(usize, usize, F)> = values
        .iter()
        .enumerate()
        .map(|(i, v)| (i * 7, 7, I(*v)))
        .collect();
    rec("", &fields, false)
}

/// グループヘッダー・要素などに共通のフィールド（分類コード・要素識別番号・階層レベル）
fn head(code: &str, number: i64, level: i64) -> Vec<(usize, usize, F<'_>)> {
    vec![(2, 4, A(code)), (12, 4, I(number)), (16, 2, I(level))]
}

/// 要素レコード。実データ区分・データ数・レコード数・代表点・属性数値
#[allow(clippy::too_many_arguments)]
fn element(
    kind: &str,
    code: &str,
    number: i64,
    level: i64,
    real: i64,
    count: i64,
    records: i64,
    point: (i64, i64),
    value: Option<i64>,
) -> Vec<u8> {
    let mut fields = head(code, number, level);
    fields.extend([
        (20, 1, I(real)),
        (27, 4, I(count)),
        (31, 4, I(records)),
        (35, 7, I(point.0)),
        (42, 7, I(point.1)),
        (65, 4, A("0803")),
    ]);
    if let Some(value) = value {
        fields.push((49, 7, I(value)));
    }
    rec(kind, &fields, false)
}

/// インデックスと図郭のレコード（地図情報レベル 500、座標値は mm、左下隅は X -40000.25 m・Y -20000.25 m）
fn header(sjis: bool, datum: i64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(rec(
        "I ",
        &[
            (2, 2, I(9)),
            (4, 30, A("テスト市")),
            (34, 3, I(1)),
            (37, 2, I(1)),
            (39, 4, I(1)),
            (43, 1, I(0)),
            (44, 1, I(0)),
            (45, 4, I(2008)),
            (49, 30, A("公共測量作業規程")),
            (79, 1, I(2)),
            (80, 1, I(0)),
        ],
        sjis,
    ));
    out.extend(rec("", &[(0, 8, A("09LD0000"))], sjis));
    out.extend(rec(
        "",
        &[
            (0, 4, A("3001")),
            (4, 4, A("3001")),
            (8, 9, A("110000000")),
            (17, 1, I(0)),
            (18, 1, I(0)),
            (19, 65, A("建物")),
        ],
        sjis,
    ));
    out.extend(rec(
        "M ",
        &[
            (2, 8, A("09LD0000")),
            (10, 20, A("図郭一")),
            (30, 5, I(500)),
            (35, 30, A("都市計画図")),
            (65, 2, I(0)),
            (67, 1, I(2)),
        ],
        sjis,
    ));
    out.extend(rec(
        "",
        &[
            (0, 7, I(-40000)),
            (7, 7, I(-20000)),
            (14, 7, I(-39700)),
            (21, 7, I(-19600)),
            (31, 6, I(9)),
            (37, 7, I(30)),
            (44, 3, I(1)),
            (47, 7, I(-39700)),
            (54, 7, I(-20000)),
            (61, 7, I(-40000)),
            (68, 7, I(-19600)),
        ],
        sjis,
    ));
    out.extend(rec("", &[(8, 8, A("09LD0001"))], sjis));
    out.extend(rec(
        "",
        &[
            (0, 4, A("0803")),
            (4, 4, A("0000")),
            (8, 1, I(0)),
            (9, 1, I(0)),
            (10, 30, A("図化機")),
            (70, 1, I(datum)),
        ],
        sjis,
    ));
    out.extend(rec(
        "",
        &[(0, 40, A("テスト測量")), (40, 4, I(-250)), (44, 4, I(-250))],
        sjis,
    ));
    out
}

/// 地物の種類を一通り含むファイル
#[cfg(test)]
fn sample() -> Vec<u8> {
    let mut out = header(false, 1);
    // レイヤヘッダー
    out.extend(rec("H ", &head("3000", 0, 1), false));
    // 面データ（建物）
    out.extend(element("E1", "3001", 1, 2, 2, 5, 1, (0, 0), None));
    out.extend(numbers(&[
        1000, 1000, 1000, 2000, 2000, 2000, 2000, 1000, 1000, 1000,
    ]));
    // 要素グループヘッダーと、その中の線データ・属性データ
    out.extend(rec("H ", &head("2255", 2, 2), false));
    out.extend(element("E2", "2255", 1, 3, 2, 3, 1, (0, 0), None));
    out.extend(numbers(&[0, 0, 0, 5000, 5000, 5000]));
    let mut attribute = element("E8", "2255", 2, 3, 5, 1, 1, (0, 0), None);
    attribute[58..65].copy_from_slice(b"(A20)  ");
    out.extend(attribute);
    out.extend(rec("", &[(0, 20, A("市道1号"))], false));
    // 点データ（標高点）。代表点の座標と、mm の属性数値の標高
    out.extend(element(
        "E5",
        "7101",
        2,
        2,
        0,
        0,
        0,
        (1000, 2000),
        Some(12345),
    ));
    // 3 次元の線データ。2 点めの Z はない（-999000）
    out.extend(element("E2", "5101", 3, 2, 3, 2, 1, (0, 0), None));
    out.extend(numbers(&[0, 0, 100_000, 1000, 0, -999_000]));
    // 円データ（中心 X 10 m・Y 10 m、半径 5 m）
    out.extend(element("E3", "3201", 4, 2, 2, 3, 1, (0, 0), None));
    out.extend(numbers(&[15000, 10000, 10000, 15000, 5000, 10000]));
    // 注記データ（漢字 4 字、字大 2.5 mm）
    out.extend(element("E7", "3001", 5, 2, 4, 4, 1, (3000, 4000), None));
    out.extend(rec(
        "",
        &[
            (0, 1, I(0)),
            (1, 7, I(0)),
            (8, 5, I(25)),
            (13, 5, I(0)),
            (18, 2, I(1)),
            (20, 64, A("公共測量")),
        ],
        false,
    ));
    // 方向データ
    out.extend(element("E6", "4101", 6, 2, 2, 2, 1, (0, 0), None));
    out.extend(numbers(&[0, 0, 100, 0]));
    // グリッド（2 行 3 列、間隔 1 m、原点は図郭の原点）。北の行の中の値が 1 つない
    out.extend(rec(
        "G ",
        &[
            (2, 4, A("7501")),
            (12, 4, I(7)),
            (16, 2, I(2)),
            (18, 4, I(2)),
            (22, 4, I(3)),
            (26, 4, I(1)),
            (30, 7, I(1000)),
            (37, 7, I(1000)),
            (44, 7, I(0)),
            (51, 7, I(0)),
        ],
        false,
    ));
    out.extend(numbers(&[1000, 2000, 3000, 4000, -999_000, 6000]));
    // 不整三角網（三角形 1 つ、時計回りに記録）
    out.extend(rec(
        "T ",
        &[
            (2, 4, A("7601")),
            (12, 4, I(8)),
            (16, 2, I(2)),
            (20, 6, I(1)),
            (26, 6, I(1)),
        ],
        false,
    ));
    out.extend(numbers(&[0, 0, 1000, 1000, 0, 2000, 0, 1000, 3000]));
    // 知らないレコードタイプ
    out.extend(rec("Z9", &[], false));
    out
}

fn lift_with(src: &[u8], budget: Budget) -> (Result<GeoModel, zumen_sdk::Error>, Vec<Code>) {
    let mut budget = budget;
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let model = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        ExtDm::read(src, &mut cx).and_then(|file| file.lift(&mut cx))
    };
    (model, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn lift(src: &[u8]) -> (GeoModel, Vec<Code>) {
    let (model, codes) = lift_with(src, Budget::default());
    (model.unwrap(), codes)
}

fn near(location: &Location, north: f64, east: f64) -> bool {
    let Position::Map(MapPoint { northing, easting }) = location.position else {
        return false;
    };
    (northing - north).abs() < 1e-9 && (easting - east).abs() < 1e-9
}

#[test]
fn splits_headers_and_their_data_records() {
    let src = sample();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = ExtDm::read(&src, &mut cx).unwrap();
    let kinds: Vec<&[u8]> = (0..file.record_count())
        .map(|i| file.record(i).unwrap().kind)
        .collect();
    assert_eq!(
        &kinds[..9],
        [b"I" as &[u8], b"", b"", b"M", b"", b"", b"", b"", b"H"]
    );
    assert_eq!(file.record_count(), 30);
    assert_eq!(kinds[29], b"");
    // 判定の規則は持たない
    assert!(ExtDm::SIGNATURES.is_empty());
}

#[test]
fn moves_the_index_and_the_sheet() {
    let (model, codes) = lift(&sample());
    assert_eq!(
        codes,
        [
            codes::EXTDM_DATUM_ASSUMED,
            codes::EXTDM_CURVE_APPROXIMATED,
            codes::EXTDM_RECORD_UNSUPPORTED
        ]
    );
    assert_eq!(model.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    assert_eq!(model.height, None);
    let index = &model.features[0];
    assert_eq!(index.kind, "extdm:I");
    assert_eq!(index.id, "0");
    assert_eq!(index.name.as_deref(), Some("テスト市"));
    let value = |name: &str| {
        index
            .attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.clone())
    };
    assert_eq!(
        value("extdm:使用分類コード/3001/内容記述"),
        Some(Value::Text("建物".into()))
    );
    assert_eq!(
        value("extdm:図郭識別番号"),
        Some(Value::Text("09LD0000".into()))
    );

    let sheet = &model.features[1];
    assert_eq!(sheet.kind, "extdm:M");
    assert_eq!(sheet.id, "3");
    assert_eq!(sheet.name.as_deref(), Some("図郭一"));
    assert!(
        sheet
            .attributes
            .iter()
            .any(|a| a.name == "extdm:隣接図郭識別番号/2"
                && a.value == Value::Text("09LD0001".into()))
    );
    // 図郭の四隅。左下隅は端数（mm）を足す
    let Form::Area { outer, .. } = &sheet.geometries[0].form else {
        panic!();
    };
    assert_eq!(outer.len(), 4);
    assert!(near(&outer[0], -40000.25, -20000.25));
    assert!(near(&outer[2], -39700.0, -19600.0));
}

#[test]
fn nests_elements_under_groups_and_places_coordinates() {
    let (model, _) = lift(&sample());
    let kinds: Vec<(&str, Option<&str>, Option<usize>)> = model
        .features
        .iter()
        .map(|f| (f.kind.as_str(), f.code.as_deref(), f.parent))
        .collect();
    assert_eq!(
        kinds,
        [
            ("extdm:I", None, None),
            ("extdm:M", None, None),
            ("extdm:H", Some("3000"), Some(1)),
            ("extdm:E1", Some("3001"), Some(2)),
            ("extdm:H", Some("2255"), Some(2)),
            ("extdm:E2", Some("2255"), Some(4)),
            ("extdm:E8", Some("2255"), Some(4)),
            ("extdm:E5", Some("7101"), Some(2)),
            ("extdm:E2", Some("5101"), Some(2)),
            ("extdm:E3", Some("3201"), Some(2)),
            ("extdm:E7", Some("3001"), Some(2)),
            ("extdm:E6", Some("4101"), Some(2)),
            ("extdm:G", Some("7501"), Some(2)),
            ("extdm:T", Some("7601"), Some(2)),
        ]
    );

    // 面データ。座標は図郭の原点（左下隅）からの mm。閉じる点は除く
    let building = &model.features[3];
    assert_eq!(building.id, "9");
    let Form::Area { outer, .. } = &building.geometries[0].form else {
        panic!();
    };
    assert_eq!(outer.len(), 4);
    assert!(near(&outer[0], -39999.25, -19999.25));
    assert!(near(&outer[2], -39998.25, -19998.25));
    assert!(
        building
            .attributes
            .iter()
            .any(|a| a.name == "extdm:要素識別番号" && a.value == Value::Integer(1))
    );
    assert!(
        building
            .attributes
            .iter()
            .any(|a| a.name == "extdm:取得年月" && a.value == Value::Text("0803".into()))
    );

    // 属性データ
    let attribute = &model.features[6];
    assert!(
        attribute
            .attributes
            .iter()
            .any(|a| a.name == "extdm:属性データ" && a.value == Value::Text("市道1号".into()))
    );
    assert!(
        attribute
            .attributes
            .iter()
            .any(|a| a.name == "extdm:属性データの書式" && a.value == Value::Text("(A20)".into()))
    );
}

#[test]
fn reads_points_heights_curves_annotations_and_directions() {
    let (model, _) = lift(&sample());
    // 点データは代表点に、mm の属性数値を標高にする
    let Form::Point(point) = model.features[7].geometries[0].form else {
        panic!();
    };
    assert!(near(&point, -39999.25, -19998.25));
    assert_eq!(point.height, Some(12.345));

    // 3 次元座標の Z。ない値は標高を持たない
    let Form::Line(points) = &model.features[8].geometries[0].form else {
        panic!();
    };
    assert_eq!(points[0].height, Some(100.0));
    assert_eq!(points[1].height, None);

    // 円データは、座標値の単位（1 mm）の許容差の多角形
    let Form::Area { outer, .. } = &model.features[9].geometries[0].form else {
        panic!();
    };
    assert!(outer.len() > 32, "{}", outer.len());
    for location in outer {
        let Position::Map(p) = location.position else {
            panic!();
        };
        let (dn, de) = (p.northing + 40000.25 - 10.0, p.easting + 20000.25 - 10.0);
        let r = libm::hypot(dn, de);
        assert!((r - 5.0).abs() < 1e-6, "{r}");
    }

    // 注記データ
    let annotation = &model.features[10];
    assert_eq!(annotation.name.as_deref(), Some("公共測量"));
    assert!(annotation.attributes.iter().any(|a| a.name == "extdm:字大"
        && a.value
            == Value::Measure {
                value: 2.5,
                unit: Some("mm".into())
            }));
    let Form::Point(point) = annotation.geometries[0].form else {
        panic!();
    };
    assert!(near(&point, -39997.25, -19996.25));

    // 方向データは 2 点の線
    let Form::Line(pair) = &model.features[11].geometries[0].form else {
        panic!();
    };
    assert_eq!(pair.len(), 2);
    assert!(near(&pair[1], -40000.15, -20000.25));
}

#[test]
fn makes_triangles_of_grids_and_tins() {
    let (model, _) = lift(&sample());
    let Form::Solid(Shape::Triangles {
        positions,
        triangles,
        ..
    }) = &model.features[12].geometries[0].form
    else {
        panic!();
    };
    // 6 点のうち北の行の中の 1 点がないので 5 点。点は（東・北・標高）
    assert_eq!(positions.len(), 5);
    assert_eq!(positions[0], [-20000.25, -40000.25, 1.0]);
    assert_eq!(positions[4], [-19998.25, -39999.25, 6.0]);
    // 西の四角形は南西・南東・北西の三角形だけ。東の四角形は北西の点がないので作らない
    assert_eq!(triangles, &[[0, 1, 3]]);

    // 不整三角網は、上から見て反時計回りにそろえる
    let Form::Solid(Shape::Triangles {
        positions,
        triangles,
        ..
    }) = &model.features[13].geometries[0].form
    else {
        panic!();
    };
    assert_eq!(triangles, &[[0, 1, 2]]);
    let [a, b, c] = [positions[0], positions[1], positions[2]];
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    assert!(cross > 0.0);
}

#[test]
fn reads_shift_jis_and_the_tokyo_datum() {
    let mut src = header(true, 0);
    src.extend(rec("H ", &head("3000", 0, 1), true));
    let (model, codes) = lift(&src);
    assert!(codes.is_empty(), "{codes:?}");
    assert_eq!(model.crs, Some(Crs::Tokyo(Zone::new(9).unwrap())));
    assert_eq!(model.features[0].name.as_deref(), Some("テスト市"));
    assert_eq!(model.features[1].name.as_deref(), Some("図郭一"));
}

#[test]
fn reports_files_without_a_coordinate_system() {
    // インデックスレコードのないファイル。地物と属性は移し、形は移さない
    let src = sample();
    let records: Vec<&[u8]> = src.split_inclusive(|b| *b == b'\n').collect();
    let without: Vec<u8> = records[3..].concat();
    let (model, codes) = lift(&without);
    assert_eq!(codes[0], codes::EXTDM_CRS_UNSUPPORTED);
    assert_eq!(model.crs, None);
    assert!(model.features.iter().all(|f| f.geometries.is_empty()));
    assert_eq!(model.features.len(), 13);
}

#[test]
fn reports_broken_records() {
    // 面データの座標が数でない
    let src = String::from_utf8(sample()).unwrap().replacen(
        "   1000   1000   1000   2000",
        "   1000   1000   10x0   2000",
        1,
    );
    let (model, codes) = lift(src.as_bytes());
    assert!(codes.contains(&codes::EXTDM_RECORD_INVALID), "{codes:?}");
    assert!(model.features[3].geometries.is_empty());

    // 旧 DM（バージョン 0）は、同じ書式として読み、知らせる
    let mut src = sample();
    src[79] = b'0';
    let (_, codes) = lift(&src);
    assert!(
        codes.contains(&codes::EXTDM_VERSION_UNSUPPORTED),
        "{codes:?}"
    );
}

#[test]
fn stops_at_the_budget() {
    let budget = Budget::new(Limits::default().with(Kind::Segments, Limit::Finite(4)));
    let (model, codes) = lift_with(&sample(), budget);
    assert!(model.is_err());
    assert_eq!(codes.last(), Some(&codes::BUDGET_SEGMENTS_EXCEEDED));
}

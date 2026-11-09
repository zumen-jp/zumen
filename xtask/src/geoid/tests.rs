//! 検証キットのコマンドを、テストの中で作る合成の格子で確かめる。国土地理院のモデルとその値、サービスが返した値は
//! 使わない（決定 0019）。

use super::*;

use crate::formats::tests::TempDir;

/// 説明書（2026 年 4 月）の 2 章 (1)② の格子の形。北緯 15〜50 度・東経 120〜160 度で、間隔は緯度 1 分・経度 1.5 分
/// （2101 行 × 1601 列）。値は持たない
const JPGEO2024: Extent = Extent {
    lat_min: 15.0,
    lat_max: 50.0,
    lon_min: 120.0,
    lon_max: 160.0,
    rows: 2101,
    cols: 1601,
};

/// 合成の格子の ISG。間隔は、範囲と行・列の数から求めて書く
fn isg(extent: &Extent, values: &str) -> String {
    let delta = |span: f64, n: usize| span / (n - 1) as f64;
    format!(
        "synthetic\n\
begin_of_head ================================================\n\
model name     : SYNTHETIC\n\
model year     : 2026\n\
data units     : meters\n\
data format    : grid\n\
data ordering  : N-to-S, W-to-E\n\
coord type     : geodetic\n\
coord units    : deg\n\
lat min        = {}\n\
lat max        = {}\n\
lon min        = {}\n\
lon max        = {}\n\
delta lat      = {}\n\
delta lon      = {}\n\
nrows          = {}\n\
ncols          = {}\n\
nodata         = -9999.0000\n\
ISG format     =        2.0\n\
end_of_head ==================================================\n\
{values}",
        extent.lat_min,
        extent.lat_max,
        extent.lon_min,
        extent.lon_max,
        delta(extent.lat_max - extent.lat_min, extent.rows),
        delta(extent.lon_max - extent.lon_min, extent.cols),
        extent.rows,
        extent.cols
    )
}

/// 合成の格子の形。6 行 6 列で、間隔は 0.25 度（15 分）
const SYNTHETIC: Extent = Extent {
    lat_min: 40.0,
    lat_max: 41.25,
    lon_min: 120.0,
    lon_max: 121.25,
    rows: 6,
    cols: 6,
};

/// 合成の格子。中の 2 行 2 列は無効で、その北西には値が 0 の格子点を置く
fn synthetic(values: &str) -> String {
    isg(&SYNTHETIC, values)
}

const VALUES: &str = "\
    1.0000     2.0000     3.0000     4.0000     5.0000     6.0000
    7.0000     0.0000     0.0000    30.0000     8.0000     9.0000
   10.0000     0.0000 -9999.0000 -9999.0000    11.0000    12.0000
   13.0000    14.0000 -9999.0000 -9999.0000    15.0000    16.0000
   17.0000    18.0000    19.0000    20.0000    21.0000    22.0000
   23.0000    24.0000    25.0000    26.0000    27.0000    28.0000
";

fn read(src: &str) -> Grid {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    Grid::read(
        src.as_bytes(),
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap()
}

fn dms(text: &str) -> Dms {
    Dms::parse(text).unwrap().0
}

fn kit() -> Vec<Point> {
    parse_points(&fs::read_to_string(crate::root().join(KIT_POINTS)).unwrap()).unwrap()
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn angles_are_written_as_the_service_and_geoidcalc_read_them() {
    let (value, exact) = Dms::parse("360613.5893").unwrap();
    assert!(exact);
    assert_eq!(value.to_string(), "360613.5893");
    assert_eq!(dms("1400600").to_string(), "1400600.0000");
    assert_eq!(dms("+370856.12340").to_string(), "370856.1234");
    // 秒の小数が 4 桁より細かければ丸め、丸めたことを返す
    assert_eq!(
        Dms::parse("370856.12345"),
        Some((dms("370856.1235"), false))
    );
    assert_eq!(dms("360600.0000").degrees(), 36.1);
    assert_eq!(Dms::from_degrees(36.1), dms("360600"));
    assert_eq!(
        Dms::from_degrees(120.0 + 50.0 / 60.0).to_string(),
        "1205000.0000"
    );
    for text in [
        "", "abc", "376056.0", "360660.0", "3606.1.0", "36°06'", "36 06 00",
    ] {
        assert_eq!(Dms::parse(text), None, "{text}");
    }
}

#[test]
fn the_kit_points_cover_nodes_lines_cells_and_outside() {
    let points = kit();
    assert!(points.len() >= 10, "{}", points.len());
    assert!(points.len() <= SERVICE_LIMIT);
    let mut border = 0;
    for place in [
        Place::Node,
        Place::EastWest,
        Place::NorthSouth,
        Place::Cell,
        Place::Outside,
    ] {
        let found: Vec<_> = points
            .iter()
            .filter(|p| locate(&JPGEO2024, p.at()).0 == place)
            .collect();
        assert!(found.len() >= 2, "{place:?}");
        border += found
            .iter()
            .filter(|p| locate(&JPGEO2024, p.at()).1)
            .count();
    }
    assert!(border >= 4, "{border}");
    // 説明に書いた場所と、格子の形から求めた場所が同じ
    for (id, place, edge) in [
        ("P01", Place::Node, false),
        ("P03", Place::Node, false),
        ("P04", Place::Node, true),
        ("P05", Place::Node, true),
        ("P06", Place::EastWest, false),
        ("P07", Place::NorthSouth, false),
        ("P08", Place::EastWest, true),
        ("P09", Place::NorthSouth, true),
        ("P10", Place::Cell, false),
        ("P20", Place::Outside, false),
        ("P23", Place::Outside, false),
        // 範囲の縁から 0.0001 秒内側の点。縁の線の上ではない
        ("P24", Place::NorthSouth, false),
        ("P25", Place::NorthSouth, false),
        ("P26", Place::EastWest, false),
        ("P27", Place::EastWest, false),
    ] {
        let point = points.iter().find(|p| p.id == id).unwrap();
        assert_eq!(locate(&JPGEO2024, point.at()), (place, edge), "{id}");
    }
}

#[test]
fn points_that_repeat_or_need_rounding_are_refused() {
    let points = parse_points("# コメント\n\nA 360600.0000 1400600.0000 説明 の 文\n").unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].label, "説明 の 文");
    for text in [
        "A 360600.0000 1400600.0000\nA 360700.0000 1400600.0000",
        "A 360600.0000 1400600.0000\nB 360600 1400600",
        "A 360600.00001 1400600.0000",
        "A 360600.0000",
        "A-1 360600.0000 1400600.0000",
        "A 910000.0000 1400600.0000",
    ] {
        assert!(parse_points(text).is_err(), "{text}");
    }
}

/// 選んだ際が、形・位置・場所・zumen の値のあるなしで、期待のとおりか
fn assert_edges(grid: &Grid, expected: &[(Edge, &str, &str)]) {
    let extent = grid.extent();
    let found = edges(grid);
    assert_eq!(found.len(), expected.len());
    for ((edge, at), (want, lat, lon)) in found.iter().zip(expected) {
        assert_eq!(edge, want);
        let point = Point {
            id: "E".into(),
            lat: Dms::from_degrees(at.latitude),
            lon: Dms::from_degrees(at.longitude),
            label: String::new(),
        };
        assert_eq!(
            (point.lat.to_string(), point.lon.to_string()),
            ((*lat).to_owned(), (*lon).to_owned()),
            "{edge:?}"
        );
        assert_eq!(locate(&extent, point.at()).0, edge.place(), "{edge:?}");
        assert_eq!(
            grid.interpolate(point.at()).is_some(),
            edge.has_value(),
            "{edge:?}"
        );
    }
}

/// 形ごとに、使う有効の格子点の値がどれも 0 でない、最初の際を選ぶ。北の側の 2 つの形のほかは、最初に見つかる際が
/// 値が 0 の格子点を使うので、0 でないものを先に選んだことも確かめられる
#[test]
fn edges_are_chosen_around_no_data() {
    let grid = read(&synthetic(VALUES));
    assert_edges(
        &grid,
        &[
            (Edge::Node, "410000.0000", "1204500.0000"),
            (Edge::NodeNorth, "403000.0000", "1201500.0000"),
            (Edge::EastWest, "410000.0000", "1205000.0000"),
            (Edge::EastWestNorth, "401500.0000", "1202000.0000"),
            (Edge::NorthSouth, "402500.0000", "1201500.0000"),
            (Edge::EastWestNoData, "404500.0000", "1205500.0000"),
            (Edge::NorthSouthNoData, "405500.0000", "1204500.0000"),
            (Edge::Cell, "405500.0000", "1205000.0000"),
        ],
    );
    // 無効の格子点がなければ、際の点はない
    let full = read(&synthetic(&VALUES.replace("-9999.0000", "    1.0000")));
    assert!(edges(&full).is_empty());
}

/// 北西の隅と南東の隅の格子点が無効のとき。最初と最後の行・列に接する際を選び、範囲の外の格子点は見ない
#[test]
fn edges_are_chosen_at_the_corners_of_the_grid() {
    let corners = Extent {
        lat_min: 40.0,
        lat_max: 40.75,
        lon_min: 120.0,
        lon_max: 120.75,
        rows: 4,
        cols: 4,
    };
    let grid = read(&isg(
        &corners,
        "-9999.0000 2.0 3.0 4.0\n5.0 6.0 7.0 8.0\n9.0 10.0 11.0 12.0\n13.0 14.0 15.0 -9999.0000\n",
    ));
    assert_edges(
        &grid,
        &[
            (Edge::Node, "401500.0000", "1203000.0000"),
            (Edge::NodeNorth, "403000.0000", "1200000.0000"),
            (Edge::EastWest, "401500.0000", "1203500.0000"),
            (Edge::EastWestNorth, "403000.0000", "1200500.0000"),
            (Edge::NorthSouth, "401000.0000", "1203000.0000"),
            (Edge::EastWestNoData, "404500.0000", "1201000.0000"),
            (Edge::NorthSouthNoData, "403500.0000", "1200000.0000"),
            (Edge::Cell, "404000.0000", "1200500.0000"),
        ],
    );
}

/// 間隔が 1/7 度の格子では、格子点と格子の線の多くが、秒の小数 4 桁で表せない。丸めて場所が変わる際は使わない
#[test]
fn edges_that_four_decimals_of_seconds_cannot_hold_are_skipped() {
    let seventh = Extent {
        lat_min: 40.0,
        lat_max: 40.0 + 3.0 / 7.0,
        lon_min: 120.0,
        lon_max: 120.0 + 3.0 / 7.0,
        rows: 4,
        cols: 4,
    };
    let grid = read(&isg(
        &seventh,
        "1.0 2.0 3.0 4.0\n5.0 -9999.0000 7.0 8.0\n9.0 10.0 11.0 12.0\n13.0 14.0 15.0 16.0\n",
    ));
    assert_eq!(edges(&grid).len(), Edge::ALL.len());
    let kit = parse_points("A 360600.0000 1400600.0000").unwrap();
    let points = gather(kit, &[("seventh.isg".to_owned(), grid)]).unwrap();
    // 残るのは、経度 120 度（西の縁）の南北の線の上と、格子の中だけ
    let added: Vec<(&str, &str)> = points[1..]
        .iter()
        .map(|p| (p.id.as_str(), p.label.as_str()))
        .collect();
    let label = |edge: Edge| format!("{}（seventh.isg の無効の領域の際）", edge.label());
    assert_eq!(
        added,
        [
            ("E01", label(Edge::NorthSouth).as_str()),
            ("E02", label(Edge::Cell).as_str()),
        ]
    );
}

/// 点がサービスの一括計算の上限（100 点）を超えたら止める
#[test]
fn points_over_the_service_limit_are_refused() {
    let grids = [("synthetic.isg".to_owned(), read(&synthetic(VALUES)))];
    let kit = |count: usize| -> Vec<Point> {
        (0..count)
            .map(|i| Point {
                id: format!("P{i}"),
                lat: dms("300000"),
                lon: Dms(dms("1300000").0 + i as i64 * 10_000),
                label: String::new(),
            })
            .collect()
    };
    let fits = SERVICE_LIMIT - Edge::ALL.len();
    assert_eq!(gather(kit(fits), &grids).unwrap().len(), SERVICE_LIMIT);
    let error = gather(kit(fits + 1), &grids).unwrap_err();
    assert!(error.contains("上限"), "{error}");
}

fn value(value: f64, decimals: usize) -> Reading {
    Reading::Value { value, decimals }
}

fn row(point: &Point, reading: Reading) -> Row {
    Row {
        line: 0,
        lat: point.lat,
        lon: point.lon,
        value: reading,
    }
}

#[test]
fn results_are_compared_point_by_point() {
    let grid = read(&synthetic(VALUES));
    // 格子点の上・東西の線の上（両端が有効）・片方の端が無効の線の上・範囲の外・範囲の北西の隅
    let points = parse_points(
        "A 410000 1204500\nB 410000 1205000\nC 404500 1205500\nD 500000 1400000\nE 411500 1200000",
    )
    .unwrap();
    let line = grid.interpolate(points[1].at()).unwrap().value;
    let rows = vec![
        row(&points[0], value(30.00005, 5)),
        row(&points[1], value(line - 0.00003, 4)),
        row(&points[2], Reading::None),
        row(&points[3], Reading::None),
        row(&points[4], value(1.0, 4)),
    ];
    let outcome = check(&points, &grid, &rows).unwrap();
    assert_eq!((outcome.with_value, outcome.without_value), (3, 2));
    assert_eq!(outcome.decimals, Some(4));
    let (largest, id) = outcome.largest.clone().unwrap();
    assert!((largest - 0.00005).abs() < 1e-9, "{largest}");
    assert_eq!(id, "A");
    assert!(outcome.mismatches.is_empty());
    assert!(outcome.border.is_empty());
    assert!(outcome.passed());

    // 0.1 mm を超える差と、値のあるなしの違い
    let mut wrong = rows.clone();
    wrong[0].value = value(30.0002, 4);
    wrong[1].value = Reading::None;
    wrong[2].value = value(1.0, 4);
    wrong[3].value = value(1.0, 4);
    let outcome = check(&points, &grid, &wrong).unwrap();
    assert_eq!(outcome.mismatches.len(), 4, "{:?}", outcome.mismatches);
    assert!(!outcome.passed());

    // 範囲の縁の上で、結果だけが値なしなら、境の規則の違いとして分けて示し、失敗にしない
    let mut edge = rows.clone();
    edge[4].value = Reading::None;
    let outcome = check(&points, &grid, &edge).unwrap();
    assert_eq!(outcome.border.len(), 1, "{:?}", outcome.border);
    assert!(outcome.border[0].starts_with("E（"), "{:?}", outcome.border);
    assert!(outcome.mismatches.is_empty());
    assert_eq!((outcome.with_value, outcome.without_value), (2, 2));
    assert!(outcome.passed());

    // 小数が 3 桁なら、丸めの中では合わないとしないが、0.1 mm の照合にはならない
    let mut short = rows.clone();
    short[0].value = value(30.0, 3);
    short[1].value = value((line * 1000.0).round() / 1000.0, 3);
    let outcome = check(&points, &grid, &short).unwrap();
    assert!(outcome.mismatches.is_empty(), "{:?}", outcome.mismatches);
    assert_eq!(outcome.decimals, Some(3));
    assert!(!outcome.passed());
    short[0].value = value(30.001, 3);
    assert_eq!(check(&points, &grid, &short).unwrap().mismatches.len(), 1);

    // 行の数か緯度・経度が点の一覧と違えば、比べずに止める
    assert!(check(&points, &grid, &rows[..4]).is_err());
    let mut moved = rows.clone();
    moved[1].lat = dms("410000.0001");
    assert!(check(&points, &grid, &moved).is_err());
}

#[test]
fn results_of_the_service_and_geoidcalc_are_read() {
    // サービスの一括計算の結果の形（入出力ファイル形式の説明）。コメントの行と点の名前は Shift_JIS のこともある。
    // BOM と、`#` のない見出しの行も、コメントの行とする
    let mut bytes =
        b"\xEF\xBB\xBF# \x83\x57\x83\x49\x83\x43\x83\x68\r\nlat(dms) lon(dms) geoid(m)\r\n"
            .to_vec();
    bytes.extend_from_slice(
        b"350000.0000 1350000.0000 12.345 0.1234 comment\r\n+410000.0000 1204500.0000 -9999. \x94\xcd\x88\xcd\x8a\x4f\r\n\r\n",
    );
    let parsed = parse_result(&bytes, 3).unwrap();
    assert_eq!(parsed.unreadable, 1);
    assert_eq!(parsed.comments, ["lat(dms) lon(dms) geoid(m)"]);
    assert_eq!(parsed.rows.len(), 2);
    assert_eq!(parsed.rows[0].lat, dms("350000"));
    assert_eq!(parsed.rows[0].value, value(12.345, 3));
    assert_eq!(parsed.rows[1].lat, dms("410000"));
    assert_eq!(parsed.rows[1].value, Reading::None);
    // 4 列目。どの値の行にも、その列が要る
    assert!(parse_result(&bytes, 4).is_err());
    let parsed = parse_result(b"350000.0000 1350000.0000 12.345 0.1234 comment\n", 4).unwrap();
    assert_eq!(parsed.rows[0].value, value(0.1234, 4));
    // 内挿計算プログラムの結果の形（説明書の 3 章 (3)③）
    let parsed = parse_result(
        b"# Geoid file: ./synthetic.isg\n# Program: test\n# Lat(dms) Lon(dms) Geoid(m)\n350000.0000 1350000.0000 NaN\n",
        3,
    )
    .unwrap();
    assert_eq!(parsed.comments.len(), 3);
    assert_eq!(parsed.rows[0].value, Reading::None);
    for text in [
        &b"350000.0000 1350000.0000 abc\n"[..],
        &b"350000.0000 1350000.0000\n"[..],
        &b"350000.0000\n"[..],
        &b"356000.0000 1350000.0000 1.0\n"[..],
        &b"350000.0000 1350000.0000 \xff\n"[..],
    ] {
        assert!(parse_result(text, 3).is_err(), "{text:?}");
    }
}

/// フォルダーに入力を書き、内挿計算プログラムの結果の形のファイルと比べる。値は zumen の値から作る
#[test]
fn points_and_compare_work_outside_the_repository() {
    let dir = TempDir::new();
    let isg = dir.0.join("synthetic.isg");
    fs::write(&isg, synthetic(VALUES)).unwrap();
    let folder = dir.0.join("check");
    points(path(&folder), &[path(&isg)]).unwrap();

    let kit = kit();
    let listed = parse_points(&fs::read_to_string(folder.join(POINTS)).unwrap()).unwrap();
    assert_eq!(listed.len(), kit.len() + Edge::ALL.len());
    assert_eq!(&listed[..kit.len()], &kit[..]);
    assert!(listed[kit.len()..].iter().all(|p| p.id.starts_with('E')));
    for (file, header) in [
        (SERVICE_INPUT, "# latitude(dms) longitude(dms)"),
        (GEOIDCALC_INPUT, "Unit:DMS"),
    ] {
        let text = fs::read_to_string(folder.join(file)).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), listed.len() + 1, "{file}");
        assert_eq!(lines[0], header, "{file}");
        assert_eq!(lines[1], "360600.0000 1400600.0000", "{file}");
    }

    let grid = read(&synthetic(VALUES));
    let result = |shift: f64, decimals: usize| {
        let mut out = String::from("# Geoid file: ./synthetic.isg\n# Lat(dms) Lon(dms) Geoid(m)\n");
        for p in &listed {
            match grid.interpolate(p.at()) {
                Some(u) => out.push_str(&format!(
                    "{} {} {:.*}\n",
                    p.lat,
                    p.lon,
                    decimals,
                    u.value + shift
                )),
                None => out.push_str(&format!("{} {} NaN\n", p.lat, p.lon)),
            }
        }
        out
    };
    let output = folder.join("synthetic.out");
    let run = |text: String| {
        fs::write(&output, text).unwrap();
        compare(path(&folder), path(&isg), path(&output), 3)
    };
    run(result(0.00004, 4)).unwrap();
    assert!(run(result(0.0002, 4)).unwrap_err().contains("合いません"));
    assert!(
        run(result(0.0, 3))
            .unwrap_err()
            .contains("0.1 mm の照合になりません")
    );

    // zumen の値のファイルは、内挿計算プログラムの結果と同じ形で、compare でも読める
    let values = folder.join(values_file("synthetic.isg"));
    let text = fs::read_to_string(&values).unwrap();
    assert_eq!(
        text.lines().filter(|l| !l.starts_with('#')).count(),
        listed.len()
    );
    assert!(text.lines().any(|l| l.ends_with(" NaN")));
    compare(path(&folder), path(&isg), path(&values), 3).unwrap();
}

#[test]
fn files_inside_the_repository_are_refused() {
    let error = outside_repository(&crate::root().join("xtask"), "フォルダー").unwrap_err();
    assert!(error.contains("リポジトリの外"), "{error}");
    let dir = TempDir::new();
    let inside = crate::root().join("Cargo.toml");
    let error = points(path(&dir.0), &[path(&inside)]).unwrap_err();
    assert!(error.contains("リポジトリの外"), "{error}");
    // まだないフォルダーも、リポジトリの中なら作らずに断る
    let new = crate::root().join("xtask").join("geoid-check-refused");
    let error = points(path(&new), &[path(&inside)]).unwrap_err();
    assert!(error.contains("リポジトリの外"), "{error}");
    assert!(!new.exists());

    // 結果のファイルも、リポジトリの中のものは断る
    let isg = dir.0.join("synthetic.isg");
    fs::write(&isg, synthetic(VALUES)).unwrap();
    let folder = dir.0.join("check");
    points(path(&folder), &[path(&isg)]).unwrap();
    let error = compare(path(&folder), path(&isg), path(&inside), 3).unwrap_err();
    assert!(error.contains("リポジトリの外"), "{error}");

    // ほかの Git の作業ツリーの中も断る。作業ツリーが、ほかの checkout の中にあるとき（`.claude/worktrees/` の下など）
    let other = dir.0.join("other");
    fs::create_dir_all(other.join(".git")).unwrap();
    fs::create_dir_all(other.join("geoid")).unwrap();
    let error = points(path(&other.join("geoid")), &[path(&isg)]).unwrap_err();
    assert!(error.contains("Git の作業ツリー"), "{error}");
    let error = points(path(&other.join("new")), &[path(&isg)]).unwrap_err();
    assert!(error.contains("Git の作業ツリー"), "{error}");
    assert!(!other.join("new").exists());

    // 名前の同じ格子は、zumen の値のファイルが重なるので断る
    let twice = dir.0.join("twice");
    let error = points(path(&twice), &[path(&isg), path(&isg)]).unwrap_err();
    assert!(error.contains("重なります"), "{error}");
}

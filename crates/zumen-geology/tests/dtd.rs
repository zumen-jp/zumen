//! ボーリング交換用データの読みを、要領が配る DTD（BED0500.DTD）と lxml 6.1.3 で照らす（#505）。
//!
//! 国土交通省の作成例と DTD は転載と改変が禁じられているので、同梱せず、テストも取得しない。利用者が
//! 電子納品の頁から入手して展開したフォルダーを `ZUMEN_CALS_DTD` で渡したときだけ動く。フォルダーの下の
//! `BED*.XML` を、別のプロセスで動く `xtask/verify/dtd/check.py` に渡して隣の DTD で検証させ、lxml が読んだ
//! 要素の値とコアの読みを照らす。手順は `xtask/verify/dtd/README.md`。

#![cfg(test)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_geology::{Boring, Datum, read};

type Oracle = (PathBuf, PathBuf);

/// lxml が読んだ要素の深さ・名前・値（空の値は `None`）
type Row = (usize, String, Option<String>);

/// 作成例のフォルダーと lxml。フォルダーがなければ飛ばす。フォルダーを渡したときは lxml がないと失敗にする
// コアの決定性とは別に、テストのツールの起動条件を読む。
#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn setup() -> Option<(PathBuf, Oracle)> {
    let Some(folder) = std::env::var_os("ZUMEN_CALS_DTD") else {
        assert!(
            std::env::var_os("ZUMEN_DTD_REQUIRED").is_none(),
            "ZUMEN_CALS_DTD に作成例のフォルダーが要る"
        );
        eprintln!("ZUMEN_CALS_DTD がないため DTD の照合を飛ばす（xtask/verify/dtd/README.md）");
        return None;
    };
    let python = std::env::var_os("ZUMEN_DTD_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| "python".into());
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/dtd/check.py");
    let output = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .output()
        .unwrap();
    assert!(output.status.success(), "lxml の起動に失敗: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "lxml\t6.1.3"
    );
    Some((PathBuf::from(folder), (python, script)))
}

/// フォルダーの下の `BED*.XML`。道筋の順
fn find(folder: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            find(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("BED") && name.ends_with(".XML"))
        {
            out.push(path);
        }
    }
}

/// lxml に DTD で検証させて読ませる。検証に失敗すれば落とす
fn external(oracle: &Oracle, name: &str, folder: &Path, bytes: &[u8]) -> Vec<Row> {
    let mut child = Command::new(&oracle.0)
        .arg("-I")
        .arg("-X")
        .arg("utf8")
        .arg(&oracle.1)
        .arg(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "lxml が動かなかった: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert_eq!(first, "valid", "{name}");
    let mut rows = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["doctype", root, system] => {
                assert_eq!(*root, "ボーリング情報", "{name}");
                assert!(system.starts_with("BED"), "{name}: {system}");
            }
            ["element", depth, element, _, _, _, value] => rows.push((
                depth.parse().unwrap(),
                unescape(element),
                (!value.is_empty()).then(|| whatwg(&unescape(value))),
            )),
            _ => panic!("{name}: 読めない行: {line}"),
        }
    }
    rows
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// libxml2 の Shift_JIS は JIS X 0201・JIS X 0208 の対応で、zumen は WHATWG Encoding（Windows の符号）の対応で
/// 復号する（[XML の規則](../../../docs/formats/xml.md#shift_jis-の名前)）。対応の違う 8 字を WHATWG の側に
/// そろえて比べる。作成例の観察記事は、改行の印に 0x5C の `\n` を使う
fn whatwg(value: &str) -> String {
    const JIS_TO_WHATWG: &[(char, char)] = &[
        ('\u{00A5}', '\\'),
        ('\u{203E}', '~'),
        ('\u{301C}', '\u{FF5E}'),
        ('\u{2016}', '\u{2225}'),
        ('\u{2212}', '\u{FF0D}'),
        ('\u{00A2}', '\u{FFE0}'),
        ('\u{00A3}', '\u{FFE1}'),
        ('\u{00AC}', '\u{FFE2}'),
    ];
    value
        .chars()
        .map(|c| {
            JIS_TO_WHATWG
                .iter()
                .find(|(jis, _)| *jis == c)
                .map_or(c, |(_, whatwg)| *whatwg)
        })
        .collect()
}

/// 名前が `name` の要素ごとの、子の要素の名前と値
fn groups<'a>(rows: &'a [Row], name: &str) -> Vec<Vec<(&'a str, Option<&'a str>)>> {
    let mut out = Vec::new();
    for (index, (depth, element, _)) in rows.iter().enumerate() {
        if element != name {
            continue;
        }
        let children = rows[index + 1..]
            .iter()
            .take_while(|(d, _, _)| d > depth)
            .filter(|(d, _, _)| *d == depth + 1)
            .map(|(_, child, value)| (child.as_str(), value.as_deref()))
            .collect();
        out.push(children);
    }
    out
}

/// 1 つだけある要素の子の値
fn one<'a>(rows: &'a [Row], group: &str, child: &str) -> Option<&'a str> {
    let groups = groups(rows, group);
    assert_eq!(groups.len(), 1, "{group}");
    get(&groups[0], child)
}

fn get<'a>(group: &[(&'a str, Option<&'a str>)], child: &str) -> Option<&'a str> {
    group
        .iter()
        .find(|(name, _)| *name == child)
        .and_then(|(_, value)| *value)
}

fn number(value: Option<&str>) -> Option<f64> {
    value.map(|value| value.parse().unwrap())
}

fn owned(value: Option<&str>) -> Option<String> {
    value.map(str::to_string)
}

fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    (out, diag.iter().map(|d| d.code()).collect())
}

/// lxml の値と照らす。照らした地層・貫入試験などの記録の数を返す
fn compare(name: &str, boring: &Boring, rows: &[Row]) -> usize {
    let standards: Vec<String> = rows
        .iter()
        .filter(|(depth, element, _)| *depth == 2 && element == "適用規格")
        .filter_map(|(_, _, value)| value.clone())
        .collect();
    assert_eq!(boring.standards, standards, "{name}: 適用規格");
    assert_eq!(
        boring.publication,
        owned(one(rows, "公開フラグ", "公開フラグ_コード")),
        "{name}"
    );
    let survey = "調査基本情報";
    assert_eq!(
        boring.project,
        owned(one(rows, survey, "事業工事名")),
        "{name}"
    );
    assert_eq!(boring.survey, owned(one(rows, survey, "調査名")), "{name}");
    assert_eq!(
        boring.name,
        owned(one(rows, survey, "ボーリング名")),
        "{name}"
    );
    let integer = |child| one(rows, survey, child).map(|value| value.parse::<u32>().unwrap());
    assert_eq!(boring.count, integer("ボーリング総数"), "{name}");
    assert_eq!(boring.serial, integer("ボーリング連番"), "{name}");
    let place = "経度緯度情報";
    let angle = |prefix: &str| -> Option<f64> {
        let part = |unit: &str| number(one(rows, place, &format!("{prefix}_{unit}")));
        Some(part("度")? + part("分")? / 60.0 + part("秒")? / 3600.0)
    };
    let position = boring
        .position
        .map(|point| (point.latitude, point.longitude));
    assert_eq!(position, angle("緯度").zip(angle("経度")), "{name}: 位置");
    let datum = one(rows, place, "測地系").map(|code| match code {
        "00" => Datum::Tokyo,
        "01" => Datum::Jgd2000,
        "02" => Datum::Jgd2011,
        "03" => Datum::Jgd2024,
        other => panic!("{name}: 測地系 {other}"),
    });
    assert_eq!(boring.datum, datum, "{name}: 測地系");
    assert_eq!(
        boring.address,
        owned(one(rows, "調査位置", "調査位置住所")),
        "{name}"
    );
    let period = "調査期間";
    assert_eq!(
        boring.start,
        owned(one(rows, period, "調査期間_開始年月日")),
        "{name}"
    );
    assert_eq!(
        boring.end,
        owned(one(rows, period, "調査期間_終了年月日")),
        "{name}"
    );
    let basic = "ボーリング基本情報";
    assert_eq!(
        boring.elevation.map(|height| height.value),
        number(one(rows, basic, "孔口標高")),
        "{name}: 孔口標高"
    );
    assert_eq!(
        boring.length,
        number(one(rows, basic, "総削孔長")),
        "{name}"
    );
    assert_eq!(boring.angle, number(one(rows, basic, "角度")), "{name}");
    assert_eq!(boring.direction, number(one(rows, basic, "方位")), "{name}");

    const LAYER: &str = "工学的地質区分名現場土質名";
    let layers: Vec<(f64, String, Option<String>)> = groups(rows, LAYER)
        .iter()
        .filter_map(|group| {
            let bottom = number(get(group, &format!("{LAYER}_下端深度")))?;
            let text = owned(get(group, &format!("{LAYER}_{LAYER}")))?;
            Some((
                bottom,
                text,
                owned(get(group, &format!("{LAYER}_{LAYER}記号"))),
            ))
        })
        .collect();
    let core: Vec<(f64, String, Option<String>)> = boring
        .layers
        .iter()
        .map(|layer| (layer.bottom, layer.name.clone(), layer.symbol.clone()))
        .collect();
    assert_eq!(core, layers, "{name}: 地層");
    let colors: Vec<(f64, Option<String>)> = groups(rows, "色調")
        .iter()
        .filter_map(|group| {
            let bottom = number(get(group, "色調_下端深度"))?;
            Some((bottom, owned(get(group, "色調_色調名"))))
        })
        .collect();
    let core: Vec<(f64, Option<String>)> = boring
        .colors
        .iter()
        .map(|color| (color.bottom, color.name.clone()))
        .collect();
    assert_eq!(core, colors, "{name}: 色調");
    let observations: Vec<(f64, f64, Option<String>)> = groups(rows, "観察記事")
        .iter()
        .filter_map(|group| {
            let top = number(get(group, "観察記事_上端深度"))?;
            let bottom = number(get(group, "観察記事_下端深度"))?;
            Some((top, bottom, owned(get(group, "観察記事_記事"))))
        })
        .collect();
    let core: Vec<(f64, f64, Option<String>)> = boring
        .observations
        .iter()
        .map(|observation| {
            (
                observation.top,
                observation.bottom,
                observation.text.clone(),
            )
        })
        .collect();
    assert_eq!(core, observations, "{name}: 観察記事");
    let penetrations: Vec<(f64, u32, f64)> = groups(rows, "標準貫入試験")
        .iter()
        .filter_map(|group| {
            let depth = number(get(group, "標準貫入試験_開始深度"))?;
            let blows = get(group, "標準貫入試験_合計打撃回数")?.parse().unwrap();
            let penetration = number(get(group, "標準貫入試験_合計貫入量"))?;
            Some((depth, blows, penetration))
        })
        .collect();
    let core: Vec<(f64, u32, f64)> = boring
        .penetrations
        .iter()
        .map(|test| (test.depth, test.blows, test.penetration))
        .collect();
    assert_eq!(core, penetrations, "{name}: 標準貫入試験");
    type Level = (Option<String>, Option<String>, Option<f64>, Option<String>);
    let levels: Vec<Level> = groups(rows, "孔内水位")
        .iter()
        .map(|group| {
            (
                owned(get(group, "孔内水位_測定年月日")),
                owned(get(group, "孔内水位_削孔状況コード")),
                number(get(group, "孔内水位_孔内水位")),
                owned(get(group, "孔内水位_水位種別備考")),
            )
        })
        .collect();
    let core: Vec<Level> = boring
        .water_levels
        .iter()
        .map(|level| {
            (
                level.date.clone(),
                level.state.clone(),
                level.depth,
                level.note.clone(),
            )
        })
        .collect();
    assert_eq!(core, levels, "{name}: 孔内水位");
    layers.len() + colors.len() + observations.len() + penetrations.len() + levels.len()
}

#[test]
fn boring_exchange_data_reads_as_lxml_reads_it_after_dtd_validation() {
    let Some((folder, oracle)) = setup() else {
        return;
    };
    let mut files = Vec::new();
    find(&folder, &mut files);
    assert!(!files.is_empty(), "BED*.XML の作成例がない");
    for path in &files {
        let name = path.file_name().unwrap().to_str().unwrap();
        let bytes = std::fs::read(path).unwrap();
        let rows = external(&oracle, name, path.parent().unwrap(), &bytes);
        let (boring, codes) = with_cx(|cx| read(&bytes, cx));
        let boring = boring.unwrap();
        // 読まない要素（`boring.element.unsupported`）と標高の成果の仮定は知らせる。値の誤りはないこと
        assert!(
            !codes.contains(&codes::BORING_VALUE_INVALID),
            "{name}: {codes:?}"
        );
        let records = compare(name, &boring, &rows);
        assert!(records > 0, "{name}");
        assert!(
            boring.position.is_some() && boring.elevation.is_some(),
            "{name}"
        );
    }
}

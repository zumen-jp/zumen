//! fuzz の target を検査し、種の入力を書く（#501・#517、決定 0010・0017・0262）。
//!
//! 検査は `codegen` から呼ぶ。登録した形式と L0 の crate のどれにも、同じ名前の fuzz の target があることを確かめる。
//! 登録した形式は、crate が `[package.metadata.zumen]` の `formats` で名乗る形式と、`schema/boundary.toml` の
//! `[[detection]]` と `[[modules]]` の形式。target は `fuzz/Cargo.toml` の `[[bin]]` で、`fuzz/fuzz_targets/<名前>.rs`
//! に置き、テストキットの fuzz の関数（`survives`・`survives_lift`・`survives_with`・`survives_calls`）を呼ぶ。API を
//! 通す target も、例外にせず、`survives_calls` を呼ぶことで同じ検査を通す（決定 0262）。
//!
//! `cargo xtask fuzz-seeds` は、公開サンプル、適合テストの組の合成の入力と要求、L0 の攻撃の形（ZIP 爆弾・膨らむ
//! deflate・アーカイブの入れ子・パスの走査・XML の外部実体と深い入れ子・ISO 10303-21 の深い入れ子）、EXPRESS の攻撃の
//! 形（集まりの型の深い入れ子・定義された型と上位の型の長い鎖）、合成の格子（ISG・TKY2JGD・PatchJGD の形）、線と列の
//! 参照を持つ合成シーン（ZMSC）と通り芯（IFC）を、どの target にも種として書く（決定 0258・0262）。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use zumen_base::crc::crc32;
use zumen_json::{Value, Writer};

use crate::boundary::Boundary;
use crate::sha256;

mod scene;

/// fuzz の crate のフォルダー
const DIR: &str = "fuzz";
/// 形式ではないが、受け取ったバイト列を読む L0 の crate（決定 0008）。target の名前にする
const L0: &[&str] = &["zip", "xml", "step"];
/// target が呼ぶ、テストキットの fuzz の関数の始まり（`survives`・`survives_lift`・`survives_with`・
/// `survives_calls`）
const TESTKIT: &str = "zumen_sdk::testkit::survives";
/// 深い入れ子の種の段の数。fuzz の予算の深さ（既定の 256）を超える
const DEEP: usize = 1000;
/// EXPRESS の長い鎖の種の、定義された型と実体のそれぞれの数
const CHAIN: usize = 1000;
/// 空の deflate のデータ。固定のハフマン符号の最後のブロックで、ブロックの終わりの符号（256）だけを持つ（RFC 1951 の
/// 3.2.3・3.2.6）
const EMPTY_DEFLATE: &[u8] = &[0x03, 0x00];
/// 膨らむ deflate の種の、長さ 258・距離 1 の参照の数。約 27 KB が約 4 MB の 0 に展開する
const REPEATS: usize = 16_384;

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    bin: Vec<Bin>,
}

/// fuzz の target（`[[bin]]`）
#[derive(Debug, Deserialize)]
struct Bin {
    name: String,
    path: String,
}

pub fn check(root: &Path, boundary: &Boundary) -> Result<(), String> {
    let mut required = declared(root)?;
    for detection in &boundary.detection {
        required.insert(detection.format.clone());
    }
    for module in &boundary.modules {
        required.extend(module.formats.iter().cloned());
    }
    required.extend(L0.iter().map(|name| (*name).to_string()));
    let errors = check_targets(&required, &bins(root)?, &files(root)?);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "fuzz の target の検査で問題がある（fuzz/README.md）:\n- {}",
            errors.join("\n- ")
        ))
    }
}

/// ワークスペースの crate が `[package.metadata.zumen]` の `formats` で名乗る形式
fn declared(root: &Path) -> Result<BTreeSet<String>, String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo metadata を起動できない: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata が失敗した:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata = zumen_json::parse(&output.stdout)
        .map_err(|e| format!("cargo metadata の JSON を読めない（位置 {}）", e.offset))?;
    let Some(Value::Array(packages)) = metadata.pointer("/packages") else {
        return Err("cargo metadata: packages の配列がない".to_string());
    };
    Ok(packages
        .iter()
        .filter_map(|package| match package.pointer("/metadata/zumen/formats") {
            Some(Value::Array(formats)) => Some(formats),
            _ => None,
        })
        .flatten()
        .filter_map(|format| match format {
            Value::String(format) => Some(format.clone()),
            _ => None,
        })
        .collect())
}

fn bins(root: &Path) -> Result<Vec<Bin>, String> {
    let path = format!("{DIR}/Cargo.toml");
    let text =
        fs::read_to_string(root.join(&path)).map_err(|e| format!("{path} を読めない: {e}"))?;
    let manifest: Manifest = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(manifest.bin)
}

/// `fuzz/fuzz_targets` のファイルの名前と中身
fn files(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let dir = format!("{DIR}/fuzz_targets");
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root.join(&dir)).map_err(|e| format!("{dir} を読めない: {e}"))? {
        let entry = entry.map_err(|e| format!("{dir} を読めない: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = fs::read_to_string(entry.path())
            .map_err(|e| format!("{dir}/{name} を読めない: {e}"))?;
        files.insert(name, text);
    }
    Ok(files)
}

/// 登録した形式と L0 の crate に target があり、target とファイルが一対一で、どの target もテストキットの fuzz の
/// 関数を呼ぶことを確かめる
fn check_targets(
    required: &BTreeSet<String>,
    bins: &[Bin],
    files: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    for bin in bins {
        let name = bin.name.as_str();
        if !names.insert(name) {
            errors.push(format!("target `{name}` が重なっている"));
        }
        let file = format!("{name}.rs");
        if bin.path != format!("fuzz_targets/{file}") {
            errors.push(format!(
                "target `{name}` の path は fuzz_targets/{file} にする"
            ));
        }
        match files.get(&file) {
            None => errors.push(format!(
                "target `{name}` のファイル fuzz/fuzz_targets/{file} がない"
            )),
            Some(text) if !text.contains(TESTKIT) => errors.push(format!(
                "target `{name}` が、テストキットの fuzz の関数（{TESTKIT}…）を呼ばない"
            )),
            Some(_) => {}
        }
    }
    for file in files.keys() {
        if !file
            .strip_suffix(".rs")
            .is_some_and(|name| names.contains(name))
        {
            errors.push(format!(
                "fuzz/fuzz_targets/{file} が、fuzz/Cargo.toml の [[bin]] にない"
            ));
        }
    }
    for name in required {
        if !names.contains(name.as_str()) {
            errors.push(format!(
                "`{name}` に fuzz の target がない。fuzz/fuzz_targets/{name}.rs を作り、fuzz/Cargo.toml の [[bin]] に足す"
            ));
        }
    }
    errors
}

/// 公開サンプル、適合テストの組の合成の入力、L0 と EXPRESS の攻撃の形、合成の格子・シーン・通り芯を、どの target にも
/// `fuzz/corpus/<target>/` の種として書く。ファイルの名前は、中身の SHA-256 にする
pub fn seeds(root: &Path) -> Result<(), String> {
    let mut seeds = BTreeMap::new();
    for data in samples(root)?
        .into_iter()
        .chain(cases(root)?)
        .chain(attacks()?)
        .chain(schemas())
        .chain(grids())
        .chain(std::iter::once(ifc_grid()))
        .chain(std::iter::once(scene::seed()))
    {
        seeds.insert(sha256::hex(&data), data);
    }
    let bins = bins(root)?;
    for bin in &bins {
        let dir = format!("{DIR}/corpus/{}", bin.name);
        fs::create_dir_all(root.join(&dir)).map_err(|e| format!("{dir} を作れない: {e}"))?;
        for (name, data) in &seeds {
            fs::write(root.join(&dir).join(name), data)
                .map_err(|e| format!("{dir}/{name} を書けない: {e}"))?;
        }
    }
    println!(
        "{DIR}/corpus の {} の target に、{} の種を書いた",
        bins.len(),
        seeds.len()
    );
    Ok(())
}

/// 通り芯の必要時の取得まで、小さい入力で確実に通す自作の IFC4。
fn ifc_grid() -> Vec<u8> {
    b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('grid'),'2;1');\nFILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10));\n#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,$,$);\n#21=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #30=IFCLOCALPLACEMENT($,#20);\n\
    #100=IFCGRID('0000000000000000000002',$,$,$,$,#30,$,(#300),(#301),$,.RECTANGULAR.);\n\
    #200=IFCPOLYLINE((#210,#211));\n#201=IFCPOLYLINE((#210,#212));\n\
    #210=IFCCARTESIANPOINT((0.,0.));\n#211=IFCCARTESIANPOINT((10.,0.));\n#212=IFCCARTESIANPOINT((0.,10.));\n\
    #300=IFCGRIDAXIS('U',#200,.T.);\n#301=IFCGRIDAXIS('V',#201,.F.);\n\
    ENDSEC;\nEND-ISO-10303-21;\n".to_vec()
}

/// `samples/ledger.toml` に載せた公開サンプル
fn samples(root: &Path) -> Result<Vec<Vec<u8>>, String> {
    let path = "samples/ledger.toml";
    let text =
        fs::read_to_string(root.join(path)).map_err(|e| format!("{path} を読めない: {e}"))?;
    let ledger: toml::Value = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let entries = ledger
        .get("samples")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| entry.get("path").and_then(toml::Value::as_str))
        .map(|sample| {
            fs::read(root.join("samples").join(sample))
                .map_err(|e| format!("samples/{sample} を読めない: {e}"))
        })
        .collect()
}

/// 適合テストの組（`bindings/conformance/cases`）の、`push` で入れる合成の入力と、`call` で渡す要求。ファイルを指す
/// 入力は公開サンプルなので除く。要求は 1 つずつと、組ごとに並べた配列にする（`json` の target が、配列の要素を順に
/// 呼ぶ。決定 0262）。`call_text`・`call_hex` で渡す要求のバイト列（読めない要求と、BOM を付けた要求）は、1 つずつ
/// だけにする
fn cases(root: &Path) -> Result<Vec<Vec<u8>>, String> {
    let dir = "bindings/conformance/cases";
    let mut paths = Vec::new();
    for entry in fs::read_dir(root.join(dir)).map_err(|e| format!("{dir} を読めない: {e}"))? {
        let path = entry.map_err(|e| format!("{dir} を読めない: {e}"))?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut inputs = Vec::new();
    for path in paths {
        let text = fs::read(&path).map_err(|e| format!("{} を読めない: {e}", path.display()))?;
        let case = zumen_json::parse(&text)
            .map_err(|e| format!("{} を読めない（位置 {}）", path.display(), e.offset))?;
        let Some(Value::Array(steps)) = case.pointer("/steps") else {
            continue;
        };
        let mut sequence = Writer::new();
        sequence.begin_array();
        for step in steps {
            match (step.pointer("/push/text"), step.pointer("/push/hex")) {
                (Some(Value::String(text)), _) => inputs.push(text.as_bytes().to_vec()),
                (_, Some(Value::String(hex))) => inputs.push(
                    unhex(hex)
                        .ok_or_else(|| format!("{} の push の hex を読めない", path.display()))?,
                ),
                _ => {}
            }
            if let Some(call) = step.pointer("/call") {
                let mut request = Writer::new();
                request.value(call);
                inputs.push(request.into_bytes());
                sequence.value(call);
            }
            // `call_text`・`call_hex` で渡す要求（読めない要求と、BOM を付けた要求）は、そのバイト列を種にする
            match (step.pointer("/call_text"), step.pointer("/call_hex")) {
                (Some(Value::String(text)), _) => inputs.push(text.as_bytes().to_vec()),
                (_, Some(Value::String(hex))) => inputs.push(
                    unhex(hex)
                        .ok_or_else(|| format!("{} の call_hex を読めない", path.display()))?,
                ),
                _ => {}
            }
        }
        sequence.end_array();
        inputs.push(sequence.into_bytes());
    }
    Ok(inputs)
}

/// 16 進の文字列を読む。適合テストの組と同じく、空白は飛ばす
fn unhex(hex: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = hex
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_digit(16).and_then(|digit| u8::try_from(digit).ok()))
        .collect::<Option<_>>()?;
    let (pairs, rest) = digits.as_chunks::<2>();
    rest.is_empty()
        .then(|| pairs.iter().map(|[high, low]| high << 4 | low).collect())
}

/// L0 の攻撃の形。どれも、予算で止まるか、読まずに診断を返す
fn attacks() -> Result<Vec<Vec<u8>>, String> {
    let zip = |entries: &[(&str, &[u8])]| {
        zumen_zip::write(entries).ok_or_else(|| "種の ZIP を書けない".to_string())
    };
    // アーカイブの外を指す名前（パスの走査）
    let traversal = zip(&[
        ("../up.txt", b"x".as_slice()),
        ("/root.txt", b"x".as_slice()),
        ("C:/drive.txt", b"x".as_slice()),
        ("a\\..\\..\\b.txt", b"x".as_slice()),
        ("ok.txt", b"x".as_slice()),
    ])?;
    // アーカイブの中のアーカイブの中のアーカイブ
    let inner = zip(&[("leaf.txt", b"leaf".as_slice())])?;
    let middle = zip(&[("inner.zip", inner.as_slice())])?;
    let nested = zip(&[("middle.zip", middle.as_slice())])?;
    // ZIP 爆弾。展開すると 4 GiB 近くになると記録した deflate の項目。記録で予算を超えるので、展開しない
    let mut bomb = zip(&[("bomb.txt", EMPTY_DEFLATE)])?;
    claim(&mut bomb, 0xFFFF_FFF0, None).ok_or_else(|| "種の ZIP 爆弾を作れない".to_string())?;
    // 実際に膨らむ deflate の項目。記録した大きさと CRC-32 は正しく、展開して確かめる。fuzz で記録の大きさが
    // 変われば、記録より多く膨らむデータを記録の所で止めることになる
    let swelling = swelling()?;

    let mut laughs = String::from(r#"<?xml version="1.0"?><!DOCTYPE a [<!ENTITY l0 "lol">"#);
    for level in 1..10 {
        let reference = format!("&l{};", level - 1);
        let _ = write!(laughs, r#"<!ENTITY l{level} "{}">"#, reference.repeat(10));
    }
    laughs.push_str("]><a>&l9;</a>");
    Ok(vec![
        traversal,
        nested,
        bomb,
        swelling,
        // 外部の実体と外部の DTD
        br#"<?xml version="1.0"?><!DOCTYPE a [<!ENTITY x SYSTEM "file:///etc/passwd">]><a>&x;</a>"#
            .to_vec(),
        br#"<?xml version="1.0"?><!DOCTYPE a SYSTEM "http://example.invalid/a.dtd"><a/>"#.to_vec(),
        // 実体を重ねて膨らむ文書
        laughs.into_bytes(),
        // 深い入れ子
        format!("{}{}", "<a>".repeat(DEEP), "</a>".repeat(DEEP)).into_bytes(),
        format!(
            "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=A({}{});\nENDSEC;\nEND-ISO-10303-21;\n",
            "(".repeat(DEEP),
            ")".repeat(DEEP)
        )
        .into_bytes(),
    ])
}

/// 1 つの項目に、0 に展開する deflate のデータ（[`zeros_deflated`]）を入れたアーカイブ。大きさと CRC-32 は、展開した
/// データのものを記録する
fn swelling() -> Result<Vec<u8>, String> {
    let fail = || "種の膨らむ deflate のアーカイブを作れない".to_string();
    let zeros = vec![0; REPEATS * 258 + 1];
    let mut archive =
        zumen_zip::write(&[("zeros.bin", &zeros_deflated(REPEATS))]).ok_or_else(fail)?;
    let size = u32::try_from(zeros.len()).map_err(|_| fail())?;
    claim(&mut archive, size, Some(crc32(&zeros))).ok_or_else(fail)?;
    Ok(archive)
}

/// 0 のバイトに展開する deflate のデータ。固定のハフマン符号の最後のブロックに、字の 0 を 1 つと、長さ 258・距離 1 の
/// 参照を `repeats` 個置く（RFC 1951 の 3.2.3・3.2.5・3.2.6）。展開すると `repeats * 258 + 1` バイトになる。
/// ビットは下の桁から詰め、ハフマン符号だけは上の桁から詰める（3.1.1）
fn zeros_deflated(repeats: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut pending, mut count) = (0u32, 0u32);
    let mut put = |value: u32, len: u32| {
        pending |= value << count;
        count += len;
        while count >= 8 {
            out.push(pending.to_le_bytes()[0]);
            pending >>= 8;
            count -= 8;
        }
    };
    let code = |value: u32, len: u32| value.reverse_bits() >> (32 - len);
    // 最後のブロック（BFINAL が 1）で、固定のハフマン符号（BTYPE が 01）
    put(1, 1);
    put(1, 2);
    // 字の 0（8 ビットの 00110000）
    put(code(0b0011_0000, 8), 8);
    for _ in 0..repeats {
        // 長さ 258（符号 285、8 ビットの 11000101。追加のビットはない）と、距離 1（符号 0、5 ビット）
        put(code(0b1100_0101, 8), 8);
        put(0, 5);
    }
    // ブロックの終わり（符号 256、7 ビットの 0）。残りのビットは 0 で埋める
    put(0, 7);
    if count > 0 {
        out.push(pending.to_le_bytes()[0]);
    }
    out
}

/// アーカイブの最初の項目を、deflate で展開すると `size` バイトになると記録したものにする。`crc` があれば、CRC-32 も
/// 書き換える。ヘッダーのフィールドの位置は、APPNOTE 6.3.10 の 4.3.7（ローカルヘッダー）と 4.3.12（中央ディレクトリの
/// ヘッダー）
fn claim(archive: &mut [u8], size: u32, crc: Option<u32>) -> Option<()> {
    let (local, central) = headers(archive)?;
    let mut fields = vec![
        (local + 8, 8u16.to_le_bytes().to_vec()),
        (local + 22, size.to_le_bytes().to_vec()),
        (central + 10, 8u16.to_le_bytes().to_vec()),
        (central + 24, size.to_le_bytes().to_vec()),
    ];
    if let Some(crc) = crc {
        fields.push((local + 14, crc.to_le_bytes().to_vec()));
        fields.push((central + 16, crc.to_le_bytes().to_vec()));
    }
    for (at, bytes) in fields {
        archive
            .get_mut(at..at + bytes.len())?
            .copy_from_slice(&bytes);
    }
    Some(())
}

/// アーカイブの最初の項目の、ローカルヘッダーと中央ディレクトリのヘッダーの位置。中央ディレクトリは、注釈のない終わりの
/// 記録から引く（APPNOTE 6.3.10 の 4.3.16）。項目のデータにヘッダーと同じバイトがあっても、取り違えない
fn headers(archive: &[u8]) -> Option<(usize, usize)> {
    let end = archive.len().checked_sub(22)?;
    let central = archive
        .get(end + 16..end + 20)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .and_then(|offset| usize::try_from(offset).ok())?;
    (archive.get(..4)? == b"PK\x03\x04"
        && archive.get(end..end + 4)? == b"PK\x05\x06"
        && archive.get(central..central + 4)? == b"PK\x01\x02")
        .then_some((0, central))
}

/// EXPRESS の攻撃の形（#668）。集まりの型の深い入れ子は、予算の深さで止まる。定義された型と上位の型の長い鎖は、
/// 型と実体ごとに 1 度だけたどって読む
fn schemas() -> Vec<Vec<u8>> {
    let nested = format!(
        "SCHEMA IFCSEED;
TYPE T = {}INTEGER;
END_TYPE;
END_SCHEMA;
",
        "LIST [1:?] OF ".repeat(DEEP)
    );
    let mut chains = String::from(
        "SCHEMA IFCSEED;
",
    );
    for i in 0..CHAIN {
        let next = i + 1;
        let _ = writeln!(
            chains,
            "TYPE T{i} = T{next};
END_TYPE;
ENTITY E{i} SUBTYPE OF (E{next});
END_ENTITY;"
        );
    }
    let _ = writeln!(
        chains,
        "TYPE T{CHAIN} = INTEGER;
END_TYPE;
ENTITY E{CHAIN};
END_ENTITY;
END_SCHEMA;"
    );
    vec![nested.into_bytes(), chains.into_bytes()]
}

/// 合成の格子。国土地理院のジオイド・モデルと座標の補正量は使わない（決定 0019）。形は `crates/zumen-crs/tests`
/// の合成の格子に合わせる
fn grids() -> Vec<Vec<u8>> {
    // ISG format 2.0 の、4 行 6 列のジオイド高の格子。南の 2 行の東の 2 つは無効
    let isg = "These data are synthetic.\n\
begin_of_head ================================================\n\
model name     : SYNTHETIC\n\
model year     : 2026\n\
model type     : gravimetric\n\
data type      : geoid\n\
data units     : meters\n\
data format    : grid\n\
data ordering  : N-to-S, W-to-E\n\
ref ellipsoid  : GRS80\n\
coord type     : geodetic\n\
coord units    : deg\n\
map projection : ---\n\
lat min        = 40.000000\n\
lat max        = 41.000000\n\
lon min        = 120.000000\n\
lon max        = 121.666667\n\
delta lat      = 0.333333\n\
delta lon      = 0.333333\n\
nrows          =            4\n\
ncols          =            6\n\
nodata         = -9999.0000\n\
ISG format     =          2.0\n\
end_of_head ==================================================\n\
   30.0000    31.0000    32.0000    33.0000    34.0000    35.0000\n\
   40.0000    41.0000    42.0000    43.0000    44.0000    45.0000\n\
   50.0000    51.0000    52.0000    53.0000 -9999.0000 -9999.0000\n\
   60.0000    61.0000    62.0000    63.0000 -9999.0000 -9999.0000\n";
    // TKY2JGD・PatchJGD の形の補正量の格子。3 次メッシュのコードと、緯度・経度の補正量（秒）。PatchJGD の形は、
    // 高さの補正量の列も持つ。最後の格子は、格子点を 3 次メッシュの 5 倍の間隔に置く（決定 0286）
    let mut tky2jgd = String::from("synthetic\nMeshCode   dB(sec)   dL(sec)\n");
    let mut patchjgd = String::from("synthetic\nMeshCode   dB(sec)   dL(sec)   dH(m)\n");
    let mut five = String::from("synthetic\nMeshCode   dB(sec)   dL(sec)\n");
    let code = |i: u32, j: u32| {
        format!(
            "{:02}{:02}{}{}{}{}",
            i / 80,
            j / 80,
            (i % 80) / 10,
            (j % 80) / 10,
            i % 10,
            j % 10
        )
    };
    for i in 4280..4283_u32 {
        for j in 3180..3183_u32 {
            let near = code(i, j);
            let (b, l) = (f64::from(i - 4280) * 0.01, f64::from(j - 3180) * 0.01);
            let _ = writeln!(tky2jgd, "{near} {:.6} {:.6}", 10.0 + b, -10.0 + l);
            let _ = writeln!(patchjgd, "{near} {b:.6} {l:.6} 0.000000");
            let spread = code(4280 + (i - 4280) * 5, 3180 + (j - 3180) * 5);
            let _ = writeln!(five, "{spread} {b:.6} {l:.6}");
        }
    }
    vec![
        isg.as_bytes().to_vec(),
        tky2jgd.into_bytes(),
        patchjgd.into_bytes(),
        five.into_bytes(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};

    fn bin(name: &str) -> Bin {
        Bin {
            name: name.to_string(),
            path: format!("fuzz_targets/{name}.rs"),
        }
    }

    fn target(body: &str) -> String {
        format!("fuzz_target!(|data: &[u8]| {{ {body} }});")
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn every_registered_format_has_a_target_that_uses_the_testkit() {
        let files = BTreeMap::from([
            (
                "dxf.rs".to_string(),
                target("zumen_sdk::testkit::survives_lift::<Dxf, Drawing2D>(data)"),
            ),
            (
                "zip.rs".to_string(),
                target("zumen_sdk::testkit::survives_with(data, read)"),
            ),
        ]);
        let bins = [bin("dxf"), bin("zip")];
        assert_eq!(
            check_targets(&set(&["dxf", "zip"]), &bins, &files),
            Vec::<String>::new()
        );
        // 登録した形式に target がない
        let errors = check_targets(&set(&["dxf", "las", "zip"]), &bins, &files);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].starts_with("`las` に fuzz の target がない"));
    }

    #[test]
    fn targets_and_files_match_one_to_one() {
        let files = BTreeMap::from([
            ("dxf.rs".to_string(), target("read(data);")),
            (
                "orphan.rs".to_string(),
                target("zumen_sdk::testkit::survives"),
            ),
        ]);
        let mut moved = bin("las");
        moved.path = "targets/las.rs".to_string();
        let errors = check_targets(&set(&[]), &[bin("dxf"), bin("dxf"), moved], &files);
        assert_eq!(
            errors,
            [
                "target `dxf` が、テストキットの fuzz の関数（zumen_sdk::testkit::survives…）を呼ばない",
                "target `dxf` が重なっている",
                "target `dxf` が、テストキットの fuzz の関数（zumen_sdk::testkit::survives…）を呼ばない",
                "target `las` の path は fuzz_targets/las.rs にする",
                "target `las` のファイル fuzz/fuzz_targets/las.rs がない",
                "fuzz/fuzz_targets/orphan.rs が、fuzz/Cargo.toml の [[bin]] にない",
            ]
        );
    }

    #[test]
    fn the_repository_has_a_target_for_every_registered_format() {
        let root = crate::root();
        let boundary: Boundary =
            toml::from_str(&fs::read_to_string(root.join("schema/boundary.toml")).unwrap())
                .unwrap();
        check(&root, &boundary).unwrap();
    }

    #[test]
    fn the_bomb_claims_its_size_in_both_headers() {
        let mut bomb = zumen_zip::write(&[("bomb.txt", EMPTY_DEFLATE)]).unwrap();
        claim(&mut bomb, 0x0102_0304, Some(0x0a0b_0c0d)).unwrap();
        let (local, central) = headers(&bomb).unwrap();
        assert_eq!(bomb[local + 8..local + 10], [8, 0]);
        assert_eq!(bomb[local + 14..local + 18], [0x0d, 0x0c, 0x0b, 0x0a]);
        assert_eq!(bomb[local + 22..local + 26], [4, 3, 2, 1]);
        assert_eq!(bomb[central + 10..central + 12], [8, 0]);
        assert_eq!(bomb[central + 16..central + 20], [0x0d, 0x0c, 0x0b, 0x0a]);
        assert_eq!(bomb[central + 24..central + 28], [4, 3, 2, 1]);
        assert_eq!(unhex("0aFf"), Some(vec![0x0a, 0xff]));
        assert_eq!(unhex("00 ff\n1"), None);
        assert_eq!(unhex("00 ff\n10"), Some(vec![0, 0xff, 0x10]));
        assert_eq!(unhex("0g"), None);
    }

    /// `f` を、上限 `limits` の文脈で呼ぶ。結果が `Ok` かと、出た診断の code を返す
    fn run<T, E>(
        limits: Limits,
        f: impl FnOnce(&mut Cx) -> Result<T, E>,
    ) -> (Option<T>, Vec<Code>) {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)).ok();
        (result, diagnostics.iter().map(|d| d.code()).collect())
    }

    fn extract(archive: &[u8], limits: Limits) -> (Option<Vec<u8>>, Vec<Code>) {
        run(limits, |cx| {
            zumen_zip::read(archive, cx)
                .and_then(|archive| archive.extract(&archive.entries()[0], cx))
        })
    }

    /// 膨らむ deflate のアーカイブは、記録した大きさと CRC-32 のとおりに 0 へ展開する。記録を小さくすれば記録の所で、
    /// 記録が予算を超えれば展開する前に止まる
    #[test]
    fn the_swelling_archive_inflates_to_what_it_records() {
        let archive = swelling().unwrap();
        assert!(archive.len() < 30_000, "{}", archive.len());
        let (data, found) = extract(&archive, Limits::default());
        let data = data.unwrap();
        assert_eq!(data.len(), REPEATS * 258 + 1);
        assert!(data.iter().all(|byte| *byte == 0));
        assert_eq!(found, []);

        let mut smaller = archive.clone();
        claim(&mut smaller, 1 << 20, None).unwrap();
        let (data, found) = extract(&smaller, Limits::default());
        assert_eq!(data, None);
        assert_eq!(found, [codes::ZIP_DATA_INVALID]);

        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(1 << 20));
        let (data, found) = extract(&archive, limits);
        assert_eq!(data, None);
        assert_eq!(found, [codes::BUDGET_DECODED_BYTES_EXCEEDED]);
    }

    /// EXPRESS の深い入れ子の種は、fuzz の予算の深さで止まり、長い鎖の種は、fuzz の予算の中で読める（#668）
    #[test]
    fn the_express_seeds_stop_at_the_depth_and_read_the_chains() {
        let schemas = schemas();
        let limits = zumen_sdk::testkit::fuzz_limits();
        let (nested, found) = run(limits, |cx| zumen_ifc::Express::read(&schemas[0], cx));
        assert!(nested.is_none());
        assert_eq!(found, [codes::BUDGET_DEPTH_EXCEEDED]);
        let (chains, found) = run(limits, |cx| zumen_ifc::Express::read(&schemas[1], cx));
        assert!(chains.is_some());
        assert_eq!(found, []);
    }

    /// 合成の格子は、ジオイド高と補正量の格子の読み取りで、診断なしに読める
    #[test]
    fn the_grids_are_read_without_diagnostics() {
        let grids = grids();
        let (grid, found) = run(Limits::default(), |cx| zumen_crs::Grid::read(&grids[0], cx));
        assert!(grid.is_some());
        assert_eq!(found, []);
        for shifts in &grids[1..] {
            let (grid, found) = run(Limits::default(), |cx| {
                zumen_crs::ShiftGrid::read(shifts, cx)
            });
            assert!(grid.is_some());
            assert_eq!(found, []);
        }
    }
}

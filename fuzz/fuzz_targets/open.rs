//! API の `open` の fuzz の target（#517、決定 0262）。`zumen` の `Session` に入力を渡し、`open` で形式を判定して
//! 開き、開いた文書を図面・建物・土木・地理・点群の op で読む。図面はシーンにし、図面モデルのバイナリと DXF に
//! 書いて読み直す（書き直した文書は、同じ図面なので、シーンにしない）。土木の線形からは図（`civil_drawing`）を作り、
//! シーンにする。LAS・LAZ は範囲でも読む（決定 0232）。開けない入力は、`format` で名指せる XYZ・拡張 DM と、SXF の図面の
//! 属性ファイル（SAF、決定 0249）と、文書にしない op（写真・ボーリング交換用データ・成果品）に渡す。
//!
//! どの要求の予算も、テストキットの API の呼び出しの fuzz の上限（`fuzz_call_limits`）にする。2 回呼んで、応答の
//! バイト列が同じことを確かめる。

#![no_main]

use std::collections::BTreeSet;

use libfuzzer_sys::fuzz_target;
use zumen_fuzz::{Calls, Reply, SFC, count, field, list, text};

/// 文書ごとに、シーンにする空間と用紙の数の上限
const PLACES: usize = 4;
/// 線形の図で作る用紙の数の上限（平面図・縦断図・横断図）
const SHEETS: usize = 3;
/// LAS を範囲で読むとき、初めに渡すバイト数。LAS 1.4 R15 の表 3 のヘッダー（Public Header Block）の大きさ
const LAS_HEAD: usize = 375;
/// LAS を範囲で読むとき、足りない範囲を足して開き直す回数の上限
const LAS_ROUNDS: usize = 4;
/// `entities` で返すフィールド（`schema/messages.json` の `entity_field`）
const ENTITY_FIELDS: &[&str] = &["id", "kind", "layer", "source", "bbox", "hash", "geometry"];
/// `building_elements` で返すコンポーネント（`schema/messages.json` の `building_component`）
const BUILDING_COMPONENTS: &[&str] = &[
    "identity",
    "class",
    "classification",
    "spatial",
    "parent",
    "placement",
    "type_of",
    "properties",
    "quantities",
    "material",
    "opening",
    "group",
    "connection",
    "level",
    "zone",
    "style",
    "relations",
];

/// 文書をどこまで読むか
#[derive(Clone, Copy, PartialEq, Eq)]
enum Depth {
    /// 受け取った文書。図面はシーンにし、文書から作った文書（図面モデルのバイナリ・DXF・線形の図）も読む
    Source,
    /// 線形から作った図。シーンにする
    Drawn,
    /// 受け取った文書を書き直した文書（図面モデルのバイナリ・DXF）。シーンにしない
    Copy,
}

/// 開いた文書を、領域ごとの op で読む
fn read(calls: &mut Calls, doc: &str, depth: Depth) {
    let drawing = calls.call("drawing", |out| field(out, "doc", doc));
    if drawing.ok {
        read_drawing(calls, doc, &drawing, depth);
    }
    let building = calls.call("building", |out| field(out, "doc", doc));
    if building.ok {
        calls.call("building_elements", |out| {
            field(out, "doc", doc);
            list(out, "components", BUILDING_COMPONENTS);
        });
    }
    let civil = calls.call("civil", |out| field(out, "doc", doc));
    if civil.ok {
        read_civil(calls, doc, depth);
    }
    calls.call("geo", |out| field(out, "doc", doc));
    let cloud = calls.call("pointcloud", |out| field(out, "doc", doc));
    if let Some(bounds) = cloud.result("/bounds") {
        calls.call("pointcloud", |out| {
            field(out, "doc", doc);
            out.key("region");
            out.value(bounds);
            out.key("tolerance");
            out.float(1.0);
        });
    }
}

/// 図面の文書を読み、空間と用紙をシーンにする
fn read_drawing(calls: &mut Calls, doc: &str, summary: &Reply, depth: Depth) {
    calls.call("entities", |out| {
        field(out, "doc", doc);
        list(out, "fields", ENTITY_FIELDS);
    });
    calls.call("tables", |out| field(out, "doc", doc));
    if depth == Depth::Copy {
        return;
    }
    for (index, space) in summary.items("/spaces").iter().take(PLACES).enumerate() {
        let from = text(space, "/id").map_or_else(|| format!("space/{index}"), str::to_string);
        calls.call("scene", |out| {
            field(out, "doc", doc);
            field(out, "from", &from);
        });
    }
    for (index, sheet) in summary.items("/sheets").iter().take(PLACES).enumerate() {
        let from = text(sheet, "/id").map_or_else(|| format!("sheet/{index}"), str::to_string);
        calls.call("scene", |out| {
            field(out, "doc", doc);
            field(out, "from", &from);
            out.key("print");
            out.bool(true);
        });
        if count(sheet, "/views").is_some_and(|views| views > 0) {
            calls.call("scene", |out| {
                field(out, "doc", doc);
                field(out, "from", &from);
                out.key("view");
                out.uint(0);
            });
        }
    }
    if depth != Depth::Source {
        return;
    }
    // 図面モデルのバイナリ（決定 0247）にして、開き直す
    let lifted = calls.call("lift", |out| field(out, "doc", doc));
    if let Some(binary) = lifted.blob("drawing") {
        let name = format!("{doc}.lifted");
        calls.push(&name, binary);
        if calls.call("open", |out| field(out, "input", &name)).ok {
            read(calls, &name, Depth::Copy);
        }
    }
    // DXF に書く（決定 0139）
    let to = format!("{doc}.dxf");
    // 見積もりは同じ出力名を確保せず、後の実変換を妨げない（#645）。明細は小さく制限する。
    calls.call("convert_estimate", |out| {
        field(out, "doc", doc);
        field(out, "format", "dxf");
        field(out, "to", &to);
        out.key("limit");
        out.uint(4);
    });
    if calls
        .call("convert", |out| {
            field(out, "doc", doc);
            field(out, "format", "dxf");
            field(out, "to", &to);
        })
        .ok
    {
        read(calls, &to, Depth::Copy);
    }
}

/// 土木の文書の線形の上の所を求め、線形から図を作って読む（決定 0239）
fn read_civil(calls: &mut Calls, doc: &str, depth: Depth) {
    calls.call("civil", |out| {
        field(out, "doc", doc);
        out.key("interval");
        out.float(10.0);
    });
    if depth != Depth::Source {
        return;
    }
    let to = format!("{doc}.plan.dxf");
    let drawn = calls.call("civil_drawing", |out| {
        field(out, "doc", doc);
        field(out, "to", &to);
    });
    if !drawn.ok {
        return;
    }
    read(calls, &to, Depth::Drawn);
    let sheets: Vec<String> = drawn
        .items("/sheets")
        .iter()
        .filter_map(|sheet| match sheet {
            zumen::json::Value::String(name) => Some(name.clone()),
            _ => None,
        })
        .take(SHEETS)
        .collect();
    for (index, sheet) in sheets.iter().enumerate().skip(1) {
        let to = format!("{doc}.sheet-{index}.dxf");
        if calls
            .call("civil_drawing", |out| {
                field(out, "doc", doc);
                field(out, "sheet", sheet);
                field(out, "to", &to);
            })
            .ok
        {
            read(calls, &to, Depth::Drawn);
        }
    }
}

/// LAS を範囲で読む（決定 0232）。初めの部分だけを渡して開き、足りない範囲を足して開き直す
fn read_ranged(calls: &mut Calls, data: &[u8]) {
    let head = data.get(..LAS_HEAD).unwrap_or(data);
    calls.push("part", head);
    let mut pushed = BTreeSet::new();
    for _ in 0..LAS_ROUNDS {
        let opened = calls.call("open", |out| {
            field(out, "input", "part");
            out.key("size");
            out.uint(data.len() as u64);
        });
        let needs = opened.items("/needs");
        if !opened.ok || needs.is_empty() {
            if opened.ok {
                read(calls, "part", Depth::Copy);
            }
            return;
        }
        let mut added = false;
        for need in needs {
            let (Some(name), Some(start), Some(end)) = (
                text(need, "/input"),
                count(need, "/start"),
                count(need, "/end"),
            ) else {
                panic!(
                    "open の needs に、input と、±(2^53 − 1) の中の整数の start・end がない（決定 0012）"
                );
            };
            assert!(
                start <= end && end <= data.len() as u64,
                "open の needs の範囲 {start}〜{end} が、全体（{} バイト）の中にない",
                data.len()
            );
            if !pushed.insert(name.to_string()) {
                continue;
            }
            let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) else {
                continue;
            };
            calls.push(
                name,
                data.get(start..end.min(data.len())).unwrap_or_default(),
            );
            added = true;
        }
        if !added {
            return;
        }
    }
}

/// 開けない入力を、名指せる形式と、文書にしない op に渡す
fn read_other(calls: &mut Calls) {
    for format in ["xyz", "extdm"] {
        let doc = format!("input.{format}");
        let opened = calls.call("open", |out| {
            field(out, "input", "input");
            field(out, "doc", &doc);
            field(out, "format", format);
        });
        if opened.ok {
            read(calls, &doc, Depth::Source);
        }
    }
    // SXF の図面に、属性ファイル（SAF）として結ぶ（決定 0249）。結ぶのは図面モデルへの変換なので、`drawing` まで読む
    calls.push("drawing.sfc", SFC.as_bytes());
    let opened = calls.call("open", |out| {
        field(out, "input", "drawing.sfc");
        field(out, "doc", "drawing.sfc");
        field(out, "saf", "input");
    });
    if opened.ok {
        read(calls, "drawing.sfc", Depth::Copy);
    }
    calls.call("photo", |out| field(out, "input", "input"));
    calls.call("boring", |out| field(out, "input", "input"));
    calls.call("delivery_check", |out| field(out, "archive", "input"));
}

fuzz_target!(|data: &[u8]| {
    let survived = zumen_sdk::testkit::survives_calls(|limits| {
        let mut calls = Calls::new(limits);
        calls.push("input", data);
        if calls.call("open", |out| field(out, "input", "input")).ok {
            read(&mut calls, "input", Depth::Source);
        } else {
            read_other(&mut calls);
        }
        if data.starts_with(b"LASF") {
            read_ranged(&mut calls, data);
        }
        calls.finish()
    });
    if let Err(message) = survived {
        panic!("{message}");
    }
});

//! 指定した IFC の種類と件数だけを数える。パス・名前・属性の値を出力しない。
//!
//! `ZUMEN_SAMPLES` の直下の IFC を引数で指定する。再配布しない入力の確認にも使う。

// コアのライブラリではなく、入力を指定して件数を出す検査用の実行ファイル。
#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]

use std::collections::BTreeMap;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Value, codes};
use zumen_building::Building;
use zumen_ifc::Ifc;
use zumen_sdk::{Codec, Error, Lift};

fn run<T>(f: impl FnOnce(&mut Cx) -> Result<T, Error>) -> (Result<T, Error>, Diagnostics) {
    let limits = Limits::default()
        .with(Kind::InputBytes, Limit::Finite(80 * 1024 * 1024))
        .with(Kind::DecodedBytes, Limit::Finite(1024 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(1_000_000_000))
        .with(Kind::Records, Limit::Finite(2_000_000))
        .with(Kind::Elements, Limit::Finite(1_000_000));
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    (
        f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
        diagnostics,
    )
}

fn main() {
    let Some(root) = std::env::var_os("ZUMEN_SAMPLES") else {
        eprintln!("ZUMEN_SAMPLES を指定してください。");
        std::process::exit(2);
    };
    for (index, name) in std::env::args().skip(1).enumerate() {
        // 指定フォルダーの直下に限る。出力にファイル名を混ぜない。
        let path = std::path::Path::new(&name);
        if path.components().count() != 1
            || !matches!(
                path.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            eprintln!("入力 {}: 直下のファイルを指定してください。", index + 1);
            std::process::exit(2);
        }
        let path = std::path::Path::new(&root).join(path);
        let Ok(size) = path.metadata().map(|m| m.len()) else {
            eprintln!("入力 {}: ファイルを確認できません。", index + 1);
            std::process::exit(2);
        };
        if size > 80 * 1024 * 1024 {
            std::process::exit(2);
        }
        let Ok(src) = std::fs::read(path) else {
            std::process::exit(2);
        };
        let (file, _) = run(|cx| Ifc::read(&src, cx));
        let Ok(file) = file else {
            eprintln!("入力 {}: L1 を読めません。", index + 1);
            std::process::exit(1);
        };
        let (building, diagnostics) = run(|cx| Lift::<Building>::lift(&file, cx));
        let Ok(building) = building else {
            eprintln!("入力 {}: lift が停止しました。", index + 1);
            for d in diagnostics
                .iter()
                .filter(|d| d.code().name().starts_with("budget."))
            {
                eprintln!("{}", d.code().name());
            }
            std::process::exit(1);
        };
        let mut out = Vec::new();
        if run(|cx| Ifc::write(&file, &mut out, cx)).0.is_err() || out != src {
            std::process::exit(1);
        }
        println!(
            "input={} bytes={} instances={} elements={} roundtrip=true",
            index + 1,
            size,
            file.ids().count(),
            building.elements().len()
        );
        let mut counts = BTreeMap::<String, u64>::new();
        for d in diagnostics.iter() {
            let mut key = d.code().name().to_owned();
            if d.code() == codes::IFC_ENTITY_DROPPED
                && let Some(Value::Text(kind)) = d.param("kind")
            {
                key.push(' ');
                key.push_str(kind);
            }
            *counts.entry(key).or_default() += d.count();
        }
        for (kind, count) in counts {
            println!("{kind} {count}");
        }
    }
}

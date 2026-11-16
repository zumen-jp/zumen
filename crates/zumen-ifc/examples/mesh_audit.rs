//! 指定した IFC の形の評価率と、時間・確保量を測る。入力は ZUMEN_SAMPLES 内に限る。
//!
//! パスと属性値は出力しない。`--elements` を付けると外部照合用にインスタンス番号と体積も出す。

#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]

#[path = "support/memory.rs"]
mod memory;

use std::{collections::BTreeMap, time::Instant};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Value};
use zumen_building::{
    Building, ElementRef, Opening,
    evaluation::{self, Options},
};
use zumen_geom3d::ManifoldKernel;
use zumen_ifc::Ifc;
use zumen_sdk::Codec;

fn measure(input: usize, bytes: &[u8], details: bool) -> Result<(), ()> {
    let limits = Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(64 * 1024 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(100_000_000_000));
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let baseline = memory::start();
    let start = Instant::now();
    let file = Ifc::read(bytes, &mut cx).map_err(|_| ())?;
    let read_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let building: Building = file.lift_geometry(&mut cx).map_err(|_| ())?;
    let lift_ns = start.elapsed().as_nanos();
    let (lift_peak, _) = memory::finish(baseline);
    drop(file);
    let mut diagnostics = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let baseline = memory::start();
    let start = Instant::now();
    let result = evaluation::evaluate(&building, Options::default(), &mut cx).map_err(|_| ())?;
    let mesh_ns = start.elapsed().as_nanos();
    let (mesh_peak, retained) = memory::finish(baseline);
    let mut with_geometry = 0;
    let mut drawn = 0;
    let mut complete = 0;
    let mut closed = 0;
    let mut opening_hosts = 0;
    let mut closed_hosts = 0;
    for (i, element) in result.iter().enumerate() {
        let at = ElementRef(i as u32);
        if building.geometry(at).and_then(|g| g.data().root).is_some() {
            with_geometry += 1;
            drawn += usize::from(!element.parts.is_empty());
            complete += usize::from(!element.parts.is_empty() && element.complete);
        }
        let mut solid = !element.parts.is_empty();
        let mut volume = 0.0;
        let mut triangles = 0;
        for part in &element.parts {
            solid &= ManifoldKernel
                .is_solid(&part.mesh, &mut cx)
                .map_err(|_| ())?;
            volume += part.mesh.volume() * part.placement.determinant().abs();
            triangles += part.mesh.triangles.len();
        }
        closed += usize::from(solid);
        if !element.parts.is_empty()
            && building
                .components
                .opening
                .iter()
                .any(|(_, o)| matches!(o, Opening::Voids(host) if *host == at))
        {
            opening_hosts += 1;
            closed_hosts += usize::from(solid);
        }
        if details && !element.parts.is_empty() {
            println!(
                "element input={input} instance={} volume={volume:.15} triangles={triangles} solid={solid} complete={}",
                building
                    .element(at)
                    .and_then(|e| e.source().instance)
                    .unwrap_or(0),
                element.complete
            );
        }
    }
    let mut counts = BTreeMap::<String, u64>::new();
    for d in diagnostics.iter() {
        // 形の種類と失敗理由は実装が持つ名前だけを記録する。任意の未対応型名は出さない。
        let mut key = d.code().name().to_owned();
        if let Some(Value::Text(reason)) = d.param("reason")
            && reason.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        {
            key.push(' ');
            key.push_str(reason);
        }
        *counts.entry(key).or_default() += d.count();
    }
    println!(
        "summary input={input} bytes={} elements={} geometry={with_geometry} drawn={drawn} complete={complete} closed={closed} opening_hosts={opening_hosts} closed_hosts={closed_hosts} read_ns={read_ns} lift_ns={lift_ns} mesh_ns={mesh_ns} lift_peak={lift_peak} mesh_peak={mesh_peak} mesh_retained={retained}",
        bytes.len(),
        building.elements().len()
    );
    for (code, count) in counts {
        println!("diagnostic input={input} {code} count={count}");
    }
    Ok(())
}

fn main() {
    let Some(root) = std::env::var_os("ZUMEN_SAMPLES") else {
        std::process::exit(2)
    };
    let Ok(root) = std::fs::canonicalize(root) else {
        std::process::exit(2)
    };
    let mut details = false;
    let mut input = 0;
    for name in std::env::args().skip(1) {
        if name == "--elements" {
            details = true;
            continue;
        }
        input += 1;
        let relative = std::path::Path::new(&name);
        if !relative
            .components()
            .all(|p| matches!(p, std::path::Component::Normal(_)))
        {
            std::process::exit(2);
        }
        let Ok(path) = std::fs::canonicalize(root.join(relative)) else {
            std::process::exit(2)
        };
        if !path.starts_with(&root)
            || path
                .metadata()
                .map_or(true, |m| m.len() > 512 * 1024 * 1024)
        {
            std::process::exit(2);
        }
        let Ok(bytes) = std::fs::read(path) else {
            std::process::exit(2)
        };
        if measure(input, &bytes, details).is_err() {
            eprintln!("input={input} stopped");
            std::process::exit(1);
        }
    }
}

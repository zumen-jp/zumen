//! 公開 IFC の 3D シーンの塊・許容差・時間・確保量を測る。入力名と属性値は出力しない。
#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]
#[path = "support/memory.rs"]
mod memory;

use std::{
    path::{Component, Path},
    time::Instant,
};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_ifc::Ifc;
use zumen_scene3d::{Options, build, encode};
use zumen_sdk::Codec;

fn limits() -> Limits {
    Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(64 * 1024 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(100_000_000_000))
}

fn measure(input: usize, bytes: &[u8]) -> Result<(), ()> {
    let cancel = CancelToken::new();
    let mut budget = Budget::new(limits());
    let mut diagnostics = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let start = Instant::now();
    let file = Ifc::read(bytes, &mut cx).map_err(|_| ())?;
    let building = file.lift_geometry(&mut cx).map_err(|_| ())?;
    println!("lift input={input} ns={}", start.elapsed().as_nanos());
    drop(file);
    let mut default_tolerance = None;
    // 予備評価を含む既定の経路を 1 回、明示した許容差と塊の組を順逆順で 3 回測る。
    for repeat in 0..3 {
        let mut variants = vec![
            (256, 1),
            (1024, 1),
            (4096, 1),
            (256, 4),
            (1024, 4),
            (4096, 4),
            (256, 16),
            (1024, 16),
            (4096, 16),
        ];
        if repeat % 2 == 1 {
            variants.reverse();
        }
        if repeat == 0 {
            variants.insert(0, (4096, 0));
        }
        for (chunk_instances, factor) in variants {
            let mut budget = Budget::new(limits());
            let mut diagnostics = Diagnostics::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let baseline = memory::start();
            let start = Instant::now();
            let scene = build(
                &building,
                Options {
                    tolerance: if factor == 0 {
                        None
                    } else {
                        default_tolerance.map(|v| v * f64::from(factor))
                    },
                    chunk_instances,
                    ..Options::default()
                },
                &mut cx,
            )
            .map_err(|_| ())?;
            let scene_ns = start.elapsed().as_nanos();
            default_tolerance.get_or_insert(scene.tolerance);
            let start = Instant::now();
            let blob = encode(&scene.data, &mut cx).map_err(|_| ())?;
            let encode_ns = start.elapsed().as_nanos();
            let (peak, retained) = memory::finish(baseline);
            println!(
                "scene input={input} repeat={repeat} chunk_target={chunk_instances} factor={factor} tolerance={} bytes={} chunks={} meshes={} instances={} elements={} vertices={} triangles={} scene_ns={scene_ns} encode_ns={encode_ns} peak={peak} retained={retained} diagnostics={}",
                scene.tolerance,
                blob.len(),
                scene.data.chunks.len(),
                scene.data.meshes.len(),
                scene.data.instances.len(),
                scene.data.elements.len(),
                scene
                    .data
                    .meshes
                    .iter()
                    .map(|m| m.vertices.len())
                    .sum::<usize>(),
                scene
                    .data
                    .meshes
                    .iter()
                    .map(|m| m.triangles.len())
                    .sum::<usize>(),
                diagnostics.iter().map(|d| d.count()).sum::<u64>()
            );
        }
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
    for (i, name) in std::env::args().skip(1).enumerate() {
        let relative = Path::new(&name);
        if !relative
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
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
        if measure(i + 1, &bytes).is_err() {
            eprintln!("failed input={}", i + 1);
            std::process::exit(1);
        }
    }
}

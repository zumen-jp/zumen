//! IFC を切り出して測る。既定の出力は入力番号と件数だけ。
//! `--json` は指定した 1 入力の座標・ID・GlobalId・出所を出すので、公開前に権利を確かめる。
#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]
#[path = "support/memory.rs"]
mod memory;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Component, Path},
    time::Instant,
};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    Building, Opening,
    drawing::{self, Options, Output, Paper, Role, View},
};
use zumen_crs::ProjectPoint;
use zumen_drawing::{PaperLength, Shape};
use zumen_ifc::Ifc;
use zumen_json::Writer;
use zumen_sdk::Codec;

fn limits() -> Limits {
    Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(64 * 1024 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(100_000_000_000))
}

fn details(building: &Building, out: &Output) -> Result<Vec<u8>, ()> {
    let mut json = Writer::new();
    json.begin_object();
    json.key("unit_m");
    json.float(
        building
            .document
            .units
            .iter()
            .find(|u| u.kind == "LENGTHUNIT")
            .and_then(|u| zumen_building::evaluation::length_scale(u).ok())
            .ok_or(())?,
    );
    json.key("origin");
    let p = out.frame.origin();
    json.array(&[p.x, p.y, p.z], |w, v| w.float(*v));
    json.key("complete");
    json.bool(out.complete);
    json.key("sources");
    json.begin_array();
    for source in &out.sources {
        json.begin_object();
        json.key("id");
        json.string(&source.id.to_string());
        json.key("global_id");
        match &source.global_id {
            Some(id) => json.string(id),
            None => json.null(),
        }
        json.key("instance");
        match source.instance {
            Some(id) => json.string(&id.to_string()),
            None => json.null(),
        }
        json.key("shape_instance");
        match source.shape_instance {
            Some(id) => json.string(&id.to_string()),
            None => json.null(),
        }
        json.key("class");
        match building.components.class.get(source.element) {
            Some(class) => json.string(&class.name),
            None => json.null(),
        }
        json.end_object();
    }
    json.end_array();
    json.key("lines");
    json.begin_array();
    let space = out.drawing.spaces.first().ok_or(())?;
    for line in &out.lines {
        let Shape::Line { start, end } = space.elements.get(line.index).ok_or(())?.shape else {
            return Err(());
        };
        json.begin_object();
        json.key("id");
        json.string(&line.id);
        json.key("drawing_id");
        json.string(&line.drawing_id);
        json.key("source");
        json.uint(line.source as u64);
        json.key("role");
        json.string(line.role.name());
        json.key("xy");
        json.array(&[start.x, start.y, end.x, end.y], |w, v| w.float(*v));
        json.end_object();
        if json.len() > 512 * 1024 * 1024 {
            return Err(());
        }
    }
    json.end_array();
    // 母体の線→母体の ID→元の開口関係→開口とその形の出所。線ごとの面の由来とは分ける。
    json.key("openings");
    json.begin_array();
    for (at, relation) in building.components.opening.iter() {
        let Opening::Voids(host) = relation else {
            continue;
        };
        let Some(element) = building.element(at) else {
            continue;
        };
        json.begin_object();
        json.key("host");
        json.string(&building.id(*host).ok_or(())?.to_string());
        json.key("id");
        json.string(&element.id().to_string());
        json.key("global_id");
        match &element.source().global_id {
            Some(id) => json.string(id),
            None => json.null(),
        }
        json.key("instance");
        match element.source().instance {
            Some(id) => json.string(&id.to_string()),
            None => json.null(),
        }
        json.key("shapes");
        json.begin_array();
        if let Some(geometry) = building.geometry(at) {
            for node in &geometry.data().nodes {
                if matches!(node.links, zumen_building::GeometryLinks::Leaf)
                    && let Some(id) = node.source.instance
                {
                    json.string(&id.to_string());
                }
            }
        }
        json.end_array();
        json.end_object();
    }
    json.end_array();
    json.end_object();
    Ok(json.into_bytes())
}

fn measure(
    input: usize,
    bytes: &[u8],
    floor_m: f64,
    height_m: f64,
    depth_m: f64,
    json: bool,
    bounds: bool,
) -> Result<(), ()> {
    let (mut budget, mut diag, cancel) = (
        Budget::new(limits()),
        Diagnostics::new(),
        CancelToken::new(),
    );
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let started = Instant::now();
    let file = Ifc::read(bytes, &mut cx).map_err(|_| ())?;
    let building = file.lift_geometry(&mut cx).map_err(|_| ())?;
    let lift_ns = started.elapsed().as_nanos();
    drop(file);
    if bounds {
        let meshes = zumen_building::evaluation::evaluate(
            &building,
            zumen_building::evaluation::Options::default(),
            &mut cx,
        )
        .map_err(|_| ())?;
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for mesh in meshes {
            for part in mesh.parts {
                for point in &part.mesh.positions {
                    let point = part.placement.point(*point);
                    for ((lo, hi), value) in low.iter_mut().zip(&mut high).zip(point) {
                        *lo = lo.min(value);
                        *hi = hi.max(value);
                    }
                }
            }
        }
        println!("bounds input={input} min={low:?} max={high:?}");
        return Ok(());
    }
    let units = building
        .document
        .units
        .iter()
        .find(|u| u.kind == "LENGTHUNIT")
        .and_then(|u| zumen_building::evaluation::length_scale(u).ok())
        .ok_or(())?;
    let options = Options {
        view: View::Plan {
            floor: ProjectPoint {
                x: 0.0,
                y: 0.0,
                z: floor_m / units,
            },
            height: height_m / units,
            depth: depth_m / units,
        },
        tolerance: 0.00001 / units,
        chord: 0.001 / units,
        paper: Paper {
            denominator: 100.0,
            origin: [PaperLength(0.0); 2],
            size: None,
        },
        symbols: &[],
    };
    for repeat in 0..if json { 1 } else { 3 } {
        let (mut budget, mut diag) = (Budget::new(limits()), Diagnostics::new());
        let baseline = memory::start();
        let started = Instant::now();
        let out = drawing::cut(
            &building,
            options,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .map_err(|_| ())?;
        let cut_ns = started.elapsed().as_nanos();
        let (peak, retained) = memory::finish(baseline);
        if json {
            std::io::stdout()
                .write_all(&details(&building, &out)?)
                .map_err(|_| ())?;
            return Ok(());
        }
        let s = out.statistics;
        let count = |role| out.lines.iter().filter(|line| line.role == role).count();
        println!(
            "drawing input={input} repeat={repeat} floor_m={floor_m} height_m={height_m} depth_m={depth_m} bytes={} elements={} parts={} triangles={} surfaces={} edges={} lines={} cut={} visible={} hidden={} grid={} annotation={} evaluation_failures={} projection_failures={} collapsed={} complete={} lift_ns={lift_ns} cut_ns={cut_ns} peak={peak} retained={retained} work={} logical_bytes={}",
            bytes.len(),
            s.elements,
            s.parts,
            s.triangles,
            s.surfaces,
            s.edges,
            s.lines,
            count(Role::Cut),
            count(Role::Visible),
            count(Role::Hidden),
            count(Role::Grid),
            count(Role::Annotation),
            s.evaluation_failures,
            s.projection_failures,
            s.collapsed,
            out.complete,
            budget.used(Kind::Work),
            budget.used(Kind::DecodedBytes)
        );
        if repeat == 0 {
            let mut counts = BTreeMap::new();
            for d in diag.iter() {
                *counts.entry(d.code().name()).or_insert(0) += d.count();
            }
            for (code, count) in counts {
                println!("diagnostic input={input} code={code} count={count}");
            }
        }
    }
    Ok(())
}

fn main() {
    let mut json = false;
    let mut bounds = false;
    let (mut floor, mut height, mut depth) = (0.0, 1.2, 2.0);
    let mut names = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--json" {
            json = true;
        } else if arg == "--bounds" {
            bounds = true;
        } else if let Some(v) = arg.strip_prefix("--floor-m=") {
            floor = v.parse().unwrap_or(f64::NAN);
        } else if let Some(v) = arg.strip_prefix("--height-m=") {
            height = v.parse().unwrap_or(f64::NAN);
        } else if let Some(v) = arg.strip_prefix("--depth-m=") {
            depth = v.parse().unwrap_or(f64::NAN);
        } else {
            names.push(arg);
        }
    }
    if names.is_empty()
        || json && names.len() != 1
        || ![floor, height, depth].into_iter().all(f64::is_finite)
        || depth <= 0.0
    {
        std::process::exit(2);
    }
    let Some(root) = std::env::var_os("ZUMEN_SAMPLES") else {
        std::process::exit(2)
    };
    let Ok(root) = std::fs::canonicalize(root) else {
        std::process::exit(2)
    };
    for (i, name) in names.iter().enumerate() {
        let relative = Path::new(name);
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
        if measure(i + 1, &bytes, floor, height, depth, json, bounds).is_err() {
            eprintln!("failed input={}", i + 1);
            std::process::exit(1);
        }
    }
}

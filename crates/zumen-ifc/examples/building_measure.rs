//! ZUMEN_SAMPLES 内の指定した IFC・GlobalId・文書単位の原点で面積を照らす。
//! 引数: 相対パス GlobalId x y z shape|center|inside auto|center|inside。
//! 指定した要素・壁と、分解・開口でつながる要素の形だけを取得する。
//! 出力は指定した計測値と件数だけ。元のファイル名・要素名・記録済みの数量は出さない。
#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]

use std::path::{Component, Path};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    Building, ElementRef, GlobalId, Id, Opening, ParentKind,
    measure::{
        self, Area, Boundary, InputBoundary, LengthUnit, Options, Rounding, RoundingMode,
        RoundingStage, Unit,
    },
};
use zumen_crs::ProjectPoint;
use zumen_geom3d::Tolerance;
use zumen_ifc::Ifc;
use zumen_sdk::{Codec, Lift};

fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [relative, id, x, y, z, boundary, input] = args.as_slice() else {
        return Err(());
    };
    let relative = Path::new(relative);
    if !relative
        .components()
        .all(|p| matches!(p, Component::Normal(_)))
    {
        return Err(());
    }
    let root =
        std::fs::canonicalize(std::env::var_os("ZUMEN_SAMPLES").ok_or(())?).map_err(|_| ())?;
    let path = std::fs::canonicalize(root.join(relative)).map_err(|_| ())?;
    if !path.starts_with(&root) || path.metadata().map_err(|_| ())?.len() > 512 * 1024 * 1024 {
        return Err(());
    }
    let bytes = std::fs::read(path).map_err(|_| ())?;
    let origin = ProjectPoint {
        x: x.parse().map_err(|_| ())?,
        y: y.parse().map_err(|_| ())?,
        z: z.parse().map_err(|_| ())?,
    };
    let boundary = match boundary.as_str() {
        "shape" => Boundary::Shape,
        "center" => Boundary::WallCenter,
        "inside" => Boundary::Inside,
        _ => return Err(()),
    };
    let input = match input.as_str() {
        "auto" => InputBoundary::Auto,
        "center" => InputBoundary::WallCenter,
        "inside" => InputBoundary::Inside,
        _ => return Err(()),
    };
    let limits = Limits::default()
        .with(Kind::Work, Limit::Finite(100_000_000_000))
        .with(Kind::DecodedBytes, Limit::Finite(64 * 1024 * 1024 * 1024));
    let (mut budget, mut diagnostics, cancel) =
        (Budget::new(limits), Diagnostics::new(), CancelToken::new());
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let file = Ifc::read(&bytes, &mut cx).map_err(|_| {
        eprintln!("read failed");
    })?;
    let mut building: Building = file.lift(&mut cx).map_err(|_| {
        eprintln!("lift failed");
    })?;
    let element = building
        .find(Id::Global(GlobalId::parse(id).ok_or(())?))
        .ok_or_else(|| {
            eprintln!("element missing");
        })?;
    let walls: Vec<ElementRef> = if boundary == Boundary::Shape {
        Vec::new()
    } else {
        building
            .components
            .class
            .iter()
            .filter(|(_, class)| {
                matches!(
                    class.name.as_str(),
                    "IfcWall" | "IfcWallStandardCase" | "IfcWallElementedCase"
                )
            })
            .map(|(at, _)| at)
            .collect()
    };
    // 属性は全体を保ち、面積に影響する形を取得する。対象外の形の不正を無視して
    // lift_geometry の部分結果を使う経路にはせず、個別取得の API を使う。
    let mut selected = vec![false; building.elements().len()];
    *selected.get_mut(element.0 as usize).ok_or(())? = true;
    for at in &walls {
        *selected.get_mut(at.0 as usize).ok_or(())? = true;
    }
    loop {
        let mut changed = false;
        let relations = building
            .components
            .parent
            .iter()
            .filter_map(|(child, parent)| {
                parent.of.as_ref().and_then(|parent| {
                    matches!(parent.kind, ParentKind::Aggregates | ParentKind::Nests)
                        .then_some(parent)
                        .filter(|p| building.components.spatial.get(p.target).is_none())
                        .map(|parent| (child, parent.target))
                })
            })
            .chain(
                building
                    .components
                    .opening
                    .iter()
                    .filter_map(|(at, opening)| {
                        if let Opening::Voids(host) = opening {
                            Some((at, *host))
                        } else {
                            None
                        }
                    }),
            );
        for (a, b) in relations {
            cx.check(Kind::Work, 1).map_err(|_| ())?;
            let (a, b) = (a.0 as usize, b.0 as usize);
            if selected.get(a).ok_or(())? != selected.get(b).ok_or(())? {
                *selected.get_mut(a).ok_or(())? = true;
                *selected.get_mut(b).ok_or(())? = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (at, selected) in selected.into_iter().enumerate() {
        if !selected {
            continue;
        }
        let at = ElementRef(at as u32);
        let instance = building
            .element(at)
            .and_then(|e| e.source().instance)
            .ok_or(())?;
        let geometry = file.owned_geometry(instance, &mut cx).map_err(|_| {
            eprintln!("geometry failed");
        })?;
        building
            .insert_geometry(at, geometry, &mut cx)
            .map_err(|_| ())?;
    }
    let (mut budget, mut diagnostics) = (Budget::new(limits), Diagnostics::new());
    let out = measure::areas(
        &building,
        &[element],
        Area {
            origin,
            boundary,
            input,
            walls: &walls,
        },
        Options {
            unit: Unit::Area(LengthUnit::Meter),
            rounding: Rounding {
                mode: RoundingMode::HalfAway,
                digits: 3,
                stage: RoundingStage::Total,
            },
            tolerance: Tolerance {
                length: 0.000001,
                chord: 0.001,
            },
            profile: None,
        },
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .map_err(|_| {
        eprintln!("measure failed");
    })?;
    let value = out
        .raw_total
        .map_or_else(|| "null".into(), |v| v.to_string());
    let rounded = out
        .rounded_total
        .map_or_else(|| "null".into(), |v| format!("\"{v}\""));
    println!(
        "{{\"area_m2\":{value},\"rounded_m2\":{rounded},\"work\":{},\"bytes\":{},\"diagnostics\":{}}}",
        budget.used(Kind::Work),
        budget.used(Kind::DecodedBytes),
        diagnostics.len()
    );
    Ok(())
}
fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

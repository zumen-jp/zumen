//! ifc-walls.py の建物で壁の所属と配置を変え、構造の再構築と依存伝播を照合する。

use std::{collections::BTreeSet, env, fs, process::ExitCode, time::Instant};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind};
use zumen_building::{Building, ElementRef, Transform};
use zumen_ifc::{Edit, Ifc, IfcLive, NewPlacement, Target, apply};
use zumen_live::{DepGraph, RowId};
use zumen_sdk::{Codec, Error, Lift};

const BUILDING: u64 = 8;
const FIRST: u64 = 1000;
const SECOND: u64 = 2000;
const WALL: u64 = 1010;

#[derive(Clone, Copy)]
struct Cost {
    ns: u128,
    decoded: u64,
    model: u64,
}

fn measure<T>(
    call: impl FnOnce(&mut Cx) -> Result<T, Error>,
) -> Result<(T, Vec<Diagnostic>, Cost), String> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    #[allow(clippy::disallowed_methods, reason = "計測器で実行時間を測る")]
    let start = Instant::now();
    let result = call(&mut Cx::new(&mut budget, &mut diag, &cancel));
    let ns = start.elapsed().as_nanos();
    let value = result.map_err(|e| format!("{e:?} {diag:?}"))?;
    Ok((
        value,
        diag.into_vec(),
        Cost {
            ns,
            decoded: budget.used(Kind::DecodedBytes),
            model: budget.used(Kind::ModelBytes),
        },
    ))
}

fn element(live: &IfcLive, id: u64) -> Result<ElementRef, String> {
    live.element_by_instance(id)
        .ok_or_else(|| format!("要素 #{id} がない"))
}

fn graph(
    live: &IfcLive,
    storeys: u64,
    moved: bool,
    reopened_order: &[ElementRef],
) -> Result<Cost, String> {
    let mut all = BTreeSet::new();
    let building = element(live, BUILDING)?;
    let wall = element(live, WALL)?;
    let host = element(live, if moved { SECOND } else { FIRST })?;
    let parts = &live.building().components;
    if parts
        .parent
        .get(wall)
        .and_then(|p| p.of.as_ref())
        .map(|m| m.target)
        != Some(host)
        || parts.placement.get(wall).and_then(|p| p.relative_to) != Some(host)
        || live.dep_graph().sources_of(RowId(wall.0)) != [RowId(host.0)]
    {
        return Err("移す壁の所属、配置の親、直接の依存元が一致しない".into());
    }
    for floor in 1..=storeys {
        let floor_id = 1000 * floor;
        let storey = element(live, floor_id)?;
        all.insert(storey);
        let mut children = BTreeSet::new();
        for index in 0..100 {
            let id = floor_id + 10 + index * 6;
            let child = element(live, id)?;
            all.insert(child);
            if !moved || id != WALL {
                children.insert(child);
            }
        }
        if moved && floor_id == SECOND {
            children.insert(wall);
        }
        let direct: Vec<_> = live.dep_graph().dependents_of(RowId(storey.0));
        let direct: Vec<_> = direct.into_iter().map(|r| ElementRef(r.0)).collect();
        if !direct.iter().eq(&children)
            || measure(|cx| live.affected_elements(&[storey], cx))?.0 != direct
        {
            return Err(format!("第 {floor} 階の直接の辺か推移先が期待値と違う"));
        }
    }
    let (affected, _, cost) = measure(|cx| live.affected_elements(&[building], cx))?;
    if affected != reopened_order
        || affected.len() != all.len()
        || affected.into_iter().collect::<BTreeSet<_>>() != all
    {
        return Err("建物からの推移先が全階と全壁の集合または再読の順序と違う".into());
    }
    Ok(cost)
}

struct Reference {
    building: Building,
    diagnostics: Vec<Diagnostic>,
    graph: DepGraph,
    affected: Vec<ElementRef>,
}

impl Reference {
    fn read(bytes: &[u8], reopened: &IfcLive, geometry: bool) -> Result<Self, String> {
        let file = measure(|cx| Ifc::read(bytes, cx))?.0;
        let (building, diagnostics, _) = measure(|cx| {
            if geometry {
                file.lift_geometry(cx)
            } else {
                file.lift(cx)
            }
        })?;
        let building_ref = element(reopened, BUILDING)?;
        let affected = measure(|cx| reopened.affected_elements(&[building_ref], cx))?.0;
        Ok(Self {
            building,
            diagnostics,
            graph: reopened.dep_graph().clone(),
            affected,
        })
    }

    fn check(&self, live: &IfcLive, bytes: &[u8]) -> Result<(), String> {
        let (saved, save_diagnostics, _) = measure(|cx| live.save_spf(cx))?;
        let (_, reported, _) = measure(|cx| live.report_lift_diagnostics(cx))?;
        if saved != bytes
            || !save_diagnostics.is_empty()
            || live.building() != &self.building
            || reported != self.diagnostics
            || live.dep_graph() != &self.graph
        {
            return Err("保存バイト、保持モデル、変換診断か依存グラフが再読結果と違う".into());
        }
        Ok(())
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let (path, geometry) = match args.as_slice() {
        [path] => (path, false),
        [path, flag] if flag == "--geometry" => (path, true),
        _ => return Err("使い方: ifc-structure <ifc-walls.py の出力> [--geometry]".into()),
    };
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let open = |bytes: &[u8], cx: &mut Cx| {
        if geometry {
            IfcLive::open_with_geometry(bytes, 1, cx)
        } else {
            IfcLive::open(bytes, cx)
        }
    };
    let (mut live, _, opened) = measure(|cx| open(&bytes, cx))?;
    let storeys = live
        .building()
        .components
        .class
        .iter()
        .filter(|(_, v)| v.name == "IfcBuildingStorey")
        .count() as u64;
    if !matches!(storeys, 10 | 100) {
        return Err("10 階または 100 階の生成入力を指定する".into());
    }
    let initial = Reference::read(&bytes, &live, geometry)?;
    initial.check(&live, &bytes)?;
    graph(&live, storeys, false, &initial.affected)?;
    let wall = Target::Instance(WALL);
    let second = Target::Instance(SECOND);
    let edits = [
        Edit::Place {
            element: wall,
            placement: NewPlacement {
                relative_to: Some(second),
                local: Transform::translation(0.0, 0.0, 0.0),
            },
        },
        Edit::Contain {
            element: wall,
            space: second,
        },
    ];
    let seed = b"live-structure";
    let file = measure(|cx| Ifc::read(&bytes, cx))?.0;
    let (expected, diag, _) = measure(|cx| apply(&file, &edits, seed, cx))?;
    drop(file);
    let (changed, actual, rebuilt) = measure(|cx| live.apply(&edits, seed, cx))?;
    if changed == 0 || changed != expected.changed || actual != diag || expected.bytes == bytes {
        return Err("構造編集の変更件数か診断が逐次処理と違う".into());
    }
    let reopened = measure(|cx| open(&expected.bytes, cx))?.0;
    let edited = Reference::read(&expected.bytes, &reopened, geometry)?;
    drop(reopened);
    edited.check(&live, &expected.bytes)?;
    let propagated = graph(&live, storeys, true, &edited.affected)?;
    let (ok, diag, undone) = measure(|cx| live.undo(cx))?;
    if !ok || !diag.is_empty() {
        return Err("取り消せない".into());
    }
    initial.check(&live, &bytes)?;
    graph(&live, storeys, false, &initial.affected)?;
    let (ok, diag, redone) = measure(|cx| live.redo(cx))?;
    if !ok || !diag.is_empty() {
        return Err("やり直せない".into());
    }
    edited.check(&live, &expected.bytes)?;
    graph(&live, storeys, true, &edited.affected)?;
    println!(
        "walls\tgeometry\tchanged_records\toperation\tnanoseconds\tdecoded_bytes\tmodel_bytes"
    );
    for (name, cost) in [
        ("open", opened),
        ("rebuild", rebuilt),
        ("propagate", propagated),
        ("undo", undone),
        ("redo", redone),
    ] {
        println!(
            "{}\t{geometry}\t{changed}\t{name}\t{}\t{}\t{}",
            storeys * 100,
            cost.ns,
            cost.decoded,
            cost.model
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

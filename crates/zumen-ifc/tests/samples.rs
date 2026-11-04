//! 公開サンプルの IFC（`samples/ifc`、#412）を読み、建物モデルに写す。
//!
//! - L1 で読み、変えずに書くと元のバイト列に戻る。
//! - 既定の予算で建物モデルに写せ、2 度写すと同じになる。サンプルごとの要約（バージョン・要素・階・コンポーネントの数・配置・診断・
//!   要素のハッシュ）が `samples.txt` と同じになる。要約を作り直すときは `ZUMEN_IFC_SAMPLES_WRITE=1` を付けて流し、
//!   変わった理由を PR に書く。CI はどの環境でも同じ要約を求めるので、ハッシュで環境ごとの一致も確かめる。
//! - 受け取った IFC（`ZUMEN_SAMPLES` のフォルダー）は、読めて写せることだけを確かめる。
//! - `ZUMEN_IFC_SAMPLES_OUT` にフォルダーを渡すと、要素の値を JSON に書く。毎晩、IfcOpenShell で読んだ値と
//!   照らす（`ifcopenshell/samples.py`）。
#![cfg(feature = "building")]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Limits, Value};
use zumen_building::{Building, ElementRef, Hash, Id, Literal, Opening, ParentKind, SpatialKind};
use zumen_ifc::{Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Lift};

/// `samples/` の根
#[cfg(test)]
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples")
}

/// 要約の期待のファイル
#[cfg(test)]
fn expected_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples.txt")
}

/// フォルダーの中の IFC。下のフォルダーも見る。道筋の順
#[cfg(test)]
fn ifc_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ifc"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// 公開サンプルの IFC と、`samples/` からの道筋（`ifc/gni/model_9.ifc` など）
#[cfg(test)]
fn public_samples() -> Vec<(String, PathBuf)> {
    let root = root();
    ifc_files(&root.join("ifc"))
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            (name, path)
        })
        .collect()
}

/// 既定の予算の文脈で `f` を流し、結果と診断を返す
#[cfg(test)]
fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

/// 読み、変えずに書くと元のバイト列に戻ることを確かめ、建物モデルに写す。2 度写して同じになることも確かめる
#[cfg(test)]
fn read_and_lift(name: &str, src: &[u8]) -> (IfcFile, Diagnostics, Building, Diagnostics) {
    let (file, read_diag) = with(|cx| Ifc::read(src, cx));
    let file = file.unwrap_or_else(|e| panic!("{name}: {e:?} {read_diag:?}"));
    let (written, _) = with(|cx| {
        let mut out = Vec::new();
        Ifc::write(&file, &mut out, cx).map(|()| out)
    });
    assert!(
        written.unwrap() == src,
        "{name}: 書き戻したバイト列が元と違う"
    );
    let (building, lift_diag) = with(|cx| file.lift(cx));
    let building: Building = building.unwrap_or_else(|e| panic!("{name}: {e:?} {lift_diag:?}"));
    let (again, _) = with(|cx| Lift::<Building>::lift(&file, cx));
    assert!(again.unwrap() == building, "{name}: 2 度写すと違う");
    (file, read_diag, building, lift_diag)
}

/// 診断の code と、種類を表す値（`kind`・`type`）と、件数。code と値の順
#[cfg(test)]
fn tally(diag: &Diagnostics) -> Vec<String> {
    let mut found: BTreeMap<String, u64> = BTreeMap::new();
    for d in diag.iter() {
        let mut key = d.code().name().to_owned();
        for param in ["kind", "type"] {
            if let Some(Value::Text(text)) = d.param(param) {
                write!(key, " {text}").unwrap();
            }
        }
        *found.entry(key).or_default() += d.count();
    }
    found.into_iter().map(|(k, n)| format!("{k} {n}")).collect()
}

/// 文字列か、ないときの `-`
#[cfg(test)]
fn opt(text: Option<&str>) -> String {
    text.map_or_else(|| "-".to_owned(), |t| format!("{t:?}"))
}

/// サンプルの要約。行の並びは決まっている
#[cfg(test)]
fn summary(name: &str, src: &[u8]) -> String {
    let (file, read_diag, building, lift_diag) = read_and_lift(name, src);
    let mut out = String::new();
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    line(format!("== {name}"));
    line(format!(
        "schema {} records {} bytes {}",
        String::from_utf8_lossy(file.schema()),
        file.records().len(),
        src.len()
    ));
    for d in tally(&read_diag) {
        line(format!("read {d}"));
    }
    let document = &building.document;
    if let Some(app) = &document.application {
        line(format!(
            "application {:?} {}",
            app.name,
            opt(app.version.as_deref())
        ));
    }
    line(format!(
        "document units {} contexts {} map {} true-north {}",
        document.units.len(),
        document.contexts.len(),
        document.map.is_some(),
        document.true_north.is_some()
    ));
    let elements = building.elements();
    let global = elements
        .iter()
        .filter(|e| matches!(e.id(), Id::Global(_)))
        .count();
    line(format!(
        "elements {} global-ids {global} instance-ids {}",
        elements.len(),
        elements.len() - global
    ));
    let c = &building.components;
    let mut classes: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (_, class) in c.class.iter() {
        *classes
            .entry((class.name.as_str(), class.original.as_str()))
            .or_default() += 1;
    }
    for ((class, original), n) in classes {
        if class == original {
            line(format!("class {class} {n}"));
        } else {
            line(format!("class {class} ({original}) {n}"));
        }
    }
    for (element, spatial) in c.spatial.iter() {
        if spatial.kind != SpatialKind::Storey {
            continue;
        }
        let name = c.identity.get(element).and_then(|i| i.name.as_deref());
        let elevation = spatial
            .elevation
            .as_ref()
            .map_or_else(|| "-".to_owned(), |m| m.value.to_string());
        line(format!("storey {} {elevation}", opt(name)));
    }
    let count = |n: usize| n.to_string();
    line(format!(
        "components identity {} class {} classification {} spatial {} parent {} placement {} representation {} \
         style {} type {} properties {} quantities {} material {} opening {} group {} connection {} grid {} \
         level {} zone {} extension {}",
        count(c.identity.len()),
        count(c.class.len()),
        count(c.classification.len()),
        count(c.spatial.len()),
        count(c.parent.len()),
        count(c.placement.len()),
        count(c.representation.len()),
        count(c.style.len()),
        count(c.r#type.len()),
        count(c.properties.len()),
        count(c.quantities.len()),
        count(c.material.len()),
        count(c.opening.len()),
        count(c.group.len()),
        count(c.connection.len()),
        count(c.grid.len()),
        count(c.level.len()),
        count(c.zone.len()),
        count(c.extension.len()),
    ));
    let (placed, placement_diag) = with(|cx| {
        c.placement
            .iter()
            .filter(|(element, _)| matches!(building.world_placement(*element, cx), Ok(Some(_))))
            .count()
    });
    line(format!("world-placement {placed} of {}", c.placement.len()));
    for d in tally(&placement_diag) {
        line(format!("placement {d}"));
    }
    let (relations, _) = with(|cx| building.relations(cx).is_ok());
    assert!(relations, "{name}: 関係の索引を作れない");
    for d in tally(&lift_diag) {
        line(format!("lift {d}"));
    }
    let mut hashes = String::new();
    for (at, element) in elements.iter().enumerate() {
        let hash = Hash::element(&building, ElementRef(at as u32)).unwrap();
        writeln!(hashes, "{} {hash}", element.id()).unwrap();
    }
    line(format!(
        "hashes elements {} document-values {}",
        Hash::document(hashes.as_bytes()),
        Hash::document_values(&building)
    ));
    out
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのはテストだけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn public_samples_match_the_summaries() {
    let samples = public_samples();
    assert!(!samples.is_empty(), "samples/ifc にサンプルがない");
    let mut text = String::from(
        "# samples/ifc の IFC を建物モデルに写した要約（crates/zumen-ifc/tests/samples.rs）。\n\
         # 作り直すときは ZUMEN_IFC_SAMPLES_WRITE=1 を付けて流す。\n",
    );
    for (name, path) in &samples {
        text.push_str(&summary(name, &std::fs::read(path).unwrap()));
    }
    let path = expected_path();
    if std::env::var_os("ZUMEN_IFC_SAMPLES_WRITE").is_some_and(|v| !v.is_empty()) {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    if expected != text {
        let first = expected
            .lines()
            .zip(text.lines())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| expected.lines().count().min(text.lines().count()));
        panic!(
            "要約が tests/samples.txt と違う。{} 行目から:\n期待: {:?}\n実際: {:?}",
            first + 1,
            expected.lines().nth(first),
            text.lines().nth(first)
        );
    }
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのはテストだけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn received_samples_lift() {
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return;
    };
    for path in ifc_files(Path::new(&dir)) {
        let name = path.display().to_string();
        read_and_lift(&name, &std::fs::read(&path).unwrap());
    }
}

/// 値を JSON に書く。要素は IFC のインスタンスの番号で書く
#[cfg(test)]
fn literal(out: &mut Writer, building: &Building, value: &Literal) {
    match value {
        Literal::Real(v) => out.float(*v),
        Literal::Integer(v) => out.int(*v),
        Literal::Text(v) | Literal::Enumeration(v) => out.string(v),
        Literal::Boolean(v) => out.bool(*v),
        Literal::Logical(v) => match v {
            Some(v) => out.bool(*v),
            None => out.null(),
        },
        Literal::Element(e) => match instance(building, *e) {
            Some(n) => out.uint(n),
            None => out.null(),
        },
        Literal::List(values) => out.array(values, |out, v| literal(out, building, v)),
    }
}

#[cfg(test)]
fn instance(building: &Building, element: ElementRef) -> Option<u64> {
    building.element(element)?.source().instance
}

#[cfg(test)]
fn optional_instance(out: &mut Writer, building: &Building, element: Option<ElementRef>) {
    match element.and_then(|e| instance(building, e)) {
        Some(n) => out.uint(n),
        None => out.null(),
    }
}

/// IfcOpenShell と照らす値。要素ごとに、インスタンスの番号・ID・クラス・名前・所属・型・プロジェクトの空間への
/// 変換・開口・プロパティ・数量
#[cfg(test)]
fn model_json(name: &str, building: &Building) -> Vec<u8> {
    let c = &building.components;
    let mut out = Writer::new();
    out.begin_object();
    out.key("path");
    out.string(name);
    out.key("elements");
    out.begin_array();
    for (at, element) in building.elements().iter().enumerate() {
        let e = ElementRef(at as u32);
        out.begin_object();
        out.key("instance");
        optional_instance(&mut out, building, Some(e));
        out.key("id");
        out.string(&element.id().to_string());
        out.key("global_id");
        match &element.source().global_id {
            Some(g) => out.string(g),
            None => out.null(),
        }
        out.key("class");
        match c.class.get(e) {
            Some(class) => out.string(&class.original),
            None => out.null(),
        }
        out.key("name");
        match c.identity.get(e).and_then(|i| i.name.as_deref()) {
            Some(n) => out.string(n),
            None => out.null(),
        }
        let membership = c.parent.get(e).and_then(|p| p.of.as_ref());
        out.key("parent");
        optional_instance(&mut out, building, membership.map(|m| m.target));
        out.key("parent_kind");
        match membership.map(|m| m.kind) {
            Some(ParentKind::Aggregates) => out.string("aggregates"),
            Some(ParentKind::Nests) => out.string("nests"),
            Some(ParentKind::Contained) => out.string("contained"),
            None => out.null(),
        }
        out.key("type");
        optional_instance(&mut out, building, c.r#type.get(e).map(|t| t.target));
        out.key("world");
        let (world, _) = with(|cx| building.world_placement(e, cx));
        match world.ok().flatten() {
            Some(transform) => out.array(&transform.0, |out, row| {
                out.array(row, |out, v| out.float(*v));
            }),
            None => out.null(),
        }
        out.key("voids");
        let voids = match c.opening.get(e) {
            Some(Opening::Voids(target)) => Some(*target),
            _ => None,
        };
        optional_instance(&mut out, building, voids);
        out.key("fills");
        let fills = match c.opening.get(e) {
            Some(Opening::Fills(target)) => Some(*target),
            _ => None,
        };
        optional_instance(&mut out, building, fills);
        // 同じ名前のセットがありうるので、セットは名前で引く形にせず、書いた順に並べる
        out.key("properties");
        out.begin_array();
        for set in c
            .properties
            .get(e)
            .map(|p| p.sets.as_slice())
            .unwrap_or_default()
        {
            out.begin_array();
            out.string(&set.name);
            out.begin_object();
            for property in &set.properties {
                out.key(&property.name);
                match &property.value {
                    Some(value) => literal(&mut out, building, &value.value),
                    None => out.null(),
                }
            }
            out.end_object();
            out.end_array();
        }
        out.end_array();
        out.key("quantities");
        out.begin_array();
        for set in c
            .quantities
            .get(e)
            .map(|q| q.sets.as_slice())
            .unwrap_or_default()
        {
            out.begin_array();
            out.string(&set.name);
            out.begin_object();
            for quantity in &set.quantities {
                out.key(&quantity.name);
                out.float(quantity.value.value);
            }
            out.end_object();
            out.end_array();
        }
        out.end_array();
        out.end_object();
    }
    out.end_array();
    out.end_object();
    out.into_bytes()
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのはテストだけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn writes_the_sample_models_for_other_readers() {
    let Some(dir) = std::env::var_os("ZUMEN_IFC_SAMPLES_OUT").filter(|d| !d.is_empty()) else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, path) in public_samples() {
        let src = std::fs::read(&path).unwrap();
        let (_, _, building, _) = read_and_lift(&name, &src);
        let file = name.trim_start_matches("ifc/").replace('/', "__");
        std::fs::write(
            dir.join(format!("{file}.json")),
            model_json(&name, &building),
        )
        .unwrap();
    }
}

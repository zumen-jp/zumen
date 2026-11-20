//! コアの常駐文書エンジンを、形式別の逐次 apply と照合し、速さを測る。
//! 試作の Document は使わず、AcadLive・JwwLive・SxfLive の公開入口を呼ぶ。

use std::hint::black_box;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use zumen_acad::{AcadLive, Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{
    Affine, Applied, Changes, Drawing2D, Edit, Hash, Origin, Position, Seen, Shape, id,
};
use zumen_jww::{Jww, JwwFile, JwwLive};
use zumen_sdk::{Apply, Codec, Error, Lift};
use zumen_sxf::{Sxf, SxfFile, SxfLive};

struct Probe<T> {
    result: Result<T, Error>,
    diagnostics: Vec<Diagnostic>,
    used: [u64; Kind::ALL.len()],
    micros: f64,
}

fn probe<T>(decoded_mib: u64, call: impl FnOnce(&mut Cx<'_>) -> Result<T, Error>) -> Probe<T> {
    let bytes_limit = Limit::Finite(decoded_mib.saturating_mul(1024 * 1024));
    let limits = Limits::default()
        .with(Kind::DecodedBytes, bytes_limit)
        .with(Kind::ModelBytes, bytes_limit);
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    #[allow(
        clippy::disallowed_methods,
        reason = "測定用の実行入口で経過時間を測る"
    )]
    let start = Instant::now();
    let result = call(&mut cx);
    let micros = start.elapsed().as_secs_f64() * 1e6;
    Probe {
        result,
        diagnostics: diagnostics.iter().cloned().collect(),
        used: Kind::ALL.map(|kind| budget.used(kind)),
        micros,
    }
}

fn required<T>(probe: Probe<T>, operation: &str) -> Result<T, String> {
    probe.result.map_err(|error| {
        format!(
            "{operation}: {error:?}, diagnostics={:?}, used={:?}",
            probe.diagnostics, probe.used
        )
    })
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    applied: Vec<Applied>,
    header_changed: Option<bool>,
}

enum Current {
    Dxf(DxfFile),
    Dwg(DwgFile),
    Jww(JwwFile),
    Sxf(SxfFile),
}

impl Current {
    fn open(extension: &str, bytes: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        match extension {
            "dxf" => Dxf::read(bytes, cx).map(Self::Dxf),
            "dwg" => Dwg::read(bytes, cx).map(Self::Dwg),
            "jww" => Jww::read(bytes, cx).map(Self::Jww),
            _ => Sxf::read(bytes, cx).map(Self::Sxf),
        }
    }

    fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        match self {
            Self::Dxf(file) => file.lift(cx),
            Self::Dwg(file) => file.lift(cx),
            Self::Jww(file) => file.lift(cx),
            Self::Sxf(file) => file.lift(cx),
        }
    }

    fn apply(&mut self, changes: &Changes, cx: &mut Cx<'_>) -> Result<Outcome, Error> {
        match self {
            Self::Dxf(file) => file.apply(changes, cx).map(|applied| Outcome {
                applied,
                header_changed: None,
            }),
            Self::Dwg(file) => file.apply(changes, cx).map(|applied| Outcome {
                applied,
                header_changed: None,
            }),
            Self::Jww(file) => file.apply(changes, cx).map(|(applied, changed)| Outcome {
                applied,
                header_changed: Some(changed),
            }),
            Self::Sxf(file) => file.apply(changes, cx).map(|applied| Outcome {
                applied,
                header_changed: None,
            }),
        }
    }

    fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        match self {
            Self::Dxf(file) => Dxf::write(file, &mut out, cx)?,
            Self::Dwg(file) => Dwg::write(file, &mut out, cx)?,
            Self::Jww(file) => Jww::write(file, &mut out, cx)?,
            Self::Sxf(file) => Sxf::write(file, &mut out, cx)?,
        }
        Ok(out)
    }
}

enum Live {
    Acad(Box<AcadLive>),
    Jww(Box<JwwLive>),
    Sxf(Box<SxfLive>),
}

impl Live {
    fn open(extension: &str, bytes: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        match extension {
            "dxf" | "dwg" => AcadLive::open_with_threads(bytes, threads, cx)
                .map(Box::new)
                .map(Self::Acad),
            "jww" => JwwLive::open_with_threads(bytes, threads, cx)
                .map(Box::new)
                .map(Self::Jww),
            _ => SxfLive::open_with_threads(bytes, threads, cx)
                .map(Box::new)
                .map(Self::Sxf),
        }
    }

    fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        match self {
            Self::Acad(live) => live.drawing(cx),
            Self::Jww(live) => live.drawing(cx),
            Self::Sxf(live) => live.drawing(cx),
        }
    }

    fn apply(&mut self, changes: &Changes, cx: &mut Cx<'_>) -> Result<Outcome, Error> {
        match self {
            Self::Acad(live) => live.apply(changes, cx).map(|applied| Outcome {
                applied,
                header_changed: None,
            }),
            Self::Jww(live) => live.apply(changes, cx).map(|(applied, changed)| Outcome {
                applied,
                header_changed: Some(changed),
            }),
            Self::Sxf(live) => live.apply(changes, cx).map(|applied| Outcome {
                applied,
                header_changed: None,
            }),
        }
    }

    fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(live) => live.undo(cx),
            Self::Jww(live) => live.undo(cx),
            Self::Sxf(live) => live.undo(cx),
        }
    }

    fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(live) => live.redo(cx),
            Self::Jww(live) => live.redo(cx),
            Self::Sxf(live) => live.redo(cx),
        }
    }

    fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        match self {
            Self::Acad(live) => live.save(cx),
            Self::Jww(live) => live.save(cx),
            Self::Sxf(live) => live.save(cx),
        }
    }

    fn select(&self, target: &Target, cx: &mut Cx<'_>) -> Result<usize, Error> {
        match self {
            Self::Acad(live) => {
                let mut rows = Vec::new();
                live.select_rect(target.query, &mut rows);
                Ok(rows.len())
            }
            Self::Jww(live) => match target.origin {
                Origin::Space(space) => live
                    .select_in_space(space, target.query, cx)
                    .map(|rows| rows.len()),
                _ => live.select_sheet(target.query, cx).map(|rows| rows.len()),
            },
            Self::Sxf(live) => Ok(match target.origin {
                Origin::Space(space) => live.select_in_space(space, target.query).len(),
                _ => live.select_sheet(target.query).len(),
            }),
        }
    }

    fn live_count(&self) -> u32 {
        match self {
            Self::Acad(live) => live.live_count(),
            Self::Jww(live) => live.live_count(),
            Self::Sxf(live) => live.live_count(),
        }
    }
}

struct Target {
    seen: Seen,
    origin: Origin,
    query: [f64; 4],
}

fn query<P>(shape: &Shape<P>, xy: impl Fn(&P) -> [f64; 2]) -> Option<[f64; 4]> {
    if let Shape::Line { start, end } = shape {
        let [x0, y0] = xy(start);
        let [x1, y1] = xy(end);
        return Some([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]);
    }
    let point = match shape {
        Shape::Circle { center, .. } => center,
        Shape::Point(point) => &point.position,
        Shape::Polyline(line) => &line.vertices.first()?.point,
        _ => return None,
    };
    let [x, y] = xy(point);
    Some([x, y, x, y])
}

fn fallback_query<P>(shape: &Shape<P>, xy: impl Fn(&P) -> [f64; 2]) -> Option<[f64; 4]> {
    let point = match shape {
        Shape::Symbol(symbol) => &symbol.position,
        Shape::Curve(zumen_drawing::Edge::Arc { center, .. }) => center,
        Shape::Text(text) => match &text.placement {
            zumen_drawing::Placement::Anchor { point, .. } => point,
            zumen_drawing::Placement::Between { first, .. } => first,
        },
        _ => return None,
    };
    let [x, y] = xy(point);
    Some([x, y, x, y])
}

fn target(drawing: &Drawing2D) -> Result<Target, String> {
    let desired = drawing
        .spaces
        .first()
        .map(|space| Origin::Space(space.id))
        .or_else(|| drawing.sheets.first().map(|sheet| Origin::Sheet(sheet.id)));
    let mut found = None;
    let mut fallback = None;
    id::each_element(drawing, |position, name, element| {
        let primary = match element {
            id::Found::Model(element) => query(&element.shape, |point| [point.x, point.y]),
            id::Found::Paper(element) => query(&element.shape, |point| [point.x, point.y]),
        };
        let secondary = if primary.is_none() {
            match element {
                id::Found::Model(element) => {
                    fallback_query(&element.shape, |point| [point.x, point.y])
                }
                id::Found::Paper(element) => {
                    fallback_query(&element.shape, |point| [point.x, point.y])
                }
            }
        } else {
            None
        };
        let origin = match position {
            Position::Model { space, .. }
                if drawing.spaces.iter().any(|model| model.id == space) =>
            {
                Some(Origin::Space(space))
            }
            Position::Paper { sheet, .. } => Some(Origin::Sheet(sheet)),
            _ => None,
        };
        if let Some(origin) = origin
            && Some(origin) == desired
        {
            if let Some(query) = primary {
                found = Some(Target {
                    seen: Seen {
                        id: name,
                        hash: element.hash(),
                    },
                    origin,
                    query,
                });
            } else if let Some(query) = secondary {
                fallback = Some(Target {
                    seen: Seen {
                        id: name,
                        hash: element.hash(),
                    },
                    origin,
                    query,
                });
            }
        }
        Ok::<_, String>(())
    })?;
    found
        .or(fallback)
        .ok_or_else(|| "最初の空間か用紙に、対象の要素が見つからない".to_owned())
}

fn identities(drawing: &Drawing2D) -> Vec<(String, Hash)> {
    let mut values = Vec::new();
    let result: Result<(), std::convert::Infallible> =
        id::each_element(drawing, |_, name, element| {
            values.push((name, element.hash()));
            Ok(())
        });
    match result {
        Ok(()) => values,
        Err(never) => match never {},
    }
}

fn check(label: &str, equal: bool, failed: &mut usize) {
    println!("check\t{label}\t{}", if equal { "pass" } else { "FAIL" });
    if !equal {
        *failed += 1;
    }
}

fn same_model(
    label: &str,
    actual: &Probe<Drawing2D>,
    expected: &Probe<Drawing2D>,
    failed: &mut usize,
) {
    check(
        &format!("{label}.model"),
        actual.result == expected.result && actual.result.is_ok(),
        failed,
    );
    check(
        &format!("{label}.diagnostics"),
        actual.diagnostics == expected.diagnostics,
        failed,
    );
    if let (Ok(left), Ok(right)) = (&actual.result, &expected.result) {
        check(
            &format!("{label}.ids_hashes"),
            identities(left) == identities(right),
            failed,
        );
    }
}

fn record(
    dir: Option<&Path>,
    label: &str,
    bytes: &[u8],
    state: &Probe<Drawing2D>,
) -> Result<(), String> {
    let Some(dir) = dir else {
        return Ok(());
    };
    let write = || -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(format!("{label}.bin")), bytes)?;
        let mut ids = io::BufWriter::new(std::fs::File::create(dir.join(format!("{label}.ids")))?);
        if let Ok(drawing) = &state.result {
            id::each_element(drawing, |_, name, element| {
                writeln!(ids, "{name}\t{:?}", element.hash())
            })?;
        }
        ids.flush()?;
        std::fs::write(
            dir.join(format!("{label}.diagnostics")),
            format!("{:?}\n", state.diagnostics),
        )?;
        std::fs::write(
            dir.join(format!("{label}.budget")),
            format!("{:?}\n", state.used),
        )?;
        Ok(())
    };
    write().map_err(|error| format!("検証記録の出力: {error}"))
}

fn compare(
    extension: &str,
    bytes: &[u8],
    threads: usize,
    decoded: u64,
    evidence: Option<&Path>,
) -> Result<(), String> {
    let mut failures = 0;
    let mut current = required(
        probe(decoded, |cx| Current::open(extension, bytes, cx)),
        "current.open",
    )?;
    let mut live = required(
        probe(decoded, |cx| Live::open(extension, bytes, threads, cx)),
        "live.open",
    )?;
    let before = probe(decoded, |cx| live.drawing(cx));
    let expected_before = probe(decoded, |cx| current.drawing(cx));
    same_model("initial", &before, &expected_before, &mut failures);
    let target = target(
        expected_before
            .result
            .as_ref()
            .map_err(|error| format!("drawing: {error:?}"))?,
    )?;
    let initial_save = required(probe(decoded, |cx| live.save(cx)), "initial.save")?;
    check("initial.bytes", initial_save == bytes, &mut failures);
    record(evidence, "initial", &initial_save, &before)?;
    drop(expected_before);
    let mut seen = target.seen;
    for step in 0..2 {
        let changes = Changes {
            document: None,
            edits: vec![Edit::Transform {
                target: seen,
                transform: Affine::translation(1.25, -0.75),
            }],
        };
        let expected = probe(decoded, |cx| current.apply(&changes, cx));
        let actual = probe(decoded, |cx| live.apply(&changes, cx));
        check(
            &format!("step{step}.apply"),
            actual.result == expected.result,
            &mut failures,
        );
        check(
            &format!("step{step}.apply_diagnostics"),
            actual.diagnostics == expected.diagnostics,
            &mut failures,
        );
        let applied = required(actual, "live.apply")?;
        black_box(applied);
        let applied = required(expected, "current.apply")?;
        let applied = applied.applied.first().ok_or("変更の結果がない")?;
        seen = Seen {
            id: applied.id.clone().ok_or("変更後の ID がない")?,
            hash: applied.hash.ok_or("変更後のハッシュがない")?,
        };
        let held = probe(decoded, |cx| live.drawing(cx));
        let expected_model = probe(decoded, |cx| current.drawing(cx));
        same_model(
            &format!("step{step}.held"),
            &held,
            &expected_model,
            &mut failures,
        );
        let saved = required(probe(decoded, |cx| live.save(cx)), "live.save")?;
        record(evidence, &format!("step{step}"), &saved, &held)?;
        let reference = required(probe(decoded, |cx| current.save(cx)), "current.save")?;
        check(
            &format!("step{step}.save"),
            saved == reference,
            &mut failures,
        );
        let reread = required(
            probe(decoded, |cx| Current::open(extension, &saved, cx)),
            "reread.open",
        )?;
        let reread = probe(decoded, |cx| reread.drawing(cx));
        same_model(&format!("step{step}.reread"), &held, &reread, &mut failures);
    }
    let after_save = required(probe(decoded, |cx| live.save(cx)), "after.save")?;
    let after_model = probe(decoded, |cx| live.drawing(cx));
    for _ in 0..2 {
        check(
            "undo.changed",
            required(probe(decoded, |cx| live.undo(cx)), "undo")?,
            &mut failures,
        );
    }
    let undone = probe(decoded, |cx| live.drawing(cx));
    let undone_save = required(probe(decoded, |cx| live.save(cx)), "undo.save")?;
    record(evidence, "undo", &undone_save, &undone)?;
    same_model("undo", &undone, &before, &mut failures);
    check("undo.save", undone_save == initial_save, &mut failures);
    drop(undone);
    drop(undone_save);
    drop(before);
    drop(initial_save);
    for _ in 0..2 {
        check(
            "redo.changed",
            required(probe(decoded, |cx| live.redo(cx)), "redo")?,
            &mut failures,
        );
    }
    let redone = probe(decoded, |cx| live.drawing(cx));
    let redone_save = required(probe(decoded, |cx| live.save(cx)), "redo.save")?;
    record(evidence, "redo", &redone_save, &redone)?;
    same_model("redo", &redone, &after_model, &mut failures);
    check("redo.save", redone_save == after_save, &mut failures);
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("等価の検査 {failures} 件が不一致"))
    }
}

fn report(name: &str, times: &mut [f64]) {
    times.sort_by(f64::total_cmp);
    println!(
        "time\t{name}\tmedian_us={:.3}\tmin_us={:.3}\tmax_us={:.3}\truns={}",
        times[times.len() / 2],
        times[0],
        times[times.len() - 1],
        times.len()
    );
}

fn measure(
    extension: &str,
    bytes: &[u8],
    threads: usize,
    runs: usize,
    decoded: u64,
) -> Result<(), String> {
    let file = required(
        probe(decoded, |cx| Current::open(extension, bytes, cx)),
        "current.open",
    )?;
    let drawing = required(probe(decoded, |cx| file.drawing(cx)), "current.drawing")?;
    let target = target(&drawing)?;
    drop(drawing);
    drop(file);
    let changes = Changes {
        document: None,
        edits: vec![Edit::Transform {
            target: target.seen.clone(),
            transform: Affine::translation(1.25, -0.75),
        }],
    };
    let mut open = Vec::new();
    let mut show = Vec::new();
    for _ in 0..runs {
        let opened = probe(decoded, |cx| Live::open(extension, bytes, threads, cx));
        open.push(opened.micros);
        drop(required(opened, "live.open")?);
        let shown = probe(decoded, |cx| {
            let file = Current::open(extension, bytes, cx)?;
            let drawing = file.drawing(cx)?;
            zumen_scene::build(
                &drawing,
                &zumen_scene::Options {
                    origin: target.origin,
                    tolerance: None,
                    print: false,
                    region: None,
                    palette: Default::default(),
                    chunking: zumen_scene::Chunking::PROVISIONAL,
                },
                cx,
            )
            .map_err(|(stop, _)| Error::Stopped(stop))
        });
        show.push(shown.micros);
        black_box(required(shown, "current.show")?);
    }
    report("live.open_display", &mut open);
    report("current.read_lift_scene", &mut show);
    println!(
        "ratio\tlive/current\t{:.6}",
        open[open.len() / 2] / show[show.len() / 2]
    );
    let mut live = required(
        probe(decoded, |cx| Live::open(extension, bytes, threads, cx)),
        "live.open",
    )?;
    println!("rows\t{}", live.live_count());
    let mut apply = Vec::new();
    let mut select = Vec::new();
    let mut undo = Vec::new();
    for _ in 0..runs {
        let selected = probe(decoded, |cx| live.select(&target, cx));
        select.push(selected.micros);
        let count = required(selected, "select")?;
        if count == 0 {
            return Err("選択の範囲に対象がない".to_owned());
        }
        black_box(count);
        let edited = probe(decoded, |cx| live.apply(&changes, cx));
        apply.push(edited.micros);
        black_box(required(edited, "apply")?);
        let undone = probe(decoded, |cx| live.undo(cx));
        undo.push(undone.micros);
        if !required(undone, "undo")? {
            return Err("取り消しの履歴がない".to_owned());
        }
    }
    report("commit", &mut apply);
    report("select", &mut select);
    report("undo", &mut undo);
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return Err("使い方: core-live compare|measure|open <公開または合成の図面> [--threads 数] [--runs 数] [--decoded-mib 数] [--evidence-dir フォルダー]".to_owned());
    }
    let mut threads = 1;
    let mut runs = 7;
    let mut decoded = 16 * 1024;
    let mut evidence = None;
    for option in args[2..].as_chunks::<2>().0 {
        match option[0].as_str() {
            "--threads" => threads = option[1].parse().map_err(|_| "threads は正の整数")?,
            "--runs" => runs = option[1].parse().map_err(|_| "runs は正の整数")?,
            "--decoded-mib" => decoded = option[1].parse().map_err(|_| "decoded-mib は正の整数")?,
            "--evidence-dir" => evidence = Some(std::path::PathBuf::from(&option[1])),
            _ => return Err(format!("知らない引数: {}", option[0])),
        }
    }
    if threads == 0 || runs == 0 || decoded == 0 {
        return Err("設定値は正の整数にする".to_owned());
    }
    let path = Path::new(&args[1]);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or("拡張子がない")?
        .to_ascii_lowercase();
    if !["dxf", "dwg", "jww", "sfc", "p21"].contains(&extension.as_str()) {
        return Err("対応する拡張子は dxf / dwg / jww / sfc / p21".to_owned());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    println!(
        "input\t{}\tbytes={}\tthreads={threads}\tdecoded_mib={decoded}",
        path.display(),
        bytes.len()
    );
    match args[0].as_str() {
        "compare" => {
            let evidence = evidence.map(|dir| dir.join(path.file_stem().unwrap_or_default()));
            compare(&extension, &bytes, threads, decoded, evidence.as_deref())
        }
        "measure" => measure(&extension, &bytes, threads, runs, decoded),
        "open" => {
            let live = required(
                probe(decoded, |cx| Live::open(&extension, &bytes, threads, cx)),
                "live.open",
            )?;
            drop(bytes);
            println!(
                "ready\tpid={}\trows={}",
                std::process::id(),
                live.live_count()
            );
            io::stdout().flush().map_err(|error| error.to_string())?;
            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .map_err(|error| error.to_string())?;
            black_box(live);
            Ok(())
        }
        _ => Err("操作は compare / measure / open".to_owned()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

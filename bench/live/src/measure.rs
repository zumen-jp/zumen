//! 文書エンジンの速さを測る。頼まれれば、今の実装の同じ操作も同じプロセスで測る。

use std::time::Instant;

use zumen_bench_live::born::{Add, ColorSpec, NewShape, NewSymbol, NewText, Restyle};
use zumen_bench_live::edit::Edit;
use zumen_bench_live::store::{self, Document, table};
use zumen_drawing::Affine;

use crate::random::Random;
use crate::today::Today;

/// 測り方
pub struct Options {
    /// 開くこと・保存・今の実装の操作を繰り返す回数
    pub runs: usize,
    /// 開くときのスレッドの数。なければ、1 本と、すべての論理 CPU で測る
    pub threads: Option<usize>,
    /// 今の実装も測る
    pub current: bool,
    /// 今の実装の要求に足す予算（JSON のオブジェクト）
    pub budget: Option<String>,
    /// 範囲で選ぶときの範囲。なければ、要素の数から決める
    pub region: Option<[f64; 4]>,
    /// 乱数の種
    pub seed: u64,
}

// 測定のツールなので、コアでは禁じている時計を使う（clippy.toml）。
#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e3
}

fn us(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e6
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(f64::NAN)
}

/// 「中央値（最小）」。ほかの仕事が動く端末では、最小のほうが、じゃまのない時間に近い
fn middle(values: &mut [f64]) -> String {
    let middle = median(values);
    let least = values.first().copied().unwrap_or(f64::NAN);
    format!("{middle:.2}（{least:.2}）")
}

/// 中央値・99% の値・最大の値
fn spread(values: &mut [f64]) -> [f64; 3] {
    values.sort_by(f64::total_cmp);
    let at = |part: f64| {
        let index = ((values.len() as f64 * part) as usize).min(values.len().saturating_sub(1));
        values.get(index).copied().unwrap_or(f64::NAN)
    };
    [at(0.5), at(0.99), at(1.0)]
}

fn report(name: &str, values: &mut [f64], declined: usize) {
    if values.is_empty() {
        println!("  {name}: 受け付けた回がない（受け付けない {declined} 回）");
        return;
    }
    let [middle, high, most] = spread(values);
    let declined = if declined == 0 {
        String::new()
    } else {
        format!("、受け付けない {declined} 回")
    };
    println!(
        "  {name}: 中央値 {middle:.2} µs、99% {high:.2} µs、最大 {most:.2} µs（{} 回{declined}）",
        values.len()
    );
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// 開くことを、段ごとに測る
fn opening(bytes: &[u8], threads: usize, runs: usize) -> Result<(), String> {
    let mut phases: [Vec<f64>; 6] = Default::default();
    for _ in 0..runs {
        let src = bytes.to_vec();
        let start = now();
        let parts = store::scan(&src, threads).map_err(|error| format!("開けない: {error:?}"))?;
        let scan = ms(start);
        let start = now();
        let mut document =
            Document::assemble(src, parts).map_err(|error| format!("開けない: {error:?}"))?;
        let assemble = ms(start);
        let start = now();
        document.build_display();
        let display = ms(start);
        let start = now();
        document.build_index();
        let index = ms(start);
        let shown = scan + assemble + display;
        for (phase, value) in
            phases
                .iter_mut()
                .zip([scan, assemble, display, index, shown, shown + index])
        {
            phase.push(value);
        }
    }
    let [scan, assemble, display, index, shown, ready] = phases.map(|mut phase| middle(&mut phase));
    println!(
        "  開く（{threads} スレッド、ms、中央値と最小、{runs} 回）: 読んで列にする {scan}、まとめる {assemble}、表示のバッファー {display}、索引 {index} ／ 表示まで {shown}、選べるまで {ready}"
    );
    Ok(())
}

/// 範囲で選ぶことを測る。1 回あたりのマイクロ秒（まとめて測った平均の中央値）と件数
fn selecting(document: &Document, rect: [f64; 4]) -> (f64, usize) {
    let mut found = Vec::new();
    document.select(rect, &mut found);
    let hits = found.len();
    // 件数が多いほど、1 回が長い。まとめる回数を減らす
    let batch = (200_000 / hits.max(1)).clamp(1, 1000);
    let mut times = Vec::with_capacity(9);
    for _ in 0..9 {
        let start = now();
        for _ in 0..batch {
            found.clear();
            document.select(rect, &mut found);
        }
        times.push(us(start) / batch as f64);
    }
    (median(&mut times), hits)
}

/// 今の実装の同じ操作を測る
fn current(
    bytes: &[u8],
    document: &Document,
    region: [f64; 4],
    options: &Options,
) -> Result<(), String> {
    let budget = options.budget.as_deref();
    let failed = |what: &str| {
        let what = what.to_owned();
        move |failure: crate::today::Failure| {
            format!("今の実装の{what}が失敗: {}", failure.describe())
        }
    };
    let mut show = Vec::with_capacity(options.runs);
    for _ in 0..options.runs {
        let mut today = Today::start(bytes, budget).map_err(failed("準備"))?;
        let start = now();
        today.open().map_err(failed("開くこと"))?;
        today.scene().map_err(failed("表示"))?;
        show.push(ms(start));
    }
    println!(
        "  今の実装（ms、中央値と最小、{} 回）: 表示まで {}",
        options.runs,
        middle(&mut show)
    );

    let mut today = Today::start(bytes, budget).map_err(failed("準備"))?;
    today.open().map_err(failed("開くこと"))?;
    today.scene().map_err(failed("表示"))?;
    // 文書エンジンも持つ要素を 1 つ選ぶ
    let target = today
        .first(64)
        .map_err(failed("要素を返すこと"))?
        .into_iter()
        .find(|element| {
            element
                .id
                .strip_prefix("space/0/h")
                .and_then(|hex| u64::from_str_radix(hex, 16).ok())
                .is_some_and(|handle| document.row_of(handle).is_some())
        });
    let Some(target) = target else {
        println!("  今の実装: 動かす要素を選べない");
        return Ok(());
    };
    let mut hash = target.hash.clone();
    let (mut apply, mut scene) = (Vec::new(), Vec::new());
    for _ in 0..options.runs {
        let moves = [(
            target.id.clone(),
            hash.clone(),
            [1.0, 0.0, 5.0, 0.0, 1.0, 7.0],
        )];
        let start = now();
        let applied = today.apply(&moves).map_err(failed("変更"))?;
        apply.push(ms(start));
        if let Some((_, next)) = applied.hashes.into_iter().next() {
            hash = next;
        }
        let start = now();
        today.scene().map_err(failed("表示"))?;
        scene.push(ms(start));
    }
    // 属性を変える・足す・足した要素を除く。どれも、変更のたびに全体を書き戻す
    let (mut restyle, mut add, mut remove) = (Vec::new(), Vec::new(), Vec::new());
    for index in 0..options.runs {
        let edit = format!(
            r#"{{"kind":"restyle","id":"{}","hash":"{hash}","color":{{"aci":{}}}}}"#,
            target.id,
            1 + index % 6
        );
        let start = now();
        let applied = today.edit(&[edit]).map_err(failed("属性の変更"))?;
        restyle.push(ms(start));
        if let Some((_, next)) = applied.hashes.into_iter().next() {
            hash = next;
        }
        let edit = format!(
            r#"{{"kind":"add","place":"space/0","geometry":{{"shape":"line","start":[{},{}],"end":[{},{}]}}}}"#,
            region[0], region[1], region[2], region[3]
        );
        let start = now();
        let applied = today.edit(&[edit]).map_err(failed("足す変更"))?;
        add.push(ms(start));
        let Some((id, made)) = applied.hashes.into_iter().next() else {
            continue;
        };
        let edit = format!(r#"{{"kind":"remove","id":"{id}","hash":"{made}"}}"#);
        let start = now();
        today.edit(&[edit]).map_err(failed("除く変更"))?;
        remove.push(ms(start));
    }
    let mut select = Vec::new();
    let mut hits = 0;
    for _ in 0..options.runs {
        let start = now();
        hits = today.select(region).map_err(failed("範囲で選ぶこと"))?;
        select.push(ms(start));
    }
    let mut undo = Vec::new();
    for _ in 0..options.runs {
        let name = today.stage(bytes).map_err(failed("準備"))?;
        let start = now();
        today.reopen(&name).map_err(failed("開き直し"))?;
        today.scene().map_err(failed("表示"))?;
        undo.push(ms(start));
    }
    let next = median(&mut apply) + median(&mut scene);
    println!(
        "  今の実装（ms、中央値と最小、{} 回）: 1 件を動かして書き戻す {}、表示し直す {}、続けての操作 {next:.2}、範囲で選ぶ {}（{hits} 件）、取り消し（開き直して表示） {}",
        options.runs,
        middle(&mut apply),
        middle(&mut scene),
        middle(&mut select),
        middle(&mut undo)
    );
    println!(
        "  今の実装（ms、中央値と最小、{} 回）: 1 件の色を変えて書き戻す {}、1 件の線を足して書き戻す {}、足した 1 件を除いて書き戻す {}",
        options.runs,
        middle(&mut restyle),
        middle(&mut add),
        middle(&mut remove)
    );
    Ok(())
}

/// 取り消しとやり直しの時間を分けて出す、変更の種類
const KINDS: [&str; 7] = [
    "1 件を動かす変更",
    "まとめて動かす変更",
    "1 件の属性を変える変更",
    "1 件を足す変更",
    "まとめて足す変更",
    "1 件を除く変更",
    "まとめて除く変更",
];

/// 受け付けた変更の種類を、取り消しの列と同じ順に覚える
struct Bench {
    /// 取り消しの列に積んだ変更の種類（`KINDS` の番号）
    pushed: Vec<usize>,
}

impl Bench {
    /// 変更を当てて、時間を測る。受け付ければ、変更ごとの、足した要素のハンドルを返す
    fn step(
        &mut self,
        document: &mut Document,
        kind: usize,
        edits: &[Edit],
        times: &mut Vec<f64>,
    ) -> Option<Vec<Option<u64>>> {
        let start = now();
        let done = document.apply(edits);
        let elapsed = us(start);
        let handles = done.ok()?;
        times.push(elapsed);
        self.pushed.push(kind);
        Some(handles)
    }
}

/// 属性を付けずに足す要素
fn plain(shape: NewShape) -> Edit {
    Edit::Add(Box::new(Add {
        layer: None,
        color: None,
        line_type: None,
        line_weight: None,
        shape,
    }))
}

/// 全体の保存を測る
fn saving(document: &Document, runs: usize, when: &str) {
    let mut times = Vec::with_capacity(runs);
    let mut saved = 0;
    for _ in 0..runs {
        let start = now();
        let out = document.save();
        times.push(ms(start));
        saved = out.len();
    }
    println!(
        "  全体を保存する（{when}）: 中央値 {:.2} ms（{saved} バイト）",
        median(&mut times)
    );
}

/// 変更・保存・取り消し・やり直しを測る
fn editing(document: &mut Document, handles: &[u64], extent: [f64; 4], options: &Options) {
    let [x0, y0, x1, y1] = extent;
    let span = (x1 - x0).max(y1 - y0).max(1.0);
    let mut random = Random(options.seed);
    let mut bench = Bench { pushed: Vec::new() };
    let shift = |random: &mut Random| {
        [
            1.0,
            0.0,
            random.between(-span, span) / 64.0,
            0.0,
            1.0,
            random.between(-span, span) / 64.0,
        ]
    };
    let pick = |random: &mut Random| {
        handles
            .get(random.below(handles.len()))
            .copied()
            .unwrap_or(0)
    };
    let point = |random: &mut Random| [random.between(x0, x1), random.between(y0, y1)];
    let group = handles.len().min(100);

    // 動かす
    let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
    for _ in 0..2000 {
        let edit = Edit::Transform {
            handle: pick(&mut random),
            matrix: shift(&mut random),
        };
        declined += usize::from(bench.step(document, 0, &[edit], &mut times).is_none());
    }
    report("1 件を動かす（毎回、別の要素）", &mut times, declined);

    let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
    let handle = pick(&mut random);
    for _ in 0..2000 {
        let edit = Edit::Transform {
            handle,
            matrix: [1.0, 0.0, span / 4096.0, 0.0, 1.0, span / 8192.0],
        };
        declined += usize::from(bench.step(document, 0, &[edit], &mut times).is_none());
    }
    report("1 件を動かす（同じ要素を続けて）", &mut times, declined);

    let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
    for _ in 0..2000 {
        let [x, y] = point(&mut random);
        let edit = Edit::Transform {
            handle: pick(&mut random),
            matrix: Affine::rotation(x, y, 90.0).m,
        };
        declined += usize::from(bench.step(document, 0, &[edit], &mut times).is_none());
    }
    report("1 件を回す", &mut times, declined);

    let (mut times, mut declined) = (Vec::with_capacity(200), 0);
    for _ in 0..200 {
        // 続いた 100 件を、同じだけ動かす
        let first = random.below(handles.len() - group + 1);
        let matrix = shift(&mut random);
        let edits: Vec<Edit> = handles
            .iter()
            .skip(first)
            .take(group)
            .map(|&handle| Edit::Transform { handle, matrix })
            .collect();
        declined += usize::from(bench.step(document, 1, &edits, &mut times).is_none());
    }
    report(&format!("{group} 件を 1 回で動かす"), &mut times, declined);

    // 属性を変える
    let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
    for index in 0..2000usize {
        let style = Restyle {
            color: Some(ColorSpec::Aci(1 + (index % 200) as u8)),
            ..Restyle::default()
        };
        let edit = Edit::Restyle {
            handle: pick(&mut random),
            style,
        };
        declined += usize::from(bench.step(document, 2, &[edit], &mut times).is_none());
    }
    report("1 件の色を変える", &mut times, declined);

    // 表にちょうど 1 つある画層の名前を、2 つまで使う
    let names = document.table_names(table::LAYER);
    let layers: Vec<&String> = names
        .iter()
        .filter(|name| {
            names
                .iter()
                .filter(|other| other.eq_ignore_ascii_case(name))
                .count()
                == 1
        })
        .take(2)
        .collect();
    if layers.is_empty() {
        println!("  1 件の画層を変える: 表に、指せる画層がない");
    } else {
        let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
        for index in 0..2000usize {
            let style = Restyle {
                layer: layers.get(index % layers.len()).map(|name| (*name).clone()),
                ..Restyle::default()
            };
            let edit = Edit::Restyle {
                handle: pick(&mut random),
                style,
            };
            declined += usize::from(bench.step(document, 2, &[edit], &mut times).is_none());
        }
        report("1 件の画層を変える", &mut times, declined);
    }

    println!(
        "  動かす・属性を変える変更の後: 変えたレコード {}、取り消せる変更 {} 回、変えたレコードと取り消しの列のメモリ {:.2} MiB",
        document.edited_records(),
        document.undo_len(),
        mib(document.edit_bytes())
    );
    saving(document, options.runs, "動かす・属性を変える変更の後");

    // 足す。線は、図面の幅の 64 分の 1 までの長さ
    let line = |random: &mut Random| {
        let [x, y] = point(random);
        let reach = span / 64.0;
        plain(NewShape::Line {
            start: [x, y],
            end: [
                x + random.between(-reach, reach),
                y + random.between(-reach, reach),
            ],
        })
    };
    let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
    for _ in 0..2000 {
        let edit = line(&mut random);
        declined += usize::from(bench.step(document, 3, &[edit], &mut times).is_none());
    }
    report("1 件の線を足す", &mut times, declined);

    let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
    for _ in 0..2000 {
        let edit = plain(NewShape::Text(NewText {
            content: "A-101".to_owned(),
            position: point(&mut random),
            end: None,
            horizontal: None,
            vertical: None,
            height: span / 400.0,
            rotation: 0.0,
            width_factor: None,
            oblique: None,
            backward: false,
            upside_down: false,
            style: None,
        }));
        declined += usize::from(bench.step(document, 3, &[edit], &mut times).is_none());
    }
    report("1 件の文字を足す", &mut times, declined);

    let definition = document
        .definitions
        .iter()
        .find(|definition| definition.usable)
        .map(|definition| String::from_utf8_lossy(&definition.name).into_owned());
    match definition {
        None => println!("  1 件の記号を足す: 指せるブロックの定義がない"),
        Some(name) => {
            let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
            for _ in 0..2000 {
                let edit = plain(NewShape::Symbol(NewSymbol {
                    name: name.clone(),
                    position: point(&mut random),
                    scale: [1.0, 1.0],
                    rotation: 0.0,
                    grid: None,
                }));
                declined += usize::from(bench.step(document, 3, &[edit], &mut times).is_none());
            }
            report("1 件の記号を足す", &mut times, declined);
        }
    }

    let (mut times, mut declined) = (Vec::with_capacity(200), 0);
    for _ in 0..200 {
        let edits: Vec<Edit> = (0..group).map(|_| line(&mut random)).collect();
        declined += usize::from(bench.step(document, 4, &edits, &mut times).is_none());
    }
    report(
        &format!("{group} 件の線を 1 回で足す"),
        &mut times,
        declined,
    );

    // 除く。元の要素の 8 分の 1 までを、3 通りに分けて除く
    let count = (handles.len() / 8).min(2000);
    // ハンドルの大きい順
    let mut sorted: Vec<u64> = handles.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let (top, rest) = sorted.split_at(count.min(sorted.len()));
    // 残りから、乱数で選ぶ
    let mut pool: Vec<u64> = rest.to_vec();
    let batches = (handles.len() / 8 / group.max(1)).min(200);
    let wanted = (count + batches * group).min(pool.len());
    for at in 0..wanted {
        let other = at + random.below(pool.len() - at);
        pool.swap(at, other);
    }
    let (singles, many) = pool.split_at(count.min(pool.len()));

    let (mut times, mut declined) = (Vec::with_capacity(count), 0);
    for &handle in singles {
        let edit = Edit::Remove { handle };
        declined += usize::from(bench.step(document, 5, &[edit], &mut times).is_none());
    }
    report("1 件を除く（毎回、別の要素）", &mut times, declined);

    // ハンドルの大きい元の要素を、小さい順に除く。最大の 1 件は残す
    let (largest, below) = match top.split_first() {
        Some((largest, below)) => (Some(*largest), below),
        None => (None, top),
    };
    let (mut times, mut declined) = (Vec::with_capacity(count), 0);
    for &handle in below.iter().rev() {
        let edit = Edit::Remove { handle };
        declined += usize::from(bench.step(document, 5, &[edit], &mut times).is_none());
    }
    report(
        "1 件を除く（ハンドルの大きい元の要素を、小さい順に）",
        &mut times,
        declined,
    );
    // 残した最大の 1 件を除く。すぐ下の要素は除いてあるので、生きている要素のハンドルの最大を、下へ探す
    if let Some(handle) = largest {
        let edit = [Edit::Remove { handle }];
        let mut walk = Vec::with_capacity(9);
        for _ in 0..8 {
            let start = now();
            let done = document.apply(&edit).is_ok();
            let elapsed = us(start);
            if done {
                walk.push(elapsed);
                document.undo();
            }
        }
        let declined = usize::from(bench.step(document, 5, &edit, &mut walk).is_none());
        report(
            &format!(
                "1 件を除く（ハンドルが最大の元の要素。すぐ下の {} 件は除いてある）",
                times.len()
            ),
            &mut walk,
            declined,
        );
    }

    let (mut times, mut declined) = (Vec::with_capacity(batches), 0);
    for chunk in many.chunks(group.max(1)).take(batches) {
        let edits: Vec<Edit> = chunk
            .iter()
            .map(|&handle| Edit::Remove { handle })
            .collect();
        declined += usize::from(bench.step(document, 6, &edits, &mut times).is_none());
    }
    report(&format!("{group} 件を 1 回で除く"), &mut times, declined);

    println!(
        "  足す・除く変更の後: 変えたレコード {}、行 {}（うち、除いて場所を残したもの {}）、取り消せる変更 {} 回、変えたレコードと取り消しの列のメモリ {:.2} MiB",
        document.edited_records(),
        document.rows.len(),
        document.gone_rows(),
        document.undo_len(),
        mib(document.edit_bytes())
    );
    saving(document, options.runs, "足す・除く変更の後");

    // 取り消しとやり直し。後ろから戻す
    let mut undone: [Vec<f64>; 7] = Default::default();
    for &kind in bench.pushed.iter().rev() {
        let start = now();
        let done = document.undo();
        let elapsed = us(start);
        if done && let Some(times) = undone.get_mut(kind) {
            times.push(elapsed);
        }
    }
    for (name, times) in KINDS.iter().zip(&mut undone) {
        if !times.is_empty() {
            report(&format!("取り消す（{name}）"), times, 0);
        }
    }
    let mut redone: [Vec<f64>; 7] = Default::default();
    for &kind in &bench.pushed {
        let start = now();
        let done = document.redo();
        let elapsed = us(start);
        if done && let Some(times) = redone.get_mut(kind) {
            times.push(elapsed);
        }
    }
    for (name, times) in KINDS.iter().zip(&mut redone) {
        if !times.is_empty() {
            report(&format!("やり直す（{name}）"), times, 0);
        }
    }
}

/// 1 つの入力を測る
pub fn run(path: &str, options: &Options) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path} を読めない: {error}"))?;
    println!("== {path}（{} バイト）", bytes.len());
    let all = std::thread::available_parallelism().map_or(1, usize::from);
    let thread_counts = match options.threads {
        Some(threads) => vec![threads.max(1)],
        None if all > 1 => vec![1, all],
        None => vec![1],
    };
    for &threads in &thread_counts {
        opening(&bytes, threads, options.runs)?;
    }
    let threads = thread_counts.last().copied().unwrap_or(1);
    let mut document =
        Document::open(bytes.clone(), threads).map_err(|error| format!("開けない: {error:?}"))?;
    let counts = document.counts;
    println!(
        "  要素 {}（指せない {}）、ペーパー空間 {}、Shift_JIS {}。メモリは、列・表示・索引 {:.1} MiB と、元のファイル {:.1} MiB",
        counts.elements,
        counts.unaddressable,
        counts.paper,
        counts.shift_jis,
        mib(document.resident_bytes()),
        mib(document.source_len())
    );
    let handles: Vec<u64> = document
        .rows
        .iter()
        .filter(|row| row.handle != 0)
        .map(|row| row.handle)
        .collect();
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for item in &document.boxes {
        if item.iter().all(|value| value.is_finite()) {
            extent = [
                extent[0].min(item[0]),
                extent[1].min(item[1]),
                extent[2].max(item[2]),
                extent[3].max(item[3]),
            ];
        }
    }
    if extent.iter().any(|value| !value.is_finite()) {
        extent = [0.0, 0.0, 1.0, 1.0];
    }
    let [x0, y0, x1, y1] = extent;
    let span = (x1 - x0).max(y1 - y0).max(1.0);
    // bench/editor の入力では、今の実装の測定と同じ範囲を選ぶ
    let region = options
        .region
        .or(match counts.elements {
            1_002 => Some([98.0, 1.0, 98.2, 1.2]),
            10_002 => Some([198.0, 12.0, 198.2, 12.2]),
            100_002 => Some([198.0, 124.5, 198.2, 124.7]),
            _ => None,
        })
        .unwrap_or_else(|| {
            let item = document
                .boxes
                .get(document.boxes.len() * 3 / 4)
                .copied()
                .unwrap_or(extent);
            let (x, y) = ((item[0] + item[2]) / 2.0, (item[1] + item[3]) / 2.0);
            let reach = span * 0.0005;
            [x - reach, y - reach, x + reach, y + reach]
        });

    // 範囲で選ぶ。変更の前（今の実装の測定と、同じ文書）
    let (time, hits) = selecting(&document, region);
    println!(
        "  範囲で選ぶ（開いた直後、今の実装の測定と同じ小さい範囲）: {time:.3} µs（{hits} 件）"
    );

    if let Some(locked) = document.locked {
        println!("  この文書の変更は受け付けない: {locked:?}");
    } else if !handles.is_empty() {
        editing(&mut document, &handles, extent, options);
    }

    // 範囲で選ぶ。変更の後
    let (time, hits) = selecting(&document, region);
    println!("  範囲で選ぶ（変更の後、同じ小さい範囲）: {time:.3} µs（{hits} 件）");
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (w, h) = ((x1 - x0) * 0.05, (y1 - y0) * 0.05);
    let (time, hits) = selecting(&document, [cx - w, cy - h, cx + w, cy + h]);
    println!("  範囲で選ぶ（図面の縦横 1/10 の範囲）: {time:.3} µs（{hits} 件）");
    let (time, hits) = selecting(&document, extent);
    println!("  範囲で選ぶ（図面の全体）: {time:.3} µs（{hits} 件）");

    if options.current {
        current(&bytes, &document, region, options)?;
    }
    Ok(())
}

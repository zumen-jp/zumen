//! 文書エンジンの速さを測る。頼まれれば、今の実装の同じ操作も同じプロセスで測る。

use std::time::Instant;

use zumen_bench_live::edit::Edit;
use zumen_bench_live::store::{self, Document};
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
    Ok(())
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
        let mut random = Random(options.seed);
        // 取り消しの列に積んだ変更の、要素の数
        let mut pushed: Vec<usize> = Vec::new();
        let mut step = |document: &mut Document, edits: &[Edit], times: &mut Vec<f64>| -> bool {
            let start = now();
            let done = document.apply(edits).is_ok();
            let elapsed = us(start);
            if done {
                times.push(elapsed);
                pushed.push(edits.len());
            }
            done
        };
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

        let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
        for _ in 0..2000 {
            let edit = Edit {
                handle: pick(&mut random),
                matrix: shift(&mut random),
            };
            declined += usize::from(!step(&mut document, &[edit], &mut times));
        }
        report("1 件を動かす（毎回、別の要素）", &mut times, declined);

        let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
        let handle = pick(&mut random);
        for _ in 0..2000 {
            let edit = Edit {
                handle,
                matrix: [1.0, 0.0, span / 4096.0, 0.0, 1.0, span / 8192.0],
            };
            declined += usize::from(!step(&mut document, &[edit], &mut times));
        }
        report("1 件を動かす（同じ要素を続けて）", &mut times, declined);

        let (mut times, mut declined) = (Vec::with_capacity(2000), 0);
        for _ in 0..2000 {
            let x = random.between(x0, x1);
            let y = random.between(y0, y1);
            let edit = Edit {
                handle: pick(&mut random),
                matrix: Affine::rotation(x, y, 90.0).m,
            };
            declined += usize::from(!step(&mut document, &[edit], &mut times));
        }
        report("1 件を回す", &mut times, declined);

        let (mut times, mut declined) = (Vec::with_capacity(200), 0);
        let group = handles.len().min(100);
        for _ in 0..200 {
            // 続いた 100 件を、同じだけ動かす
            let first = random.below(handles.len() - group + 1);
            let matrix = shift(&mut random);
            let edits: Vec<Edit> = handles
                .iter()
                .skip(first)
                .take(group)
                .map(|&handle| Edit { handle, matrix })
                .collect();
            declined += usize::from(!step(&mut document, &edits, &mut times));
        }
        report(&format!("{group} 件を 1 回で動かす"), &mut times, declined);
        println!(
            "  変更の後: 変えたレコード {}、取り消せる変更 {} 回、変えたレコードと取り消しの列のメモリ {:.2} MiB",
            document.edited_records(),
            document.undo_len(),
            mib(document.edit_bytes())
        );

        // 保存
        let mut times = Vec::with_capacity(options.runs);
        let mut saved = 0;
        for _ in 0..options.runs {
            let start = now();
            let out = document.save();
            times.push(ms(start));
            saved = out.len();
        }
        println!(
            "  全体を保存する: 中央値 {:.2} ms（{saved} バイト）",
            median(&mut times)
        );

        // 取り消しとやり直し。後ろから戻す
        let (mut single, mut many) = (Vec::new(), Vec::new());
        for &size in pushed.iter().rev() {
            let start = now();
            let done = document.undo();
            let elapsed = us(start);
            if done {
                if size == 1 { &mut single } else { &mut many }.push(elapsed);
            }
        }
        report("取り消す（1 件の変更）", &mut single, 0);
        report(&format!("取り消す（{group} 件の変更）"), &mut many, 0);
        let (mut single, mut many) = (Vec::new(), Vec::new());
        for &size in &pushed {
            let start = now();
            let done = document.redo();
            let elapsed = us(start);
            if done {
                if size == 1 { &mut single } else { &mut many }.push(elapsed);
            }
        }
        report("やり直す（1 件の変更）", &mut single, 0);
        report(&format!("やり直す（{group} 件の変更）"), &mut many, 0);
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

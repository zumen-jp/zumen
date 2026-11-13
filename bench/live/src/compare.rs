//! 文書エンジンと今の実装に、同じ変更を順に流して比べる。
//!
//! 変更は、種を決めた乱数で作る（平行移動・回転・拡大と縮小・鏡映・相似でない変換・恒等・逆のない変換、
//! 1 件から 100 件、ない ID、同じ要素を 2 度）。1 つ流すごとに、受け付けるかどうかと、保存したバイト列を比べる。
//! 文書エンジンの中では、取り消しとやり直しのバイト列、開き直した文書との一致、範囲で選ぶ結果も確かめる。

use std::collections::{BTreeMap, BTreeSet};

use zumen_bench_live::edit::{Decline, Edit};
use zumen_bench_live::lift::Kind;
use zumen_bench_live::store::{self, Document, Locked};
use zumen_drawing::Affine;

use crate::random::Random;
use crate::today::{Failure, Move, Today};

/// 比べ方
pub struct Options {
    /// 流す変更の数。なければ、要素の数から決める
    pub steps: Option<usize>,
    /// 乱数の種
    pub seed: u64,
    /// 文書エンジンが開くときのスレッドの数
    pub threads: usize,
    /// 違いの前後のバイト列を出す（図面の中身が出る。公開できる入力だけで使う）
    pub show: bool,
    /// 今の実装の要求に足す予算（JSON のオブジェクト）
    pub budget: Option<String>,
}

/// `a` の前に `b` を行う変換
fn compose(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[1] * b[3],
        a[0] * b[1] + a[1] * b[4],
        a[0] * b[2] + a[1] * b[5] + a[2],
        a[3] * b[0] + a[4] * b[3],
        a[3] * b[1] + a[4] * b[4],
        a[3] * b[2] + a[4] * b[5] + a[5],
    ]
}

fn id_of(handle: u64) -> String {
    format!("space/0/h{handle:X}")
}

/// モデル空間の、ハンドルで指す要素の ID からハンドルを読む
fn handle_of(id: &str) -> Option<u64> {
    let hex = id.strip_prefix("space/0/h")?;
    let handle = u64::from_str_radix(hex, 16).ok()?;
    (id_of(handle) == id).then_some(handle)
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Line => "LINE",
        Kind::Circle => "CIRCLE",
        Kind::Arc => "ARC",
        Kind::Polyline => "LWPOLYLINE",
        Kind::Text => "TEXT",
    }
}

/// 2 つのバイト列の、最初の違い
fn difference(ours: &[u8], theirs: &[u8], show: bool) -> String {
    let at = ours
        .iter()
        .zip(theirs)
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(theirs.len()));
    let mut text = format!(
        "最初の違いは {at} バイト目（文書エンジン {} バイト、今の実装 {} バイト）",
        ours.len(),
        theirs.len()
    );
    if show {
        let window = |bytes: &[u8]| {
            let part = bytes
                .get(at.saturating_sub(200)..(at + 200).min(bytes.len()))
                .unwrap_or_default();
            String::from_utf8_lossy(part)
                .replace("\r\n", "|")
                .replace('\n', "|")
        };
        text += &format!(
            "\n    文書エンジン: {}\n    今の実装    : {}",
            window(ours),
            window(theirs)
        );
    }
    text
}

/// 数えたもの
#[derive(Default)]
struct Tally {
    steps: usize,
    /// 両方が受け付け、バイト列が同じ
    equal: usize,
    /// 両方が受け付けない。文書エンジンの理由と、今の実装の診断ごと
    declined: BTreeMap<String, usize>,
    /// 今の実装だけが受け付けた。この試作が扱わないと決めているもの
    gaps: BTreeMap<String, usize>,
    /// 違い
    bugs: usize,
    /// 流した変更の種類
    labels: BTreeMap<&'static str, [usize; 2]>,
    /// 動かした要素の種類
    kinds: BTreeMap<&'static str, usize>,
    undo_checks: usize,
    deep_checks: usize,
    /// 開き直した文書と、表示のバッファーが、並びまで同じだった回数
    same_buffers: usize,
    select_checks: usize,
    resyncs: usize,
}

struct Run<'a> {
    options: &'a Options,
    today: Today,
    document: Document,
    /// 今の文書のバイト列（今の実装が返したもの）
    current: Vec<u8>,
    /// 文書エンジンの取り消しの列の、始まりのバイト列
    base: Vec<u8>,
    /// 要素のハッシュ（今の実装のもの）
    hashes: BTreeMap<String, String>,
    /// 両方が同じ ID で持つ要素のハンドル
    covered: Vec<u64>,
    /// どちらにもないハンドル
    unknown: u64,
    extent: [f64; 4],
    /// 直前の平行移動
    last_move: Option<(Vec<u64>, [f64; 2])>,
    random: Random,
    tally: Tally,
}

impl Run<'_> {
    fn bug(&mut self, step: usize, label: &str, text: String) {
        self.tally.bugs += 1;
        if self.tally.bugs <= 12 {
            println!("  違い（{step} 番目・{label}）: {text}");
        }
    }

    /// 文書エンジンを、今の実装のバイト列から開き直す
    fn resync(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.document = Document::open(bytes.to_vec(), self.options.threads)
            .map_err(|error| format!("今の実装が書いたファイルを開けない: {error:?}"))?;
        self.base = bytes.to_vec();
        let document = &self.document;
        self.covered
            .retain(|handle| document.row_of(*handle).is_some());
        self.tally.resyncs += 1;
        Ok(())
    }

    fn learn(&mut self, hashes: Vec<(String, String)>) {
        for (id, hash) in hashes {
            self.hashes.insert(id, hash);
        }
    }

    /// 変換を 1 つ作る
    fn matrix(&mut self) -> (&'static str, [f64; 6]) {
        let [x0, y0, x1, y1] = self.extent;
        let span = (x1 - x0).max(y1 - y0).max(1.0);
        let random = &mut self.random;
        let x = random.between(x0, x1);
        let y = random.between(y0, y1);
        let (x, y) = (random.rounded(x), random.rounded(y));
        let about = |m: [f64; 4]| {
            [
                m[0],
                m[1],
                x - m[0] * x - m[1] * y,
                m[2],
                m[3],
                y - m[2] * x - m[3] * y,
            ]
        };
        match random.below(100) {
            0..=36 => {
                let dx = random.between(-span, span) / 8.0;
                let dy = random.between(-span, span) / 8.0;
                let (dx, dy) = (random.rounded(dx), random.rounded(dy));
                ("平行移動", [1.0, 0.0, dx, 0.0, 1.0, dy])
            }
            37..=39 => {
                let dx = random.between(-1.0, 1.0) * 1e-7;
                let dy = random.between(-1.0, 1.0) * 1e9;
                ("平行移動", [1.0, 0.0, dx, 0.0, 1.0, dy])
            }
            40..=54 => {
                let turn = random.between(0.0, 360.0);
                let degrees = random.pick(&[90.0, 180.0, 270.0, -90.0, 45.0, 30.0, turn]);
                ("回転", Affine::rotation(x, y, degrees).m)
            }
            55..=64 => {
                let free = random.between(0.25, 4.0);
                let scale = random.pick(&[2.0, 0.5, 1.5, 0.75, 3.0, 1.0 / 3.0, free]);
                ("拡大と縮小", about([scale, 0.0, 0.0, scale]))
            }
            65..=72 => match random.below(3) {
                0 => ("鏡映", about([-1.0, 0.0, 0.0, 1.0])),
                1 => ("鏡映", about([1.0, 0.0, 0.0, -1.0])),
                _ => ("鏡映", about([0.0, 1.0, 1.0, 0.0])),
            },
            73..=79 => {
                let degrees = random.between(0.0, 360.0);
                let scale = random.between(0.5, 2.0);
                let dx = random.between(-span, span) / 8.0;
                let turn = Affine::rotation(x, y, degrees).m;
                let grow = about([scale, 0.0, 0.0, scale]);
                let shift = [1.0, 0.0, dx, 0.0, 1.0, -dx];
                ("相似", compose(shift, compose(turn, grow)))
            }
            80..=86 => {
                let sx = random.pick(&[2.0, 0.5, 1.0, 1.25]);
                let sy = random.pick(&[3.0, 0.25, 1.5, 0.8]);
                ("縦横で違う拡大", about([sx, 0.0, 0.0, sy]))
            }
            87..=90 => {
                let slope = random.pick(&[0.5, -0.25, 1.0]);
                ("せん断", about([1.0, slope, 0.0, 1.0]))
            }
            91..=95 => ("恒等", [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            _ => match random.below(3) {
                0 => ("逆のない変換", [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                1 => ("逆のない変換", [0.0; 6]),
                _ => ("逆のない変換", [1.0, 2.0, 3.0, 2.0, 4.0, 5.0]),
            },
        }
    }

    /// 変更を 1 つ作る。指す要素のハンドルと、要素ごとの行列
    fn draw(&mut self) -> (&'static str, Vec<u64>, Vec<[f64; 6]>) {
        if let Some((targets, [dx, dy])) = self.last_move.take()
            && self.random.below(100) < 30
        {
            let back = [1.0, 0.0, -dx, 0.0, 1.0, -dy];
            let matrices = vec![back; targets.len()];
            return ("平行移動を戻す", targets, matrices);
        }
        let count = match self.random.below(100) {
            0..=77 => 1,
            78..=92 => 2 + self.random.below(7),
            _ => 9 + self.random.below(92),
        }
        .min(self.covered.len());
        let mut chosen = BTreeSet::new();
        let mut targets = Vec::with_capacity(count + 1);
        while targets.len() < count {
            let at = self.random.below(self.covered.len());
            if chosen.insert(at)
                && let Some(handle) = self.covered.get(at)
            {
                targets.push(*handle);
            }
        }
        let (mut label, first) = self.matrix();
        let mut matrices = vec![first; targets.len()];
        if count > 1 && self.random.below(100) < 30 {
            for matrix in &mut matrices {
                *matrix = self.matrix().1;
            }
            label = "要素ごとに違う変換";
        }
        match self.random.below(100) {
            0..=1 => {
                targets.push(self.unknown);
                matrices.push(first);
                label = "ない ID";
            }
            2..=3 => {
                if let Some(again) = targets.first().copied() {
                    targets.push(again);
                    matrices.push(first);
                    label = "同じ要素を 2 度";
                }
            }
            _ => {}
        }
        if label == "平行移動" {
            self.last_move = Some((targets.clone(), [first[2], first[5]]));
        }
        (label, targets, matrices)
    }

    /// 取り消しとやり直しのバイト列を確かめる。`expected` は、変更の後のバイト列
    fn check_undo(&mut self, step: usize, label: &str, expected: &[u8]) {
        self.tally.undo_checks += 1;
        if !self.document.undo() {
            self.bug(step, label, "取り消せない".to_owned());
            return;
        }
        let back = self.document.save();
        if back != self.current {
            let text = difference(&back, &self.current, self.options.show);
            self.bug(
                step,
                label,
                format!("取り消した後のバイト列が、変更の前と違う。{text}"),
            );
        }
        if !self.document.redo() {
            self.bug(step, label, "やり直せない".to_owned());
            return;
        }
        let again = self.document.save();
        if again != expected {
            let text = difference(&again, expected, self.options.show);
            self.bug(
                step,
                label,
                format!("やり直した後のバイト列が、変更の後と違う。{text}"),
            );
        }
    }

    /// 文書エンジンの中の値を、保存したバイト列から開き直した文書と比べ、範囲で選ぶ結果を総当たりと比べる
    fn check_deep(&mut self, step: usize, label: &str) -> Result<(), String> {
        self.tally.deep_checks += 1;
        let reopened = Document::open(self.document.save(), self.options.threads)
            .map_err(|error| format!("保存したファイルを開けない: {error:?}"))?;
        if !self.document.same_content(&reopened) {
            self.bug(step, label, "開き直した文書と、要素の列が違う".to_owned());
        }
        if self.document.display.canonical() != reopened.display.canonical() {
            self.bug(
                step,
                label,
                "開き直した文書と、表示のバッファーが違う".to_owned(),
            );
        }
        if self.document.display == reopened.display {
            self.tally.same_buffers += 1;
        }
        if self.document.boxes != reopened.boxes {
            self.bug(step, label, "開き直した文書と、要素の箱が違う".to_owned());
        }
        let [x0, y0, x1, y1] = self.extent;
        let span = (x1 - x0).max(y1 - y0).max(1.0);
        let mut found = Vec::new();
        for _ in 0..24 {
            let x = self.random.between(x0 - span * 0.1, x1 + span * 0.1);
            let y = self.random.between(y0 - span * 0.1, y1 + span * 0.1);
            let size = span * self.random.pick(&[0.001, 0.01, 0.1, 0.5]);
            let rect = [
                x,
                y,
                x + size * self.random.unit(),
                y + size * self.random.unit(),
            ];
            found.clear();
            self.document.select(rect, &mut found);
            let brute: Vec<u32> = (0u32..)
                .zip(&self.document.boxes)
                .filter(|(_, item)| {
                    item[0] <= item[2]
                        && item[1] <= item[3]
                        && item[0] <= rect[2]
                        && item[2] >= rect[0]
                        && item[1] <= rect[3]
                        && item[3] >= rect[1]
                })
                .map(|(row, _)| row)
                .collect();
            self.tally.select_checks += 1;
            if found != brute {
                self.bug(
                    step,
                    label,
                    format!(
                        "範囲で選ぶ結果が総当たりと違う（{} 件と {} 件）",
                        found.len(),
                        brute.len()
                    ),
                );
            }
        }
        Ok(())
    }

    fn step(&mut self, step: usize) -> Result<(), String> {
        let (label, targets, matrices) = self.draw();
        let fallback = "v2:00000000000000000000000000000000";
        let moves: Vec<Move> = targets
            .iter()
            .zip(&matrices)
            .map(|(handle, matrix)| {
                let id = id_of(*handle);
                let hash = self
                    .hashes
                    .get(&id)
                    .map_or_else(|| fallback.to_owned(), String::clone);
                (id, hash, *matrix)
            })
            .collect();
        let edits: Vec<Edit> = targets
            .iter()
            .zip(&matrices)
            .map(|(&handle, &matrix)| Edit { handle, matrix })
            .collect();
        let theirs = self.today.apply(&moves);
        let ours = self.document.apply(&edits);
        self.tally.steps += 1;
        let accepted = usize::from(theirs.is_ok());
        if let Some(count) = self
            .tally
            .labels
            .entry(label)
            .or_default()
            .get_mut(accepted)
        {
            *count += 1;
        }
        match (theirs, ours) {
            (Ok(applied), Ok(())) => {
                let expected = applied.file.unwrap_or_else(|| self.current.clone());
                let saved = self.document.save();
                if saved == expected {
                    self.tally.equal += 1;
                    self.check_undo(step, label, &expected);
                    for handle in &targets {
                        if let Some(row) = self.document.row_of(*handle)
                            && let Some(row) = self.document.rows.get(row as usize)
                        {
                            *self.tally.kinds.entry(kind_name(row.kind)).or_default() += 1;
                        }
                    }
                } else {
                    let text = difference(&saved, &expected, self.options.show);
                    self.bug(step, label, format!("保存したバイト列が違う。{text}"));
                    self.resync(&expected)?;
                }
                self.learn(applied.hashes);
                self.current = expected;
            }
            (Err(failure), Err(decline)) => {
                let key = format!("{decline:?} ／ {}", failure.describe());
                *self.tally.declined.entry(key).or_default() += 1;
            }
            (Ok(applied), Err(decline)) => {
                let expected = applied.file.unwrap_or_else(|| self.current.clone());
                let outside = matches!(
                    decline,
                    Decline::Locked(Locked::HandleSeed) | Decline::Unreadable
                );
                if outside {
                    *self.tally.gaps.entry(format!("{decline:?}")).or_default() += 1;
                } else {
                    self.bug(
                        step,
                        label,
                        format!(
                            "今の実装は受け付けたが、文書エンジンは受け付けない（{decline:?}）"
                        ),
                    );
                }
                self.learn(applied.hashes);
                self.resync(&expected)?;
                self.current = expected;
            }
            (Err(failure), Ok(())) => {
                self.bug(
                    step,
                    label,
                    format!(
                        "今の実装は受け付けない（{}）が、文書エンジンは受け付けた",
                        describe(&failure)
                    ),
                );
                self.document.undo();
                if self.document.save() != self.current {
                    let current = self.current.clone();
                    self.resync(&current)?;
                }
            }
        }
        Ok(())
    }

    /// すべて取り消して始まりのバイト列と比べ、すべてやり直して今のバイト列と比べる
    fn finish(&mut self, steps: usize) -> Result<(), String> {
        let label = "終わり";
        let mut undone = 0;
        while self.document.undo() {
            undone += 1;
        }
        let back = self.document.save();
        if back != self.base {
            let text = difference(&back, &self.base, self.options.show);
            self.bug(
                steps,
                label,
                format!("{undone} 回取り消したバイト列が、始まりと違う。{text}"),
            );
        }
        if self.document.edited_records() != 0 {
            self.bug(
                steps,
                label,
                "すべて取り消したのに、変えたレコードが残る".to_owned(),
            );
        }
        self.check_deep(steps, "すべて取り消した後")?;
        while self.document.redo() {}
        let again = self.document.save();
        if again != self.current {
            let text = difference(&again, &self.current, self.options.show);
            self.bug(
                steps,
                label,
                format!("すべてやり直したバイト列が、今と違う。{text}"),
            );
        }
        self.check_deep(steps, "すべてやり直した後")
    }
}

fn describe(failure: &Failure) -> String {
    failure.describe()
}

/// 1 つの入力を比べる。見つけた違いの数を返す
pub fn run(path: &str, options: &Options) -> Result<usize, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path} を読めない: {error}"))?;
    println!("== {path}（{} バイト）", bytes.len());
    let mut today = Today::opened(&bytes, options.budget.as_deref())
        .map_err(|failure| format!("今の実装が開けない: {}", failure.describe()))?;
    let document = Document::open(bytes.clone(), options.threads)
        .map_err(|error| format!("文書エンジンが開けない: {error:?}"))?;
    let elements = today
        .elements()
        .map_err(|failure| format!("今の実装が要素を返さない: {}", failure.describe()))?;

    // ID を比べる
    let counts = document.counts;
    println!(
        "  文書エンジン: レコード {}、ENTITIES {}、要素 {}（指せない {}）、ペーパー空間 {}、要素にならない {}、Shift_JIS {}、文字を読めない {}",
        counts.records,
        counts.entities,
        counts.elements,
        counts.unaddressable,
        counts.paper,
        counts.dropped,
        counts.shift_jis,
        counts.unreadable
    );
    if let Some(locked) = document.locked {
        println!("  文書エンジンは、この文書の変更を受け付けない: {locked:?}");
    }
    let mut hashes = BTreeMap::new();
    let mut covered = Vec::new();
    let mut pairs: BTreeMap<(&'static str, &str), usize> = BTreeMap::new();
    let mut outside: BTreeMap<String, usize> = BTreeMap::new();
    let mut known = BTreeSet::new();
    for element in &elements {
        known.insert(element.id.as_str());
        hashes.insert(element.id.clone(), element.hash.clone());
        let row = handle_of(&element.id).and_then(|handle| {
            Some((
                handle,
                document.rows.get(document.row_of(handle)? as usize)?,
            ))
        });
        match row {
            Some((handle, row)) => {
                covered.push(handle);
                *pairs
                    .entry((kind_name(row.kind), element.kind.as_str()))
                    .or_default() += 1;
            }
            None => {
                let place = element.id.rsplit_once('/').map_or("", |(place, _)| place);
                *outside
                    .entry(format!("{place} の {}", element.kind))
                    .or_default() += 1;
            }
        }
    }
    let mut bugs = 0;
    let extra: Vec<String> = document
        .rows
        .iter()
        .filter(|row| row.handle != 0)
        .map(|row| id_of(row.handle))
        .filter(|id| !known.contains(id.as_str()))
        .collect();
    println!(
        "  今の実装の要素 {} 件のうち、文書エンジンが同じ ID で持つもの {} 件",
        elements.len(),
        covered.len()
    );
    for ((ours, theirs), count) in &pairs {
        println!("    {ours} ＝ {theirs}: {count}");
    }
    for (what, count) in &outside {
        println!("    文書エンジンが持たない: {what}: {count}");
    }
    if !extra.is_empty() {
        bugs += extra.len();
        println!(
            "  違い: 文書エンジンだけが持つ要素が {} 件（最初は {}）",
            extra.len(),
            extra.first().map_or("", String::as_str)
        );
    }
    // 開いた結果は、分けて読む区間の数に依らない
    for parts in [1, 2, 3, 7, 16] {
        let split = store::scan_split(&bytes, parts)
            .and_then(|scanned| Document::assemble(bytes.clone(), scanned))
            .map_err(|error| format!("文書エンジンが {parts} 個に分けて開けない: {error:?}"))?;
        let mut split = split;
        split.build_display();
        split.build_index();
        if !(document.same_content(&split)
            && document.display == split.display
            && document.boxes == split.boxes
            && document.grid == split.grid
            && document.counts == split.counts
            && document.locked == split.locked)
        {
            bugs += 1;
            println!("  違い: {parts} 個に分けて開いた文書が、ふつうに開いた文書と違う");
        }
    }
    if covered.is_empty() {
        println!("  比べる要素がない");
        return Ok(bugs);
    }

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
    let largest = covered.iter().copied().max().unwrap_or(0);
    let mut unknown = largest + 0x1000;
    while known.contains(id_of(unknown).as_str()) || document.row_of(unknown).is_some() {
        unknown += 1;
    }
    let steps = options
        .steps
        .unwrap_or_else(|| (4_000_000 / covered.len().max(1)).clamp(80, 4000));
    let mut run = Run {
        options,
        today,
        document,
        current: bytes.clone(),
        base: bytes,
        hashes,
        covered,
        unknown,
        extent,
        last_move: None,
        random: Random(options.seed),
        tally: Tally::default(),
    };
    let deep_every = (steps / 16).max(1);
    for step in 0..steps {
        run.step(step)?;
        if (step + 1) % deep_every == 0 {
            run.check_deep(step, "途中")?;
        }
    }
    run.finish(steps)?;

    let tally = &run.tally;
    let declined: usize = tally.declined.values().sum();
    let gaps: usize = tally.gaps.values().sum();
    println!(
        "  変更 {} 回: 両方が受け付けてバイト列が同じ {}、両方が受け付けない {}、今の実装だけが受け付ける（試作の範囲の外） {}、違い {}",
        tally.steps, tally.equal, declined, gaps, tally.bugs
    );
    for (label, [no, yes]) in &tally.labels {
        println!("    {label}: 今の実装が受け付けた {yes}、受け付けない {no}");
    }
    for (kind, count) in &tally.kinds {
        println!("    動かした {kind}: {count}");
    }
    for (key, count) in &tally.declined {
        println!("    両方が受け付けない: {key}: {count}");
    }
    for (key, count) in &tally.gaps {
        println!("    試作の範囲の外: {key}: {count}");
    }
    println!(
        "  文書エンジンの中の確かめ: 取り消しとやり直し {} 回、開き直した文書との一致 {} 回（うち、表示のバッファーが並びまで同じ {} 回）、範囲で選ぶ {} 回、開き直し {} 回、最後の文書 {} バイト",
        tally.undo_checks,
        tally.deep_checks,
        tally.same_buffers,
        tally.select_checks,
        tally.resyncs,
        run.current.len()
    );
    Ok(bugs + tally.bugs)
}

//! エディターの設計（#693・決定 0288）のための測り。`prepare.mjs` が書いた場合の一覧（plan.txt）を、ネイティブの
//! セッションで流す。`run.mjs` が同じ一覧を wasm で流す。
//!
//! ```text
//! measure <plan.txt> time [名前に含む文字]    時間を測る。場合ごとに中央値・最小・最大を書く
//! measure <plan.txt> work [名前に含む文字]    作業量を測る。通る最小の予算を、単位ごとに二分探索する
//! ```
//!
//! 場合ごとに新しいセッションを作り、準備の push と call を流してから、測る call（1 つか、続けて流すいくつか）を
//! 流す。測る時間には、応答の blob を取り出す時間を含める。

use std::path::{Path, PathBuf};
use std::time::Instant;

use zumen::{CancelToken, Response, Session};

/// 場合の一覧の 1 つ
struct Case {
    name: String,
    repeat: usize,
    kinds: Vec<String>,
    setup: Vec<Step>,
    timed: Vec<String>,
}

/// 準備の手順
enum Step {
    Push { name: String, path: PathBuf },
    Call(String),
}

fn read_plan(path: &Path) -> Vec<Case> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let text = std::fs::read_to_string(path).expect("場合の一覧を読めない");
    let mut cases = Vec::new();
    let mut current: Option<Case> = None;
    for line in text.lines() {
        let mut parts = line.splitn(4, '\t');
        match parts.next() {
            Some("case") => {
                let name = parts.next().expect("名前").to_owned();
                let repeat = parts.next().expect("回数").parse().expect("回数");
                let kinds = match parts.next().expect("単位") {
                    "-" => Vec::new(),
                    kinds => kinds.split(',').map(str::to_owned).collect(),
                };
                current = Some(Case {
                    name,
                    repeat,
                    kinds,
                    setup: Vec::new(),
                    timed: Vec::new(),
                });
            }
            Some("push") => {
                let case = current.as_mut().expect("case の前の push");
                let name = parts.next().expect("入力の名前").to_owned();
                let file = parts.next().expect("ファイル");
                case.setup.push(Step::Push {
                    name,
                    path: dir.join(file),
                });
            }
            Some("call") => {
                let case = current.as_mut().expect("case の前の call");
                case.setup.push(Step::Call(
                    line.split_once('\t').expect("要求").1.to_owned(),
                ));
            }
            Some("time") => {
                let case = current.as_mut().expect("case の前の time");
                case.timed
                    .push(line.split_once('\t').expect("要求").1.to_owned());
            }
            Some("end") => cases.push(current.take().expect("case の前の end")),
            _ => {}
        }
    }
    cases
}

/// 準備を流したセッション
fn prepared(case: &Case) -> Session {
    let mut session = Session::open().expect("セッション");
    let cancel = CancelToken::new();
    for step in &case.setup {
        match step {
            Step::Push { name, path } => {
                let data = std::fs::read(path).expect("入力を読めない");
                session.push(name, &data).expect("push");
            }
            Step::Call(request) => {
                if let Err(error) = session.call_bytes(request.as_bytes(), &cancel) {
                    panic!("{} の準備が失敗: {error:?}", case.name);
                }
            }
        }
    }
    session
}

/// 測る call を順に流し、応答の blob を取り出す。応答と blob のバイト数の和を返す。どれかが失敗なら `None`
fn run(session: &mut Session, requests: &[String]) -> Option<(usize, usize)> {
    let mut replies = 0;
    let mut blobs = 0;
    for request in requests {
        let reply = session
            .call_bytes(request.as_bytes(), &CancelToken::new())
            .ok()?;
        let response = Response::parse(&reply).expect("応答を読めない");
        replies += reply.len();
        for blob in &response.blobs {
            let id = u32::try_from(blob.id).expect("blob の番号");
            blobs += session.take(id).expect("take").len();
        }
    }
    Some((replies, blobs))
}

// 測定のツールなので、コアでは禁じている時計を使う（clippy.toml）。
#[allow(clippy::disallowed_methods)]
fn time(case: &Case) {
    let mut times = Vec::with_capacity(case.repeat);
    let mut sizes = (0, 0);
    for _ in 0..case.repeat {
        let mut session = prepared(case);
        let start = Instant::now();
        let result = run(&mut session, &case.timed);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        match result {
            Some(result) => sizes = result,
            None => {
                println!("native\t{}\tfailed", case.name);
                return;
            }
        }
        times.push(elapsed);
    }
    times.sort_by(f64::total_cmp);
    let median = times[times.len() / 2];
    println!(
        "native\t{}\t{median:.2}\t{:.2}\t{:.2}\t{}\t{}",
        case.name,
        times[0],
        times[times.len() - 1],
        sizes.0,
        sizes.1
    );
}

/// 単位 `kind` の上限を `limit` にして、測る call が通るか
fn passes(case: &Case, kind: &str, limit: u64) -> bool {
    let mut session = prepared(case);
    let requests: Vec<String> = case
        .timed
        .iter()
        .map(|request| request.replacen('{', &format!("{{\"budget\":{{\"{kind}\":{limit}}},"), 1))
        .collect();
    run(&mut session, &requests).is_some()
}

fn work(case: &Case) {
    for kind in &case.kinds {
        // 通らない下限と、通る上限を作り、その間を二分探索する
        let mut low = 0u64;
        let mut high = 1u64;
        while !passes(case, kind, high) {
            low = high;
            high = high.checked_mul(4).expect("上限が大きすぎる");
        }
        while high - low > 1 {
            let mid = low + (high - low) / 2;
            if passes(case, kind, mid) {
                high = mid;
            } else {
                low = mid;
            }
        }
        // 0 で通るなら 0
        let used = if low == 0 && passes(case, kind, 0) {
            0
        } else {
            high
        };
        println!("native\t{}\t{kind}\t{used}", case.name);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let plan = args
        .get(1)
        .expect("使い方: measure <plan.txt> time|work [名前に含む文字]");
    let mode = args.get(2).map_or("time", String::as_str);
    let filter = args.get(3).map_or("", String::as_str);
    for case in read_plan(Path::new(plan)) {
        if !case.name.contains(filter) {
            continue;
        }
        match mode {
            "time" => time(&case),
            "work" if !case.kinds.is_empty() => work(&case),
            "work" => {}
            other => panic!("知らない mode: {other}"),
        }
    }
}

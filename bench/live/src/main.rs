//! 文書エンジンの試作の入口。
//!
//! - `live compare <DXF>…` 今の実装と同じ変更を流して、保存したバイト列を比べる
//! - `live measure <DXF>…` 速さを測る
//! - `live grow <入力> <出力> <枚数>` 測るための大きい入力を作る
//! - `live cases <入力> <出力> [件数]` 書き方の違う図形を足した入力を作る
//! - `live open <DXF>…` 開いたまま待つ。プロセスのメモリを外から測るために使う

mod cases;
mod compare;
mod measure;
mod random;
mod today;

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::Duration;

use zumen_bench_live::store::Document;

const USAGE: &str = "使い方:
  live compare <DXF>… [--steps 回数] [--seed 種] [--threads 本数] [--budget JSON] [--show] [--hashes]
  live measure <DXF>… [--runs 回数] [--seed 種] [--threads 本数] [--region x0,y0,x1,y1] [--current] [--budget JSON]
  live grow <入力の DXF> <出力の DXF> <枚数> [--threads 本数]
  live cases <入力の DXF> <出力の DXF> [件数] [--seed 種] [--variant plain|loose|no-seed|low-seed|two-sections|no-tables|empty]
  live open <DXF>… [--hold ミリ秒] [--threads 本数] [--current] [--budget JSON]";

/// 値を取るオプション
const VALUED: [&str; 8] = [
    "steps", "seed", "threads", "runs", "budget", "region", "hold", "variant",
];
/// 値を取らないオプション
const FLAGS: [&str; 3] = ["current", "show", "hashes"];

struct Args {
    positional: Vec<String>,
    values: BTreeMap<String, String>,
    flags: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        positional: Vec::new(),
        values: BTreeMap::new(),
        flags: Vec::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.strip_prefix("--") {
            Some(name) if VALUED.contains(&name) => {
                let value = rest.next().ok_or(format!("--{name} の値がない"))?;
                parsed.values.insert(name.to_owned(), value.clone());
            }
            Some(name) if FLAGS.contains(&name) => parsed.flags.push(name.to_owned()),
            Some(name) => return Err(format!("知らないオプション: --{name}")),
            None => parsed.positional.push(arg.clone()),
        }
    }
    Ok(parsed)
}

impl Args {
    fn number<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, String> {
        self.values
            .get(name)
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| format!("--{name} の値を読めない: {value}"))
            })
            .transpose()
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|flag| flag == name)
    }

    fn region(&self) -> Result<Option<[f64; 4]>, String> {
        let Some(text) = self.values.get("region") else {
            return Ok(None);
        };
        let numbers: Vec<f64> = text
            .split(',')
            .map(|part| part.trim().parse())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("--region の値を読めない: {text}"))?;
        <[f64; 4]>::try_from(numbers)
            .map(Some)
            .map_err(|_| format!("--region は 4 つの数: {text}"))
    }
}

fn run(args: &[String]) -> Result<bool, String> {
    let (command, rest) = args.split_first().ok_or(USAGE)?;
    let args = parse(rest)?;
    let all = std::thread::available_parallelism().map_or(1, usize::from);
    let seed = args.number("seed")?.unwrap_or(1);
    match command.as_str() {
        "compare" if !args.positional.is_empty() => {
            let options = compare::Options {
                steps: args.number("steps")?,
                seed,
                threads: args.number("threads")?.unwrap_or(all).max(1),
                show: args.flag("show"),
                budget: args.values.get("budget").cloned(),
                hashes: args.flag("hashes"),
            };
            let mut differences = 0;
            for path in &args.positional {
                differences += compare::run(path, &options)?;
            }
            println!("違いの合計: {differences}");
            Ok(differences == 0)
        }
        "measure" if !args.positional.is_empty() => {
            let options = measure::Options {
                runs: args.number("runs")?.unwrap_or(7).max(1),
                threads: args.number("threads")?,
                current: args.flag("current"),
                budget: args.values.get("budget").cloned(),
                region: args.region()?,
                seed,
            };
            for path in &args.positional {
                measure::run(path, &options)?;
            }
            Ok(true)
        }
        "grow" => {
            let [input, output, tiles] = args.positional.as_slice() else {
                return Err(USAGE.to_owned());
            };
            let tiles: usize = tiles
                .parse()
                .map_err(|_| format!("枚数を読めない: {tiles}"))?;
            let threads = args.number("threads")?.unwrap_or(all).max(1);
            let src =
                std::fs::read(input).map_err(|error| format!("{input} を読めない: {error}"))?;
            let out = zumen_bench_live::grow::grow(&src, tiles, threads)
                .map_err(|error| format!("増やせない: {error:?}"))?;
            std::fs::write(output, &out)
                .map_err(|error| format!("{output} に書けない: {error}"))?;
            println!("{output}: {} バイト", out.len());
            Ok(true)
        }
        "cases" => {
            let (input, output, count) = match args.positional.as_slice() {
                [input, output] => (input, output, 400),
                [input, output, count] => (
                    input,
                    output,
                    count
                        .parse()
                        .map_err(|_| format!("件数を読めない: {count}"))?,
                ),
                _ => return Err(USAGE.to_owned()),
            };
            let src =
                std::fs::read(input).map_err(|error| format!("{input} を読めない: {error}"))?;
            let variant = match args.values.get("variant") {
                Some(name) => {
                    cases::Variant::parse(name).ok_or(format!("変種を読めない: {name}"))?
                }
                None => cases::Variant::Plain,
            };
            let count = if variant == cases::Variant::Empty {
                0
            } else {
                count
            };
            let out = cases::cases(&src, count, seed, variant)?;
            std::fs::write(output, &out)
                .map_err(|error| format!("{output} に書けない: {error}"))?;
            println!(
                "{output}: {} バイト（足した図形 {count} 個、変種 {variant:?}）",
                out.len()
            );
            Ok(true)
        }
        "open" if !args.positional.is_empty() => {
            let hold = Duration::from_millis(args.number("hold")?.unwrap_or(0));
            let threads = args.number("threads")?.unwrap_or(all).max(1);
            let budget = args.values.get("budget").map(String::as_str);
            for path in &args.positional {
                if args.flag("current") {
                    let bytes = std::fs::read(path)
                        .map_err(|error| format!("{path} を読めない: {error}"))?;
                    let failed =
                        |failure: today::Failure| format!("今の実装が失敗: {}", failure.describe());
                    let mut today = today::Today::start(&bytes, budget).map_err(failed)?;
                    // 入力はセッションに渡してある。こちらの写しは持たない
                    drop(bytes);
                    today.open().map_err(failed)?;
                    let scene = today.scene().map_err(failed)?;
                    println!("{path}: 今の実装で開いて、表示のデータ {scene} バイトを作った");
                    std::thread::sleep(hold);
                } else {
                    let document = Document::open_path(std::path::Path::new(path), threads)?;
                    println!(
                        "{path}: 文書エンジンで開いた。要素 {}、列・表示・索引 {} バイト、元のファイル保持 {} バイト（元 {} バイト）",
                        document.counts.elements,
                        document.resident_bytes(),
                        document.source_stored_bytes(),
                        document.source_len()
                    );
                    std::thread::sleep(hold);
                }
            }
            Ok(true)
        }
        _ => Err(USAGE.to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

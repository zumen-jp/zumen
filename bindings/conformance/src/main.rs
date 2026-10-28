//! 適合試験を、コアを直接呼ぶ形で流し、ハッシュの一覧を出す。
//!
//! - 引数なし: 一覧を標準出力に書く
//! - `--write`: 一覧を `hashes.txt` に書く。応答を変えた PR では、これで作り直し、変わった理由を PR に書く（決定 0007）

use std::process::ExitCode;

use zumen_conformance::{Direct, cases, run, write_hashes};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = match args.as_slice() {
        [] => false,
        [flag] if flag == "--write" => true,
        _ => {
            eprintln!("使い方: cargo run -p zumen-conformance -- [--write]");
            return ExitCode::FAILURE;
        }
    };
    let cases = match cases() {
        Ok(cases) => cases,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::FAILURE;
        }
    };
    let report = run(&cases, || Box::new(Direct::default()));
    if !report.failures.is_empty() {
        eprintln!("期待と違った:\n{}", report.failures.join("\n"));
        return ExitCode::FAILURE;
    }
    if write {
        if let Err(problem) = write_hashes(&report.hashes) {
            eprintln!("{problem}");
            return ExitCode::FAILURE;
        }
    } else {
        for line in &report.hashes {
            println!("{line}");
        }
    }
    ExitCode::SUCCESS
}

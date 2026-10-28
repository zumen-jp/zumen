//! 適合試験を、コアを直接呼ぶ形と、C ABI の関数を呼ぶ形で流し、どちらも `hashes.txt` と一致することを確かめる。

use zumen_conformance::{CAbi, Direct, Engine, Report, cases, differences, read_hashes, run};

fn check(report: Report) {
    assert!(
        report.failures.is_empty(),
        "期待と違った:\n{}",
        report.failures.join("\n")
    );
    let expected = read_hashes().unwrap();
    assert!(
        report.hashes == expected,
        "ハッシュの一覧が hashes.txt と合わない。応答を変えたなら `cargo run -p zumen-conformance -- --write` で作り直し、変わった理由を PR に書く:\n{}",
        differences(&expected, &report.hashes)
    );
}

#[test]
fn rust_directly() {
    check(run(&cases().unwrap(), || {
        Box::new(Direct::default()) as Box<dyn Engine>
    }));
}

#[test]
fn through_the_c_abi() {
    check(run(&cases().unwrap(), || {
        Box::new(CAbi::default()) as Box<dyn Engine>
    }));
}

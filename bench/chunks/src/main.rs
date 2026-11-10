use std::path::PathBuf;
use std::time::Instant;

use zumen_bench_chunks::{CASES, POLICIES, audit, bytes, prepare, render};

// 時計を読むのは計測ハーネスだけで、コアには持ち込まない。
#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("output directory"));
    let repeats: usize = args.next().unwrap_or_else(|| "7".into()).parse().unwrap();
    assert!(
        (3..=99).contains(&repeats) && repeats % 2 == 1,
        "repeats must be odd and between 3 and 99"
    );
    std::fs::create_dir_all(&out).unwrap();
    let mut rows = Vec::new();
    for (case, name) in CASES.iter().enumerate() {
        prepare(case);
        for (policy, label) in POLICIES.iter().enumerate() {
            render(policy); // ウォームアップと比較元のバイト。準備と監査は時間に含めない。
            let expected = bytes();
            let metrics = String::from_utf8(audit()).unwrap();
            let mut times = Vec::new();
            for _ in 0..repeats {
                let start = now();
                render(policy);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(expected, bytes(), "repeated scene bytes changed");
                assert_eq!(metrics.as_bytes(), audit());
            }
            times.sort_by(f64::total_cmp);
            std::fs::write(out.join(format!("{name}-{label}.bin")), expected).unwrap();
            rows.push(format!("{{\"case\":\"{name}\",\"policy\":\"{label}\",\"median_ms\":{},\"metrics\":{metrics}}}",times[times.len()/2]));
        }
    }
    std::fs::write(
        out.join("native.json"),
        format!(
            "{{\"schema\":1,\"repeats\":{repeats},\"warmups\":1,\"results\":[{}]}}\n",
            rows.join(",")
        ),
    )
    .unwrap();
}

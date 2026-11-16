//! 指定した IFC の L1 の時間と確保量を測る（#541）。パス・名前・属性値は出力しない。
//!
//! `ZUMEN_SAMPLES` の中の相対パスを引数で渡す。各入力を 3 回読み、元バイトの往復も確かめる。
//! peak は入力バッファーを含むヒープ確保量であり、OS の常駐メモリではない。

#![allow(clippy::disallowed_methods, clippy::print_stdout, clippy::print_stderr)]
#![allow(
    unsafe_code,
    reason = "検証用の実行ファイルで System の確保量だけを数える"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_ifc::{Ifc, Part, Record, Value};
use zumen_sdk::Codec;

struct Counting;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(size: usize) {
    let current = CURRENT.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(current, Ordering::Relaxed);
}

// SAFETY: 同じレイアウト・ポインターを System にそのまま渡す。成功した確保と解放だけを数える。
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc の呼び出し側が layout の契約を満たす。
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: alloc と同じ。ゼロ埋めも System に任せる。
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: このアロケーターから返したポインターとレイアウトをそのまま渡す。
        unsafe { System.dealloc(pointer, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: 古い確保と新しい大きさの契約は呼び出し側が満たす。
        let changed = unsafe { System.realloc(pointer, layout, size) };
        if !changed.is_null() {
            if size >= layout.size() {
                grow(size - layout.size());
            } else {
                CURRENT.fetch_sub(layout.size() - size, Ordering::Relaxed);
            }
        }
        changed
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn measure(input: usize, bytes: &[u8]) -> Result<(), ()> {
    for run in 1..=3 {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let base = CURRENT.load(Ordering::Relaxed);
        PEAK.store(base, Ordering::Relaxed);
        let start = Instant::now();
        let file = Ifc::read(bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .map_err(|_| ())?;
        let nanos = start.elapsed().as_nanos();
        let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base) + bytes.len();
        let retained = CURRENT.load(Ordering::Relaxed).saturating_sub(base) + bytes.len();
        let mut parts = 0;
        let mut values = 0;
        let mut stack = Vec::new();
        for record in file.records() {
            parts += record.parts().len();
            stack.extend(record.parts().iter().flat_map(Part::args));
            while let Some(value) = stack.pop() {
                values += 1;
                match value {
                    Value::List(items) => stack.extend(items),
                    Value::Typed(part) => stack.extend(part.args()),
                    _ => {}
                }
            }
        }
        let mut output = Vec::new();
        Ifc::write(
            &file,
            &mut output,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .map_err(|_| ())?;
        if output != bytes {
            return Err(());
        }
        println!(
            "input={input} run={run} bytes={} read_ns={nanos} peak={peak} retained={retained} records={} parts={parts} values={values} roundtrip=true",
            bytes.len(),
            file.records().len(),
        );
    }
    Ok(())
}

fn main() {
    let Some(root) = std::env::var_os("ZUMEN_SAMPLES") else {
        eprintln!("ZUMEN_SAMPLES を指定してください。");
        std::process::exit(2);
    };
    let Ok(root) = std::fs::canonicalize(root) else {
        std::process::exit(2);
    };
    println!(
        "record_size={} part_size={} value_size={}",
        size_of::<Record>(),
        size_of::<Part>(),
        size_of::<Value>(),
    );
    for (index, name) in std::env::args().skip(1).enumerate() {
        let relative = std::path::Path::new(&name);
        if !relative
            .components()
            .all(|p| matches!(p, std::path::Component::Normal(_)))
        {
            std::process::exit(2);
        }
        let Ok(path) = std::fs::canonicalize(root.join(relative)) else {
            std::process::exit(2);
        };
        if !path.starts_with(&root) {
            std::process::exit(2);
        }
        let Ok(size) = path.metadata().map(|m| m.len()) else {
            std::process::exit(2);
        };
        if size > 512 * 1024 * 1024 {
            std::process::exit(2);
        }
        let Ok(bytes) = std::fs::read(path) else {
            std::process::exit(2);
        };
        if measure(index + 1, &bytes).is_err() {
            eprintln!("入力 {}: 読み取りか往復が失敗しました。", index + 1);
            std::process::exit(1);
        }
    }
}

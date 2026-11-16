//! 実行ファイルの検証用アロケーター。OS の常駐量でなく、System に渡す確保量を数える。
#![allow(
    unsafe_code,
    reason = "検証用の実行ファイルで System の確保量だけを数える"
)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
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

pub fn start() -> usize {
    let base = CURRENT.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    base
}
pub fn finish(base: usize) -> (usize, usize) {
    (
        PEAK.load(Ordering::Relaxed).saturating_sub(base),
        CURRENT.load(Ordering::Relaxed).saturating_sub(base),
    )
}

//! 適合試験（`bindings/conformance`）の組を、C の側（`conformance.c`、ヘッダーだけ）と C++ の側（`conformance.cpp`、
//! `zumen.hpp` の包み）を通して流し、状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することを確かめる（決定 0198）。
//!
//! 組の手順を読んで応答を確かめるのは、Rust の試験の道具（`zumen-conformance`）が受け持つ。C と C++ には JSON を
//! 読む標準のライブラリがないため。C++ の側では、シーンの blob を `zumen::Scene` で読み、応答の数と合うかも確かめる。

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use zumen_c::SceneCounts;
use zumen_conformance::{Engine, Report, cases, differences, read_hashes, run};
use zumen_json::Value;
use zumen_scene::layout;

/// C か C++ の側の関数
struct Side {
    new: unsafe extern "C" fn() -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    session: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32,
    push: unsafe extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize) -> i32,
    call: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32,
    take: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    output: unsafe extern "C" fn(*const c_void, *mut *const u8, *mut usize),
}

const C: Side = Side {
    new: zumen_c::zumen_c_engine_new,
    free: zumen_c::zumen_c_engine_free,
    session: zumen_c::zumen_c_engine_session,
    push: zumen_c::zumen_c_engine_push,
    call: zumen_c::zumen_c_engine_call,
    take: zumen_c::zumen_c_engine_take,
    output: zumen_c::zumen_c_engine_output,
};

const CPP: Side = Side {
    new: zumen_c::zumen_cpp_engine_new,
    free: zumen_c::zumen_cpp_engine_free,
    session: zumen_c::zumen_cpp_engine_session,
    push: zumen_c::zumen_cpp_engine_push,
    call: zumen_c::zumen_cpp_engine_call,
    take: zumen_c::zumen_cpp_engine_take,
    output: zumen_c::zumen_cpp_engine_output,
};

/// 直前の応答が返したシーンの blob の番号と数
struct LastScene {
    blob: u32,
    counts: Value,
}

/// シーンを読んで確かめた結果
#[derive(Default)]
struct Scenes {
    read: usize,
    failures: Vec<String>,
}

/// C か C++ の側を通す形
struct Native {
    side: &'static Side,
    engine: *mut c_void,
    /// C++ の側では、シーンの blob を読み手で読む
    scenes: Option<Rc<RefCell<Scenes>>>,
    last_scene: Option<LastScene>,
}

impl Native {
    fn new(side: &'static Side, scenes: Option<Rc<RefCell<Scenes>>>) -> Self {
        let engine = unsafe { (side.new)() };
        assert!(!engine.is_null(), "試験の形を作れない");
        Self {
            side,
            engine,
            scenes,
            last_scene: None,
        }
    }

    /// 書かれたバッファーを写す。書かれていなければ、成功のときだけ空にする（`zumen-conformance` の C ABI の形と同じ）
    fn output(&self, status: i32) -> Option<Vec<u8>> {
        let mut data = ptr::null();
        let mut len = 0;
        unsafe { (self.side.output)(self.engine, &mut data, &mut len) };
        if data.is_null() {
            return (status == 0).then(Vec::new);
        }
        Some(unsafe { std::slice::from_raw_parts(data, len) }.to_vec())
    }

    fn check_scene(&self, scenes: &mut Scenes, last: &LastScene, blob: &[u8]) {
        let mut counts = SceneCounts::default();
        if unsafe { zumen_c::zumen_cpp_read_scene(blob.as_ptr(), blob.len(), &mut counts) } != 0 {
            scenes
                .failures
                .push("シーンを zumen::Scene で読めない".to_string());
            return;
        }
        scenes.read += 1;
        let count = |key: &str| match last.counts.pointer(&format!("/{key}")) {
            Some(Value::Number(n)) => n.to_safe_integer().unwrap_or(-1),
            _ => -1,
        };
        let checks = [
            ("大版", i64::from(counts.major), i64::from(layout::MAJOR)),
            (
                "区画の数",
                i64::from(counts.sections),
                layout::SECTIONS.len() as i64,
            ),
            ("塊の数", i64::from(counts.chunks), count("chunks")),
            (
                "chunk_origins の行",
                counts.chunk_origins as i64,
                count("chunks"),
            ),
            ("text_xy の行", counts.text_xy as i64, count("texts")),
            ("point_xy の行", counts.point_xy as i64, count("points")),
            ("glyph_xy の行", counts.glyph_xy as i64, count("glyphs")),
            (
                "line_xy と face_xy の行",
                counts.line_and_face_xy as i64,
                count("vertices"),
            ),
            ("引けない text_strings の行", counts.bad_texts as i64, 0),
        ];
        for (label, actual, expected) in checks {
            if actual != expected {
                scenes.failures.push(format!(
                    "シーン {}: {label}が {actual}（期待は {expected}）",
                    scenes.read
                ));
            }
        }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        unsafe { (self.side.free)(self.engine) };
    }
}

impl Engine for Native {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        unsafe { (self.side.session)(self.engine, options.as_ptr(), options.len()) }
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        unsafe {
            (self.side.push)(
                self.engine,
                name.as_ptr(),
                name.len(),
                data.as_ptr(),
                data.len(),
            )
        }
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        let status = unsafe { (self.side.call)(self.engine, request.as_ptr(), request.len()) };
        let output = self.output(status);
        self.last_scene = output
            .as_deref()
            .and_then(|bytes| zumen_json::parse(bytes).ok())
            .and_then(|response| {
                let Some(Value::Array(blobs)) = response.pointer("/blobs") else {
                    return None;
                };
                let blob = blobs.iter().find_map(|blob| {
                    match (blob.pointer("/kind"), blob.pointer("/id")) {
                        (Some(Value::String(kind)), Some(Value::Number(id))) if kind == "scene" => {
                            id.to_safe_integer().and_then(|id| u32::try_from(id).ok())
                        }
                        _ => None,
                    }
                })?;
                let counts = response.pointer("/result/counts")?.clone();
                Some(LastScene { blob, counts })
            });
        (status, output)
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let status = unsafe { (self.side.take)(self.engine, blob) };
        let output = self.output(status);
        if let (Some(scenes), Some(last), Some(bytes)) = (&self.scenes, &self.last_scene, &output)
            && status == 0
            && last.blob == blob
        {
            self.check_scene(&mut scenes.borrow_mut(), last, bytes);
        }
        (status, output)
    }
}

fn check(report: Report) {
    assert!(
        report.failures.is_empty(),
        "期待と違った:\n{}",
        report.failures.join("\n")
    );
    let expected = read_hashes().unwrap();
    assert!(
        report.hashes == expected,
        "ハッシュの一覧が hashes.txt と合わない:\n{}",
        differences(&expected, &report.hashes)
    );
}

#[test]
fn c_runs_the_cases_through_the_header() {
    check(run(&cases().unwrap(), || {
        Box::new(Native::new(&C, None)) as Box<dyn Engine>
    }));
}

#[test]
fn cpp_runs_the_cases_through_the_wrapper() {
    let scenes = Rc::new(RefCell::new(Scenes::default()));
    let shared = scenes.clone();
    check(run(&cases().unwrap(), move || {
        Box::new(Native::new(&CPP, Some(shared.clone()))) as Box<dyn Engine>
    }));
    let scenes = scenes.borrow();
    assert!(
        scenes.failures.is_empty(),
        "シーンが応答の数と合わない:\n{}",
        scenes.failures.join("\n")
    );
    assert!(scenes.read > 0, "シーンの blob を 1 つも読んでいない");
}

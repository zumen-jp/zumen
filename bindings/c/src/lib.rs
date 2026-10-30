//! C のヘッダー（`include/zumen.h`・`zumen_scene.h`）と C++ の包み（`include/zumen.hpp`）を、C と C++ のコンパイラーで
//! 確かめる試験（`tests/smoke.rs`・`tests/conformance.rs`）。成果物には入らない。

// 試験の C と C++ が呼ぶ zm_* の関数を、試験のプログラムにリンクする
extern crate zumen_abi1;

use std::ffi::c_void;

unsafe extern "C" {
    /// C の試験。成功なら 0、失敗なら失敗した行の番号を返す
    pub fn zumen_c_smoke() -> i32;
    /// C++ の試験。成功なら 0、失敗なら失敗した行の番号を返す
    pub fn zumen_cpp_smoke() -> i32;

    /// C の適合試験の形を作る。`zumen_c_engine_free` で捨てる
    pub fn zumen_c_engine_new() -> *mut c_void;
    /// C の適合試験の形を捨てる
    pub fn zumen_c_engine_free(engine: *mut c_void);
    /// セッションを作り直す
    pub fn zumen_c_engine_session(
        engine: *mut c_void,
        options: *const u8,
        options_len: usize,
    ) -> i32;
    /// 入力を足す
    pub fn zumen_c_engine_push(
        engine: *mut c_void,
        name: *const u8,
        name_len: usize,
        data: *const u8,
        data_len: usize,
    ) -> i32;
    /// 要求を渡す。応答は `zumen_c_engine_output` で見る
    pub fn zumen_c_engine_call(engine: *mut c_void, request: *const u8, request_len: usize) -> i32;
    /// バイナリの結果を取り出す。結果は `zumen_c_engine_output` で見る
    pub fn zumen_c_engine_take(engine: *mut c_void, blob: u32) -> i32;
    /// 直前に書かれたバッファー。書かれていなければ `ptr` は NULL
    pub fn zumen_c_engine_output(engine: *const c_void, ptr: *mut *const u8, len: *mut usize);

    /// C++ の適合試験の形を作る。`zumen_cpp_engine_free` で捨てる
    pub fn zumen_cpp_engine_new() -> *mut c_void;
    /// C++ の適合試験の形を捨てる
    pub fn zumen_cpp_engine_free(engine: *mut c_void);
    /// セッションを作り直す
    pub fn zumen_cpp_engine_session(
        engine: *mut c_void,
        options: *const u8,
        options_len: usize,
    ) -> i32;
    /// 入力を足す
    pub fn zumen_cpp_engine_push(
        engine: *mut c_void,
        name: *const u8,
        name_len: usize,
        data: *const u8,
        data_len: usize,
    ) -> i32;
    /// 要求を渡す。応答は `zumen_cpp_engine_output` で見る
    pub fn zumen_cpp_engine_call(
        engine: *mut c_void,
        request: *const u8,
        request_len: usize,
    ) -> i32;
    /// バイナリの結果を取り出す。結果は `zumen_cpp_engine_output` で見る
    pub fn zumen_cpp_engine_take(engine: *mut c_void, blob: u32) -> i32;
    /// 直前に書かれたバッファー。書かれていなければ `ptr` は NULL
    pub fn zumen_cpp_engine_output(engine: *const c_void, ptr: *mut *const u8, len: *mut usize);
    /// シーンの blob を C++ の読み手（`zumen::Scene`）で読んで数える。読めたら 0
    pub fn zumen_cpp_read_scene(blob: *const u8, len: usize, out: *mut SceneCounts) -> i32;
}

/// C++ の読み手でシーンを読んで数えたもの（`tests/conformance.cpp` の `zumen_cpp_scene_counts`）
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneCounts {
    /// 大版
    pub major: u32,
    /// blob にある区画の数
    pub sections: u32,
    /// 塊の数
    pub chunks: u32,
    /// chunk_origins の行
    pub chunk_origins: u64,
    /// text_xy の行
    pub text_xy: u64,
    /// point_xy の行
    pub point_xy: u64,
    /// glyph_xy の行
    pub glyph_xy: u64,
    /// line_xy と face_xy の行
    pub line_and_face_xy: u64,
    /// text_strings の行のうち、strings の区画から引けなかったもの
    pub bad_texts: u64,
}

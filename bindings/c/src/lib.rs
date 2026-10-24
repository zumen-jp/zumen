//! C のヘッダー（`include/zumen.h`）を、C と C++ のコンパイラで確かめる試験（`tests/smoke.rs`）。成果物には入らない。

// 試験の C と C++ が呼ぶ zm_* の関数を、試験のプログラムにリンクする
extern crate zumen_abi1;

unsafe extern "C" {
    /// C の試験。成功なら 0、失敗なら失敗した行の番号を返す
    pub fn zumen_c_smoke() -> i32;
    /// C++ の試験。成功なら 0、失敗なら失敗した行の番号を返す
    pub fn zumen_cpp_smoke() -> i32;
}

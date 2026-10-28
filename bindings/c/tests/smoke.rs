//! 生成したヘッダーで、C と C++ から境界の関数を呼ぶ。
//!
//! 32 bit の Windows では、ヘッダーの呼び出し規約（stdcall）がコアと合わないと、リンクできない。

#[test]
fn c_calls_the_boundary_through_the_header() {
    assert_eq!(
        unsafe { zumen_c::zumen_c_smoke() },
        0,
        "smoke.c のこの行で失敗した"
    );
}

#[test]
fn cpp_calls_the_boundary_through_the_header() {
    assert_eq!(
        unsafe { zumen_c::zumen_cpp_smoke() },
        0,
        "smoke.cpp のこの行で失敗した"
    );
}

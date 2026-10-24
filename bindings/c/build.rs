//! 試験の C と C++ を、生成したヘッダー（include/zumen.h）で組み立てる（決定 0011）。
//!
//! 警告を失敗にして、C99 と C++17（docs/support.md の下限）で通ることを確かめる。

fn main() {
    for path in ["include/zumen.h", "tests/smoke.c", "tests/smoke.cpp"] {
        println!("cargo:rerun-if-changed={path}");
    }
    build("tests/smoke.c", false, "zumen_c_smoke");
    build("tests/smoke.cpp", true, "zumen_cpp_smoke");
}

fn build(file: &str, cpp: bool, name: &str) {
    let mut build = cc::Build::new();
    build
        .cpp(cpp)
        .file(file)
        .include("include")
        .warnings(true)
        .extra_warnings(true)
        .warnings_into_errors(true);
    if build.get_compiler().is_like_msvc() {
        // MSVC は C99 を選べないので、C11 で確かめる。ソースは UTF-8 として読む
        build.flag("/utf-8");
        build.flag(if cpp { "/std:c++17" } else { "/std:c11" });
    } else {
        build.flag("-pedantic");
        build.flag(if cpp { "-std=c++17" } else { "-std=c99" });
    }
    build.compile(name);
}

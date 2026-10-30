//! 試験の C と C++ を、生成したヘッダー（include/zumen.h・zumen_scene.h）と C++ の包み（include/zumen.hpp）で
//! 組み立てる（決定 0011・0198）。
//!
//! 警告を失敗にして、C99 と C++17（docs/support.md の下限）で通ることを確かめる。

fn main() {
    for path in [
        "include/zumen.h",
        "include/zumen_scene.h",
        "include/zumen.hpp",
        "tests/smoke.c",
        "tests/smoke.cpp",
        "tests/conformance.c",
        "tests/conformance.cpp",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    build(
        &["tests/smoke.c", "tests/conformance.c"],
        false,
        "zumen_c_tests",
    );
    build(
        &["tests/smoke.cpp", "tests/conformance.cpp"],
        true,
        "zumen_cpp_tests",
    );
}

fn build(files: &[&str], cpp: bool, name: &str) {
    let mut build = cc::Build::new();
    build
        .cpp(cpp)
        .files(files)
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

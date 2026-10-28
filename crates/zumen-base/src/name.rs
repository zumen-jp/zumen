//! 入力の名前の規則（決定 0010・0012）。
//!
//! コアはファイルシステムを読まない。ホストが渡す入力と、入れ物（ZIP など）の中の項目は、
//! `/` で区切った相対の名前で指す。外部参照や電子納品のフォルダーも、この名前で解決する。

/// 名前が、`/` で区切った安全な相対の名前か。
///
/// 次のものを含む名前は使わない。ホストがこの名前をパスにしても、決めたフォルダーの外を指さないように。
/// - 空の部分（空の名前、先頭・末尾の `/`、続いた `/`）。先頭の `/` は絶対パスになる
/// - `.` と `..` の部分
/// - `\`（Windows の区切り）と `:`（ドライブ名など）
/// - NUL
pub fn is_safe(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', ':', '\0'])
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_relative_names() {
        for name in [
            "a.sfc",
            "DRAWING/D0PL001Z.P21",
            "図面/平面図.jww",
            "a/b/c.dxf",
            ".hidden",
            "a..b",
        ] {
            assert!(is_safe(name), "{name}");
        }
    }

    #[test]
    fn rejects_names_that_could_leave_the_folder() {
        for name in [
            "",
            "/etc/passwd",
            "a/",
            "a//b",
            "./a",
            "a/../b",
            "..",
            "a\\b",
            "..\\a",
            "C:a",
            "C:\\a",
            "\\\\server\\share",
            "a:b",
            "a\0b",
        ] {
            assert!(!is_safe(name), "{name:?}");
        }
    }
}

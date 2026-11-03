//! ZIP（L0）の fuzz の target（#501）。中央ディレクトリを読み、項目を展開し、`.zip` の項目は書庫として開いて辿る。
//! 展開した量（ZIP 爆弾）と書庫の入れ子が予算で止まり、読めた項目の名前が展開先の外を指さない（パスの走査がない）
//! ことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_sdk::{Cx, Error};
use zumen_zip::Archive;

/// 展開先のフォルダーの外を指さない名前か。`/` と `\` のどちらで区切っても `..` の部分がなく、根やドライブから
/// 始まらない
fn stays_inside(name: &str) -> bool {
    !name.starts_with(['/', '\\'])
        && !name.contains(':')
        && name.split(['/', '\\']).all(|part| part != "..")
}

/// 項目を展開し、`.zip` の項目は書庫として開いて辿る。止めたときだけ失敗を返し、壊れた項目は飛ばす
fn walk(archive: &Archive, cx: &mut Cx) -> Result<(), Error> {
    for entry in archive.entries() {
        assert!(
            stays_inside(entry.name()),
            "展開先の外を指す名前を読んだ: {:?}",
            entry.name()
        );
        if entry.is_directory() {
            continue;
        }
        let result = if entry.name().to_ascii_lowercase().ends_with(".zip") {
            archive.open(entry, cx).and_then(|inner| walk(&inner, cx))
        } else {
            archive.extract(entry, cx).map(drop)
        };
        if let Err(Error::Stopped(stop)) = result {
            return Err(Error::Stopped(stop));
        }
    }
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let walked =
        zumen_sdk::testkit::survives_with(data, |src, cx| walk(&zumen_zip::read(src, cx)?, cx));
    if let Err(message) = walked {
        panic!("{message}");
    }
});

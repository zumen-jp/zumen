use super::*;
use zumen_base::{Budget, CancelToken};
use zumen_sdk::Codec;

/// 表のオブジェクトのデータ `data` の、項目の数と、所有者・リアクター・拡張辞書の後ろに並ぶハンドル（項目と、
/// 項目の後ろのもの）。`handles` は、R2010 から後のハンドルのストリームの始まり
fn listed(data: &[u8], handles: Option<usize>, release: Release) -> (usize, Vec<u64>) {
    let mut bits = Bits::with_release(data, release);
    let handles_at = match handles {
        Some(handles_at) => {
            bits.bot().unwrap();
            handles_at
        }
        None => {
            let [size_at, _, handles_at] = object_fields(data, release).unwrap();
            bits.seek(size_at + 32);
            handles_at
        }
    };
    bits.handle().unwrap();
    skip_extended_data(&mut bits).unwrap();
    let reactors = usize::try_from(bits.bl().unwrap()).unwrap();
    let xdic = release < Release::R2004 || !bits.bit().unwrap();
    if release >= Release::R2013 {
        bits.bit().unwrap();
    }
    let count = usize::try_from(bits.bl().unwrap()).unwrap();
    bits.seek(handles_at);
    for _ in 0..(1 + reactors + usize::from(xdic)) {
        bits.handle().unwrap();
    }
    let mut found = Vec::new();
    // ハンドルは 8 ビット以上で、終わりの埋めるビットは 8 ビットより少ない
    while bits.pos() + 8 <= data.len() * 8 {
        found.push(bits.handle().unwrap().value);
    }
    (count, found)
}

/// 項目を変えた表のオブジェクトのデータ `changed` の、項目の数とハンドル。R2010 から後（`modular`）は、ハンドルの
/// ストリームの大きさ（MC）から始まる
fn relisted(changed: &[u8], modular: bool, release: Release) -> (usize, Vec<u64>) {
    if !modular {
        return listed(changed, None, release);
    }
    let mut at = 0;
    let bits = usize::try_from(modular_char(changed, &mut at, false).unwrap()).unwrap();
    let data = changed.get(at..).unwrap();
    listed(data, Some(data.len() * 8 - bits), release)
}

fn change(
    data: &[u8],
    handles: Option<usize>,
    entry: Entry,
    type_code: u16,
    release: Release,
) -> Option<Vec<u8>> {
    match handles {
        Some(handles_at) => add_entry_r2010(data, handles_at, entry, type_code, release),
        None => add_entry(data, entry, type_code, release),
    }
}

/// 公開サンプルの DWG（R2000・R2004・R2013・R2018）の表のオブジェクトは、項目の後ろに、線種なら ByBlock と ByLayer、
/// ブロックならモデル空間と用紙空間、寸法スタイルなら RC の数だけのハンドルを持ち、ほかの表は持たない。項目を足しても
/// 除いても、後ろのハンドルは残る。落とすと、AutoCAD 2027 は線種を足した DWG を開けない（#683）
#[test]
fn entries_are_added_and_removed_before_the_handles_that_follow_them() {
    const NEW: u64 = 0x7FFF_FFF0;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "dwg"))
        .collect();
    paths.sort();
    let mut releases = BTreeSet::new();
    for path in paths {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let file = crate::Dwg::read(&std::fs::read(&path).unwrap(), &mut cx).unwrap();
        let db = file.objects(&mut cx).unwrap();
        let name = |handle: &u64| match db.get(*handle).and_then(|object| object.first(2)) {
            Some(Data::Text(name)) => String::from_utf8_lossy(name).into_owned(),
            _ => String::new(),
        };
        releases.insert(file.version().to_vec());
        for entry in &file.entries {
            let (type_code, after): (u16, Option<&[&str]>) = match entry.type_code {
                kinds::LAYER_CONTROL => (kinds::LAYER, Some(&[])),
                kinds::STYLE_CONTROL => (kinds::STYLE, Some(&[])),
                kinds::LTYPE_CONTROL => (kinds::LTYPE, Some(&["ByBlock", "ByLayer"])),
                kinds::BLOCK_RECORD_CONTROL => {
                    (kinds::BLOCK_RECORD, Some(&["*Model_Space", "*Paper_Space"]))
                }
                // 後ろのハンドルの数は、図面が RC に持つ
                kinds::DIMSTYLE_CONTROL => (kinds::DIMSTYLE, None),
                _ => continue,
            };
            let at = format!("{} {type_code:#X}", path.display());
            let data = file.data(entry);
            let (count, before) = listed(data, entry.handles, file.release);
            let rest = before.get(count..).unwrap();
            if let Some(after) = after {
                assert_eq!(rest.iter().map(name).collect::<Vec<_>>(), after, "{at}");
            }
            let modular = entry.handles.is_some();
            let added = change(
                data,
                entry.handles,
                Entry::Add(NEW),
                type_code,
                file.release,
            )
            .unwrap();
            let mut expected = before.clone();
            expected.insert(count, NEW);
            assert_eq!(
                relisted(&added, modular, file.release),
                (count + 1, expected),
                "{at}"
            );
            // 足したデータから除くと、ハンドルの並びが元に戻る
            let (again, handles_at) = match modular {
                true => {
                    let mut skip = 0;
                    let bits =
                        usize::try_from(modular_char(&added, &mut skip, false).unwrap()).unwrap();
                    let data = added.get(skip..).unwrap();
                    (data, Some(data.len() * 8 - bits))
                }
                false => (added.as_slice(), None),
            };
            let removed = change(
                again,
                handles_at,
                Entry::Remove(NEW),
                type_code,
                file.release,
            )
            .unwrap();
            assert_eq!(
                relisted(&removed, modular, file.release),
                (count, before.clone()),
                "{at}"
            );
            // 項目の後ろのハンドルが指すものは、除けない
            for &handle in rest {
                let refused = change(
                    data,
                    entry.handles,
                    Entry::Remove(handle),
                    type_code,
                    file.release,
                );
                assert_eq!(refused, None, "{at}");
            }
        }
    }
    let expected: BTreeSet<Vec<u8>> = [b"AC1015", b"AC1018", b"AC1027", b"AC1032"]
        .iter()
        .map(|version| version.to_vec())
        .collect();
    assert_eq!(releases, expected);
}

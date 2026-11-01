//! コーデックの試験の道具（決定 0017 のテストキット）。
//!
//! どれも、食い違いを人向けの文で返す。試験の側で `unwrap` する。予算は、[`stops_at_same_place`] のほかは既定の
//! 上限を使う。

use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};

use crate::{ByteRange, Codec, Error, FileModel};

/// 既定の予算で読む。結果と、足された診断を返す
pub fn read<C: Codec>(src: &[u8]) -> (Result<C::File, Error>, Vec<Diagnostic>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = C::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag.into_vec())
}

/// 既定の予算で書く
pub fn write<C: Codec>(file: &C::File) -> Result<Vec<u8>, String> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    C::write(
        file,
        &mut out,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .map_err(|e| format!("書けない: {e:?}（診断: {:?}）", diag.into_vec()))?;
    Ok(out)
}

fn read_ok<C: Codec>(src: &[u8]) -> Result<C::File, String> {
    match read::<C>(src) {
        (Ok(file), _) => Ok(file),
        (Err(e), diag) => Err(format!("読めない: {e:?}（診断: {diag:?}）")),
    }
}

/// 読んで、変えずに書くと、元のバイト列と一致する（L1 の往復）
pub fn roundtrip<C: Codec>(src: &[u8]) -> Result<(), String> {
    let out = write::<C>(&read_ok::<C>(src)?)?;
    if out == src {
        return Ok(());
    }
    let at = out
        .iter()
        .zip(src)
        .position(|(a, b)| a != b)
        .unwrap_or(out.len().min(src.len()));
    Err(format!(
        "書いたバイト列が元と違う（先頭から {at} バイトの所。元は {} バイト、書いたものは {} バイト）",
        src.len(),
        out.len()
    ))
}

/// レコードの元のバイト範囲が、重ならずに順に並び、入力の中に収まる
pub fn records_in_order<C: Codec>(src: &[u8]) -> Result<(), String> {
    let file = read_ok::<C>(src)?;
    let mut end = 0;
    for index in 0..file.record_count() {
        let record = file
            .record(index)
            .ok_or_else(|| format!("{index} 番目のレコードを辿れない"))?;
        let bytes = record
            .bytes
            .ok_or_else(|| format!("読んだ {index} 番目のレコードに、元のバイト範囲がない"))?;
        if bytes.start < end || bytes.end < bytes.start || bytes.end > src.len() as u64 {
            return Err(format!(
                "{index} 番目のレコードの範囲 {}..{} が、前のレコードと重なるか、入力の外にある",
                bytes.start, bytes.end
            ));
        }
        end = bytes.end;
    }
    Ok(())
}

/// 2 回読むと、同じレコードと同じ診断になる（決定 0007）
pub fn deterministic<C: Codec>(src: &[u8]) -> Result<(), String> {
    let (first, first_diag) = read::<C>(src);
    let (second, second_diag) = read::<C>(src);
    if first_diag != second_diag {
        return Err("2 回の読み込みで、診断が違う".to_string());
    }
    match (first, second) {
        (Ok(a), Ok(b)) => {
            let same = a.record_count() == b.record_count()
                && (0..a.record_count()).all(|i| a.record(i) == b.record(i));
            if same {
                Ok(())
            } else {
                Err("2 回の読み込みで、レコードが違う".to_string())
            }
        }
        (Err(a), Err(b)) if a == b => Ok(()),
        _ => Err("2 回の読み込みで、成否が違う".to_string()),
    }
}

/// `edit` が成功し、書いたものを読み直すと、変えていないレコードが元のバイトのまま、同じ順に残る
pub fn edit_keeps_untouched_records<C: Codec>(
    src: &[u8],
    edit: impl FnOnce(&mut C::File, &mut Cx) -> Result<(), Error>,
) -> Result<(), String> {
    let mut file = read_ok::<C>(src)?;
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    edit(&mut file, &mut Cx::new(&mut budget, &mut diag, &cancel))
        .map_err(|e| format!("変更できない: {e:?}（診断: {:?}）", diag.into_vec()))?;
    let out = write::<C>(&file)?;
    let reread = read_ok::<C>(&out)?;
    if reread.record_count() != file.record_count() {
        return Err(format!(
            "書いたものを読み直すと、レコードの数が {} から {} に変わる",
            file.record_count(),
            reread.record_count()
        ));
    }
    let mut touched = 0;
    for index in 0..file.record_count() {
        let before = file.record(index).and_then(|r| r.bytes);
        let after = reread.record(index).and_then(|r| r.bytes);
        let Some(before) = before else {
            touched += 1;
            continue;
        };
        if slice(src, before) != after.and_then(|a| slice(&out, a)) {
            return Err(format!(
                "変えていない {index} 番目のレコードのバイトが変わる"
            ));
        }
    }
    if touched == 0 {
        return Err("変更したレコードがない".to_string());
    }
    Ok(())
}

/// `edit` が失敗し、ファイルモデルを書くと元のバイト列のままで、元のバイト範囲を持たないレコードもない
pub fn failed_edit_changes_nothing<C: Codec>(
    src: &[u8],
    edit: impl FnOnce(&mut C::File, &mut Cx) -> Result<(), Error>,
) -> Result<Vec<Diagnostic>, String> {
    let mut file = read_ok::<C>(src)?;
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    if edit(&mut file, &mut Cx::new(&mut budget, &mut diag, &cancel)).is_ok() {
        return Err("失敗するはずの変更が成功した".to_string());
    }
    if let Some(index) =
        (0..file.record_count()).find(|&i| file.record(i).is_some_and(|r| r.bytes.is_none()))
    {
        return Err(format!(
            "失敗した変更の後に、{index} 番目のレコードが変わっている"
        ));
    }
    if write::<C>(&file)? != src {
        return Err("失敗した変更の後に書くと、元のバイト列と違う".to_string());
    }
    Ok(diag.into_vec())
}

fn slice(src: &[u8], range: ByteRange) -> Option<&[u8]> {
    src.get(usize::try_from(range.start).ok()?..usize::try_from(range.end).ok()?)
}

/// 予算で止めると、同じ作業量で同じ所で止まる（決定 0010）。既定の予算で読んだときのレコードの数の半分を
/// 上限にして 2 回読み、どちらも同じ理由で止まり、同じ診断と同じ使った量になることを確かめる。レコードを 2 つ
/// より少なく数える入力では、止めずに 2 回読んで同じになることだけを確かめる
pub fn stops_at_same_place<C: Codec>(src: &[u8]) -> Result<(), String> {
    let full = limited::<C>(src, Limits::default());
    let records = full.used.get(&Kind::Records).copied().unwrap_or(0);
    let limits = if records < 2 {
        Limits::default()
    } else {
        Limits::default().with(Kind::Records, Limit::Finite(records / 2))
    };
    let first = limited::<C>(src, limits);
    let second = limited::<C>(src, limits);
    if records >= 2 && first.stopped.is_none() {
        return Err(format!(
            "レコードの上限 {} で止まらなかった（既定の予算で {records} 数えた）",
            records / 2
        ));
    }
    if first != second {
        return Err(format!(
            "同じ上限で 2 回読むと結果が違う（1 回目: {first:?}、2 回目: {second:?}）"
        ));
    }
    Ok(())
}

/// 上限を決めて読んだ結果。比べられるように、止まった理由・診断・使った量だけを持つ
#[derive(Debug, PartialEq)]
struct Limited {
    read: bool,
    stopped: Option<Stop>,
    diagnostics: Vec<Diagnostic>,
    used: BTreeMap<Kind, u64>,
}

fn limited<C: Codec>(src: &[u8], limits: Limits) -> Limited {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = C::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    let read = result.is_ok();
    let stopped = match result {
        Err(Error::Stopped(stop)) => Some(stop),
        _ => None,
    };
    let used = Kind::ALL
        .iter()
        .map(|kind| (*kind, budget.used(*kind)))
        .collect();
    Limited {
        read,
        stopped,
        diagnostics: diag.into_vec(),
        used,
    }
}

/// 同じバイト列から、記録した一覧と同じ ID が出る（ID の契約試験）。`ids` は、バイト列を読んで ID の一覧を
/// 作る関数で、2 回呼んで同じになることも確かめる。`recorded` は 1 行に 1 つの ID で、空の行は飛ばす。ID を
/// 変えるには、先に決定の記録が要る
pub fn ids_match(
    src: &[u8],
    recorded: &str,
    mut ids: impl FnMut(&[u8]) -> Result<Vec<String>, String>,
) -> Result<(), String> {
    let found = ids(src)?;
    if ids(src)? != found {
        return Err("同じバイト列から 2 回作った ID の一覧が違う".to_string());
    }
    let expected: Vec<&str> = recorded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if let Some(at) = found
        .iter()
        .zip(&expected)
        .position(|(found, expected)| found != expected)
    {
        return Err(format!(
            "{at} 番目の ID が記録と違う（記録: {:?}、作った ID: {:?}）",
            expected.get(at),
            found.get(at)
        ));
    }
    if found.len() != expected.len() {
        return Err(format!(
            "ID の数が記録と違う（記録: {}、作った ID: {}）",
            expected.len(),
            found.len()
        ));
    }
    Ok(())
}

/// fuzz の上限。どの入力でも、すぐに止まるようにする
fn fuzz_limits() -> Limits {
    Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(64 << 20))
        .with(Kind::Records, Limit::Finite(1 << 20))
        .with(Kind::Elements, Limit::Finite(1 << 20))
        .with(Kind::Segments, Limit::Finite(1 << 20))
        .with(Kind::Work, Limit::Finite(1 << 24))
}

/// fuzz で渡すバイト列を読む（決定 0010・0017）。小さい予算の中で止まり、2 回読んで同じ結果（読めたか・止まった
/// 理由・診断・使った量）になることを確かめる。パニックしないことは、呼ぶ側の fuzz の道具が確かめる。cargo-fuzz の
/// target から呼び、`Err` ならパニックにして知らせる
pub fn survives<C: Codec>(data: &[u8]) -> Result<(), String> {
    let first = limited::<C>(data, fuzz_limits());
    let second = limited::<C>(data, fuzz_limits());
    if first != second {
        return Err(format!(
            "同じバイト列を 2 回読むと結果が違う（1 回目: {first:?}、2 回目: {second:?}）"
        ));
    }
    Ok(())
}

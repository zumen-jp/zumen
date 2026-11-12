//! 書き戻しや再符号化が参照する、DWG オブジェクトのデータとハンドルのビット位置の走査。

use super::*;

/// バージョン `release`（R2004 まで）の図形でないオブジェクトのデータの、ハンドルのストリームの位置（RL）を書く所と、
/// 型ごとのフィールドの始まりと、ハンドルのストリームの始まりの、ビットの位置
pub(in crate::dwg) fn object_fields(data: &[u8], release: Release) -> Option<[usize; 3]> {
    let mut bits = Bits::with_release(data, release);
    bits.bs()?;
    let size_at = bits.pos();
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    bits.bl()?;
    // R2004 からは、リアクターの数の後に、拡張辞書を持たないフラグ（B）が続く
    if release >= Release::R2004 {
        bits.bit()?;
    }
    let start = bits.pos();
    (start <= handles_at).then_some([size_at, start, handles_at])
}

/// バージョン `release`（R2010 から）の図形でないオブジェクトのデータ（ハンドルのストリームの大きさ（MC）の後）の、型ごとの
/// フィールドの始まりと、主のデータの終わりの、ビットの位置。`handles_at` は、ハンドルのストリームの始まり（決定 0089）
pub(in crate::dwg) fn r2010_object_fields(
    data: &[u8],
    handles_at: usize,
    release: Release,
) -> Option<[usize; 2]> {
    let mut bits = Bits::with_release(data, release);
    bits.bot()?;
    let end = bits.split_strings(handles_at)?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    bits.bl()?;
    bits.bit()?;
    // R2013 からは、DS のデータを持つかの B が続く
    if release >= Release::R2013 {
        bits.bit()?;
    }
    let start = bits.pos();
    (start <= end).then_some([start, end])
}

/// バージョン `release`（R2010 から）の図形のデータ（ハンドルのストリームの大きさ（MC）の後）の、共通のフィールドの色の始まりと、
/// 型ごとのフィールドの始まりと、主のデータの終わり（文字列のストリームの始まりか、その有無のフラグ）の、ビットの位置。
/// `handles_at` は、ハンドルのストリームの始まり（決定 0085）
pub(in crate::dwg) fn r2010_entity_fields(
    data: &[u8],
    handles_at: usize,
    release: Release,
) -> Option<[usize; 3]> {
    let mut bits = Bits::with_release(data, release);
    bits.bot()?;
    let end = bits.split_strings(handles_at)?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    let start = bits.pos();
    (start <= end).then_some([common.color_at, start, end])
}

/// バージョン `release`（R2004 まで）の図形のデータの、ハンドルのストリームの位置（RL）を書く所と、共通のフィールドの色の
/// 始まりと、型ごとのフィールドの始まりと、ハンドルのストリームの始まりの、ビットの位置
pub(in crate::dwg) fn entity_fields(data: &[u8], release: Release) -> Option<[usize; 4]> {
    let mut bits = Bits::with_release(data, release);
    bits.bs()?;
    let size_at = bits.pos();
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    let start = bits.pos();
    (start <= handles_at).then_some([size_at, common.color_at, start, handles_at])
}

/// 図形のハンドルのストリームの、画層と線種のハンドルのビットの範囲（#723）
#[derive(Default)]
pub(in crate::dwg) struct StyleHandles {
    /// 画層のハンドル
    pub(in crate::dwg) layer: Range<usize>,
    /// 線種のハンドル。線種のフラグが 3 でなければ `None`
    pub(in crate::dwg) ltype: Option<Range<usize>>,
    /// 色を DBCOLOR のハンドルで指すか（R2004 から）
    pub(in crate::dwg) color_handle: bool,
}

/// バージョン `release` の図形のデータ `data` の、画層と線種のハンドルの位置（#723）。ハンドルの並びは、所有者（entmode が 0）・
/// リアクター・拡張辞書・前後の図形・色（DBCOLOR）・画層・線種（フラグが 3）の順（ODS 5.4.1 の 20.4.1、[`read`] と同じ）。
/// R2010 からは、`data` はハンドルのストリームの大きさ（MC）の後で、`handles_at` がハンドルのストリームの始まり
pub(in crate::dwg) fn style_handles(
    data: &[u8],
    release: Release,
    handles_at: Option<usize>,
) -> Option<StyleHandles> {
    let mut bits = Bits::with_release(data, release);
    let handles_at = match handles_at {
        Some(at) => {
            bits.bot()?;
            bits.split_strings(at)?;
            at
        }
        None => {
            bits.bs()?;
            usize::try_from(bits.rl()?).ok()?
        }
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    bits.seek(handles_at);
    if common.entmode == 0 {
        bits.handle()?;
    }
    for _ in 0..common.reactors {
        bits.handle()?;
    }
    if common.xdic {
        bits.handle()?;
    }
    if !common.no_links {
        bits.handle()?;
        bits.handle()?;
    }
    if common.color_handle {
        bits.handle()?;
    }
    let start = bits.pos();
    bits.handle()?;
    let layer = start..bits.pos();
    let ltype = if common.ltype_flags == 3 {
        let start = bits.pos();
        bits.handle()?;
        Some(start..bits.pos())
    } else {
        None
    };
    Some(StyleHandles {
        layer,
        ltype,
        color_handle: common.color_handle,
    })
}

/// バージョン `release` の LAYER のレコードのデータ `data` の、線種のハンドルのビットの範囲（#723）。ハンドルの並びは、
/// 所有者・リアクター・拡張辞書・外部参照のブロック・印刷スタイル・材質（R2010 から）・線種（[`table_record`] と同じ）。
/// R2010 からは、`data` はハンドルのストリームの大きさ（MC）の後で、`handles_at` がハンドルのストリームの始まり
pub(in crate::dwg) fn layer_line_type(
    data: &[u8],
    release: Release,
    handles_at: Option<usize>,
) -> Option<Range<usize>> {
    let mut bits = Bits::with_release(data, release);
    let handles_at = match handles_at {
        Some(at) => {
            bits.bot()?;
            bits.split_strings(at)?;
            at
        }
        None => {
            bits.bs()?;
            usize::try_from(bits.rl()?).ok()?
        }
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    // R2004 からは、拡張辞書を持たないフラグ（B）がある
    let xdic = release < Release::R2004 || !bits.bit()?;
    bits.seek(handles_at);
    bits.handle()?;
    for _ in 0..reactors {
        bits.handle()?;
    }
    if xdic {
        bits.handle()?;
    }
    // 外部参照のブロックと印刷スタイル。R2010 からは材質も
    let skipped = if release >= Release::R2010 { 3 } else { 2 };
    for _ in 0..skipped {
        bits.handle()?;
    }
    let start = bits.pos();
    bits.handle()?;
    Some(start..bits.pos())
}

/// R2000 の図形の、図形の並びをたどるのに要るもの（決定 0051）
pub(in crate::dwg) struct Links {
    /// 前後の図形のハンドルを持たないことを表す B のビットの位置
    pub(in crate::dwg) no_links_at: usize,
    /// リアクターの数
    pub(in crate::dwg) reactors: u32,
    /// 拡張辞書のハンドル。なければ 0
    pub(in crate::dwg) xdic: u64,
    /// 前後の図形のハンドルのビットの範囲。持たなければ、拡張辞書の後ろの空の範囲
    pub(in crate::dwg) at: Range<usize>,
    /// 後ろの図形のハンドル。持たなければ、ハンドルの 1 つ後ろ（図面で確かめた）。最後の図形は 0
    pub(in crate::dwg) next: u64,
}

/// ハンドル `handle` の R2000 の図形のデータの、図形の並びをたどるのに要るもの
pub(in crate::dwg) fn entity_links(data: &[u8], handle: u64) -> Option<Links> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.bs()?;
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    if bits.bit()? {
        let size = usize::try_from(bits.rl()?).ok()?;
        bits.skip_bytes(size)?;
    }
    let entmode = bits.bb()?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    let no_links_at = bits.pos();
    let no_links = bits.bit()?;
    bits.seek(handles_at);
    if entmode == 0 {
        bits.handle()?;
    }
    for _ in 0..reactors {
        bits.handle()?;
    }
    let xdic = bits.handle()?.resolve(handle)?;
    let start = bits.pos();
    let next = if no_links {
        handle.checked_add(1)?
    } else {
        bits.handle()?;
        bits.handle()?.resolve(handle)?
    };
    Some(Links {
        no_links_at,
        reactors,
        xdic,
        at: start..bits.pos(),
        next,
    })
}

/// ハンドル `handle` の R2000 のブロックのレコードのデータの、最初と最後の図形のハンドルのビットの範囲と、
/// その 2 つのハンドル（決定 0051）。ハンドルの並びは、所有者・リアクター・拡張辞書・外部参照のブロック・
/// BLOCK・最初の図形・最後の図形・ENDBLK・配置（図面で確かめた）。図形がなければ、どちらも 0
pub(in crate::dwg) fn block_links(data: &[u8], handle: u64) -> Option<(Range<usize>, u64, u64)> {
    let mut bits = Bits::with_release(data, Release::R2000);
    bits.bs()?;
    let handles_at = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    bits.seek(handles_at);
    for _ in 0..reactors + 4 {
        bits.handle()?;
    }
    let start = bits.pos();
    let first = bits.handle()?.resolve(handle)?;
    let last = bits.handle()?.resolve(handle)?;
    Some((start..bits.pos(), first, last))
}

/// バージョン `release`（R2004 から）の図形のデータの、リアクターの数と、拡張辞書のハンドルを持つか。R2010 からは、
/// データはハンドルのストリームの大きさ（MC）の後（決定 0091）
pub(in crate::dwg) fn entity_refs(data: &[u8], release: Release) -> Option<(u32, bool)> {
    let mut bits = Bits::with_release(data, release);
    if release >= Release::R2010 {
        bits.bot()?;
    } else {
        bits.bs()?;
        bits.rl()?;
    }
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let common = entity_data(&mut bits)?;
    Some((common.reactors, common.xdic))
}

/// R2004 から後のブロックのレコードが持つ図形（決定 0077・0091）
pub(in crate::dwg) struct Owned {
    /// ハンドルのストリームの位置（RL）を書く所のビットの位置。R2010 からは使わない
    pub(in crate::dwg) size_at: usize,
    /// 持つ図形の数（BL）のビットの範囲
    pub(in crate::dwg) count: Range<usize>,
    /// ハンドルのストリームの始まりのビットの位置
    pub(in crate::dwg) handles_at: usize,
    /// 持つ図形のハンドルの、ビットの範囲とハンドルの並び
    pub(in crate::dwg) handles: Vec<(Range<usize>, u64)>,
    /// 持つ図形のハンドルの終わり（ENDBLK のハンドルの始まり）のビットの位置
    pub(in crate::dwg) end: usize,
}

/// ハンドル `handle` のバージョン `release`（R2004 から）のブロックのレコードのデータの、持つ図形（決定 0077）。外部参照の
/// ブロックは `None`。持つ図形の数は、読み込まれたフラグ（B）の後に BL で置かれ、ハンドルは、所有者・リアクター・拡張辞書・
/// 外部参照のブロック・BLOCK の後に、その数だけ並ぶ（図面で確かめた）。R2010 からは、データはハンドルのストリームの
/// 大きさ（MC）の後で、ハンドルのストリームの始まり `handles` を受け取る。外部参照の番号はない（決定 0091）
pub(in crate::dwg) fn owned(
    data: &[u8],
    handle: u64,
    release: Release,
    handles: Option<usize>,
) -> Option<Owned> {
    let OwnedHeader {
        mut bits,
        size_at,
        count,
        count_range,
        handles_at,
    } = owned_header(data, release, handles)?;
    let mut handles = Vec::with_capacity(count.min(data.len()));
    for _ in 0..count {
        let at = bits.pos();
        let owned = bits.handle()?.resolve(handle)?;
        handles.push((at..bits.pos(), owned));
    }
    Some(Owned {
        size_at,
        count: count_range,
        handles_at,
        handles,
        end: bits.pos(),
    })
}

pub(super) struct OwnedHeader<'a> {
    bits: Bits<'a>,
    size_at: usize,
    count: usize,
    count_range: Range<usize>,
    handles_at: usize,
}

pub(super) fn owned_header(
    data: &[u8],
    release: Release,
    handles: Option<usize>,
) -> Option<OwnedHeader<'_>> {
    let mut bits = Bits::with_release(data, release);
    let r2010 = release >= Release::R2010;
    let (size_at, handles_at) = if r2010 {
        bits.bot()?;
        let at = handles?;
        bits.split_strings(at)?;
        (0, at)
    } else {
        bits.bs()?;
        let size_at = bits.pos();
        (size_at, usize::try_from(bits.rl()?).ok()?)
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let reactors = u32::try_from(bits.bl()?).ok()?;
    let xdic = !bits.bit()?;
    // R2013 からは、DS のデータを持つかの B が続く
    if release >= Release::R2013 {
        bits.bit()?;
    }
    // 名前・参照されるフラグ・外部参照の番号・外部参照に依るフラグ
    bits.text()?;
    bits.bit()?;
    if !r2010 {
        bits.bs()?;
    }
    bits.bit()?;
    // 名前のないフラグ・属性を持つフラグ・外部参照のフラグ・重ねる外部参照のフラグ・読み込まれたフラグ
    let flags: Vec<bool> = (0..5).map(|_| bits.bit()).collect::<Option<_>>()?;
    if flags.get(2..4)?.contains(&true) {
        return None;
    }
    let start = bits.pos();
    let count = usize::try_from(bits.bl()?).ok()?;
    let count_range = start..bits.pos();
    bits.seek(handles_at);
    for _ in 0..reactors + u32::from(xdic) + 3 {
        bits.handle()?;
    }
    (count <= data.len().checked_sub(bits.pos() / 8)?).then_some(OwnedHeader {
        bits,
        size_at,
        count,
        count_range,
        handles_at,
    })
}

/// R2004 のブロックが持つ順にハンドルを読む。再符号化では編集用のビット範囲の割り当てを省く。
pub(in crate::dwg) fn owned_ids(data: &[u8], id: u64, cx: &mut Cx<'_>) -> Result<Vec<u64>, Error> {
    let OwnedHeader {
        mut bits, count, ..
    } = owned_header(data, Release::R2004, None).ok_or(Error::Unsupported)?;
    cx.check(Kind::DecodedBytes, (count as u64).saturating_mul(8))?;
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        cx.check(Kind::Work, 1)?;
        ids.push(
            bits.handle()
                .and_then(|h| h.resolve(id))
                .ok_or(Error::Unsupported)?,
        );
    }
    Ok(ids)
}

/// 拡張データ（EED）を読み飛ばす（ODS 5.4.1 の 28）
pub(in crate::dwg) fn skip_extended_data(bits: &mut Bits) -> Option<()> {
    loop {
        let size = bits.bs()?;
        if size == 0 {
            return Some(());
        }
        bits.handle()?;
        bits.skip_bytes(usize::try_from(size).ok()?)?;
    }
}

// ODS 5.4.1 の 20.1・21。再符号化に使うヘッダーの情報と、先頭のリアクターへのカーソル。
pub(in crate::dwg) struct DxfCommon<'a> {
    pub(in crate::dwg) handles: Bits<'a>,
    pub(in crate::dwg) count: u32,
    pub(in crate::dwg) picture: Option<(usize, usize)>,
    pub(in crate::dwg) plotstyle: bool,
}

pub(in crate::dwg) fn common_for_dxf(data: &[u8], entity: bool) -> Option<DxfCommon<'_>> {
    let mut bits = Bits::with_release(data, Release::R2004);
    bits.bs()?;
    let handles = usize::try_from(bits.rl()?).ok()?;
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    let (owner, reactors, picture, plotstyle) = if entity {
        let common = entity_data(&mut bits)?;
        (
            common.entmode == 0,
            common.reactors,
            common.picture,
            common.plotstyle_handle,
        )
    } else {
        (true, u32::try_from(bits.bl()?).ok()?, None, false)
    };
    bits.seek(handles);
    if owner {
        bits.handle()?;
    }
    Some(DxfCommon {
        handles: bits,
        count: reactors,
        picture,
        plotstyle,
    })
}

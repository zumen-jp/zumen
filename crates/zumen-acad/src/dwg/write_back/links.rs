//! 図形の追加と削除で、前後の図形・所有者・リアクターのハンドルをつなぎ直す。

use super::*;

/// 表から除ける図形の種類（決定 0051）
pub(super) const REMOVABLE: &[&[u8]] = &[
    b"LINE",
    b"CIRCLE",
    b"ARC",
    b"POINT",
    b"ELLIPSE",
    b"RAY",
    b"XLINE",
    b"SOLID",
    b"TRACE",
    b"3DFACE",
    b"TEXT",
    b"MTEXT",
    b"ATTDEF",
    b"LWPOLYLINE",
    b"SPLINE",
    b"INSERT",
    b"SHAPE",
];

/// 表に足せる図形の種類と型と、型ごとのフィールドのグループ・コード（決定 0052〜0060）。型ごとのフィールドを元のデータ
/// なしで符号化でき、型ごとのハンドルが表のレコードか、足した図形を指すものだけか、ないもの
pub(super) const ADDABLE: &[(&[u8], u16, &[i16])] = &[
    (b"LINE", kinds::LINE, LINE),
    (b"CIRCLE", kinds::CIRCLE, CIRCLE),
    (b"ARC", kinds::ARC, ARC),
    (b"POINT", kinds::POINT, POINT),
    (b"ELLIPSE", kinds::ELLIPSE, ELLIPSE),
    (b"RAY", kinds::RAY, RAY),
    (b"XLINE", kinds::XLINE, RAY),
    (b"SOLID", kinds::SOLID, SOLID),
    (b"TRACE", kinds::TRACE, SOLID),
    (b"3DFACE", kinds::FACE3D, FACE3D),
    (b"TEXT", kinds::TEXT, TEXT),
    (b"ATTDEF", kinds::ATTDEF, ATTDEF),
    (b"INSERT", kinds::INSERT, INSERT),
    (b"MTEXT", kinds::MTEXT, MTEXT),
    (b"LWPOLYLINE", kinds::LWPOLYLINE, LWPOLYLINE),
    (b"SPLINE", kinds::SPLINE, SPLINE),
    (b"ATTRIB", kinds::ATTRIB, ATTRIB),
    (b"SEQEND", kinds::SEQEND, &[]),
    (b"LEADER", kinds::LEADER, NEW_LEADER),
    // 足したブロックのレコードの BLOCK と ENDBLK（決定 0068）。BLOCK の名前（2）と基点は、レコードに合わせる
    (b"BLOCK", kinds::BLOCK, &[2, 10, 20, 30]),
    (b"ENDBLK", kinds::ENDBLK, &[]),
];

/// 足したブロックのレコードが持つもの（決定 0068）
#[derive(Default)]
pub(super) struct NewBlock {
    /// BLOCK と ENDBLK のハンドル
    pub(super) block: Option<u64>,
    pub(super) endblk: Option<u64>,
    /// BLOCK の基点
    pub(super) base: [f64; 3],
    /// 図形のハンドル。足した順
    pub(super) entities: Vec<u64>,
    /// このブロックを挿す、足した INSERT のハンドル
    pub(super) inserts: Vec<u64>,
    /// ATTDEF を持つか
    pub(super) attdef: bool,
}

/// 足す図形を符号化するのに要るもの
pub(super) struct NewEntity<'a> {
    pub(super) object: &'a Object,
    pub(super) handle: u64,
    pub(super) type_code: u16,
    /// entmode。2 はモデル空間、1 は用紙空間、0 はほかのブロック
    pub(super) mode: u8,
    /// 所有者のブロックのレコードのハンドル
    pub(super) owner: u64,
    /// 画層のハンドル
    pub(super) layer: u64,
    /// 線種のフラグと、フラグが 3 のときの線種のハンドル
    pub(super) ltype: (u8, u64),
    /// 型ごとに指す表のレコード（TEXT・ATTDEF の文字スタイル、INSERT のブロック）のハンドル
    pub(super) pointed: Option<u64>,
    /// R2018 からの複数行の ATTRIB の、埋め込みの MTEXT が指す ByLayer の線種と材質のハンドル
    pub(super) embedded: Option<(u64, u64)>,
    /// INSERT が持つ ATTRIB と、SEQEND のハンドル
    pub(super) children: Option<(Vec<u64>, u64)>,
}

impl NewEntity<'_> {
    /// 足す LEADER の注記（340）のハンドル。なければ 0
    pub(super) fn annotation(&self) -> Option<u64> {
        match self.object.first(340) {
            None => Some(0),
            Some(Data::Handle(handle)) => Some(*handle),
            Some(_) => None,
        }
    }

    /// 前後の図形のハンドルを `prev` と `next` にした、バージョン `release` の図形のデータ。共通のフィールドは、色（62）が
    /// なければ ByLayer、線種の尺度（48）がなければ 1、非表示（60）がなければ 0 にし、リアクターと拡張辞書と
    /// 図形の絵を持たず、印刷スタイルと線の太さは ByLayer にする。R2000 では前後の図形のハンドルを持ち、
    /// R2004 では持たず、拡張辞書を持たないフラグを書き、MTEXT の背景のフィールド（BL）を 0 にする（決定 0079）
    pub(super) fn encode(&self, prev: u64, next: u64, release: Release) -> Option<Vec<u8>> {
        if release >= Release::R2010 {
            return self.encode_r2010(release);
        }
        let object = self.object;
        let r2004 = release == Release::R2004;
        let mut common = BitWriter::default();
        common.handle(0, self.handle);
        // 拡張データがない
        common.bs(0);
        common.bit(false);
        common.bb(self.mode);
        common.bl(0);
        // R2000 は前後の図形のハンドルを持つフラグ、R2004 は拡張辞書を持たないフラグ
        common.bit(r2004);
        common.bs(small(object, 62)?.unwrap_or(256));
        common.bd(real(object, 48)?.unwrap_or(1.0));
        common.bb(self.ltype.0);
        common.bb(0);
        common.bs(small(object, 60)?.unwrap_or(0));
        common.rc(lineweight_index(-1)?.try_into().ok()?);
        let mut fields = if r2004 && self.type_code == kinds::MINSERT {
            self.minsert()?
        } else {
            own_fields(BitWriter::default(), object, object, self.type_code, &[], 0)?
        };
        if r2004 && object.kind == b"MTEXT" {
            fields.bl(0);
        }
        // R2004 の属性を持つ INSERT は、型ごとのフィールドの終わりに ATTRIB の数（BL）を持つ（決定 0080）
        if r2004
            && self.type_code != kinds::MINSERT
            && let Some((attribs, _)) = &self.children
        {
            fields.bl(i32::try_from(attribs.len()).ok()?);
        }
        let mut handles = BitWriter::default();
        if self.mode == 0 {
            handles.handle(4, self.owner);
        }
        if !r2004 {
            handles.handle(3, 0);
            handles.handle(4, prev);
            handles.handle(4, next);
        }
        handles.handle(5, self.layer);
        if self.ltype.0 == 3 {
            handles.handle(5, self.ltype.1);
        }
        // LEADER は、寸法スタイルの前に注記を指す。なければ 0（決定 0185）
        if object.kind == b"LEADER" {
            handles.handle(5, self.annotation()?);
        }
        if let Some(pointed) = self.pointed {
            handles.handle(5, pointed);
        }
        // INSERT は、ブロックに続けて、R2000 では最初と最後の ATTRIB を、R2004 ではすべての ATTRIB を固い
        // 所有で指し、SEQEND を指す（決定 0080）
        if let Some((attribs, seqend)) = &self.children {
            if r2004 {
                for &attrib in attribs {
                    handles.handle(3, attrib);
                }
            } else {
                handles.handle(4, *attribs.first()?);
                handles.handle(4, *attribs.last()?);
            }
            handles.handle(3, *seqend);
        }
        let mut out = BitWriter::default();
        out.bs(i16::try_from(self.type_code).ok()?);
        out.rl(i32::try_from(out.len() + 32 + common.len() + fields.len()).ok()?);
        for part in [common, fields, handles] {
            let len = part.len();
            out.copy(&part.into_bytes(), 0..len)?;
        }
        Some(out.into_bytes())
    }
    /// R2004 から後の、足す MINSERT の型ごとのフィールド。属性を持てば、属性が続くフラグの後、列と行のフィールドの前に ATTRIB の
    /// 数（BL）を書く（ODS 5.4.1 の 20.4.9）
    pub(super) fn minsert(&self) -> Option<BitWriter> {
        let mut fields = BitWriter::default();
        insert(&mut fields, self.object, false, None)?;
        if let Some((attribs, _)) = &self.children {
            fields.bl(i32::try_from(attribs.len()).ok()?);
        }
        grid_fields(&mut fields, self.object)?;
        Some(fields)
    }

    /// R2010 から後の形で符号化したデータ（決定 0092）。ハンドルのストリームの大きさ（MC）から始まる。材質は画層の
    /// もので、影のフラグは 0、表示スタイルは持たない
    pub(super) fn encode_r2010(&self, release: Release) -> Option<Vec<u8>> {
        let object = self.object;
        let mut out = BitWriter::default();
        out.bot(i16::try_from(self.type_code).ok()?);
        out.handle(0, self.handle);
        // 拡張データと図形の絵がない
        out.bs(0);
        out.bit(false);
        out.bb(self.mode);
        out.bl(0);
        // 拡張辞書を持たないフラグ。R2013 からは、DS のデータを持たないフラグ
        out.bit(true);
        if release >= Release::R2013 {
            out.bit(false);
        }
        out.bs(small(object, 62)?.unwrap_or(256));
        out.bd(real(object, 48)?.unwrap_or(1.0));
        out.bb(self.ltype.0);
        out.bb(0);
        // 材質のフラグ（BB）・影のフラグ（RC）・表示スタイルを持つかの 3 つの B
        out.bb(0);
        out.rc(0);
        for _ in 0..3 {
            out.bit(false);
        }
        out.bs(small(object, 60)?.unwrap_or(0));
        out.rc(lineweight_index(-1)?.try_into().ok()?);
        let mut fields = BitWriter::with_strings();
        match object.kind.as_slice() {
            b"ATTRIB" => r2010_attribute(&mut fields, None, object, &[], [0, 0], release)?,
            b"MTEXT" => r2010_mtext(&mut fields, None, object, &[], 0, 0, release)?,
            b"SPLINE" => spline(&mut fields, object, &[], 0, release)?,
            // 注釈の枠の高さと幅を持たないバージョンがあるので、バージョンを渡す（#345）
            b"LEADER" => leader(&mut fields, object, &[], [0, 0], release)?,
            _ if self.type_code == kinds::MINSERT => {
                let minsert = self.minsert()?;
                let len = minsert.len();
                fields.copy(&minsert.into_bytes(), 0..len)?;
            }
            _ => fields = own_fields(fields, object, object, self.type_code, &[], 0)?,
        }
        // 属性を持つ INSERT は、R2004 と同じく型ごとのフィールドの終わりに ATTRIB の数（BL）を持つ（決定 0093）
        if self.type_code != kinds::MINSERT
            && let Some((attribs, _)) = &self.children
        {
            fields.bl(i32::try_from(attribs.len()).ok()?);
        }
        let mut handles = BitWriter::default();
        if self.mode == 0 {
            handles.handle(4, self.owner);
        }
        handles.handle(5, self.layer);
        if self.ltype.0 == 3 {
            handles.handle(5, self.ltype.1);
        }
        // LEADER は、寸法スタイルの前に注記を指す。なければ 0（決定 0185）
        if object.kind == b"LEADER" {
            handles.handle(5, self.annotation()?);
        }
        if let Some(pointed) = self.pointed {
            handles.handle(5, pointed);
        }
        // R2018 からの MTEXT は、アプリケーションの名前を指す。サンプルの図面と同じく 0 とする（決定 0098）
        if object.kind == b"MTEXT" && release >= Release::R2018 {
            handles.handle(5, 0);
        }
        // 複数行の ATTRIB の埋め込みの MTEXT は、所有者（0）・画層・線種・材質・文字スタイル・アプリケーションの
        // 名前（0）を指す。サンプルの図面と同じ並び（決定 0102）
        if let Some((ltype, material)) = self.embedded {
            handles.handle(4, 0);
            for handle in [self.layer, ltype, material, self.pointed?, 0] {
                handles.handle(5, handle);
            }
        }
        if let Some((attribs, seqend)) = &self.children {
            for &attrib in attribs {
                handles.handle(3, attrib);
            }
            handles.handle(3, *seqend);
        }
        r2010_assemble(out, fields, handles)
    }
}

/// BLOCK の基点（10・20・30）。どれもなければ原点。一部だけなら `None`
pub(super) fn base_point(object: &Object) -> Option<[f64; 3]> {
    let point = [real(object, 10)?, real(object, 20)?, real(object, 30)?];
    if point.iter().all(Option::is_none) {
        return Some([0.0; 3]);
    }
    Some([point[0]?, point[1]?, point[2]?])
}

/// 名前のオブジェクトの辞書の項目 `group`（ACAD_MATERIAL など）が指す辞書の、項目 `name` が指すオブジェクトの
/// ハンドル
pub(super) fn dictionary_item(db: &AcadDb, group: &[u8], name: &[u8]) -> Option<u64> {
    let item = |dictionary: &Object, name: &[u8]| {
        dictionary.values.windows(2).find_map(|pair| match pair {
            [
                Value {
                    code: 3,
                    data: Data::Text(found),
                },
                Value {
                    code: 350 | 360,
                    data: Data::Handle(handle),
                },
            ] if found.eq_ignore_ascii_case(name) => Some(*handle),
            _ => None,
        })
    };
    let mut dictionaries = db.objects().iter().filter(|o| o.kind == b"DICTIONARY");
    let group = dictionaries.clone().find_map(|o| item(o, group))?;
    item(dictionaries.find(|o| o.handle() == Some(group))?, name)
}

/// 種類 `kind` の表のレコードのうち、名前（2）が `name` のもののハンドル。大文字と小文字は区別しない
pub(super) fn record_handle(db: &AcadDb, kind: &[u8], name: &[u8]) -> Option<u64> {
    db.objects()
        .iter()
        .filter(|o| o.kind == kind && o.section == Section::Tables)
        .find(|o| text_of(o, 2).is_some_and(|n| n.eq_ignore_ascii_case(name)))?
        .handle()
}

/// ハンドル `handle` の図形のデータ `data` の、前後の図形のハンドルを `prev` と `next` にしたもの。
/// 前後の図形のハンドルを持たない図形は、持つようにする。ハンドルのストリームの位置は変わらない
pub(super) fn relink(data: &[u8], handle: u64, prev: u64, next: u64) -> Option<Vec<u8>> {
    let links = entity_links(data, handle)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..links.no_links_at)?;
    out.bit(false);
    out.copy(data, links.no_links_at + 1..links.at.start)?;
    out.handle(4, prev);
    out.handle(4, next);
    out.copy(data, links.at.end..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `handle` のブロックのレコードのデータ `data` の、最初と最後の図形のハンドルを、`first` と
/// `last` にしたもの
pub(super) fn reblock(data: &[u8], handle: u64, first: u64, last: u64) -> Option<Vec<u8>> {
    let (at, _, _) = block_links(data, handle)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..at.start)?;
    out.handle(4, first);
    out.handle(4, last);
    out.copy(data, at.end..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `block` の R2004 のブロックのレコードのデータ `data` から、持つ図形のうち `gone` の `count` 個を
/// 除いたもの（決定 0077）。持つ図形の数（BL）を減らし、そのハンドルを除く
pub(super) fn disown(
    data: &[u8],
    block: u64,
    count: usize,
    gone: &BTreeSet<u64>,
) -> Option<Vec<u8>> {
    let owned = owned(data, block, Release::R2004, None)?;
    let kept: Vec<_> = owned
        .handles
        .iter()
        .filter(|(_, handle)| !gone.contains(handle))
        .collect();
    (owned.handles.len().checked_sub(kept.len())? == count).then_some(())?;
    let first = owned.handles.first()?.0.start;
    let last = owned.handles.last()?.0.end;
    let mut fields = BitWriter::default();
    fields.copy(data, owned.size_at + 32..owned.count.start)?;
    fields.bl(i32::try_from(kept.len()).ok()?);
    fields.copy(data, owned.count.end..owned.handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.size_at)?;
    out.rl(i32::try_from(owned.size_at + 32 + fields.len()).ok()?);
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, owned.handles_at..first)?;
    for (at, _) in kept {
        out.copy(data, at.clone())?;
    }
    out.copy(data, last..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `block` の R2010 から後のブロックのレコードのデータ `data`（ハンドルのストリームの大きさ（MC）の後）から、
/// `gone` の `count` 個の図形を除いたもの（決定 0091）。持つ図形の数（BL）を減らし、文字列のストリームは元のまま写して、
/// MC を付け直す。`handles_at` はハンドルのストリームの始まり
pub(super) fn disown_r2010(
    data: &[u8],
    handles_at: usize,
    block: u64,
    count: usize,
    gone: &BTreeSet<u64>,
    release: Release,
) -> Option<Vec<u8>> {
    let owned = owned(data, block, release, Some(handles_at))?;
    let kept: Vec<_> = owned
        .handles
        .iter()
        .filter(|(_, handle)| !gone.contains(handle))
        .collect();
    (owned.handles.len().checked_sub(kept.len())? == count).then_some(())?;
    let first = owned.handles.first()?.0.start;
    let last = owned.handles.last()?.0.end;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.count.start)?;
    out.bl(i32::try_from(kept.len()).ok()?);
    out.copy(data, owned.count.end..handles_at)?;
    let handles = out.len();
    out.copy(data, handles_at..first)?;
    for (at, _) in kept {
        out.copy(data, at.clone())?;
    }
    out.copy(data, last..data.len() * 8)?;
    with_modular_size(out, handles)
}

/// R2004 の DWG に足せるものか（決定 0079〜0081）。型ごとのフィールドが R2000 と同じ形の図形か、MTEXT か、
/// 属性を持つ INSERT とその ATTRIB・SEQEND か、表のレコードと、足したブロックの BLOCK・ENDBLK だけを
/// 足せる。格子を持つもの（MINSERT）も足せる
pub(super) fn r2004_addable(object: &Object) -> bool {
    const RECORDS: [&[u8]; 8] = [
        b"LAYER",
        b"STYLE",
        b"LTYPE",
        b"APPID",
        b"DIMSTYLE",
        b"VIEW",
        b"VPORT",
        b"BLOCK_RECORD",
    ];
    if object.section == Section::Tables {
        return RECORDS.contains(&object.kind.as_slice());
    }
    const KINDS: [&[u8]; 21] = [
        b"LEADER",
        b"BLOCK",
        b"ENDBLK",
        b"LINE",
        b"CIRCLE",
        b"ARC",
        b"POINT",
        b"ELLIPSE",
        b"RAY",
        b"XLINE",
        b"SOLID",
        b"TRACE",
        b"3DFACE",
        b"TEXT",
        b"ATTDEF",
        b"MTEXT",
        b"LWPOLYLINE",
        b"SPLINE",
        b"INSERT",
        b"ATTRIB",
        b"SEQEND",
    ];
    matches!(object.section, Section::Entities | Section::Blocks)
        && KINDS.contains(&object.kind.as_slice())
}

/// ハンドル `block` の R2004 のブロックのレコードのデータ `data` の、持つ図形の終わりに `handles` を加えた
/// もの（決定 0079）。持つ図形の数（BL）を増やし、ハンドルは固い所有（符号 3）で書く（図面で確かめた）
pub(super) fn adopt(data: &[u8], block: u64, handles: &[u64]) -> Option<Vec<u8>> {
    let owned = owned(data, block, Release::R2004, None)?;
    let mut fields = BitWriter::default();
    fields.copy(data, owned.size_at + 32..owned.count.start)?;
    fields.bl(i32::try_from(owned.handles.len().checked_add(handles.len())?).ok()?);
    fields.copy(data, owned.count.end..owned.handles_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.size_at)?;
    out.rl(i32::try_from(owned.size_at + 32 + fields.len()).ok()?);
    let len = fields.len();
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, owned.handles_at..owned.end)?;
    for &handle in handles {
        out.handle(3, handle);
    }
    out.copy(data, owned.end..data.len() * 8)?;
    Some(out.into_bytes())
}

/// ハンドル `block` の R2010 から後のブロックのレコードのデータ `data`（ハンドルのストリームの大きさ（MC）の後）の、
/// 持つ図形の終わりに `handles` を加えたもの（決定 0092）。`handles_at` はハンドルのストリームの始まり
pub(super) fn adopt_r2010(
    data: &[u8],
    handles_at: usize,
    block: u64,
    handles: &[u64],
    release: Release,
) -> Option<Vec<u8>> {
    let owned = owned(data, block, release, Some(handles_at))?;
    let mut out = BitWriter::default();
    out.copy(data, 0..owned.count.start)?;
    out.bl(i32::try_from(owned.handles.len().checked_add(handles.len())?).ok()?);
    out.copy(data, owned.count.end..handles_at)?;
    let at = out.len();
    out.copy(data, handles_at..owned.end)?;
    for &handle in handles {
        out.handle(3, handle);
    }
    out.copy(data, owned.end..data.len() * 8)?;
    with_modular_size(out, at)
}

/// R2010 から後の DWG に足せるものか（決定 0092〜0102）。型ごとのフィールドと文字列のストリームが R2000 と同じ形に
/// 書ける図形と、MTEXT と、属性を持つ INSERT とその ATTRIB・SEQEND と、SPLINE（R2013 からは
/// 制御点のものだけ）と、
/// UCS のほかの表のレコードと、足したブロックを足せる。格子を持つもの（MINSERT）も足せる
pub(super) fn r2010_addable(object: &Object) -> bool {
    // 表のレコードは、LAYER・STYLE・LTYPE・APPID（決定 0094）と、DIMSTYLE・VIEW・ブロックのレコード
    // （決定 0095）を足せる
    if object.section == Section::Tables {
        return matches!(
            object.kind.as_slice(),
            b"LAYER"
                | b"STYLE"
                | b"LTYPE"
                | b"APPID"
                | b"DIMSTYLE"
                | b"VIEW"
                | b"VPORT"
                | b"BLOCK_RECORD"
        );
    }
    // 足したブロックの BLOCK・ENDBLK と、ブロックの持つ図形も足せる（決定 0095）
    let kind = object.kind.as_slice();
    if matches!(kind, b"BLOCK" | b"ENDBLK") {
        return object.section == Section::Blocks;
    }
    const KINDS: [&[u8]; 18] = [
        b"LEADER",
        b"LINE",
        b"CIRCLE",
        b"ARC",
        b"POINT",
        b"ELLIPSE",
        b"RAY",
        b"XLINE",
        b"SOLID",
        b"TRACE",
        b"3DFACE",
        b"TEXT",
        b"LWPOLYLINE",
        b"INSERT",
        b"MTEXT",
        b"SPLINE",
        b"ATTRIB",
        b"SEQEND",
    ];
    matches!(object.section, Section::Entities | Section::Blocks) && KINDS.contains(&kind)
}

pub(super) fn offset(object: &Object) -> Option<usize> {
    match object.origin {
        Origin::Dwg { offset, .. } => Some(offset),
        Origin::Dxf { .. } | Origin::New => None,
    }
}

/// MULTILEADER のデータ `data` の実数（BD）の値を、`old` から `new` に変えたもの（決定 0191）。読む側
/// （`multileader`）で実数のビットの範囲を求め、変わった実数だけを符号化し直し、ほかのビットは元のまま写す。
/// 値の数・並び・実数でない値は変えられない（枝の線の頂点の数、注記の文字、ハンドルなど）。R2004 までのバージョンだけを
/// 扱う。R2010 から後は、文字列のストリームを分けたテストの図面をまだ作っていないので断る
pub(super) fn callout(
    data: &[u8],
    old: &Object,
    new: &Object,
    release: Release,
) -> Option<Vec<u8>> {
    if old.values.len() != new.values.len() {
        return None;
    }
    let mut changed = Vec::new();
    for (at, (before, after)) in old.values.iter().zip(&new.values).enumerate() {
        match (before, after) {
            _ if before == after => {}
            (
                Value {
                    code,
                    data: Data::Real(_),
                },
                Value {
                    code: next,
                    data: Data::Real(value),
                },
            ) if code == next => changed.push((at, *value)),
            _ => return None,
        }
    }
    let mut bits = Bits::with_release(data, release);
    let [size_at, _, start, end] = entity_fields(data, release)?;
    bits.seek(start);
    let mut reals = Vec::new();
    let tagged = multileader(&mut bits, release, &mut 0, Some(&mut reals))?;
    // 型ごとのフィールドの値は、文脈の区切り（R2010 からは、その前のバージョンの 270）から並ぶ。読む側は 0 のハンドルと置かない
    // ハンドルを除くので、読み直した値のハンドルは、表の値と符号が合わなければ飛ばす
    let first = old.values.iter().position(|v| {
        v.code == 300 && matches!(&v.data, Data::Text(t) if t.as_slice() == b"CONTEXT_DATA{")
    })?;
    let mut at = first.checked_sub(usize::from(release >= Release::R2010))?;
    let mut found = BTreeMap::new();
    for value in &tagged {
        let same = old.values.get(at).is_some_and(|v| {
            v.code == value.code
                && matches!(v.data, Data::Real(_)) == matches!(value.data, Data::Real(_))
        });
        match value.data {
            Data::Real(index) if same => {
                found.insert(at, reals.get(index as usize)?.clone());
            }
            _ if same => {}
            Data::Handle(_) => continue,
            _ => return None,
        }
        at += 1;
    }
    let mut writes = Vec::with_capacity(changed.len());
    for (at, value) in changed {
        writes.push((found.get(&at)?.clone(), value));
    }
    writes.sort_by_key(|(range, _)| range.start);
    let mut fields = BitWriter::default();
    let mut pos = start;
    for (range, value) in writes {
        fields.copy(data, pos..range.start)?;
        fields.bd(value);
        pos = range.end;
    }
    fields.copy(data, pos..end)?;
    let len = fields.len();
    let mut out = BitWriter::default();
    out.copy(data, 0..size_at)?;
    out.rl(i32::try_from(start + len).ok()?);
    out.copy(data, size_at + 32..start)?;
    out.copy(&fields.into_bytes(), 0..len)?;
    out.copy(data, end..data.len() * 8)?;
    Some(out.into_bytes())
}

/// `old` になく `new` が持つリアクターの組（`102 {ACAD_REACTORS`・330・`102 }`）のハンドルと、組を除いた `new`。
/// DWG を読む側はリアクターを値に置かないので、組は書き戻しで足したものだけ（決定 0191）。`old` が組を持てば
/// 何も除かない
pub(super) fn without_reactors(old: &Object, new: &Object) -> (Vec<u64>, Object) {
    let marker = |value: &Value, text: &[u8]| {
        value.code == 102 && matches!(&value.data, Data::Text(t) if t.as_slice() == text)
    };
    let open = |object: &Object| {
        object
            .values
            .iter()
            .position(|v| marker(v, b"{ACAD_REACTORS"))
    };
    let mut copy = new.clone();
    let (None, Some(at)) = (open(old), open(new)) else {
        return (Vec::new(), copy);
    };
    let Some(close) = new.values.iter().skip(at).position(|v| marker(v, b"}")) else {
        return (Vec::new(), copy);
    };
    let mut handles = Vec::new();
    for value in copy.values.drain(at..=at + close).skip(1) {
        match value {
            Value {
                code: 330,
                data: Data::Handle(handle),
            } => handles.push(handle),
            value if marker(&value, b"}") => {}
            _ => return (Vec::new(), new.clone()),
        }
    }
    (handles, copy)
}

/// バージョン `release` の図形のデータ `data` のリアクターに、ハンドル `reactors` を足したもの（決定 0191）。リアクターの数（BL）
/// を増やし、ハンドルのストリームのリアクターの終わりに柔らかい参照（符号 4）で書く。R2010 から後は、`handles_at` が
/// ハンドルのストリームの始まりで、`data` はハンドルのストリームの大きさ（MC）の後。返すデータは、R2010 から後は MC から
/// 始まる
pub(super) fn react(
    data: &[u8],
    handles_at: Option<usize>,
    release: Release,
    reactors: &[u64],
) -> Option<Vec<u8>> {
    let mut bits = Bits::with_release(data, release);
    let (size_at, handles_at) = match handles_at {
        Some(at) => {
            bits.bot()?;
            (None, at)
        }
        None => {
            bits.bs()?;
            let size_at = bits.pos();
            (Some(size_at), usize::try_from(bits.rl()?).ok()?)
        }
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    if bits.bit()? {
        let size = if release >= Release::R2010 {
            bits.bll()?
        } else {
            u64::try_from(bits.rl()?).ok()?
        };
        bits.skip_bytes(usize::try_from(size).ok()?)?;
    }
    let entmode = bits.bb()?;
    let count_at = bits.pos();
    let count = u32::try_from(bits.bl()?).ok()?;
    let count_end = bits.pos();
    if count_end > handles_at {
        return None;
    }
    bits.seek(handles_at);
    if entmode == 0 {
        bits.handle()?;
    }
    for _ in 0..count {
        bits.handle()?;
    }
    let insert_at = bits.pos();
    let total = i32::try_from(u64::from(count).checked_add(reactors.len() as u64)?).ok()?;
    let mut out = BitWriter::default();
    match size_at {
        Some(size_at) => {
            let mut fields = BitWriter::default();
            fields.copy(data, size_at + 32..count_at)?;
            fields.bl(total);
            fields.copy(data, count_end..handles_at)?;
            out.copy(data, 0..size_at)?;
            out.rl(i32::try_from(size_at + 32 + fields.len()).ok()?);
            let len = fields.len();
            out.copy(&fields.into_bytes(), 0..len)?;
        }
        None => {
            out.copy(data, 0..count_at)?;
            out.bl(total);
            out.copy(data, count_end..handles_at)?;
        }
    }
    let at = out.len();
    out.copy(data, handles_at..insert_at)?;
    for &reactor in reactors {
        out.handle(4, reactor);
    }
    out.copy(data, insert_at..data.len() * 8)?;
    match size_at {
        Some(_) => Some(out.into_bytes()),
        None => with_modular_size(out, at),
    }
}

/// 拡張辞書の組（`102 {ACAD_XDICTIONARY`・360・`102 }`）の 360 の値と、組を除いたオブジェクト
pub(super) fn without_xdictionary(object: &Object) -> (Option<Data>, Object) {
    let mut copy = object.clone();
    let at = copy.values.windows(3).position(|values| {
        matches!(values, [open, dictionary, close]
            if open.code == 102
                && open.data == Data::Text(b"{ACAD_XDICTIONARY".to_vec())
                && dictionary.code == 360
                && close.code == 102
                && close.data == Data::Text(b"}".to_vec()))
    });
    let Some(at) = at else {
        return (None, copy);
    };
    let removed: Vec<Value> = copy.values.drain(at..at + 3).collect();
    (removed.get(1).map(|value| value.data.clone()), copy)
}

pub(super) fn unsupported(kind: &[u8]) -> Box<Diagnostic> {
    let kind = String::from_utf8_lossy(kind).into_owned();
    Box::new(Diagnostic::new(codes::DWG_CHANGE_UNSUPPORTED).with("kind", kind))
}

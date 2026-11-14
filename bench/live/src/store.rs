//! 文書エンジンの状態と、DXF を開いてページ列・表示バッファー・空間索引を組む処理。
//!
//! 元のバイト列は写さずに持ち、レコードの先頭位置だけを `Vec<u32>` に置く。モデル空間の 6 種類の図形
//! （LINE・CIRCLE・ARC・LWPOLYLINE・TEXT・INSERT）と ATTRIB は、並列のスレッドで直接ページ列に持ち上げ、
//! 画層・線種・文字スタイル・寸法スタイル・ブロック定義は `zumen-acad` の `SingleCatalog` と共有する。

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;
use std::thread;

pub use zumen_acad::Charset;
use zumen_acad::single::{
    self, CatalogShell, Follow, Followed, RawTag, SingleCatalog, SingleTables,
};
use zumen_acad::{Value, charset as charset_from_header};
use zumen_base::Ref;
use zumen_drawing::{
    DimensionStyle, Element, Layer, LayerId, LineType, LineTypeId, ModelPoint, SymbolDefinition,
    TextStyle, Unit,
};

use crate::display::{Display, RowBuffers};
use crate::edit::History;
use crate::grid::Grid;
use crate::lift::{
    self, AttribLift, AttribRow, BLOCK, DropReason, Geom, INSERT_FOLLOWED, InsertRow, Kind, Lift,
    LineTypeRef, NO_DEFINITION, POLY_CLOSED, Pools, TextRow,
};
use crate::paged::{Paged, RecordSlice, SourceBytes, TextSpan};
use crate::scan::{self, Next, Span};
use crate::value::{self, Val};

/// バイナリ DXF の先頭印
const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

/// 名前で指す定義の表（`Born` と同じ 5 種類）
const TABLE_KINDS: [&[u8]; 5] = [b"LAYER", b"LTYPE", b"STYLE", b"BLOCK_RECORD", b"DIMSTYLE"];

/// 公開する表の種類名
pub mod table {
    /// 画層の表
    pub const LAYER: &[u8] = b"LAYER";
    /// 線種の表
    pub const LTYPE: &[u8] = b"LTYPE";
    /// 文字スタイルの表
    pub const STYLE: &[u8] = b"STYLE";
}

/// 開くときの誤り
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// バイナリの DXF
    BinaryDxf,
    /// 4 GiB 以上の入力
    TooLarge,
    /// タグとして切れない位置がある
    InvalidTag {
        /// 入力の先頭からのバイト位置
        byte: usize,
    },
}

/// 変更・追加されたレコードの 1 つのタグ
#[derive(Clone, Debug)]
pub struct OTag {
    /// グループ・コード
    pub code: i16,
    /// 元のバイト範囲（足したタグでは使わない）
    pub span: Span,
    /// 書き換えた値（改行を除く）
    pub edited: Option<Vec<u8>>,
    /// 新しく足したタグか
    pub fresh: bool,
}

impl OTag {
    /// 新しく足すタグを作る
    pub fn added(code: i16, value: Vec<u8>) -> Self {
        Self {
            code,
            span: Span {
                code,
                start: 0,
                value: 0,
                value_end: 0,
                end: 0,
            },
            edited: Some(value),
            fresh: true,
        }
    }

    /// 値の行のバイト列（改行を除く）
    pub fn raw<'a>(&'a self, src: &'a [u8]) -> &'a [u8] {
        match &self.edited {
            Some(bytes) => bytes.as_slice(),
            None => src
                .get(self.span.value as usize..self.span.value_end as usize)
                .unwrap_or_default(),
        }
    }

    /// 展開済みのレコード区間から値の行のバイト列（改行を除く）を借りる
    #[inline]
    pub fn raw_in<'a>(&'a self, slice: &RecordSlice<'a>) -> &'a [u8] {
        match &self.edited {
            Some(bytes) => bytes.as_slice(),
            None => slice.slice(self.span.value, self.span.value_end),
        }
    }

    /// `zumen_acad::Value` に変換する
    pub fn to_value_in(&self, slice: &RecordSlice<'_>) -> Value {
        value::value(self.code, self.raw_in(slice))
    }

    /// レコードの種類 `kind` に合わせて `zumen_acad::Value` に変換する（`DIMSTYLE` の 5 を文字列として扱う）
    pub fn to_value_for_in(&self, kind: &[u8], slice: &RecordSlice<'_>) -> Value {
        value::value_of(kind, self.code, self.raw_in(slice))
    }

    /// 保持している追加ヒープバイト数
    pub fn heap_bytes(&self) -> usize {
        self.edited.as_ref().map_or(0, Vec::capacity)
    }
}

/// 折れ線のページ列の 1 行（24B）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolylineRow {
    /// 一定の幅（43）。0.0 は幅なし
    pub width: f64,
    /// 最初の頂点の番号
    pub first: u32,
    /// 頂点の数
    pub count: u32,
    /// 旗（`POLY_CLOSED`・`POLY_GENERATION`・`POLY_WIDTHS`）
    pub flags: u8,
}

/// 変更または追加された 1 レコードの差分保持
#[derive(Clone, Debug)]
pub struct OverlayRecord {
    /// `raw` の先頭バイトのファイル内オフセット
    pub base: u32,
    /// 元のレコードの未圧縮バイト列（`HEADER` や追加レコードでは空）
    pub raw: Box<[u8]>,
    /// 今のタグ列
    pub tags: Vec<OTag>,
}

impl OverlayRecord {
    /// 追加されたレコード（すべてのタグが `edited: Some(...)`）を作る
    pub fn born(tags: Vec<OTag>) -> Self {
        Self {
            base: 0,
            raw: Box::new([]),
            tags,
        }
    }

    /// 元のレコードのスライスから `raw` と `tags` を組む
    pub fn from_slice(rs: RecordSlice<'_>) -> Self {
        let tags = read_record_spans_slice(&rs)
            .into_iter()
            .map(|span| OTag {
                code: span.code,
                span,
                edited: None,
                fresh: false,
            })
            .collect();
        Self {
            base: rs.base as u32,
            raw: rs.bytes.into(),
            tags,
        }
    }

    /// 保持しているヒープバイト数
    pub fn heap_bytes(&self) -> usize {
        self.raw.len()
            + self.tags.capacity() * size_of::<OTag>()
            + self.tags.iter().map(OTag::heap_bytes).sum::<usize>()
    }
}

/// モデル空間の 6 種類の図形の 1 行（32B）
#[derive(Clone, Copy, Debug)]
pub struct Row {
    handle_raw: u64,
    /// レコードの番号（足した図形は `source_records()` 以上）
    pub record: u32,
    /// 種類別のページ列の中の番号
    pub shape: u32,
    /// 画層の番号（`LayerId` の中身）
    pub layer: u32,
    /// 詰め込んだ色
    pub color: u32,
    /// 追加の共通属性（1 始まり、`0` なら既定）
    pub extra_style: u32,
    /// 図形の種類
    pub kind: Kind,
    /// 生きているか
    pub alive: bool,
    has_handle: bool,
}

impl Row {
    /// 行を作る
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        record: u32,
        handle: Option<u64>,
        kind: Kind,
        shape: u32,
        layer: u32,
        color: u32,
        extra_style: u32,
        alive: bool,
    ) -> Self {
        let (handle_raw, has_handle) = match handle {
            Some(h) => (h, true),
            None => (0, false),
        };
        Self {
            handle_raw,
            record,
            shape,
            layer,
            color,
            extra_style,
            kind,
            alive,
            has_handle,
        }
    }

    /// ハンドル（5）
    #[inline]
    pub const fn handle(&self) -> Option<u64> {
        if self.has_handle {
            Some(self.handle_raw)
        } else {
            None
        }
    }
}

/// 種類別の形のページ列と可変長プール
#[derive(Default)]
pub struct Shapes {
    /// 線分 `[x0, y0, x1, y1]`
    pub lines: Paged<[f64; 4]>,
    /// 円 `[cx, cy, r]`
    pub circles: Paged<[f64; 3]>,
    /// 円弧 `[cx, cy, r, start_deg, end_deg]`
    pub arcs: Paged<[f64; 5]>,
    /// 折れ線
    pub polylines: Paged<PolylineRow>,
    /// 1 行の文字
    pub texts: Paged<TextRow>,
    /// 記号の配置
    pub inserts: Paged<InsertRow>,
    /// 頂点・文字・属性などのプール
    pub pools: Pools,
}

impl Shapes {
    /// 行が指す形を読む
    pub fn geom(&self, kind: Kind, index: u32) -> Geom {
        let idx = index as usize;
        match kind {
            Kind::Line => Geom::Line(self.lines.get(idx).copied().unwrap_or_default()),
            Kind::Circle => Geom::Circle(self.circles.get(idx).copied().unwrap_or_default()),
            Kind::Arc => Geom::Arc(self.arcs.get(idx).copied().unwrap_or_default()),
            Kind::Polyline => {
                let row = self.polylines.get(idx).copied().unwrap_or(PolylineRow {
                    width: 0.0,
                    first: 0,
                    count: 0,
                    flags: 0,
                });
                Geom::Polyline {
                    first: row.first,
                    count: row.count,
                    flags: row.flags,
                    width: row.width,
                }
            }
            Kind::Text => Geom::Text(self.texts.get(idx).copied().unwrap_or(TextRow {
                point: [0.0; 2],
                height: 1.0,
                rotation: 0.0,
                content: TextSpan::EMPTY,
                style: TextSpan::STANDARD,
                extra: 0,
                h_align: 0,
                v_align: 0,
                flags: 0,
                pad: 0,
            })),
            Kind::Insert => Geom::Insert(self.inserts.get(idx).copied().unwrap_or(InsertRow {
                position: [0.0; 2],
                scale: [1.0; 2],
                rotation: 0.0,
                name: TextSpan::EMPTY,
                definition: NO_DEFINITION,
                attrib_first: 0,
                attrib_count: 0,
                attached: 0,
                extra: 0,
                flags: 0,
                pad: [0; 3],
            })),
        }
    }

    /// 形を末尾に足して種類と番号を返す
    pub fn push(&mut self, geom: Geom) -> (Kind, u32) {
        match geom {
            Geom::Line(v) => {
                let id = self.lines.len() as u32;
                self.lines.push(v);
                (Kind::Line, id)
            }
            Geom::Circle(v) => {
                let id = self.circles.len() as u32;
                self.circles.push(v);
                (Kind::Circle, id)
            }
            Geom::Arc(v) => {
                let id = self.arcs.len() as u32;
                self.arcs.push(v);
                (Kind::Arc, id)
            }
            Geom::Polyline {
                first,
                count,
                flags,
                width,
            } => {
                let id = self.polylines.len() as u32;
                self.polylines.push(PolylineRow {
                    width,
                    first,
                    count,
                    flags,
                });
                (Kind::Polyline, id)
            }
            Geom::Text(v) => {
                let id = self.texts.len() as u32;
                self.texts.push(v);
                (Kind::Text, id)
            }
            Geom::Insert(v) => {
                let id = self.inserts.len() as u32;
                self.inserts.push(v);
                (Kind::Insert, id)
            }
        }
    }

    /// 既存のスロットの形を書き換える
    pub fn set(&mut self, kind: Kind, index: u32, geom: Geom) {
        let idx = index as usize;
        match (kind, geom) {
            (Kind::Line, Geom::Line(v)) => {
                if let Some(slot) = self.lines.get_mut(idx) {
                    *slot = v;
                }
            }
            (Kind::Circle, Geom::Circle(v)) => {
                if let Some(slot) = self.circles.get_mut(idx) {
                    *slot = v;
                }
            }
            (Kind::Arc, Geom::Arc(v)) => {
                if let Some(slot) = self.arcs.get_mut(idx) {
                    *slot = v;
                }
            }
            (
                Kind::Polyline,
                Geom::Polyline {
                    first,
                    count,
                    flags,
                    width,
                },
            ) => {
                if let Some(slot) = self.polylines.get_mut(idx) {
                    *slot = PolylineRow {
                        width,
                        first,
                        count,
                        flags,
                    };
                }
            }
            (Kind::Text, Geom::Text(v)) => {
                if let Some(slot) = self.texts.get_mut(idx) {
                    *slot = v;
                }
            }
            (Kind::Insert, Geom::Insert(v)) => {
                if let Some(slot) = self.inserts.get_mut(idx) {
                    *slot = v;
                }
            }
            _ => {}
        }
    }

    /// 種類の配列の末尾を 1 つ取り出す（足した行の取り消し用）
    pub fn pop(&mut self, kind: Kind) {
        match kind {
            Kind::Line => {
                self.lines.pop();
            }
            Kind::Circle => {
                self.circles.pop();
            }
            Kind::Arc => {
                self.arcs.pop();
            }
            Kind::Polyline => {
                self.polylines.pop();
            }
            Kind::Text => {
                self.texts.pop();
            }
            Kind::Insert => {
                self.inserts.pop();
            }
        }
    }

    /// 確保しているバイト数
    pub fn bytes(&self) -> usize {
        self.lines.bytes()
            + self.circles.bytes()
            + self.arcs.bytes()
            + self.polylines.bytes()
            + self.texts.bytes()
            + self.inserts.bytes()
            + self.pools.bytes()
    }
}

/// セクションの種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    Header,
    Classes,
    Tables,
    Blocks,
    Entities,
    Objects,
    Other,
    Outside,
}

impl SectionKind {
    fn from_name(raw: &[u8]) -> Self {
        match raw {
            b"HEADER" => Self::Header,
            b"CLASSES" => Self::Classes,
            b"TABLES" => Self::Tables,
            b"BLOCKS" => Self::Blocks,
            b"ENTITIES" => Self::Entities,
            b"OBJECTS" => Self::Objects,
            _ => Self::Other,
        }
    }

    pub(crate) fn has_db_objects(self) -> bool {
        self != Self::Outside
    }
}

/// 開いた図面の件数の内訳
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// 元のファイルのレコード数
    pub records: u32,
    /// `ENTITIES` セクション内のオブジェクト数
    pub entities: u32,
    /// 生きているモデル空間の 6 種類の図形数
    pub elements: u32,
    /// ハンドルがないか重複してハンドル ID で指せない図形数
    pub unaddressable: u32,
    /// 6 種類のうち値や平面条件で落とした件数
    pub dropped: u32,
    /// ペーパー空間（67 = 1）の件数
    pub paper: u32,
    /// 6 種類以外の ENTITIES の件数
    pub unlifted_entities: u32,
}

/// 開いたときに記録した 6 種類の図形と単位の診断の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpenDiagnosticKind {
    /// `$INSUNITS` がないか不明（`dxf.units.missing`）
    UnitsMissing,
    /// グループ・コードの値が無効（`dxf.lift.invalid`）
    LiftInvalid,
    /// 平面外（`dxf.non-planar.dropped`）
    NonPlanar,
    /// 未対応の文字コード（`dxf.code-page.unsupported`）
    CodePage,
    /// 切り抜きの値が無効で落とした（`dxf.clip.dropped`）
    ClipDropped,
    /// 参照先のブロック定義がない（`dxf.block.missing`）
    BlockMissing,
}

impl OpenDiagnosticKind {
    /// 今の実装の診断コード文字列
    pub fn code_str(self) -> &'static str {
        match self {
            Self::UnitsMissing => "dxf.units.missing",
            Self::LiftInvalid => "dxf.lift.invalid",
            Self::NonPlanar => "dxf.non-planar.dropped",
            Self::CodePage => "dxf.code-page.unsupported",
            Self::ClipDropped => "dxf.clip.dropped",
            Self::BlockMissing => "dxf.block.missing",
        }
    }
}

/// 開いたときの診断の 1 件
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenDiagnostic {
    /// 診断の種類
    pub kind: OpenDiagnosticKind,
    /// 出所のレコード番号
    pub record: Option<u32>,
    /// 出所のハンドル
    pub handle: Option<u64>,
}

/// 6 種類以外（またはペーパー空間）の図形が参照する画層・線種の記録
#[derive(Clone, Debug)]
pub struct NonSliceUsage {
    /// レコード番号
    pub record: u32,
    /// 参照している画層
    pub layers: Vec<LayerId>,
    /// 参照している線種
    pub line_types: Vec<LineTypeId>,
}

/// ヘッダーの変数と書き戻し位置
#[derive(Clone, Debug, Default)]
pub(crate) struct HeaderState {
    pub record: Option<u32>,
    /// HEADER レコードの全タグのバイト範囲
    pub spans: Vec<Span>,
    /// `$HANDSEED` の値のタグ範囲 `(start_tag_idx, tag_count)`
    pub handseed: Option<(usize, usize)>,
    /// `$EXTMIN` の値のタグ範囲 `(start_tag_idx, tag_count)`
    pub extmin: Option<(usize, usize)>,
    /// `$EXTMAX` の値のタグ範囲 `(start_tag_idx, tag_count)`
    pub extmax: Option<(usize, usize)>,
    /// `$HANDSEED` 以外の HEADER 変数が持つハンドル
    pub handles: BTreeSet<u64>,
    pub insunits: Option<i64>,
    pub plimmin: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Default)]
enum BaseHandles {
    #[default]
    Empty,
    Dense {
        min: u64,
        slots: Vec<u32>,
    },
    Sparse(Vec<(u64, u32)>),
}

/// 一意なハンドルから行番号を引く省メモリ索引。
///
/// 元の要素はハンドルの幅が狭ければ連続配列（`4 B/件`）、広ければソート済み配列（`16 B/件`）に置き、
/// 足した要素だけを小さい `BTreeMap` に持つ。
#[derive(Clone, Debug, Default)]
pub struct HandleIndex {
    base: BaseHandles,
    base_max_cursor: usize,
    added: BTreeMap<u64, u32>,
}

impl HandleIndex {
    const NONE_SLOT: u32 = u32::MAX;
    const DUP_SLOT: u32 = u32::MAX - 1;

    fn with_range(min_h: Option<u64>, max_h: Option<u64>, est_rows: usize) -> Self {
        let base = match (min_h, max_h) {
            (Some(min), Some(max)) if max >= min => {
                let span = max - min;
                let limit = (est_rows as u64).saturating_mul(8).max(4096);
                if span <= limit
                    && let Ok(len) = usize::try_from(span + 1)
                {
                    BaseHandles::Dense {
                        min,
                        slots: vec![Self::NONE_SLOT; len],
                    }
                } else {
                    BaseHandles::Sparse(Vec::with_capacity(est_rows))
                }
            }
            _ => BaseHandles::Empty,
        };
        Self {
            base,
            base_max_cursor: usize::MAX,
            added: BTreeMap::new(),
        }
    }

    /// 構築中に元の行 `(handle, row_idx)` を入れる。
    /// `Dense` のときは重複検出の結果（`0`: 新規、`2`: 2 件目の衝突、`1`: 3 件目以降の衝突）を返す
    #[inline]
    fn push_base_raw(&mut self, handle: u64, row_idx: u32) -> Option<u32> {
        match &mut self.base {
            BaseHandles::Empty => Some(0),
            BaseHandles::Dense { min, slots } => {
                let off = (handle - *min) as usize;
                let slot = slots.get_mut(off)?;
                match *slot {
                    Self::NONE_SLOT => {
                        *slot = row_idx;
                        Some(0)
                    }
                    Self::DUP_SLOT => Some(1),
                    _ => {
                        *slot = Self::DUP_SLOT;
                        Some(2)
                    }
                }
            }
            BaseHandles::Sparse(pairs) => {
                pairs.push((handle, row_idx));
                None
            }
        }
    }

    fn finish_base<F>(&mut self, duplicate_handles: &mut BTreeSet<u64>, mut on_dup: F) -> u32
    where
        F: FnMut(u64),
    {
        let mut extra_unaddressable = 0u32;
        match &mut self.base {
            BaseHandles::Empty => {
                self.base_max_cursor = usize::MAX;
            }
            BaseHandles::Dense { min, slots } => {
                for &dup_h in duplicate_handles.iter() {
                    if let Some(off) = dup_h
                        .checked_sub(*min)
                        .and_then(|d| usize::try_from(d).ok())
                        && let Some(slot) = slots.get_mut(off)
                    {
                        *slot = Self::NONE_SLOT;
                    }
                }
                self.base_max_cursor = slots
                    .iter()
                    .rposition(|&s| s != Self::NONE_SLOT)
                    .unwrap_or(usize::MAX);
            }
            BaseHandles::Sparse(pairs) => {
                pairs.sort_unstable_by_key(|&(h, _)| h);
                let mut write = 0usize;
                let mut i = 0usize;
                while i < pairs.len() {
                    let h = pairs[i].0;
                    let mut j = i + 1;
                    while j < pairs.len() && pairs[j].0 == h {
                        j += 1;
                    }
                    if j - i == 1 && !duplicate_handles.contains(&h) {
                        pairs[write] = pairs[i];
                        write += 1;
                    } else {
                        duplicate_handles.insert(h);
                        on_dup(h);
                        extra_unaddressable += (j - i) as u32;
                    }
                    i = j;
                }
                pairs.truncate(write);
                self.base_max_cursor = if pairs.is_empty() {
                    usize::MAX
                } else {
                    pairs.len() - 1
                };
            }
        }
        extra_unaddressable
    }

    /// 生きている行番号を引く
    #[inline]
    pub fn get(&self, handle: u64, rows: &Paged<Row>) -> Option<u32> {
        if let Some(&idx) = self.added.get(&handle) {
            return rows
                .get(idx as usize)
                .is_some_and(|r| r.alive)
                .then_some(idx);
        }
        match &self.base {
            BaseHandles::Empty => None,
            BaseHandles::Dense { min, slots } => {
                let off = usize::try_from(handle.checked_sub(*min)?).ok()?;
                let &idx = slots.get(off)?;
                (idx != Self::NONE_SLOT && rows.get(idx as usize).is_some_and(|r| r.alive))
                    .then_some(idx)
            }
            BaseHandles::Sparse(pairs) => {
                let pos = pairs.binary_search_by_key(&handle, |&(h, _)| h).ok()?;
                let idx = pairs.get(pos)?.1;
                rows.get(idx as usize)
                    .is_some_and(|r| r.alive)
                    .then_some(idx)
            }
        }
    }

    /// 足した行を登録する
    pub fn insert_added(&mut self, handle: u64, row_idx: u32) {
        self.added.insert(handle, row_idx);
    }

    /// 行が削除（または `pop`）されたときに索引を更新する
    pub fn remove_row(&mut self, handle: u64, row_idx: u32, is_added: bool, rows: &Paged<Row>) {
        if is_added {
            if self.added.get(&handle) == Some(&row_idx) {
                self.added.remove(&handle);
            }
            return;
        }
        self.shrink_cursor(rows);
    }

    /// 削除されていた元の行が復活したときにカーソルを戻す
    pub fn revive_base(&mut self, handle: u64) {
        match &self.base {
            BaseHandles::Empty => {}
            BaseHandles::Dense { min, slots } => {
                if let Some(off) = handle
                    .checked_sub(*min)
                    .and_then(|d| usize::try_from(d).ok())
                    && slots.get(off).is_some_and(|&s| s != Self::NONE_SLOT)
                    && (self.base_max_cursor == usize::MAX || off > self.base_max_cursor)
                {
                    self.base_max_cursor = off;
                }
            }
            BaseHandles::Sparse(pairs) => {
                if let Ok(pos) = pairs.binary_search_by_key(&handle, |&(h, _)| h)
                    && (self.base_max_cursor == usize::MAX || pos > self.base_max_cursor)
                {
                    self.base_max_cursor = pos;
                }
            }
        }
    }

    fn shrink_cursor(&mut self, rows: &Paged<Row>) {
        if self.base_max_cursor == usize::MAX {
            return;
        }
        match &self.base {
            BaseHandles::Empty => {
                self.base_max_cursor = usize::MAX;
            }
            BaseHandles::Dense { slots, .. } => {
                let mut cur = self.base_max_cursor;
                loop {
                    if let Some(&idx) = slots.get(cur)
                        && idx != Self::NONE_SLOT
                        && rows.get(idx as usize).is_some_and(|r| r.alive)
                    {
                        self.base_max_cursor = cur;
                        return;
                    }
                    if cur == 0 {
                        self.base_max_cursor = usize::MAX;
                        return;
                    }
                    cur -= 1;
                }
            }
            BaseHandles::Sparse(pairs) => {
                let mut cur = self.base_max_cursor;
                loop {
                    if let Some(&(_, idx)) = pairs.get(cur)
                        && rows.get(idx as usize).is_some_and(|r| r.alive)
                    {
                        self.base_max_cursor = cur;
                        return;
                    }
                    if cur == 0 {
                        self.base_max_cursor = usize::MAX;
                        return;
                    }
                    cur -= 1;
                }
            }
        }
    }

    /// 生きている最大のハンドルを `O(1)` で返す
    pub fn max_live(&self, rows: &Paged<Row>) -> Option<u64> {
        let base_max = if self.base_max_cursor == usize::MAX {
            None
        } else {
            match &self.base {
                BaseHandles::Empty => None,
                BaseHandles::Dense { min, slots } => slots
                    .get(self.base_max_cursor)
                    .filter(|&&idx| {
                        idx != Self::NONE_SLOT && rows.get(idx as usize).is_some_and(|r| r.alive)
                    })
                    .map(|_| *min + self.base_max_cursor as u64),
                BaseHandles::Sparse(pairs) => pairs
                    .get(self.base_max_cursor)
                    .filter(|&&(_, idx)| rows.get(idx as usize).is_some_and(|r| r.alive))
                    .map(|&(h, _)| h),
            }
        };
        let added_max = self.added.last_key_value().map(|(&h, _)| h);
        match (base_max, added_max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }

    /// `removed` に含まれるハンドルを除いた生きている最大のハンドルを返す
    pub fn max_live_excluding(&self, rows: &Paged<Row>, removed: &BTreeSet<u64>) -> Option<u64> {
        let added_max = self
            .added
            .keys()
            .rev()
            .find(|h| !removed.contains(h))
            .copied();
        let base_max = if self.base_max_cursor == usize::MAX {
            None
        } else {
            match &self.base {
                BaseHandles::Empty => None,
                BaseHandles::Dense { min, slots } => {
                    let mut cur = self.base_max_cursor;
                    loop {
                        let h = *min + cur as u64;
                        if let Some(&idx) = slots.get(cur)
                            && idx != Self::NONE_SLOT
                            && !removed.contains(&h)
                            && rows.get(idx as usize).is_some_and(|r| r.alive)
                        {
                            break Some(h);
                        }
                        if cur == 0 {
                            break None;
                        }
                        cur -= 1;
                    }
                }
                BaseHandles::Sparse(pairs) => {
                    let mut cur = self.base_max_cursor;
                    loop {
                        if let Some(&(h, idx)) = pairs.get(cur)
                            && !removed.contains(&h)
                            && rows.get(idx as usize).is_some_and(|r| r.alive)
                        {
                            break Some(h);
                        }
                        if cur == 0 {
                            break None;
                        }
                        cur -= 1;
                    }
                }
            }
        };
        match (base_max, added_max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }

    /// 索引が確保しているバイト数
    pub fn bytes(&self) -> usize {
        let base_b = match &self.base {
            BaseHandles::Empty => 0,
            BaseHandles::Dense { slots, .. } => slots.capacity() * size_of::<u32>(),
            BaseHandles::Sparse(pairs) => pairs.capacity() * size_of::<(u64, u32)>(),
        };
        base_b + self.added.len() * size_of::<(u64, u32)>()
    }
}

/// 常駐する DXF 文書
pub struct Document {
    pub(crate) src: SourceBytes,
    pub(crate) record_starts: Vec<u32>,
    /// 件数の内訳
    pub counts: Counts,
    /// 長さの単位
    pub unit: Unit,
    /// `$ACADVER`（前後の空白を除いたもの）
    pub version: Vec<u8>,
    /// 文字の符号化
    pub charset: Charset,
    /// 画層の表
    pub layers: Vec<Layer>,
    /// ASCII 小文字の画層名から引く初期表
    pub by_layer_name: BTreeMap<String, LayerId>,
    /// 線種の表
    pub line_types: Vec<LineType>,
    /// ASCII 小文字の線種名から引く初期表
    pub by_line_type_name: BTreeMap<String, LineTypeId>,
    /// 寸法スタイルの表
    pub dimension_styles: Vec<DimensionStyle>,
    /// 文字スタイルの表
    pub text_styles: Vec<TextStyle>,
    /// ブロック定義の表
    pub symbols: Vec<SymbolDefinition>,
    /// 大文字・小文字を区別するブロック定義名から最初の定義番号への対応
    pub def_by_name: BTreeMap<String, u32>,
    /// モデル空間の `SORTENTSTABLE`
    pub model_sort: Option<(Ref, BTreeMap<u64, u64>)>,
    /// モデル空間の 6 種類の図形の行
    pub rows: Paged<Row>,
    /// 一意なハンドルから行番号への索引（重複したハンドルは入れない）
    pub by_handle: HandleIndex,
    /// ENTITIES 内で重複していたハンドル
    pub duplicate_handles: BTreeSet<u64>,
    pub(crate) base_row_count: u32,
    pub(crate) added_by_record: BTreeMap<u32, u32>,
    /// 種類別の形とプール
    pub shapes: Shapes,
    /// 行ごとの囲む箱 `[min_x, min_y, max_x, max_y]`
    pub boxes: Paged<[f64; 4]>,
    /// 空間の格子索引
    pub grid: Grid,
    /// 表示用の頂点・文字・インスタンスのバッファー
    pub display: Display,
    /// 開いたときの診断
    pub open_diagnostics: Vec<OpenDiagnostic>,
    pub(crate) overlay: BTreeMap<u32, OverlayRecord>,
    pub(crate) gone: BTreeSet<u32>,
    pub(crate) history: History,
    pub(crate) next_added_record: u32,
    pub(crate) entities: Vec<Range<u32>>,
    pub(crate) entities_end: Option<u32>,
    pub(crate) sections: Vec<(SectionKind, Range<u32>)>,
    pub(crate) header: HeaderState,
    pub(crate) tables_cache: SingleTables,
    pub(crate) has_duplicate_object_handle: bool,
    pub(crate) objects_by_handle: BTreeMap<u64, u32>,
    pub(crate) fixed_max_handle: Option<u64>,
    pub(crate) live_object_handles: BTreeSet<u64>,
    pub(crate) added_follower_handles: BTreeSet<u64>,
    pub(crate) forget_refs: Vec<(u64, u32)>,
    pub(crate) plain: bool,
    pub(crate) ent_family: Vec<(u32, Followed)>,
    pub(crate) layer_uses: Vec<u32>,
    pub(crate) line_type_uses: Vec<u32>,
    pub(crate) non_slice_usages: Vec<NonSliceUsage>,
}

/// 分割した 1 区間の中の印
enum Mark {
    Section(u32, SectionKind),
    EndSec(u32),
    Eof(u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OtherKind {
    NonObject,
    Seqend,
    Lifted,
    Other,
}

/// 分割した 1 区間で拾った ENTITIES 候補
enum PartItem {
    Lifted {
        local_rec: u32,
        lifted: lift::Lifted,
        has_xdict: bool,
        has_100: bool,
    },
    Attrib {
        local_rec: u32,
        handle: Option<u64>,
        lift: AttribLift,
        has_100: bool,
    },
    Paper {
        local_rec: u32,
        handle: Option<u64>,
        owner: Option<u64>,
        has_100: bool,
    },
    Dropped {
        local_rec: u32,
        handle: Option<u64>,
        reason: DropReason,
        has_100: bool,
    },
    OtherEntity {
        local_rec: u32,
        handle: Option<u64>,
        other_kind: OtherKind,
        has_100: bool,
    },
}

/// 1 区間の `INSERT`・`ATTRIB`・`SEQEND` の並びが単純（`plain`）かの要約（32 バイト）
#[derive(Clone, Copy, Debug)]
struct PlainSummary {
    is_empty: bool,
    ok: bool,
    needs_incoming: bool,
    closes_incoming: bool,
    required_owner: Option<u64>,
    outgoing_open: Option<Option<u64>>,
}

impl Default for PlainSummary {
    fn default() -> Self {
        Self {
            is_empty: true,
            ok: true,
            needs_incoming: false,
            closes_incoming: false,
            required_owner: None,
            outgoing_open: None,
        }
    }
}

impl PlainSummary {
    fn feed(&mut self, role: Followed) {
        if !self.ok {
            return;
        }
        if matches!(role.kind, Follow::Polyline { .. } | Follow::Vertex) {
            self.is_empty = false;
            self.ok = false;
            return;
        }
        if self.is_empty {
            self.is_empty = false;
            if matches!(role.kind, Follow::Attrib | Follow::Seqend) {
                self.needs_incoming = true;
            }
        }
        if self.needs_incoming && !self.closes_incoming {
            match role.kind {
                Follow::Insert { .. } => {
                    self.ok = false;
                }
                Follow::Attrib => {
                    if let Some(ow) = role.owner {
                        if let Some(req) = self.required_owner {
                            if req != ow {
                                self.ok = false;
                            }
                        } else {
                            self.required_owner = Some(ow);
                        }
                    }
                }
                Follow::Seqend => {
                    if let Some(ow) = role.owner {
                        if let Some(req) = self.required_owner {
                            if req != ow {
                                self.ok = false;
                            }
                        } else {
                            self.required_owner = Some(ow);
                        }
                    }
                    self.closes_incoming = true;
                    self.outgoing_open = None;
                }
                Follow::Polyline { .. } | Follow::Vertex => {
                    self.ok = false;
                }
            }
            return;
        }
        match role.kind {
            Follow::Insert { opens } => {
                if self.outgoing_open.is_some() {
                    self.ok = false;
                } else if opens {
                    self.outgoing_open = Some(role.handle);
                }
            }
            Follow::Attrib => match self.outgoing_open {
                None => self.ok = false,
                Some(parent_h) => {
                    if role.owner.is_some() && role.owner != parent_h {
                        self.ok = false;
                    }
                }
            },
            Follow::Seqend => match self.outgoing_open {
                None => self.ok = false,
                Some(parent_h) => {
                    if role.owner.is_some() && role.owner != parent_h {
                        self.ok = false;
                    }
                    self.outgoing_open = None;
                }
            },
            Follow::Polyline { .. } | Follow::Vertex => {
                self.ok = false;
            }
        }
    }

    fn feed_non_family(&mut self) {
        if (self.needs_incoming && !self.closes_incoming) || self.outgoing_open.is_some() {
            self.ok = false;
        }
    }
}

fn combine_plain_summaries(summaries: impl IntoIterator<Item = PlainSummary>) -> bool {
    let mut cur: Option<Option<u64>> = None;
    for s in summaries {
        if !s.ok {
            return false;
        }
        if s.is_empty {
            continue;
        }
        if s.needs_incoming {
            let Some(parent_h) = cur else {
                return false;
            };
            if s.required_owner.is_some() && s.required_owner != parent_h {
                return false;
            }
            if s.closes_incoming {
                cur = s.outgoing_open;
            }
        } else {
            if cur.is_some() {
                return false;
            }
            cur = s.outgoing_open;
        }
    }
    cur.is_none()
}

/// 1 スレッドが読んだ区間の結果
pub struct Part {
    starts: Vec<u32>,
    record_count: u32,
    marks: Vec<Mark>,
    eof_end: Option<u32>,
    prefix_items_end: usize,
    items: Vec<PartItem>,
    prefix_plain: PlainSummary,
    body_plain: PlainSummary,
    pools: Pools,
    prefix_non_ent_handles: Vec<(u32, u64)>,
    known_non_ent_handles: Vec<(u32, u64)>,
    forget_refs: Vec<(u64, u32)>,
}

/// 段階計測のためにスキャン結果とページ圧縮済みソースをまとめた中間状態
pub struct Scanned {
    source: SourceBytes,
    parts: Vec<Part>,
    guessed_charset: Charset,
}

fn is_section_delimiter(kind: &[u8]) -> bool {
    matches!(kind, b"SECTION" | b"ENDSEC" | b"EOF")
}

fn is_non_object_kind(kind: &[u8]) -> bool {
    matches!(kind, b"SECTION" | b"ENDSEC" | b"TABLE" | b"ENDTAB" | b"EOF")
}

/// 先頭 256 KiB 以内に `HEADER` があれば `$ACADVER` と `$DWGCODEPAGE` から文字コードを先に決める
fn pre_scan_header(src: &[u8]) -> Charset {
    let limit = src.len().min(256 * 1024);
    let head = src.get(..limit).unwrap_or_default();
    let mut pos = 0usize;
    let mut in_header = false;
    let mut current_var: Option<&[u8]> = None;
    let mut first_val_for_var = false;
    let mut version: Option<&[u8]> = None;
    let mut code_page: Option<&[u8]> = None;
    let mut seen_vars: BTreeSet<&[u8]> = BTreeSet::new();
    while pos < head.len() {
        let Next::Tag(span) = scan::next_tag(head, pos) else {
            break;
        };
        pos = span.end as usize;
        let val = head
            .get(span.value as usize..span.value_end as usize)
            .unwrap_or_default()
            .trim_ascii();
        if span.code == 0 {
            if val == b"SECTION" {
                if let Next::Tag(next) = scan::next_tag(head, pos)
                    && next.code == 2
                {
                    let sec = head
                        .get(next.value as usize..next.value_end as usize)
                        .unwrap_or_default()
                        .trim_ascii();
                    pos = next.end as usize;
                    if sec == b"HEADER" {
                        in_header = true;
                        continue;
                    }
                }
                if in_header {
                    break;
                }
            } else if in_header && val == b"ENDSEC" {
                break;
            }
            continue;
        }
        if !in_header {
            continue;
        }
        if span.code == 9 {
            current_var = seen_vars.insert(val).then_some(val);
            first_val_for_var = true;
            continue;
        }
        if first_val_for_var {
            first_val_for_var = false;
            match (current_var, span.code) {
                (Some(b"$ACADVER"), 1) => version = Some(val),
                (Some(b"$DWGCODEPAGE"), 3) => code_page = Some(val),
                _ => {}
            }
        }
    }
    charset_from_header(version, code_page)
}

struct PartBuilder {
    charset: Charset,
    starts: Vec<u32>,
    marks: Vec<Mark>,
    eof_end: Option<u32>,
    items: Vec<PartItem>,
    prefix_items_end: Option<usize>,
    prefix_plain: PlainSummary,
    body_plain: PlainSummary,
    in_prefix: bool,
    pools: Pools,
    prefix_non_ent_handles: Vec<(u32, u64)>,
    known_non_ent_handles: Vec<(u32, u64)>,
    forget_refs: Vec<(u64, u32)>,
    sec_state: Option<SectionKind>,
}

impl PartBuilder {
    fn new(at_zero: bool, charset: Charset) -> Self {
        Self {
            charset,
            starts: Vec::new(),
            marks: Vec::new(),
            eof_end: None,
            items: Vec::new(),
            prefix_items_end: None,
            prefix_plain: PlainSummary::default(),
            body_plain: PlainSummary::default(),
            in_prefix: !at_zero,
            pools: Pools::default(),
            prefix_non_ent_handles: Vec::new(),
            known_non_ent_handles: Vec::new(),
            forget_refs: Vec::new(),
            sec_state: at_zero.then_some(SectionKind::Outside),
        }
    }

    /// 1 レコード分のタグ列を処理する。`EOF` に達したら `true` を返す
    fn push_record(&mut self, rec_start: u32, tags: &[Span], buf: &RecordSlice<'_>) -> bool {
        let local_rec = self.starts.len() as u32;
        self.starts.push(rec_start);

        let Some(&first) = tags.first() else {
            return false;
        };
        if first.code != 0 {
            return false;
        }
        let kind_raw = buf.slice(first.value, first.value_end).trim_ascii();
        let body = tags.get(1..).unwrap_or_default();

        if kind_raw == b"EOF" {
            self.marks.push(Mark::Eof(local_rec, first.end));
            self.eof_end = Some(first.end);
            return true;
        }
        if kind_raw == b"SECTION" {
            if self.prefix_items_end.is_none() {
                self.prefix_items_end = Some(self.items.len());
            }
            self.in_prefix = false;
            let sec_name = body
                .iter()
                .find(|t| t.code == 2)
                .map(|t| buf.slice(t.value, t.value_end).trim_ascii())
                .unwrap_or_default();
            let sk = SectionKind::from_name(sec_name);
            self.sec_state = Some(sk);
            self.marks.push(Mark::Section(local_rec, sk));
            return false;
        }
        if kind_raw == b"ENDSEC" {
            if self.prefix_items_end.is_none() {
                self.prefix_items_end = Some(self.items.len());
            }
            self.in_prefix = false;
            self.sec_state = Some(SectionKind::Outside);
            self.marks.push(Mark::EndSec(local_rec));
            return false;
        }

        let obj_handle = if !is_section_delimiter(kind_raw) {
            handle_in_spans_slice(kind_raw, body, buf)
        } else {
            None
        };
        if let Some(h) = obj_handle {
            match self.sec_state {
                None => self.prefix_non_ent_handles.push((local_rec, h)),
                Some(sk) if sk.has_db_objects() && sk != SectionKind::Entities => {
                    self.known_non_ent_handles.push((local_rec, h));
                }
                Some(_) => {}
            }
        }

        let mut has_100 = false;
        let mut has_102 = false;
        let mut has_360 = false;
        for t in body {
            match t.code {
                100 => has_100 = true,
                102 => has_102 = true,
                360 => has_360 = true,
                _ => {}
            }
        }
        if self.sec_state != Some(SectionKind::Outside) && (has_102 || kind_raw == b"DICTIONARY") {
            collect_forget_refs_in_spans(kind_raw, body, buf, local_rec, &mut self.forget_refs);
        }

        if !matches!(self.sec_state, None | Some(SectionKind::Entities)) {
            return false;
        }

        let slice_kind = Kind::of(kind_raw);
        let needs_raw = slice_kind.is_some()
            || matches!(kind_raw, b"ATTRIB" | b"POLYLINE" | b"VERTEX" | b"SEQEND");
        let mut raw_inline = [RawTag {
            code: 0,
            value: b"",
        }; 32];
        let mut raw_heap: Vec<RawTag<'_>> = Vec::new();
        let raw_slice: &[RawTag<'_>] = if !needs_raw {
            &[]
        } else if let Some(dst) = raw_inline.get_mut(..body.len()) {
            for (slot, t) in dst.iter_mut().zip(body) {
                *slot = RawTag {
                    code: t.code,
                    value: buf.slice(t.value, t.value_end),
                };
            }
            dst
        } else {
            raw_heap.reserve(body.len());
            for t in body {
                raw_heap.push(RawTag {
                    code: t.code,
                    value: buf.slice(t.value, t.value_end),
                });
            }
            &raw_heap
        };

        if matches!(
            kind_raw,
            b"INSERT" | b"POLYLINE" | b"ATTRIB" | b"VERTEX" | b"SEQEND"
        ) {
            if let Some(role) = single::followed_tags(kind_raw, raw_slice) {
                if self.in_prefix {
                    self.prefix_plain.feed(role);
                } else {
                    self.body_plain.feed(role);
                }
            }
        } else if !is_non_object_kind(kind_raw) {
            if self.in_prefix {
                self.prefix_plain.feed_non_family();
            } else {
                self.body_plain.feed_non_family();
            }
        }

        if let Some(slice_kind) = slice_kind {
            match lift::lift(self.charset, slice_kind, raw_slice, &mut self.pools) {
                Lift::Element(lifted) => {
                    let has_xdict = slice_kind == Kind::Insert && has_102 && has_360;
                    self.items.push(PartItem::Lifted {
                        local_rec,
                        lifted,
                        has_xdict,
                        has_100,
                    });
                }
                Lift::Paper => {
                    let owner = first_owner_in_spans(body, buf);
                    self.items.push(PartItem::Paper {
                        local_rec,
                        handle: obj_handle,
                        owner,
                        has_100,
                    });
                }
                Lift::Not(reason) => {
                    self.items.push(PartItem::Dropped {
                        local_rec,
                        handle: obj_handle,
                        reason,
                        has_100,
                    });
                }
            }
        } else if kind_raw == b"ATTRIB" {
            let lifted = lift::attrib(self.charset, raw_slice, 0, &mut self.pools);
            self.items.push(PartItem::Attrib {
                local_rec,
                handle: obj_handle,
                lift: lifted,
                has_100,
            });
        } else {
            let other_kind = if is_non_object_kind(kind_raw) {
                OtherKind::NonObject
            } else if kind_raw == b"SEQEND" {
                OtherKind::Seqend
            } else if single::is_other_lifted_entity(kind_raw) {
                OtherKind::Lifted
            } else {
                OtherKind::Other
            };
            self.items.push(PartItem::OtherEntity {
                local_rec,
                handle: obj_handle,
                other_kind,
                has_100,
            });
        }
        false
    }

    fn finish(self) -> Part {
        let record_count = self.starts.len() as u32;
        let prefix_items_end = self.prefix_items_end.unwrap_or(self.items.len());
        Part {
            starts: self.starts,
            record_count,
            marks: self.marks,
            eof_end: self.eof_end,
            prefix_items_end,
            items: self.items,
            prefix_plain: self.prefix_plain,
            body_plain: self.body_plain,
            pools: self.pools,
            prefix_non_ent_handles: self.prefix_non_ent_handles,
            known_non_ent_handles: self.known_non_ent_handles,
            forget_refs: self.forget_refs,
        }
    }
}

fn scan_part(src: &[u8], range: Range<usize>, charset: Charset) -> Result<Part, OpenError> {
    let mut builder = PartBuilder::new(range.start == 0, charset);
    let mut pos = range.start;
    let mut tags: Vec<Span> = Vec::with_capacity(64);
    let full_slice = RecordSlice {
        base: 0,
        bytes: src,
    };

    while pos < range.end {
        let rec_start = pos as u32;
        tags.clear();
        loop {
            match scan::next_tag(src, pos) {
                Next::Tag(span) => {
                    if span.code == 0 && !tags.is_empty() {
                        break;
                    }
                    pos = span.end as usize;
                    tags.push(span);
                    if pos >= src.len() {
                        break;
                    }
                    if span.code == 0 {
                        let val = src
                            .get(span.value as usize..span.value_end as usize)
                            .unwrap_or_default()
                            .trim_ascii();
                        if val == b"EOF" {
                            break;
                        }
                    }
                    if pos >= range.end {
                        match scan::next_tag(src, pos) {
                            Next::Tag(peek) if peek.code == 0 => break,
                            Next::End => break,
                            Next::Invalid => return Err(OpenError::InvalidTag { byte: pos }),
                            Next::Tag(_) => {}
                        }
                    }
                }
                Next::End => break,
                Next::Invalid => return Err(OpenError::InvalidTag { byte: pos }),
            }
        }
        if tags.is_empty() {
            break;
        }
        if builder.push_record(rec_start, &tags, &full_slice) {
            break;
        }
    }
    Ok(builder.finish())
}

fn has_two_newlines(bytes: &[u8]) -> bool {
    let mut count = 0usize;
    for &b in bytes {
        if b == b'\n' {
            count += 1;
            if count >= 2 {
                return true;
            }
        }
    }
    false
}

fn shift_span(mut span: Span, base: u32) -> Span {
    span.start += base;
    span.value += base;
    span.value_end += base;
    span.end += base;
    span
}

fn scan_source_part(
    source: &SourceBytes,
    range: Range<usize>,
    charset: Charset,
) -> Result<Part, OpenError> {
    let mut builder = PartBuilder::new(range.start == 0, charset);
    let mut pos = range.start;
    let mut next_page = pos >> SourceBytes::PAGE_BITS;
    let mut buf_base = next_page << SourceBytes::PAGE_BITS;
    let mut buf = Vec::with_capacity(SourceBytes::PAGE_SIZE * 4);
    let mut tags: Vec<Span> = Vec::with_capacity(64);

    let ensure_two_lines = |buf: &mut Vec<u8>, next_page: &mut usize, local_pos: usize| {
        while *next_page < source.page_count() {
            let tail = buf.get(local_pos..).unwrap_or_default();
            let probe = tail.get(..tail.len().min(512)).unwrap_or(tail);
            if has_two_newlines(probe) || has_two_newlines(tail) {
                break;
            }
            source.append_page(buf, *next_page);
            *next_page += 1;
        }
    };

    while pos < range.end {
        if pos - buf_base >= SourceBytes::PAGE_SIZE * 2 {
            let drop_n = pos - buf_base;
            buf.drain(..drop_n);
            buf_base = pos;
        }
        let rec_start = pos as u32;
        tags.clear();
        loop {
            let local_pos = pos - buf_base;
            ensure_two_lines(&mut buf, &mut next_page, local_pos);
            match scan::next_tag(&buf, local_pos) {
                Next::Tag(local_span) => {
                    let span = shift_span(local_span, buf_base as u32);
                    if span.code == 0 && !tags.is_empty() {
                        break;
                    }
                    pos = span.end as usize;
                    tags.push(span);
                    if pos >= source.len() {
                        break;
                    }
                    if span.code == 0 {
                        let val = buf
                            .get(local_span.value as usize..local_span.value_end as usize)
                            .unwrap_or_default()
                            .trim_ascii();
                        if val == b"EOF" {
                            break;
                        }
                    }
                    if pos >= range.end {
                        let peek_local = pos - buf_base;
                        ensure_two_lines(&mut buf, &mut next_page, peek_local);
                        match scan::next_tag(&buf, peek_local) {
                            Next::Tag(peek) if peek.code == 0 => break,
                            Next::End => break,
                            Next::Invalid => return Err(OpenError::InvalidTag { byte: pos }),
                            Next::Tag(_) => {}
                        }
                    }
                }
                Next::End => break,
                Next::Invalid => return Err(OpenError::InvalidTag { byte: pos }),
            }
        }
        if tags.is_empty() {
            break;
        }
        let slice = RecordSlice {
            base: buf_base,
            bytes: &buf,
        };
        if builder.push_record(rec_start, &tags, &slice) {
            break;
        }
    }
    Ok(builder.finish())
}

fn split_points_source(source: &SourceBytes, parts: usize) -> Vec<usize> {
    let n = source.len();
    let parts = parts.max(1);
    let pages = source.page_count();
    if parts == 1 || n < parts * SourceBytes::PAGE_SIZE || pages < parts {
        return vec![0, n];
    }
    let page_nl = source.page_newlines();
    let mut out = vec![0usize];
    let mut summed_pages = 0usize;
    let mut before_nl = 0usize;
    let mut scratch = Vec::with_capacity(SourceBytes::PAGE_SIZE * 2);

    for k in 1..parts {
        let target_page = pages * k / parts;
        for &nl in page_nl.get(summed_pages..target_page).unwrap_or_default() {
            before_nl += usize::from(nl);
        }
        summed_pages = target_page;
        let at = target_page << SourceBytes::PAGE_BITS;
        scratch.clear();
        let mut p = target_page;
        source.append_page(&mut scratch, p);
        p += 1;
        if p < pages {
            source.append_page(&mut scratch, p);
            p += 1;
        }
        let prev_ended_lf = if target_page == 0 {
            true
        } else {
            source.with_range(at - 1..at, |rs| rs.bytes == b"\n")
        };
        let mut local = 0usize;
        let mut number = before_nl;
        if !prev_ended_lf {
            match scratch.iter().position(|&b| b == b'\n') {
                Some(nl_pos) => {
                    local = nl_pos + 1;
                    number += 1;
                }
                None => continue,
            }
        }
        if number % 2 == 1 {
            if let Some(nl_pos) = scratch
                .get(local..)
                .and_then(|s| s.iter().position(|&b| b == b'\n'))
            {
                local += nl_pos + 1;
            } else {
                continue;
            }
        }
        let found = loop {
            while p < pages && !has_two_newlines(scratch.get(local..).unwrap_or_default()) {
                source.append_page(&mut scratch, p);
                p += 1;
            }
            match scan::next_tag(&scratch, local) {
                Next::Tag(tag) if tag.code == 0 => break Some(at + local),
                Next::Tag(tag) => local = tag.end as usize,
                Next::End | Next::Invalid => break None,
            }
        };
        if let Some(cut) = found
            && out.last().is_some_and(|&last| cut > last)
        {
            out.push(cut);
        }
    }
    out.push(n);
    out
}

fn handle_in_spans_slice(kind_raw: &[u8], spans: &[Span], buf: &RecordSlice<'_>) -> Option<u64> {
    let expected = if kind_raw == b"DIMSTYLE" { 105 } else { 5 };
    spans
        .iter()
        .filter(|s| s.code == expected)
        .find_map(|span| {
            let raw = buf.slice(span.value, span.value_end);
            match value::typed(expected, raw) {
                Val::Handle(h) => Some(h),
                _ => None,
            }
        })
}

fn collect_forget_refs_in_spans(
    kind_raw: &[u8],
    spans: &[Span],
    buf: &RecordSlice<'_>,
    local_rec: u32,
    out: &mut Vec<(u64, u32)>,
) {
    let is_dict = kind_raw == b"DICTIONARY";
    let mut depth = 0usize;
    let mut prev_was_key = false;
    for span in spans {
        let raw = buf.slice(span.value, span.value_end);
        match span.code {
            102 if raw.starts_with(b"{") => {
                depth += 1;
                prev_was_key = false;
            }
            102 if raw.starts_with(b"}") => {
                depth = depth.saturating_sub(1);
                prev_was_key = false;
            }
            330 | 360 if depth > 0 => {
                if let Val::Handle(h) = value::typed(span.code, raw) {
                    out.push((h, local_rec));
                }
                prev_was_key = false;
            }
            3 if is_dict && depth == 0 => {
                prev_was_key = true;
            }
            350 | 360 if is_dict && depth == 0 && prev_was_key => {
                if let Val::Handle(h) = value::typed(span.code, raw) {
                    out.push((h, local_rec));
                }
                prev_was_key = false;
            }
            _ => {
                prev_was_key = false;
            }
        }
    }
}

fn first_owner_in_spans(spans: &[Span], buf: &RecordSlice<'_>) -> Option<u64> {
    let mut depth = 0usize;
    for span in spans {
        let raw = buf.slice(span.value, span.value_end);
        match span.code {
            102 if raw.trim_ascii().starts_with(b"{") => depth += 1,
            102 if raw.trim_ascii().starts_with(b"}") => depth = depth.saturating_sub(1),
            330 if depth == 0 => {
                return match value::typed(330, raw) {
                    Val::Handle(h) => Some(h),
                    _ => None,
                };
            }
            _ => {}
        }
    }
    None
}

/// テスト用: 小さい入力でも強制的に最大 `parts` 区間へ切る分割点を作る
fn split_points_forced(src: &[u8], parts: usize) -> Vec<usize> {
    let n = src.len();
    let parts = parts.max(1);
    if parts == 1 || n == 0 {
        return vec![0, n];
    }
    let mut out = vec![0usize];
    let mut pos = 0usize;
    let mut rec_positions = Vec::new();
    while pos < n {
        let Next::Tag(span) = scan::next_tag(src, pos) else {
            break;
        };
        if span.code == 0 && pos > 0 {
            rec_positions.push(pos);
        }
        pos = span.end as usize;
    }
    if !rec_positions.is_empty() {
        for k in 1..parts {
            let idx = rec_positions.len() * k / parts;
            if let Some(&cut) = rec_positions.get(idx)
                && out.last().is_some_and(|&last| cut > last)
            {
                out.push(cut);
            }
        }
    }
    out.push(n);
    out
}

fn scan_with_cuts(src: &[u8], cuts: &[usize], charset: Charset) -> Result<Vec<Part>, OpenError> {
    if cuts.len() <= 2 {
        return Ok(vec![scan_part(src, 0..src.len(), charset)?]);
    }
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(cuts.len() - 1);
    for pair in cuts.windows(2) {
        if pair[0] < pair[1] {
            ranges.push(pair[0]..pair[1]);
        }
    }
    thread::scope(|scope| {
        let handles: Vec<_> = ranges
            .into_iter()
            .map(|r| scope.spawn(move || scan_part(src, r, charset)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err(OpenError::InvalidTag { byte: 0 })))
            .collect()
    })
}

fn scan_source_with_cuts(
    source: &SourceBytes,
    cuts: &[usize],
    charset: Charset,
) -> Result<Vec<Part>, OpenError> {
    if cuts.len() <= 2 {
        return Ok(vec![scan_source_part(source, 0..source.len(), charset)?]);
    }
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(cuts.len() - 1);
    for pair in cuts.windows(2) {
        if pair[0] < pair[1] {
            ranges.push(pair[0]..pair[1]);
        }
    }
    thread::scope(|scope| {
        let handles: Vec<_> = ranges
            .into_iter()
            .map(|r| scope.spawn(move || scan_source_part(source, r, charset)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err(OpenError::InvalidTag { byte: 0 })))
            .collect()
    })
}

/// 圧縮済みソースを `threads` 本のスレッドで読んで列にする
pub fn scan_source(source: SourceBytes, threads: usize) -> Result<Scanned, OpenError> {
    if source.starts_with(BINARY_SENTINEL) {
        return Err(OpenError::BinaryDxf);
    }
    if source.len() >= u32::MAX as usize {
        return Err(OpenError::TooLarge);
    }
    let head_len = source.len().min(256 * 1024);
    let guessed_charset = source.with_range(0..head_len, |rs| pre_scan_header(rs.bytes));
    let cuts = split_points_source(&source, threads.max(1));
    let parts = scan_source_with_cuts(&source, &cuts, guessed_charset)?;
    Ok(Scanned {
        source,
        parts,
        guessed_charset,
    })
}

/// 入力を `threads` 本のスレッドでページ圧縮しながら読んで列にする（段階計測用の入口）
pub fn scan(src: &[u8], threads: usize) -> Result<Scanned, OpenError> {
    if src.starts_with(BINARY_SENTINEL) {
        return Err(OpenError::BinaryDxf);
    }
    if src.len() >= u32::MAX as usize {
        return Err(OpenError::TooLarge);
    }
    let threads = threads.max(1);
    let guessed_charset = pre_scan_header(src);
    let cuts = scan::split_points(src, threads);
    let source = SourceBytes::compress_slice(src, threads);
    let parts = scan_with_cuts(src, &cuts, guessed_charset)?;
    Ok(Scanned {
        source,
        parts,
        guessed_charset,
    })
}

/// 小さい入力でも `parts` 個の区間に分けて読む（分割境界の検証用）
pub fn scan_split(src: &[u8], parts: usize) -> Result<Scanned, OpenError> {
    if src.starts_with(BINARY_SENTINEL) {
        return Err(OpenError::BinaryDxf);
    }
    if src.len() >= u32::MAX as usize {
        return Err(OpenError::TooLarge);
    }
    let guessed_charset = pre_scan_header(src);
    let cuts = split_points_forced(src, parts);
    let scanned_parts = scan_with_cuts(src, &cuts, guessed_charset)?;
    let source = SourceBytes::compress_slice(src, parts.clamp(1, 4));
    Ok(Scanned {
        source,
        parts: scanned_parts,
        guessed_charset,
    })
}

/// `ENTITIES` の親子候補列から、親レコード番号ごとの子レコード番号列と、単純な並び（`plain`）かを求める
pub fn attach_family(family: &[(u32, Followed)]) -> (BTreeMap<u32, Vec<u32>>, BTreeSet<u32>, bool) {
    let mut by_handle: BTreeMap<u64, usize> = BTreeMap::new();
    let mut duplicate_handles = false;
    for (idx, (_, role)) in family.iter().enumerate() {
        if let Some(h) = role.handle
            && by_handle.insert(h, idx).is_some()
        {
            duplicate_handles = true;
        }
    }
    let mut attached: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut owned: BTreeSet<u32> = BTreeSet::new();
    let mut open: Option<usize> = None;
    let mut plain = !duplicate_handles;

    for (idx, &(rec, role)) in family.iter().enumerate() {
        match role.kind {
            Follow::Insert { opens } => {
                if open.is_some() {
                    plain = false;
                }
                open = opens.then_some(idx);
            }
            Follow::Polyline { opens } => {
                plain = false;
                open = opens.then_some(idx);
            }
            Follow::Attrib | Follow::Vertex | Follow::Seqend => {
                let parent = role
                    .owner
                    .and_then(|owner| by_handle.get(&owner).copied())
                    .or(open)
                    .filter(|&p| {
                        family
                            .get(p)
                            .is_some_and(|(_, pr)| single::followed_expected(role.kind, pr.kind))
                    });
                if parent != open || open.is_none() || role.kind == Follow::Vertex {
                    plain = false;
                }
                if let Some(p) = parent {
                    owned.insert(rec);
                    if role.kind != Follow::Seqend
                        && let Some(&(parent_rec, _)) = family.get(p)
                    {
                        attached.entry(parent_rec).or_default().push(rec);
                    }
                } else {
                    plain = false;
                }
                if role.kind == Follow::Seqend {
                    open = None;
                }
            }
        }
    }
    if open.is_some() {
        plain = false;
    }
    (attached, owned, plain)
}

fn read_record_spans_slice(rs: &RecordSlice<'_>) -> Vec<Span> {
    let mut out = Vec::new();
    let mut local_pos = 0usize;
    let base32 = rs.base as u32;
    while local_pos < rs.bytes.len() {
        let Next::Tag(span) = scan::next_tag(rs.bytes, local_pos) else {
            break;
        };
        local_pos = span.end as usize;
        out.push(shift_span(span, base32));
    }
    out
}

fn read_record_spans(src: &SourceBytes, range: Range<usize>) -> Vec<Span> {
    src.with_range(range, |rs| read_record_spans_slice(&rs))
}

fn decode_record(src: &SourceBytes, range: Range<usize>) -> Option<(Vec<u8>, Vec<Value>)> {
    src.with_range(range, |rs| {
        let spans = read_record_spans_slice(&rs);
        let first = spans.first()?;
        if first.code != 0 {
            return None;
        }
        let kind = rs.slice(first.value, first.value_end).trim_ascii().to_vec();
        let values = spans
            .get(1..)
            .unwrap_or_default()
            .iter()
            .map(|s| {
                let raw = rs.slice(s.value, s.value_end);
                value::value_of(&kind, s.code, raw)
            })
            .collect();
        Some((kind, values))
    })
}

fn rebuild_ent_family(
    src: &SourceBytes,
    record_starts: &[u32],
    entities: &[Range<u32>],
) -> Vec<(u32, Followed)> {
    let mut out = Vec::new();
    for range in entities {
        for rec in range.start..range.end {
            let s = record_starts.get(rec as usize).copied().unwrap_or(0) as usize;
            let e = record_starts
                .get(rec as usize + 1)
                .copied()
                .unwrap_or(src.len() as u32) as usize;
            src.with_range(s..e, |rs| {
                let Next::Tag(first) = scan::next_tag(rs.bytes, 0) else {
                    return;
                };
                if first.code != 0 {
                    return;
                }
                let kind_raw = rs
                    .bytes
                    .get(first.value as usize..first.value_end as usize)
                    .unwrap_or_default()
                    .trim_ascii();
                if !matches!(
                    kind_raw,
                    b"INSERT" | b"POLYLINE" | b"ATTRIB" | b"VERTEX" | b"SEQEND"
                ) {
                    return;
                }
                let spans = read_record_spans_slice(&rs);
                let raw_tags: Vec<RawTag<'_>> = spans
                    .get(1..)
                    .unwrap_or_default()
                    .iter()
                    .map(|sp| RawTag {
                        code: sp.code,
                        value: rs.slice(sp.value, sp.value_end),
                    })
                    .collect();
                if let Some(role) = single::followed_tags(kind_raw, &raw_tags) {
                    out.push((rec, role));
                }
            });
        }
    }
    out
}

fn parse_header(
    src: &SourceBytes,
    header_sec: Option<Range<u32>>,
    record_starts: &[u32],
) -> (Vec<u8>, Charset, HeaderState) {
    let Some(sec) = header_sec else {
        return (
            Vec::new(),
            charset_from_header(None, None),
            HeaderState::default(),
        );
    };
    let rec = sec.start;
    let Some(&start) = record_starts.get(rec as usize) else {
        return (
            Vec::new(),
            charset_from_header(None, None),
            HeaderState::default(),
        );
    };
    let end = record_starts
        .get(rec as usize + 1)
        .copied()
        .unwrap_or(src.len() as u32);
    src.with_range(start as usize..end as usize, |rs| {
        let spans = read_record_spans_slice(&rs);
        let mut state = HeaderState {
            record: Some(rec),
            ..HeaderState::default()
        };
        let mut version_opt: Option<Vec<u8>> = None;
        let mut code_page_opt: Option<Vec<u8>> = None;
        let mut seen_vars: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut plim_x: Option<f64> = None;
        let mut plim_y: Option<f64> = None;

        let mut pos = 1usize;
        for chunk in spans
            .get(1..)
            .unwrap_or_default()
            .chunk_by(|_, next| next.code != 9)
        {
            let chunk_start = pos;
            pos += chunk.len();
            let Some((head, body)) = chunk.split_first() else {
                continue;
            };
            if head.code != 9 {
                continue;
            }
            let head_raw = rs.slice(head.value, head.value_end);
            let name = head_raw.trim_ascii().to_vec();
            if !seen_vars.insert(name.clone()) {
                continue;
            }
            let body_range = (chunk_start + 1, body.len());
            if name != b"$HANDSEED" {
                for span in body {
                    let raw = rs.slice(span.value, span.value_end);
                    if let Val::Handle(h) = value::typed(span.code, raw) {
                        state.handles.insert(h);
                    }
                }
            }
            match name.as_slice() {
                b"$ACADVER" => {
                    if let Some(first) = body.first()
                        && first.code == 1
                    {
                        version_opt =
                            Some(rs.slice(first.value, first.value_end).trim_ascii().to_vec());
                    }
                }
                b"$DWGCODEPAGE" => {
                    if let Some(first) = body.first()
                        && first.code == 3
                    {
                        code_page_opt =
                            Some(rs.slice(first.value, first.value_end).trim_ascii().to_vec());
                    }
                }
                b"$HANDSEED" if !body.is_empty() => {
                    state.handseed = Some(body_range);
                }
                b"$EXTMIN" if !body.is_empty() => {
                    state.extmin = Some(body_range);
                }
                b"$EXTMAX" if !body.is_empty() => {
                    state.extmax = Some(body_range);
                }
                b"$INSUNITS" => {
                    for span in body {
                        if span.code == 70 && state.insunits.is_none() {
                            let raw = rs.slice(span.value, span.value_end);
                            if let Val::Int(v) = value::typed(70, raw) {
                                state.insunits = Some(v);
                            }
                        }
                    }
                }
                b"$PLIMMIN" => {
                    for span in body {
                        let raw = rs.slice(span.value, span.value_end);
                        if span.code == 10
                            && plim_x.is_none()
                            && let Val::Real(v) = value::typed(10, raw)
                        {
                            plim_x = Some(v);
                        } else if span.code == 20
                            && plim_y.is_none()
                            && let Val::Real(v) = value::typed(20, raw)
                        {
                            plim_y = Some(v);
                        }
                    }
                }
                _ => {}
            }
        }
        state.spans = spans;
        if let (Some(x), Some(y)) = (plim_x, plim_y)
            && x.is_finite()
            && y.is_finite()
        {
            state.plimmin = Some([x, y]);
        }
        let charset = charset_from_header(version_opt.as_deref(), code_page_opt.as_deref());
        (version_opt.unwrap_or_default(), charset, state)
    })
}

fn shift_geom_pools(
    geom: &mut Geom,
    vertex_base: u32,
    text_base: u32,
    text_extra_base: u32,
    insert_extra_base: u32,
) {
    match geom {
        Geom::Line(_) | Geom::Circle(_) | Geom::Arc(_) => {}
        Geom::Polyline { first, .. } => {
            *first += vertex_base;
        }
        Geom::Text(text) => {
            *text = text.shifted_spans(text_base, text_extra_base);
        }
        Geom::Insert(ins) => {
            ins.name = ins.name.shifted(text_base);
            if ins.extra != 0 {
                ins.extra += insert_extra_base;
            }
        }
    }
}

fn merge_pools(dst: &mut Pools, src: Pools) -> (u32, u32, u32, u32, u32) {
    let vertex_base = dst.vertices.len() as u32;
    let width_base = dst.widths.len() as u32;
    let text_extra_base = dst.text_extras.len() as u32;
    let insert_extra_base = dst.insert_extras.len() as u32;
    let extra_style_base = dst.extra_styles.len() as u32;

    dst.vertices.extend_paged(src.vertices);
    dst.widths.extend_paged(src.widths);
    for (poly_first, w_start) in src.width_first {
        dst.width_first
            .push((poly_first + vertex_base, w_start + width_base));
    }
    let text_base = dst.text.append_pool(src.text);
    dst.text_extras.extend_paged(src.text_extras);
    dst.insert_extras.extend_paged(src.insert_extras);
    for idx in 0..src.extra_styles.len() {
        if let Some(mut es) = src.extra_styles.get(idx).copied() {
            if let LineTypeRef::Named(ref mut span) = es.line_type {
                *span = span.shifted(text_base);
            }
            dst.extra_styles.push(es);
        }
    }
    (
        vertex_base,
        text_base,
        text_extra_base,
        insert_extra_base,
        extra_style_base,
    )
}

/// 決定 0118 と一致する図形の囲む箱 `[min_x, min_y, max_x, max_y]`
pub fn bounds(pools: &Pools, geom: &Geom) -> [f64; 4] {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut include = |[x, y]: [f64; 2]| {
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        if x < min_x {
            min_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if x > max_x {
            max_x = x;
        }
        if y > max_y {
            max_y = y;
        }
    };
    match geom {
        Geom::Line([x0, y0, x1, y1]) => {
            include([*x0, *y0]);
            include([*x1, *y1]);
        }
        Geom::Circle([cx, cy, r]) => {
            let r = r.abs();
            include([cx - r, cy - r]);
            include([cx + r, cy + r]);
        }
        Geom::Arc([cx, cy, r, start, end]) => {
            let r = r.abs();
            let sweep = if (end - start).abs() >= 360.0 {
                360.0
            } else {
                (end - start).rem_euclid(360.0)
            };
            for deg in [*start, *end] {
                let rad = deg.to_radians();
                include([cx + r * libm::cos(rad), cy + r * libm::sin(rad)]);
            }
            for (cardinal, pt) in [
                (0.0, [cx + r, *cy]),
                (90.0, [*cx, cy + r]),
                (180.0, [cx - r, *cy]),
                (270.0, [*cx, cy - r]),
            ] {
                if (cardinal - start).rem_euclid(360.0) <= sweep {
                    include(pt);
                }
            }
        }
        Geom::Polyline {
            first,
            count,
            flags,
            ..
        } => {
            for idx in 0..*count {
                if let Some(&[x, y, _]) = pools.vertices.get((*first + idx) as usize) {
                    include([x, y]);
                }
            }
            let closed = flags & POLY_CLOSED != 0;
            for (p, q, bulge) in pools.edges_of(*first, *count, closed) {
                if bulge == 0.0 || !bulge.is_finite() || p == q {
                    continue;
                }
                let dx = q[0] - p[0];
                let dy = q[1] - p[1];
                let factor = (1.0 - bulge * bulge) / (4.0 * bulge);
                let cx = 0.5 * (p[0] + q[0]) - factor * dy;
                let cy = 0.5 * (p[1] + q[1]) + factor * dx;
                let r = ((p[0] - cx) * (p[0] - cx) + (p[1] - cy) * (p[1] - cy)).sqrt();
                let start_pt = if bulge > 0.0 { p } else { q };
                let from = libm::atan2(start_pt[1] - cy, start_pt[0] - cx).to_degrees();
                let sweep = 4.0 * libm::atan(bulge.abs()).to_degrees();
                for (cardinal, pt) in [
                    (0.0, [cx + r, cy]),
                    (90.0, [cx, cy + r]),
                    (180.0, [cx - r, cy]),
                    (270.0, [cx, cy - r]),
                ] {
                    if (cardinal - from).rem_euclid(360.0) <= sweep {
                        include(pt);
                    }
                }
            }
        }
        Geom::Text(text) => match text.place(pools) {
            crate::lift::Place::Anchor { point, .. } => include(point),
            crate::lift::Place::Between { first, second, .. } => {
                include(first);
                include(second);
            }
        },
        Geom::Insert(ins) => {
            include(ins.position);
            for attrib_idx in ins.attrib_first..ins.attrib_first.saturating_add(ins.attrib_count) {
                let Some(attrib) = pools.attribs.get(attrib_idx as usize) else {
                    continue;
                };
                match attrib.text.place(pools) {
                    crate::lift::Place::Anchor { point, .. } => include(point),
                    crate::lift::Place::Between { first, second, .. } => {
                        include(first);
                        include(second);
                    }
                }
                if attrib.is_formatted()
                    && let Ok(pos) = pools
                        .formatted_attribs
                        .binary_search_by_key(&attrib_idx, |&(k, _)| k)
                    && let Some((_, fmt)) = pools.formatted_attribs.get(pos)
                {
                    include([fmt.point.x, fmt.point.y]);
                }
            }
        }
    }
    [min_x, min_y, max_x, max_y]
}

impl Document {
    /// DXF のバイト列を `threads` 本のスレッドで開く
    pub fn open(src: Vec<u8>, threads: usize) -> Result<Self, OpenError> {
        let scanned = scan(&src, threads)?;
        drop(src);
        let mut doc = Self::assemble_scanned(scanned)?;
        doc.build_display();
        doc.build_index();
        Ok(doc)
    }

    /// メモリ上のスライスを複製せずに `threads` 本のスレッドで開く
    pub fn open_slice(src: &[u8], threads: usize) -> Result<Self, OpenError> {
        let scanned = scan(src, threads)?;
        let mut doc = Self::assemble_scanned(scanned)?;
        doc.build_display();
        doc.build_index();
        Ok(doc)
    }

    /// ファイルを丸ごと `Vec<u8>` に確保せず、ストリーミング圧縮しながら `threads` 本のスレッドで開く
    pub fn open_path(path: &std::path::Path, threads: usize) -> Result<Self, String> {
        let source = SourceBytes::compress_file(path, threads)
            .map_err(|error| format!("{} を読めない: {error}", path.display()))?;
        let scanned =
            scan_source(source, threads).map_err(|error| format!("開けない: {error:?}"))?;
        let mut doc =
            Self::assemble_scanned(scanned).map_err(|error| format!("開けない: {error:?}"))?;
        doc.build_display();
        doc.build_index();
        Ok(doc)
    }

    /// スキャン済みの中間結果から文書を組み上げる（互換入口）
    pub fn assemble(src: Vec<u8>, scanned: Scanned) -> Result<Self, OpenError> {
        drop(src);
        Self::assemble_scanned(scanned)
    }

    /// スキャン済みの中間結果から文書を組み上げる（表示バッファーと格子索引の前まで）
    pub fn assemble_scanned(mut scanned: Scanned) -> Result<Self, OpenError> {
        let (record_starts, sections, entities, entities_end) =
            build_layout(scanned.source.len(), &mut scanned.parts);
        let header_sec = sections
            .iter()
            .find(|(sk, _)| *sk == SectionKind::Header)
            .map(|(_, r)| r.clone());
        let (version, charset, header) = parse_header(&scanned.source, header_sec, &record_starts);

        if charset != scanned.guessed_charset {
            let cuts = split_points_source(&scanned.source, scanned.parts.len().max(1));
            scanned.parts = scan_source_with_cuts(&scanned.source, &cuts, charset)?;
            for part in &mut scanned.parts {
                part.starts = Vec::new();
            }
        }

        Ok(assemble_inner(
            scanned.source,
            scanned.parts,
            record_starts,
            sections,
            entities,
            entities_end,
            version,
            charset,
            header,
        ))
    }

    /// 表示バッファーを組む
    pub fn build_display(&mut self) {
        let mut display = Display::with_rows(self.rows.len());
        for row_idx in 0..self.rows.len() as u32 {
            if let Some(r) = self.rows.get(row_idx as usize) {
                let geom = self.shapes.geom(r.kind, r.shape);
                display.push(&geom, &self.shapes.pools, [r.layer, r.color]);
                if !r.alive {
                    display.hide(row_idx);
                }
            }
        }
        self.display = display;
    }

    /// 空間索引を組む
    pub fn build_index(&mut self) {
        self.grid = Grid::build(&self.boxes);
    }

    /// 元のファイルのバイト数
    pub fn source_len(&self) -> usize {
        self.src.len()
    }

    /// ページ圧縮した元のバイト列が保持に使っているバイト数
    pub fn source_stored_bytes(&self) -> usize {
        self.src.stored_bytes()
    }

    /// 元のファイルのレコード数
    pub fn source_records(&self) -> u32 {
        self.counts.records
    }

    /// ハンドルから生きている行番号を引く（重複していたハンドルは `None`）
    pub fn row_of(&self, handle: u64) -> Option<u32> {
        self.by_handle.get(handle, &self.rows)
    }

    /// レコード番号から生きている行番号を引く
    pub fn row_by_record(&self, record: u32) -> Option<u32> {
        if record >= self.counts.records {
            let &idx = self.added_by_record.get(&record)?;
            return self
                .rows
                .get(idx as usize)
                .is_some_and(|r| r.alive)
                .then_some(idx);
        }
        let mut lo = 0usize;
        let mut hi = self.base_row_count as usize;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let Some(r) = self.rows.get(mid) else {
                break;
            };
            match r.record.cmp(&record) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return r.alive.then_some(mid as u32),
            }
        }
        None
    }

    /// レコードの元のバイト範囲を展開して `f` に渡す（追加されたレコードは空スライス）
    pub fn with_record_slice<R>(&self, record: u32, f: impl FnOnce(RecordSlice<'_>) -> R) -> R {
        if record >= self.counts.records {
            return f(RecordSlice {
                base: 0,
                bytes: &[],
            });
        }
        if let Some(ov) = self.overlay.get(&record)
            && !ov.raw.is_empty()
        {
            return f(RecordSlice {
                base: ov.base as usize,
                bytes: &ov.raw,
            });
        }
        self.src.with_range(self.record_range(record), f)
    }

    /// 指定した表（`table::LAYER`・`table::LTYPE`・`table::STYLE`）に定義されている名前列を返す
    pub fn table_names(&self, kind: &[u8]) -> Vec<String> {
        self.tables_cache
            .names
            .get(kind)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// 変更・追加・削除したレコードの件数
    pub fn edited_records(&self) -> usize {
        let mut touched: BTreeSet<u32> = self.overlay.keys().copied().collect();
        touched.extend(self.gone.iter().copied());
        touched.len()
    }

    /// 削除されて空きスロットになっている行の件数
    pub fn gone_rows(&self) -> usize {
        (0..self.rows.len())
            .filter(|&i| self.rows.get(i).is_some_and(|r| !r.alive))
            .count()
    }

    /// 積まれている取り消し段数
    pub fn undo_len(&self) -> usize {
        self.history.undo_len()
    }

    /// `overlay`・`gone`・履歴が保持する差分用のメモリバイト数
    pub fn edit_bytes(&self) -> usize {
        let overlay_bytes: usize = self
            .overlay
            .values()
            .map(|v| size_of::<(u32, OverlayRecord)>() + v.heap_bytes())
            .sum();
        let gone_bytes = self.gone.len() * size_of::<u32>();
        overlay_bytes + gone_bytes + self.history.bytes()
    }

    /// ファイル順に並べた生きている各行の表示プリミティブ列を返す
    pub fn live_display(&self) -> Vec<RowBuffers> {
        self.live_row_indices()
            .into_iter()
            .map(|r| self.display.row(r))
            .collect()
    }

    /// ファイル順に並べた生きている要素の囲む箱列を返す
    pub fn live_boxes(&self) -> Vec<[f64; 4]> {
        self.live_row_indices()
            .into_iter()
            .filter_map(|i| self.boxes.get(i as usize).copied())
            .collect()
    }

    /// 別の `Document` と、生きている要素の ID・種類・画層・ハッシュがすべて一致するか
    pub fn same_content(&self, other: &Self) -> bool {
        let a = crate::inspect::live_elements(self);
        let b = crate::inspect::live_elements(other);
        a.len() == b.len()
            && a.iter().zip(&b).all(|(x, y)| {
                x.id == y.id && x.kind == y.kind && x.layer == y.layer && x.hash == y.hash
            })
    }

    /// 元のレコードのバイト範囲
    pub fn record_range(&self, record: u32) -> Range<usize> {
        let start = self
            .record_starts
            .get(record as usize)
            .copied()
            .unwrap_or(self.src.len() as u32) as usize;
        let end = self
            .record_starts
            .get(record as usize + 1)
            .copied()
            .unwrap_or(self.src.len() as u32) as usize;
        start..end
    }

    /// レコードの今のタグ列（変更・追加されたものは `overlay`、それ以外は `src` から読む）
    pub fn record_tags(&self, record: u32) -> Vec<OTag> {
        if let Some(ov) = self.overlay.get(&record) {
            return ov.tags.clone();
        }
        read_record_spans(&self.src, self.record_range(record))
            .into_iter()
            .map(|span| OTag {
                code: span.code,
                span,
                edited: None,
                fresh: false,
            })
            .collect()
    }

    /// レコードの種類（0）と値列（0 の次から）を返す
    pub fn record_kind_and_values(&self, record: u32) -> Option<(Vec<u8>, Vec<Value>)> {
        if let Some(ov) = self.overlay.get(&record) {
            let first = ov.tags.first()?;
            if first.code != 0 {
                return None;
            }
            return self.with_record_slice(record, |rs| {
                let kind = first.raw_in(&rs).trim_ascii().to_vec();
                let values = ov
                    .tags
                    .get(1..)
                    .unwrap_or_default()
                    .iter()
                    .map(|t| t.to_value_for_in(&kind, &rs))
                    .collect();
                Some((kind, values))
            });
        }
        decode_record(&self.src, self.record_range(record))
    }

    /// レコードの最初の有効なハンドル（5。`DIMSTYLE` は 105）を返す
    pub fn record_handle(&self, record: u32) -> Option<u64> {
        if let Some(ov) = self.overlay.get(&record) {
            let first = ov.tags.first()?;
            if first.code != 0 {
                return None;
            }
            return self.with_record_slice(record, |rs| {
                let kind = first.raw_in(&rs).trim_ascii();
                let expected = if kind == b"DIMSTYLE" { 105 } else { 5 };
                ov.tags
                    .iter()
                    .skip(1)
                    .filter(|t| t.code == expected)
                    .find_map(|t| match value::typed(expected, t.raw_in(&rs)) {
                        Val::Handle(h) => Some(h),
                        _ => None,
                    })
            });
        }
        self.with_record_slice(record, |rs| {
            let spans = read_record_spans_slice(&rs);
            let first = spans.first()?;
            if first.code != 0 {
                return None;
            }
            let kind = rs.slice(first.value, first.value_end).trim_ascii();
            handle_in_spans_slice(kind, spans.get(1..).unwrap_or_default(), &rs)
        })
    }

    /// 元のバイト列を除いた、列・表示・索引・表の保持バイト数
    pub fn resident_bytes(&self) -> usize {
        self.record_starts.capacity() * size_of::<u32>()
            + self.header.spans.capacity() * size_of::<Span>()
            + self.by_handle.bytes()
            + self.rows.bytes()
            + self.shapes.bytes()
            + self.boxes.bytes()
            + self.grid.bytes()
            + self.display.bytes()
    }

    /// 矩形と重なる生きている行番号を `out` に返す
    pub fn select(&self, rect: [f64; 4], out: &mut Vec<u32>) {
        self.grid.query(&self.boxes, rect, out);
        out.retain(|&row| self.rows.get(row as usize).is_some_and(|r| r.alive));
    }

    /// 行番号から `zumen_drawing::Element<ModelPoint>` を組み立てる
    pub fn element(&self, row_index: u32) -> Option<Element<ModelPoint>> {
        let row = self.rows.get(row_index as usize)?;
        if !row.alive {
            return None;
        }
        let geom = self.shapes.geom(row.kind, row.shape);
        let record = (row.record < self.counts.records).then_some(row.record);
        lift::element_of(
            row.handle(),
            record,
            row.layer,
            row.color,
            row.extra_style,
            &geom,
            &self.shapes.pools,
        )
    }

    /// ファイル上の並び順の比較キー
    pub fn row_sort_key(&self, row: &Row) -> (u32, u32) {
        if row.record < self.counts.records {
            (row.record, 0)
        } else {
            (self.entities_end.unwrap_or(u32::MAX), row.record)
        }
    }

    /// 行番号の列を、ファイル上の並び順に並べ替える
    pub fn sort_rows_by_file_order(&self, rows: &mut [u32]) {
        if self.entities.len() > 1 && self.next_added_record > self.counts.records {
            rows.sort_by_key(|&i| {
                self.rows
                    .get(i as usize)
                    .map_or((u32::MAX, u32::MAX), |r| self.row_sort_key(r))
            });
        }
    }

    /// ファイル上の並び順で、生きている行番号をすべて返す
    pub fn live_row_indices(&self) -> Vec<u32> {
        let mut indices: Vec<u32> = (0..self.rows.len() as u32)
            .filter(|&i| self.rows.get(i as usize).is_some_and(|r| r.alive))
            .collect();
        self.sort_rows_by_file_order(&mut indices);
        indices
    }

    /// 画層 `id` が今の図面モデルに生きているか
    pub fn is_layer_live(&self, id: LayerId) -> bool {
        let idx = id.0 as usize;
        self.layers
            .get(idx)
            .is_some_and(|l| l.source != Ref::default())
            || self.layer_uses.get(idx).copied().unwrap_or(0) > 0
    }

    /// 線種 `id` が今の図面モデルに生きているか
    pub fn is_line_type_live(&self, id: LineTypeId) -> bool {
        let idx = id.0 as usize;
        self.line_types
            .get(idx)
            .is_some_and(|lt| lt.source != Ref::default())
            || self.line_type_uses.get(idx).copied().unwrap_or(0) > 0
    }

    /// `TableLookup` と同じ規則（大文字・小文字を区別して生きている画層がちょうど 1 つ）で画層を引く
    pub fn lookup_live_layer(&self, name: &str) -> Option<LayerId> {
        let mut found = None;
        for (idx, layer) in self.layers.iter().enumerate() {
            let id = LayerId(idx as u32);
            if self.is_layer_live(id) && layer.name == name {
                if found.is_some() {
                    return None;
                }
                found = Some(id);
            }
        }
        found
    }

    /// `TableLookup` と同じ規則（大文字・小文字を区別して生きている線種がちょうど 1 つ）で線種を引く
    pub fn lookup_live_line_type(&self, name: &str) -> Option<LineTypeId> {
        let mut found = None;
        for (idx, lt) in self.line_types.iter().enumerate() {
            let id = LineTypeId(idx as u32);
            if self.is_line_type_live(id) && lt.name == name {
                if found.is_some() {
                    return None;
                }
                found = Some(id);
            }
        }
        found
    }

    /// 画層の参照数を増減する
    pub fn adjust_layer_use(&mut self, id: LayerId, delta: i32) {
        let idx = id.0 as usize;
        if idx >= self.layer_uses.len() {
            self.layer_uses.resize(idx + 1, 0);
        }
        if let Some(slot) = self.layer_uses.get_mut(idx) {
            *slot = (*slot as i64 + i64::from(delta)).max(0) as u32;
        }
    }

    /// 線種の参照数を増減する
    pub fn adjust_line_type_use(&mut self, id: LineTypeId, delta: i32) {
        let idx = id.0 as usize;
        if idx >= self.line_type_uses.len() {
            self.line_type_uses.resize(idx + 1, 0);
        }
        if let Some(slot) = self.line_type_uses.get_mut(idx) {
            *slot = (*slot as i64 + i64::from(delta)).max(0) as u32;
        }
    }

    pub(crate) fn adjust_row_uses(&mut self, row: Row, delta: i32) {
        self.adjust_layer_use(LayerId(row.layer), delta);
        if let Some(es_idx) = row.extra_style.checked_sub(1)
            && let Some(es) = self.shapes.pools.extra_styles.get(es_idx as usize)
            && matches!(es.line_type, LineTypeRef::Named(_))
        {
            self.adjust_line_type_use(LineTypeId(es.line_type_id), delta);
        }
        if row.kind == Kind::Insert
            && let Some(&ins) = self.shapes.inserts.get(row.shape as usize)
        {
            for idx in ins.attrib_first..ins.attrib_first.saturating_add(ins.attrib_count) {
                if let Some(&attr) = self.shapes.pools.attribs.get(idx as usize) {
                    self.adjust_layer_use(LayerId(attr.layer), delta);
                    if let Some(es_idx) = attr.extra_style.checked_sub(1)
                        && let Some(es) = self.shapes.pools.extra_styles.get(es_idx as usize)
                        && matches!(es.line_type, LineTypeRef::Named(_))
                    {
                        self.adjust_line_type_use(LineTypeId(es.line_type_id), delta);
                    }
                }
            }
        }
    }

    /// 生きている全オブジェクトの最大のハンドルを `O(1)` で返す
    pub fn live_max_handle(&self) -> Option<u64> {
        [
            self.fixed_max_handle,
            self.by_handle.max_live(&self.rows),
            self.live_object_handles.last().copied(),
            self.added_follower_handles.last().copied(),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    /// 今の `ENTITIES` に合わせてサブクラスマーカー（100）を書くか
    pub fn live_markers(&self) -> bool {
        if self.version.as_slice() >= b"AC1012" {
            return true;
        }
        for range in &self.entities {
            for rec in range.start..range.end {
                if self.gone.contains(&rec) {
                    continue;
                }
                let tags = self.record_tags(rec);
                let Some(first) = tags.first() else {
                    continue;
                };
                if first.code == 0 {
                    let is_non_obj = self.with_record_slice(rec, |rs| {
                        is_non_object_kind(first.raw_in(&rs).trim_ascii())
                    });
                    if !is_non_obj && tags.iter().skip(1).any(|t| t.code == 100) {
                        return true;
                    }
                }
            }
        }
        for (&rec, ov) in self.overlay.range(self.counts.records..) {
            if !self.gone.contains(&rec) && ov.tags.iter().skip(1).any(|t| t.code == 100) {
                return true;
            }
        }
        false
    }

    pub(crate) fn ensure_layer_id(&mut self, name: &str) -> LayerId {
        let lower = name.to_ascii_lowercase();
        if let Some(&id) = self.by_layer_name.get(&lower) {
            return id;
        }
        let id = LayerId(self.layers.len() as u32);
        self.by_layer_name.insert(lower, id);
        self.layers.push(Layer {
            id,
            follows_placement: name == "0",
            name: name.to_owned(),
            state: None,
            style: None,
            description: None,
            source: Ref::default(),
        });
        self.layer_uses.push(0);
        id
    }

    pub(crate) fn ensure_line_type_id(&mut self, name: &str) -> LineTypeId {
        let lower = name.to_ascii_lowercase();
        if let Some(&id) = self.by_line_type_name.get(&lower) {
            return id;
        }
        let id = LineTypeId(self.line_types.len() as u32);
        self.by_line_type_name.insert(lower, id);
        self.line_types.push(LineType {
            id,
            name: name.to_owned(),
            definition: None,
            source: Ref::default(),
        });
        self.line_type_uses.push(0);
        id
    }

    pub(crate) fn lift_single_attrib_record(&mut self, rec: u32) -> Option<AttribRow> {
        let range = if rec >= self.counts.records {
            0..0
        } else {
            self.record_range(rec)
        };
        let charset = self.charset;
        let counts_records = self.counts.records;
        let lifted_res = if let Some(ov) = self.overlay.get(&rec) {
            let pools = &mut self.shapes.pools;
            let body = ov.tags.get(1..).unwrap_or_default();
            if rec >= counts_records || !ov.raw.is_empty() {
                let rs = RecordSlice {
                    base: ov.base as usize,
                    bytes: &ov.raw,
                };
                let raw_tags: Vec<RawTag<'_>> = body
                    .iter()
                    .map(|t| RawTag {
                        code: t.code,
                        value: t.raw_in(&rs),
                    })
                    .collect();
                lift::attrib(charset, &raw_tags, rec, pools)
            } else {
                self.src.with_range(range, |rs| {
                    let raw_tags: Vec<RawTag<'_>> = body
                        .iter()
                        .map(|t| RawTag {
                            code: t.code,
                            value: t.raw_in(&rs),
                        })
                        .collect();
                    lift::attrib(charset, &raw_tags, rec, pools)
                })
            }
        } else {
            let pools = &mut self.shapes.pools;
            self.src.with_range(range, |rs| {
                let spans = read_record_spans_slice(&rs);
                let body = spans.get(1..).unwrap_or_default();
                let raw_tags: Vec<RawTag<'_>> = body
                    .iter()
                    .map(|s| RawTag {
                        code: s.code,
                        value: rs.slice(s.value, s.value_end),
                    })
                    .collect();
                lift::attrib(charset, &raw_tags, rec, pools)
            })
        };
        match lifted_res {
            AttribLift::Row(mut row, layer_span) => {
                self.bind_attrib_ids(&mut row, layer_span);
                return Some(row);
            }
            AttribLift::Multiline => {}
            AttribLift::Not(_) => return None,
        }
        let (_, values) = self.record_kind_and_values(rec)?;
        let mut cat = SingleCatalog::new(self.charset, &self.version, self.header.insunits, None);
        let attr = cat.read_multiline_attrib(values, rec).ok()?;
        let layer_name = cat.layer_name(attr.layer).to_owned();
        let lt_name = match attr.style.line_type {
            zumen_drawing::Inherit::Own(id) => cat.line_type_name(id).map(str::to_owned),
            _ => None,
        };
        let mut row = lift::push_lifted_attribute(attr, rec, &mut self.shapes.pools)?;
        if let Some(lt_name) = lt_name {
            let lt_id = self.ensure_line_type_id(&lt_name);
            let span = self.shapes.pools.push_utf8(&lt_name)?;
            if let Some(es_idx) = row.extra_style.checked_sub(1)
                && let Some(es) = self.shapes.pools.extra_styles.get_mut(es_idx as usize)
            {
                es.line_type = LineTypeRef::Named(span);
                es.line_type_id = lt_id.0;
            }
        }
        row.layer = self.ensure_layer_id(&layer_name).0;
        Some(row)
    }

    pub(crate) fn bind_attrib_ids(&mut self, row: &mut AttribRow, layer_span: TextSpan) {
        if let Some(es_idx) = row.extra_style.checked_sub(1)
            && let Some(LineTypeRef::Named(span)) = self
                .shapes
                .pools
                .extra_styles
                .get(es_idx as usize)
                .map(|es| es.line_type)
        {
            let name = self.shapes.pools.str_of(span).to_owned();
            let lt_id = self.ensure_line_type_id(&name);
            if let Some(es) = self.shapes.pools.extra_styles.get_mut(es_idx as usize) {
                es.line_type_id = lt_id.0;
            }
        }
        let layer_name = self.shapes.pools.str_of(layer_span).to_owned();
        row.layer = self.ensure_layer_id(&layer_name).0;
    }

    pub(crate) fn bind_lifted_ids(&mut self, lifted: &mut lift::Lifted) -> u32 {
        if let Some(es_idx) = lifted.extra_style.checked_sub(1)
            && let Some(LineTypeRef::Named(span)) = self
                .shapes
                .pools
                .extra_styles
                .get(es_idx as usize)
                .map(|es| es.line_type)
        {
            let name = self.shapes.pools.str_of(span).to_owned();
            let lt_id = self.ensure_line_type_id(&name);
            if let Some(es) = self.shapes.pools.extra_styles.get_mut(es_idx as usize) {
                es.line_type_id = lt_id.0;
            }
        }
        let layer_name = self.shapes.pools.str_of(lifted.layer).to_owned();
        let layer_id = self.ensure_layer_id(&layer_name).0;
        if let Geom::Insert(ref mut ins) = lifted.geom {
            let name = self.shapes.pools.str_of(ins.name);
            ins.definition = self.def_by_name.get(name).copied().unwrap_or(NO_DEFINITION);
        }
        layer_id
    }

    /// レコードの今の内容から既存の行を持ち上げ直し、形・囲む箱・格子索引・表示バッファーを更新する
    pub fn refresh_row_from_record(
        &mut self,
        row_index: u32,
        attrib_records: Option<&[u32]>,
    ) -> bool {
        let Some(old_row) = self.rows.get(row_index as usize).copied() else {
            return false;
        };

        if old_row.alive {
            self.adjust_row_uses(old_row, -1);
        }

        let range = if old_row.record >= self.counts.records {
            0..0
        } else {
            self.record_range(old_row.record)
        };
        let charset = self.charset;
        let counts_records = self.counts.records;
        let lifted_res = if let Some(ov) = self.overlay.get(&old_row.record) {
            let pools = &mut self.shapes.pools;
            let body = ov.tags.get(1..).unwrap_or_default();
            if old_row.record >= counts_records || !ov.raw.is_empty() {
                let rs = RecordSlice {
                    base: ov.base as usize,
                    bytes: &ov.raw,
                };
                let raw_tags: Vec<RawTag<'_>> = body
                    .iter()
                    .map(|t| RawTag {
                        code: t.code,
                        value: t.raw_in(&rs),
                    })
                    .collect();
                lift::lift(charset, old_row.kind, &raw_tags, pools)
            } else {
                self.src.with_range(range, |rs| {
                    let raw_tags: Vec<RawTag<'_>> = body
                        .iter()
                        .map(|t| RawTag {
                            code: t.code,
                            value: t.raw_in(&rs),
                        })
                        .collect();
                    lift::lift(charset, old_row.kind, &raw_tags, pools)
                })
            }
        } else {
            let pools = &mut self.shapes.pools;
            self.src.with_range(range, |rs| {
                let spans = read_record_spans_slice(&rs);
                let body = spans.get(1..).unwrap_or_default();
                let raw_tags: Vec<RawTag<'_>> = body
                    .iter()
                    .map(|s| RawTag {
                        code: s.code,
                        value: rs.slice(s.value, s.value_end),
                    })
                    .collect();
                lift::lift(charset, old_row.kind, &raw_tags, pools)
            })
        };
        let Lift::Element(mut lifted) = lifted_res else {
            if old_row.alive {
                self.adjust_row_uses(old_row, 1);
            }
            return false;
        };
        let layer_id = self.bind_lifted_ids(&mut lifted);

        if let Geom::Insert(ref mut ins) = lifted.geom {
            if let Some(prev_ins) = self.shapes.inserts.get(old_row.shape as usize).copied()
                && let Some(clip) = prev_ins.clip(&self.shapes.pools).cloned()
            {
                let _ = self.shapes.pools.set_insert_clip(ins, clip);
            }
            let records_vec: Vec<u32> = match attrib_records {
                Some(list) => list.to_vec(),
                None => {
                    if let Some(prev_ins) = self.shapes.inserts.get(old_row.shape as usize) {
                        self.shapes
                            .pools
                            .attribs_of(prev_ins)
                            .map(|a| a.record)
                            .collect()
                    } else {
                        Vec::new()
                    }
                }
            };
            ins.attached = records_vec.len() as u32;
            ins.attrib_first = self.shapes.pools.attribs.len() as u32;
            let mut lifted_count = 0u32;
            for rec in records_vec {
                if let Some(attr_row) = self.lift_single_attrib_record(rec) {
                    self.shapes.pools.attribs.push(attr_row);
                    lifted_count += 1;
                }
            }
            ins.attrib_count = lifted_count;
        }

        let new_box = bounds(&self.shapes.pools, &lifted.geom);
        let geom_copy = lifted.geom;
        self.shapes.set(old_row.kind, old_row.shape, lifted.geom);
        let new_row = Row::new(
            old_row.record,
            lifted.handle,
            old_row.kind,
            old_row.shape,
            layer_id,
            lifted.color,
            lifted.extra_style,
            old_row.alive,
        );
        if let Some(slot) = self.rows.get_mut(row_index as usize) {
            *slot = new_row;
        }
        if let Some(slot) = self.boxes.get_mut(row_index as usize) {
            *slot = new_box;
        }
        if new_row.alive {
            self.adjust_row_uses(new_row, 1);
            self.grid.update(row_index, new_box);
            self.display.update(
                row_index,
                &geom_copy,
                &self.shapes.pools,
                [new_row.layer, new_row.color],
            );
        }
        true
    }

    /// `!self.plain` の図面で `ENTITIES` の追加・削除があったとき、生きている親子関係を再計算して変わった `INSERT` 行を更新する
    pub fn reattach_live(&mut self) {
        let end_at = self.entities_end.unwrap_or(u32::MAX);
        let mut live_family: Vec<(u32, Followed)> = Vec::with_capacity(self.ent_family.len() + 8);
        let mut inserted_added = false;

        let push_added = |out: &mut Vec<(u32, Followed)>, doc: &Self| {
            let empty_rs = RecordSlice {
                base: 0,
                bytes: &[],
            };
            for (&rec, ov) in doc.overlay.range(doc.counts.records..) {
                if doc.gone.contains(&rec) {
                    continue;
                }
                let Some(first) = ov.tags.first() else {
                    continue;
                };
                if first.code != 0 {
                    continue;
                }
                let kind = first.raw_in(&empty_rs).trim_ascii();
                if matches!(
                    kind,
                    b"INSERT" | b"POLYLINE" | b"ATTRIB" | b"VERTEX" | b"SEQEND"
                ) {
                    let raw_tags: Vec<RawTag<'_>> = ov
                        .tags
                        .get(1..)
                        .unwrap_or_default()
                        .iter()
                        .map(|t| RawTag {
                            code: t.code,
                            value: t.raw_in(&empty_rs),
                        })
                        .collect();
                    if let Some(role) = single::followed_tags(kind, &raw_tags) {
                        out.push((rec, role));
                    }
                }
            }
        };

        for &(rec, role) in &self.ent_family {
            if !inserted_added && rec >= end_at {
                push_added(&mut live_family, self);
                inserted_added = true;
            }
            if !self.gone.contains(&rec) {
                live_family.push((rec, role));
            }
        }
        if !inserted_added {
            push_added(&mut live_family, self);
        }

        let (attached_map, _, _) = attach_family(&live_family);
        let insert_rows: Vec<(u32, u32, u32)> = (0..self.rows.len() as u32)
            .filter_map(|idx| {
                let r = self.rows.get(idx as usize)?;
                (r.alive && r.kind == Kind::Insert).then_some((idx, r.record, r.shape))
            })
            .collect();
        for (row_idx, rec, shape_idx) in insert_rows {
            let want: &[u32] = attached_map.get(&rec).map_or(&[], Vec::as_slice);
            let same = self
                .shapes
                .inserts
                .get(shape_idx as usize)
                .is_some_and(|ins| {
                    ins.attached as usize == want.len()
                        && ins.attrib_count == ins.attached
                        && self
                            .shapes
                            .pools
                            .attribs_of(ins)
                            .map(|a| a.record)
                            .eq(want.iter().copied())
                });
            if !same {
                let _ = self.refresh_row_from_record(row_idx, Some(want));
            }
        }
    }
}

type LayoutParts = (
    Vec<u32>,
    Vec<(SectionKind, Range<u32>)>,
    Vec<Range<u32>>,
    Option<u32>,
);

fn build_layout(src_len: usize, parts: &mut [Part]) -> LayoutParts {
    let total_est: usize = parts.iter().map(|p| p.starts.len()).sum();
    let mut record_starts = Vec::with_capacity(total_est + 1);
    let mut sections = Vec::new();
    let mut entities = Vec::new();
    let mut ends = Vec::new();
    let mut open: Option<(SectionKind, u32)> = None;
    let mut final_end = src_len as u32;
    let mut saw_eof = false;

    for part in parts {
        if saw_eof {
            part.starts = Vec::new();
            continue;
        }
        let base = record_starts.len() as u32;
        let mut stop_local = part.starts.len() as u32;
        for mark in &part.marks {
            match *mark {
                Mark::Section(local, sk) => {
                    let at = base + local;
                    if let Some((prev_sk, start)) = open.take() {
                        sections.push((prev_sk, start..at));
                        if prev_sk == SectionKind::Entities {
                            entities.push(start..at);
                        }
                    }
                    open = Some((sk, at));
                }
                Mark::EndSec(local) => {
                    let at = base + local;
                    if let Some((prev_sk, start)) = open.take() {
                        sections.push((prev_sk, start..at + 1));
                        if prev_sk == SectionKind::Entities {
                            entities.push(start..at);
                            ends.push(at);
                        }
                    }
                }
                Mark::Eof(local, eof_byte_end) => {
                    let at = base + local;
                    if let Some((prev_sk, start)) = open.take() {
                        sections.push((prev_sk, start..at));
                        if prev_sk == SectionKind::Entities {
                            entities.push(start..at);
                        }
                    }
                    stop_local = local + 1;
                    final_end = eof_byte_end;
                    saw_eof = true;
                    break;
                }
            }
        }
        record_starts.extend_from_slice(
            part.starts
                .get(..stop_local as usize)
                .unwrap_or(&part.starts),
        );
        part.starts = Vec::new();
        if let Some(eof_byte) = part.eof_end
            && saw_eof
        {
            final_end = eof_byte;
        }
    }
    if let Some((prev_sk, start)) = open.take() {
        let end = record_starts.len() as u32;
        sections.push((prev_sk, start..end));
        if prev_sk == SectionKind::Entities {
            entities.push(start..end);
        }
    }
    record_starts.push(final_end);
    let entities_end = ends.first().copied();
    (record_starts, sections, entities, entities_end)
}

#[allow(clippy::too_many_arguments)]
fn assemble_inner(
    src: SourceBytes,
    mut parts: Vec<Part>,
    record_starts: Vec<u32>,
    sections: Vec<(SectionKind, Range<u32>)>,
    entities: Vec<Range<u32>>,
    entities_end: Option<u32>,
    version: Vec<u8>,
    charset: Charset,
    header: HeaderState,
) -> Document {
    let total_records = record_starts.len().saturating_sub(1) as u32;

    let mut part_bases = Vec::with_capacity(parts.len());
    let mut cur_base = 0u32;
    for part in &parts {
        part_bases.push(cur_base);
        cur_base += part.record_count;
    }

    let section_at = |rec: u32| -> Option<SectionKind> {
        sections
            .iter()
            .find(|(_, r)| r.contains(&rec))
            .map(|(sk, _)| *sk)
    };

    let mut non_ent_handles: Vec<(u32, u64)> = Vec::new();
    let mut non_ent_seen: BTreeSet<u64> = BTreeSet::new();
    let mut has_duplicate_object_handle = false;
    let mut forget_refs: Vec<(u64, u32)> = Vec::new();
    let mut plain_summaries: Vec<PlainSummary> = Vec::with_capacity(parts.len() * 2);
    let mut first_paper_owner: Option<u64> = None;
    let mut ent_seen_handles: BTreeSet<u64> = BTreeSet::new();
    let mut duplicate_handles: BTreeSet<u64> = BTreeSet::new();
    let mut prev_ent_handle: Option<u64> = None;
    let mut ent_handles_monotonic = true;
    let mut min_ent_h: Option<u64> = None;
    let mut max_ent_h: Option<u64> = None;
    let mut min_all_ent_h: Option<u64> = None;
    let mut max_all_ent_h: Option<u64> = None;
    let mut est_lifted_rows = 0usize;
    let mut has_non_plain_item = false;
    let mut any_has_100 = false;

    for (part, &base) in parts.iter().zip(&part_bases) {
        if base >= total_records {
            break;
        }
        let prefix_sec = section_at(base);
        if prefix_sec.is_some_and(|sk| sk.has_db_objects() && sk != SectionKind::Entities) {
            for &(local_rec, h) in &part.prefix_non_ent_handles {
                let rec = base + local_rec;
                if rec < total_records {
                    non_ent_handles.push((rec, h));
                    if h != 0 && !non_ent_seen.insert(h) {
                        has_duplicate_object_handle = true;
                    }
                }
            }
        }
        for &(local_rec, h) in &part.known_non_ent_handles {
            let rec = base + local_rec;
            if rec < total_records {
                non_ent_handles.push((rec, h));
                if h != 0 && !non_ent_seen.insert(h) {
                    has_duplicate_object_handle = true;
                }
            }
        }
        for &(target_h, local_rec) in &part.forget_refs {
            let rec = base + local_rec;
            if rec < total_records && section_at(rec).is_some_and(SectionKind::has_db_objects) {
                forget_refs.push((target_h, rec));
            }
        }
        if prefix_sec == Some(SectionKind::Entities) {
            plain_summaries.push(part.prefix_plain);
        }
        plain_summaries.push(part.body_plain);
    }
    forget_refs.sort_unstable();
    forget_refs.dedup();
    let max_non_ent_handle = non_ent_seen.last().copied().unwrap_or(0);

    for (part, &base) in parts.iter().zip(&part_bases) {
        if base >= total_records {
            break;
        }
        let prefix_sec = section_at(base);
        let item_start = if prefix_sec == Some(SectionKind::Entities) {
            0
        } else {
            part.prefix_items_end
        };
        for item in part.items.get(item_start..).unwrap_or_default() {
            let (local_rec, h_opt) = match item {
                PartItem::Lifted {
                    local_rec,
                    lifted,
                    has_100,
                    ..
                } => {
                    if *has_100 && base + *local_rec < total_records {
                        any_has_100 = true;
                    }
                    if base + *local_rec < total_records {
                        est_lifted_rows += 1;
                        if let Some(h) = lifted.handle {
                            min_ent_h = Some(min_ent_h.map_or(h, |m| m.min(h)));
                            max_ent_h = Some(max_ent_h.map_or(h, |m| m.max(h)));
                        }
                    }
                    (*local_rec, lifted.handle)
                }
                PartItem::Attrib {
                    local_rec,
                    handle,
                    lift,
                    has_100,
                } => {
                    if base + *local_rec < total_records {
                        if *has_100 {
                            any_has_100 = true;
                        }
                        if matches!(lift, AttribLift::Multiline | AttribLift::Not(_)) {
                            has_non_plain_item = true;
                        }
                    }
                    (*local_rec, *handle)
                }
                PartItem::Paper {
                    local_rec,
                    handle,
                    owner,
                    has_100,
                } => {
                    if base + *local_rec < total_records {
                        has_non_plain_item = true;
                        if *has_100 {
                            any_has_100 = true;
                        }
                        if first_paper_owner.is_none() {
                            first_paper_owner = *owner;
                        }
                    }
                    (*local_rec, *handle)
                }
                PartItem::Dropped {
                    local_rec,
                    handle,
                    has_100,
                    ..
                } => {
                    if base + *local_rec < total_records {
                        has_non_plain_item = true;
                        if *has_100 {
                            any_has_100 = true;
                        }
                    }
                    (*local_rec, *handle)
                }
                PartItem::OtherEntity {
                    local_rec,
                    handle,
                    other_kind,
                    has_100,
                } => {
                    if base + *local_rec < total_records
                        && *other_kind != OtherKind::NonObject
                        && *has_100
                    {
                        any_has_100 = true;
                    }
                    (*local_rec, *handle)
                }
            };
            if base + local_rec < total_records
                && let Some(h) = h_opt
            {
                min_all_ent_h = Some(min_all_ent_h.map_or(h, |m| m.min(h)));
                max_all_ent_h = Some(max_all_ent_h.map_or(h, |m| m.max(h)));
                if h != 0 && h <= max_non_ent_handle && non_ent_seen.contains(&h) {
                    has_duplicate_object_handle = true;
                }
                if prev_ent_handle.is_some_and(|p| h <= p) {
                    ent_handles_monotonic = false;
                }
                prev_ent_handle = Some(h);
            }
        }
    }
    if !ent_handles_monotonic {
        if let (Some(lo), Some(hi)) = (min_all_ent_h, max_all_ent_h)
            && hi.saturating_sub(lo) <= 16_777_216
        {
            let mut bits = vec![0u64; ((hi - lo) as usize / 64) + 1];
            for (part, &base) in parts.iter().zip(&part_bases) {
                if base >= total_records {
                    break;
                }
                let item_start = if section_at(base) == Some(SectionKind::Entities) {
                    0
                } else {
                    part.prefix_items_end
                };
                for item in part.items.get(item_start..).unwrap_or_default() {
                    let (local_rec, h_opt) = match item {
                        PartItem::Lifted {
                            local_rec, lifted, ..
                        } => (*local_rec, lifted.handle),
                        PartItem::Attrib {
                            local_rec, handle, ..
                        }
                        | PartItem::Paper {
                            local_rec, handle, ..
                        }
                        | PartItem::Dropped {
                            local_rec, handle, ..
                        }
                        | PartItem::OtherEntity {
                            local_rec, handle, ..
                        } => (*local_rec, *handle),
                    };
                    if base + local_rec < total_records
                        && let Some(h) = h_opt
                    {
                        let off = (h - lo) as usize;
                        let mask = 1u64 << (off & 63);
                        if let Some(word) = bits.get_mut(off >> 6) {
                            if *word & mask != 0 {
                                duplicate_handles.insert(h);
                                if h != 0 {
                                    has_duplicate_object_handle = true;
                                }
                            } else {
                                *word |= mask;
                            }
                        }
                    }
                }
            }
        } else {
            for (part, &base) in parts.iter().zip(&part_bases) {
                if base >= total_records {
                    break;
                }
                let item_start = if section_at(base) == Some(SectionKind::Entities) {
                    0
                } else {
                    part.prefix_items_end
                };
                for item in part.items.get(item_start..).unwrap_or_default() {
                    let (local_rec, h_opt) = match item {
                        PartItem::Lifted {
                            local_rec, lifted, ..
                        } => (*local_rec, lifted.handle),
                        PartItem::Attrib {
                            local_rec, handle, ..
                        }
                        | PartItem::Paper {
                            local_rec, handle, ..
                        }
                        | PartItem::Dropped {
                            local_rec, handle, ..
                        }
                        | PartItem::OtherEntity {
                            local_rec, handle, ..
                        } => (*local_rec, *handle),
                    };
                    if base + local_rec < total_records
                        && let Some(h) = h_opt
                        && !ent_seen_handles.insert(h)
                    {
                        duplicate_handles.insert(h);
                        if h != 0 {
                            has_duplicate_object_handle = true;
                        }
                    }
                }
            }
        }
    }

    let mut cat_items: Vec<(u8, Vec<u8>, Vec<Value>, u32)> = Vec::new();
    let mut objects_by_handle: BTreeMap<u64, u32> = BTreeMap::new();
    let mut dup_object_handles: BTreeSet<u64> = BTreeSet::new();
    let mut has_spatial_filter = false;
    let mut names_map: BTreeMap<&'static [u8], BTreeMap<String, String>> = BTreeMap::new();
    let mut model_owner: Option<u64> = None;

    let mut tables_reader = src.page_reader();
    for &(sk, ref range) in &sections {
        if !matches!(
            sk,
            SectionKind::Tables | SectionKind::Blocks | SectionKind::Objects
        ) {
            continue;
        }
        for rec in range.start..range.end {
            let r_start = record_starts.get(rec as usize).copied().unwrap_or(0) as usize;
            let r_end = record_starts
                .get(rec as usize + 1)
                .copied()
                .unwrap_or(src.len() as u32) as usize;
            tables_reader.with_range(r_start..r_end, |rs| {
                let spans = read_record_spans_slice(&rs);
                let Some(first) = spans.first() else {
                    return;
                };
                if first.code != 0 {
                    return;
                }
                let kind_raw = rs.slice(first.value, first.value_end).trim_ascii();
                if is_non_object_kind(kind_raw) {
                    return;
                }
                let body = spans.get(1..).unwrap_or_default();

                if let Some(&table_key) = TABLE_KINDS.iter().find(|&&tk| tk == kind_raw)
                    && let Some(name_span) = body.iter().find(|s| s.code == 2)
                {
                    let raw_name = rs.slice(name_span.value, name_span.value_end);
                    if let Some(decoded) = charset.decode(raw_name) {
                        if table_key == b"BLOCK_RECORD"
                            && decoded.eq_ignore_ascii_case("*Model_Space")
                        {
                            model_owner = handle_in_spans_slice(kind_raw, body, &rs);
                        }
                        names_map
                            .entry(table_key)
                            .or_default()
                            .insert(decoded.to_lowercase(), decoded);
                    }
                }

                if sk == SectionKind::Objects {
                    if let Some(h) = handle_in_spans_slice(kind_raw, body, &rs) {
                        if let std::collections::btree_map::Entry::Vacant(e) =
                            objects_by_handle.entry(h)
                        {
                            e.insert(rec);
                        } else {
                            dup_object_handles.insert(h);
                        }
                    }
                    if kind_raw == b"SPATIAL_FILTER" {
                        has_spatial_filter = true;
                    }
                    if !single::is_catalog_object(kind_raw) && kind_raw != b"XRECORD" {
                        return;
                    }
                }

                let values: Vec<Value> = body
                    .iter()
                    .map(|s| {
                        let raw = rs.slice(s.value, s.value_end);
                        value::value_of(kind_raw, s.code, raw)
                    })
                    .collect();
                let sec_code = match sk {
                    SectionKind::Tables => 0,
                    SectionKind::Blocks => 1,
                    _ => 2,
                };
                cat_items.push((sec_code, kind_raw.to_vec(), values, rec));
            });
        }
    }

    let live_object_handles: BTreeSet<u64> = objects_by_handle
        .keys()
        .copied()
        .filter(|h| !dup_object_handles.contains(h))
        .collect();
    let mut fixed_max_handle: Option<u64> = None;
    let mut fold_fixed = |h: u64| {
        fixed_max_handle = Some(fixed_max_handle.map_or(h, |m| m.max(h)));
    };
    for &(rec, h) in &non_ent_handles {
        if section_at(rec) != Some(SectionKind::Objects) || !live_object_handles.contains(&h) {
            fold_fixed(h);
        }
    }

    let mut catalog = SingleCatalog::new(charset, &version, header.insunits, header.plimmin);
    catalog.register_and_read_tables(cat_items, first_paper_owner);

    let mut open_diagnostics = Vec::new();
    if catalog.unit() == Unit::Unknown {
        open_diagnostics.push(OpenDiagnostic {
            kind: OpenDiagnosticKind::UnitsMissing,
            record: None,
            handle: None,
        });
    }

    let fast_plain = entities.len() <= 1
        && duplicate_handles.is_empty()
        && !has_non_plain_item
        && combine_plain_summaries(plain_summaries);
    let (ent_family, attached_map, owned_set, plain, all_plain_attribs) = if fast_plain {
        (Vec::new(), BTreeMap::new(), BTreeSet::new(), true, true)
    } else {
        let fam = rebuild_ent_family(&src, &record_starts, &entities);
        let (m, o, p) = attach_family(&fam);
        (fam, m, o, p, false)
    };

    let mut shapes = Shapes::default();
    let mut rows: Paged<Row> = Paged::new();
    let mut boxes: Paged<[f64; 4]> = Paged::new();
    let mut by_handle = HandleIndex::with_range(min_ent_h, max_ent_h, est_lifted_rows);
    let mut non_slice_usages: Vec<NonSliceUsage> = Vec::new();
    let mut counts = Counts {
        records: total_records,
        ..Counts::default()
    };

    let mut general_attribs: BTreeMap<u32, (AttribLift, u32, u32, u32)> = BTreeMap::new();
    let mut part_bases_and_shifts: Vec<(u32, u32, u32, u32, u32, u32)> =
        Vec::with_capacity(parts.len());

    for (part, &base) in parts.iter_mut().zip(&part_bases) {
        if base >= total_records {
            break;
        }
        let prefix_sec = section_at(base);
        let item_start = if prefix_sec == Some(SectionKind::Entities) {
            0
        } else {
            part.prefix_items_end
        };
        let src_pools = core::mem::take(&mut part.pools);
        let (vertex_base, text_base, text_extra_base, insert_extra_base, extra_style_base) =
            merge_pools(&mut shapes.pools, src_pools);
        part_bases_and_shifts.push((
            base,
            vertex_base,
            text_base,
            text_extra_base,
            insert_extra_base,
            extra_style_base,
        ));

        if !all_plain_attribs {
            for item in part.items.get(item_start..).unwrap_or_default() {
                if let PartItem::Attrib {
                    local_rec,
                    ref lift,
                    ..
                } = *item
                {
                    let rec = base + local_rec;
                    if rec < total_records {
                        general_attribs
                            .insert(rec, (*lift, text_base, text_extra_base, extra_style_base));
                    }
                }
            }
        }
    }

    let intern_extra_lt = |extra_style: u32, pools: &mut Pools, cat: &mut SingleCatalog| {
        if let Some(es_idx) = extra_style.checked_sub(1)
            && let Some(LineTypeRef::Named(span)) =
                pools.extra_styles.get(es_idx as usize).map(|e| e.line_type)
        {
            let name = pools.str_of(span);
            let lt_id = cat.intern_line_type(name).0;
            if let Some(es) = pools.extra_styles.get_mut(es_idx as usize) {
                es.line_type_id = lt_id;
            }
        }
    };
    let mut zero_layer_id: Option<u32> = None;
    let mut last_layer: Option<(TextSpan, u32)> = None;
    let mut resolve_layer = |span: TextSpan, pools: &Pools, cat: &mut SingleCatalog| -> u32 {
        if span == TextSpan::ZERO {
            if let Some(id) = zero_layer_id {
                return id;
            }
            let id = cat.intern_layer("0").0;
            zero_layer_id = Some(id);
            return id;
        }
        if let Some((prev_span, prev_id)) = last_layer
            && prev_span == span
        {
            return prev_id;
        }
        let id = cat.intern_layer(pools.str_of(span)).0;
        last_layer = Some((span, id));
        id
    };

    let mut open_fast_insert: Option<u32> = None;

    for (
        part,
        &(base, vertex_base, text_base, text_extra_base, insert_extra_base, extra_style_base),
    ) in parts.into_iter().zip(&part_bases_and_shifts)
    {
        let item_start = if section_at(base) == Some(SectionKind::Entities) {
            0
        } else {
            part.prefix_items_end
        };

        for item in part.items.into_iter().skip(item_start) {
            match item {
                PartItem::Lifted {
                    local_rec,
                    lifted,
                    has_xdict,
                    ..
                } => {
                    let rec = base + local_rec;
                    if rec >= total_records {
                        continue;
                    }
                    counts.entities += 1;
                    if !all_plain_attribs && owned_set.contains(&rec) {
                        if let Some(h) = lifted.handle {
                            fold_fixed(h);
                        }
                        continue;
                    }
                    let mut geom = lifted.geom;
                    shift_geom_pools(
                        &mut geom,
                        vertex_base,
                        text_base,
                        text_extra_base,
                        insert_extra_base,
                    );
                    let extra_style = if lifted.extra_style == 0 {
                        0
                    } else {
                        lifted.extra_style + extra_style_base
                    };
                    let layer_span = lifted.layer.shifted(text_base);

                    intern_extra_lt(extra_style, &mut shapes.pools, &mut catalog);
                    let layer_id = resolve_layer(layer_span, &shapes.pools, &mut catalog);

                    if let Geom::Insert(ref mut ins) = geom {
                        if has_xdict && has_spatial_filter {
                            let r_start =
                                record_starts.get(rec as usize).copied().unwrap_or(0) as usize;
                            let r_end = record_starts
                                .get(rec as usize + 1)
                                .copied()
                                .unwrap_or(src.len() as u32)
                                as usize;
                            if let Some((_, ins_vals)) = decode_record(&src, r_start..r_end) {
                                let res = single::symbol_clip(&ins_vals, |h| {
                                    let obj_rec = *objects_by_handle.get(&h)?;
                                    let os =
                                        record_starts.get(obj_rec as usize).copied().unwrap_or(0)
                                            as usize;
                                    let oe = record_starts
                                        .get(obj_rec as usize + 1)
                                        .copied()
                                        .unwrap_or(src.len() as u32)
                                        as usize;
                                    let (k, v) = decode_record(&src, os..oe)?;
                                    Some((Cow::Owned(k), Cow::Owned(v)))
                                });
                                match res {
                                    Ok(Some(clip)) => {
                                        let _ = shapes.pools.set_insert_clip(ins, clip);
                                    }
                                    Ok(None) => {}
                                    Err(_) => {
                                        open_diagnostics.push(OpenDiagnostic {
                                            kind: OpenDiagnosticKind::ClipDropped,
                                            record: Some(rec),
                                            handle: lifted.handle,
                                        });
                                    }
                                }
                            }
                        }

                        if all_plain_attribs {
                            ins.attrib_first = shapes.pools.attribs.len() as u32;
                            ins.attrib_count = 0;
                            ins.attached = 0;
                        } else if let Some(att_recs) = attached_map.get(&rec) {
                            ins.attached = att_recs.len() as u32;
                            ins.attrib_first = shapes.pools.attribs.len() as u32;
                            let mut ok_count = 0u32;
                            for &att_rec in att_recs {
                                if let Some((att_lift, tb, teb, esb)) =
                                    general_attribs.get(&att_rec).copied()
                                {
                                    match att_lift {
                                        AttribLift::Row(mut arow, layer_span) => {
                                            arow.record = att_rec;
                                            arow.text = arow.text.shifted_spans(tb, teb);
                                            arow.tag = arow.tag.shifted(tb);
                                            let layer_span = layer_span.shifted(tb);
                                            if arow.extra_style != 0 {
                                                arow.extra_style += esb;
                                            }
                                            intern_extra_lt(
                                                arow.extra_style,
                                                &mut shapes.pools,
                                                &mut catalog,
                                            );
                                            arow.layer = resolve_layer(
                                                layer_span,
                                                &shapes.pools,
                                                &mut catalog,
                                            );
                                            shapes.pools.attribs.push(arow);
                                            ok_count += 1;
                                        }
                                        AttribLift::Multiline => {
                                            let as_ = record_starts
                                                .get(att_rec as usize)
                                                .copied()
                                                .unwrap_or(0)
                                                as usize;
                                            let ae = record_starts
                                                .get(att_rec as usize + 1)
                                                .copied()
                                                .unwrap_or(src.len() as u32)
                                                as usize;
                                            if let Some((_, vals)) = decode_record(&src, as_..ae) {
                                                match catalog.read_multiline_attrib(vals, att_rec) {
                                                    Ok(attr) => {
                                                        let lid = attr.layer;
                                                        let lt_id_opt = match attr.style.line_type {
                                                            zumen_drawing::Inherit::Own(id) => {
                                                                Some(id)
                                                            }
                                                            _ => None,
                                                        };
                                                        let ltname = lt_id_opt.and_then(|id| {
                                                            catalog
                                                                .line_type_name(id)
                                                                .map(str::to_owned)
                                                        });
                                                        if let Some(mut arow) =
                                                            lift::push_lifted_attribute(
                                                                attr,
                                                                att_rec,
                                                                &mut shapes.pools,
                                                            )
                                                        {
                                                            if let (Some(lt_id), Some(ltname)) =
                                                                (lt_id_opt, ltname)
                                                                && let Some(span) =
                                                                    shapes.pools.push_utf8(&ltname)
                                                                && let Some(es_idx) =
                                                                    arow.extra_style.checked_sub(1)
                                                                && let Some(es) = shapes
                                                                    .pools
                                                                    .extra_styles
                                                                    .get_mut(es_idx as usize)
                                                            {
                                                                es.line_type =
                                                                    LineTypeRef::Named(span);
                                                                es.line_type_id = lt_id.0;
                                                            }
                                                            arow.layer = lid.0;
                                                            shapes.pools.attribs.push(arow);
                                                            ok_count += 1;
                                                        }
                                                    }
                                                    Err(code) => {
                                                        let kind = match code {
                                                            "dxf.non-planar.dropped" => {
                                                                Some(OpenDiagnosticKind::NonPlanar)
                                                            }
                                                            "dxf.code-page.unsupported" => {
                                                                Some(OpenDiagnosticKind::CodePage)
                                                            }
                                                            "dxf.lift.invalid" => Some(
                                                                OpenDiagnosticKind::LiftInvalid,
                                                            ),
                                                            _ => None,
                                                        };
                                                        if let Some(kind) = kind {
                                                            open_diagnostics.push(OpenDiagnostic {
                                                                kind,
                                                                record: Some(att_rec),
                                                                handle: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        AttribLift::Not(reason) => {
                                            open_diagnostics.push(OpenDiagnostic {
                                                kind: match reason {
                                                    DropReason::Invalid => {
                                                        OpenDiagnosticKind::LiftInvalid
                                                    }
                                                    DropReason::NonPlanar => {
                                                        OpenDiagnosticKind::NonPlanar
                                                    }
                                                    DropReason::CodePage => {
                                                        OpenDiagnosticKind::CodePage
                                                    }
                                                },
                                                record: Some(rec),
                                                handle: lifted.handle,
                                            });
                                        }
                                    }
                                }
                            }
                            ins.attrib_count = ok_count;
                        }
                    }

                    let (kind, shape_idx) = shapes.push(geom);
                    let row_idx = rows.len() as u32;
                    rows.push(Row::new(
                        rec,
                        lifted.handle,
                        kind,
                        shape_idx,
                        layer_id,
                        lifted.color,
                        extra_style,
                        true,
                    ));
                    if all_plain_attribs {
                        open_fast_insert = match geom {
                            Geom::Insert(ins) if ins.flags & INSERT_FOLLOWED != 0 => Some(row_idx),
                            _ => None,
                        };
                    }
                    let bx = bounds(&shapes.pools, &geom);
                    boxes.push(bx);
                    match lifted.handle {
                        Some(h) if !duplicate_handles.contains(&h) => {
                            match by_handle.push_base_raw(h, row_idx) {
                                Some(0) | None => {}
                                Some(n) => {
                                    duplicate_handles.insert(h);
                                    fold_fixed(h);
                                    counts.unaddressable += n;
                                }
                            }
                        }
                        Some(h) => {
                            fold_fixed(h);
                            counts.unaddressable += 1;
                        }
                        None => {
                            counts.unaddressable += 1;
                        }
                    }
                    counts.elements += 1;
                }
                PartItem::Attrib {
                    local_rec,
                    handle,
                    lift,
                    ..
                } => {
                    let rec = base + local_rec;
                    if rec >= total_records {
                        continue;
                    }
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    counts.entities += 1;
                    if all_plain_attribs {
                        if let (Some(parent_row_idx), AttribLift::Row(mut arow, layer_span)) =
                            (open_fast_insert, lift)
                        {
                            arow.record = rec;
                            arow.text = arow.text.shifted_spans(text_base, text_extra_base);
                            arow.tag = arow.tag.shifted(text_base);
                            let layer_span = layer_span.shifted(text_base);
                            if arow.extra_style != 0 {
                                arow.extra_style += extra_style_base;
                            }
                            intern_extra_lt(arow.extra_style, &mut shapes.pools, &mut catalog);
                            arow.layer = resolve_layer(layer_span, &shapes.pools, &mut catalog);
                            shapes.pools.attribs.push(arow);
                            if let Some(pr) = rows.get(parent_row_idx as usize).copied()
                                && let Some(ins) = shapes.inserts.get_mut(pr.shape as usize)
                            {
                                ins.attrib_count += 1;
                                ins.attached += 1;
                                let ins_copy = *ins;
                                let new_bx = bounds(&shapes.pools, &Geom::Insert(ins_copy));
                                if let Some(b_slot) = boxes.get_mut(parent_row_idx as usize) {
                                    *b_slot = new_bx;
                                }
                            }
                        }
                    } else if !owned_set.contains(&rec) {
                        counts.unlifted_entities += 1;
                    }
                }
                PartItem::Paper {
                    local_rec, handle, ..
                } => {
                    let rec = base + local_rec;
                    if rec >= total_records {
                        continue;
                    }
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    counts.entities += 1;
                    open_fast_insert = None;
                    counts.paper += 1;
                    if catalog.has_paper_mm() {
                        feed_non_slice_entity(
                            &src,
                            &record_starts,
                            rec,
                            &attached_map,
                            &mut catalog,
                            &mut non_slice_usages,
                        );
                    }
                }
                PartItem::Dropped {
                    local_rec,
                    handle,
                    reason,
                    ..
                } => {
                    let rec = base + local_rec;
                    if rec >= total_records {
                        continue;
                    }
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    counts.entities += 1;
                    open_fast_insert = None;
                    counts.dropped += 1;
                    open_diagnostics.push(OpenDiagnostic {
                        kind: match reason {
                            DropReason::Invalid => OpenDiagnosticKind::LiftInvalid,
                            DropReason::NonPlanar => OpenDiagnosticKind::NonPlanar,
                            DropReason::CodePage => OpenDiagnosticKind::CodePage,
                        },
                        record: Some(rec),
                        handle,
                    });
                }
                PartItem::OtherEntity {
                    local_rec,
                    handle,
                    other_kind,
                    ..
                } => {
                    let rec = base + local_rec;
                    if rec >= total_records {
                        continue;
                    }
                    if let Some(h) = handle {
                        fold_fixed(h);
                    }
                    open_fast_insert = None;
                    if other_kind != OtherKind::NonObject {
                        counts.entities += 1;
                    }
                    if !all_plain_attribs && owned_set.contains(&rec) {
                        continue;
                    }
                    match other_kind {
                        OtherKind::NonObject | OtherKind::Seqend => {}
                        OtherKind::Other => {
                            counts.unlifted_entities += 1;
                        }
                        OtherKind::Lifted => {
                            counts.unlifted_entities += 1;
                            feed_non_slice_entity(
                                &src,
                                &record_starts,
                                rec,
                                &attached_map,
                                &mut catalog,
                                &mut non_slice_usages,
                            );
                        }
                    }
                }
            }
        }
    }

    counts.unaddressable += by_handle.finish_base(&mut duplicate_handles, &mut fold_fixed);
    let base_row_count = rows.len() as u32;

    let CatalogShell {
        unit,
        layers,
        by_layer_name,
        line_types,
        by_line_type_name,
        dimension_styles,
        text_styles,
        symbols,
        model_sort,
    } = catalog.finish();

    let mut def_by_name: BTreeMap<String, u32> = BTreeMap::new();
    let mut blocks_for_born: BTreeMap<String, String> = BTreeMap::new();
    for (idx, sym) in symbols.iter().enumerate() {
        if sym.key.namespace == BLOCK {
            def_by_name
                .entry(sym.key.name.clone())
                .or_insert(idx as u32);
            blocks_for_born.insert(sym.key.name.to_lowercase(), sym.key.name.clone());
        }
    }

    let mut layer_uses = vec![0u32; layers.len()];
    let mut line_type_uses = vec![0u32; line_types.len()];
    let mut bump_layer = |id: u32| {
        let idx = id as usize;
        if idx >= layer_uses.len() {
            layer_uses.resize(idx + 1, 0);
        }
        if let Some(s) = layer_uses.get_mut(idx) {
            *s += 1;
        }
    };
    let mut bump_lt = |extra_style: u32, pools: &Pools| {
        if let Some(es_idx) = extra_style.checked_sub(1)
            && let Some(es) = pools.extra_styles.get(es_idx as usize)
            && matches!(es.line_type, LineTypeRef::Named(_))
        {
            let idx = es.line_type_id as usize;
            if idx >= line_type_uses.len() {
                line_type_uses.resize(idx + 1, 0);
            }
            if let Some(s) = line_type_uses.get_mut(idx) {
                *s += 1;
            }
        }
    };

    let mut last_def: Option<(TextSpan, Option<u32>)> = None;
    for row_idx in 0..rows.len() {
        let Some(r) = rows.get(row_idx).copied() else {
            continue;
        };
        bump_layer(r.layer);
        bump_lt(r.extra_style, &shapes.pools);
        if r.kind == Kind::Insert
            && let Some(ins) = shapes.inserts.get_mut(r.shape as usize)
        {
            let found = if let Some((prev_span, prev_def)) = last_def
                && prev_span == ins.name
            {
                prev_def
            } else {
                let name = shapes.pools.str_of(ins.name);
                let def = def_by_name.get(name).copied();
                last_def = Some((ins.name, def));
                def
            };
            match found {
                Some(def_id) => ins.definition = def_id,
                None => {
                    ins.definition = NO_DEFINITION;
                    open_diagnostics.push(OpenDiagnostic {
                        kind: OpenDiagnosticKind::BlockMissing,
                        record: Some(r.record),
                        handle: r.handle(),
                    });
                }
            }
            for idx in ins.attrib_first..ins.attrib_first.saturating_add(ins.attrib_count) {
                if let Some(&attr) = shapes.pools.attribs.get(idx as usize) {
                    bump_layer(attr.layer);
                    bump_lt(attr.extra_style, &shapes.pools);
                }
            }
        }
    }

    for u in &non_slice_usages {
        for &lid in &u.layers {
            if let Some(s) = layer_uses.get_mut(lid.0 as usize) {
                *s += 1;
            }
        }
        for &ltid in &u.line_types {
            if let Some(s) = line_type_uses.get_mut(ltid.0 as usize) {
                *s += 1;
            }
        }
    }
    for sym in &symbols {
        for el in &sym.space.elements {
            if let Some(s) = layer_uses.get_mut(el.layer.0 as usize) {
                *s += 1;
            }
            if let zumen_drawing::Inherit::Own(ltid) = el.style.line_type
                && let Some(s) = line_type_uses.get_mut(ltid.0 as usize)
            {
                *s += 1;
            }
        }
        for ad in &sym.attributes {
            if let Some(s) = layer_uses.get_mut(ad.attribute.layer.0 as usize) {
                *s += 1;
            }
            if let zumen_drawing::Inherit::Own(ltid) = ad.attribute.style.line_type
                && let Some(s) = line_type_uses.get_mut(ltid.0 as usize)
            {
                *s += 1;
            }
        }
    }

    let markers = version.as_slice() >= b"AC1012" || any_has_100;

    let tables_cache = SingleTables {
        model_owner,
        markers,
        version: version.clone(),
        charset,
        dwg_seed: None,
        names: Arc::new(names_map),
        blocks: Arc::new(blocks_for_born),
    };

    Document {
        src,
        record_starts,
        counts,
        unit,
        version,
        charset,
        layers,
        by_layer_name,
        line_types,
        by_line_type_name,
        dimension_styles,
        text_styles,
        symbols,
        def_by_name,
        model_sort,
        rows,
        by_handle,
        duplicate_handles,
        base_row_count,
        added_by_record: BTreeMap::new(),
        shapes,
        boxes,
        grid: Grid::default(),
        display: Display::with_rows(0),
        open_diagnostics,
        overlay: BTreeMap::new(),
        gone: BTreeSet::new(),
        history: History::new(),
        next_added_record: total_records,
        entities,
        entities_end,
        sections,
        header,
        tables_cache,
        has_duplicate_object_handle,
        objects_by_handle,
        fixed_max_handle,
        live_object_handles,
        added_follower_handles: BTreeSet::new(),
        forget_refs,
        plain,
        ent_family,
        layer_uses,
        line_type_uses,
        non_slice_usages,
    }
}

fn feed_non_slice_entity(
    src: &SourceBytes,
    record_starts: &[u32],
    rec: u32,
    attached_map: &BTreeMap<u32, Vec<u32>>,
    catalog: &mut SingleCatalog,
    usages: &mut Vec<NonSliceUsage>,
) {
    let rs = record_starts.get(rec as usize).copied().unwrap_or(0) as usize;
    let re = record_starts
        .get(rec as usize + 1)
        .copied()
        .unwrap_or(src.len() as u32) as usize;
    let fast_done = src.with_range(rs..re, |slice| {
        let spans = read_record_spans_slice(&slice);
        let Some(first) = spans.first() else {
            return Ok(());
        };
        let kind_raw = slice.slice(first.value, first.value_end).trim_ascii();
        let body = spans.get(1..).unwrap_or_default();
        let raw_layer = body
            .iter()
            .find(|s| s.code == 8)
            .map(|s| slice.slice(s.value, s.value_end))
            .unwrap_or(b"0");
        let raw_lt = body
            .iter()
            .find(|s| s.code == 6)
            .map(|s| slice.slice(s.value, s.value_end));
        let has_followers = attached_map.get(&rec).is_some_and(|v| !v.is_empty());

        if !has_followers
            && kind_raw != b"MULTILEADER"
            && raw_layer.is_ascii()
            && raw_lt.is_none_or(<[u8]>::is_ascii)
            && let Ok(layer_str) = core::str::from_utf8(raw_layer)
            && catalog.has_layer(layer_str)
        {
            let lt_known = match raw_lt.and_then(|b| core::str::from_utf8(b).ok()) {
                None => Some(None),
                Some(s)
                    if s.is_empty()
                        || s.eq_ignore_ascii_case("BYLAYER")
                        || s.eq_ignore_ascii_case("BYBLOCK") =>
                {
                    Some(None)
                }
                Some(s) if catalog.has_line_type(s) => Some(Some(catalog.intern_line_type(s))),
                Some(_) => None,
            };
            if let Some(lt_opt) = lt_known {
                let lid = catalog.intern_layer(layer_str);
                usages.push(NonSliceUsage {
                    record: rec,
                    layers: vec![lid],
                    line_types: lt_opt.into_iter().collect(),
                });
                return Ok(());
            }
        }
        Err((raw_layer.to_vec(), raw_lt.map(<[u8]>::to_vec)))
    });
    let Err((raw_layer, raw_lt)) = fast_done else {
        return;
    };

    let Some((kind, values)) = decode_record(src, rs..re) else {
        return;
    };
    let mut attached_records = Vec::new();
    if let Some(att_recs) = attached_map.get(&rec) {
        for &ar in att_recs {
            let ars = record_starts.get(ar as usize).copied().unwrap_or(0) as usize;
            let are = record_starts
                .get(ar as usize + 1)
                .copied()
                .unwrap_or(src.len() as u32) as usize;
            if let Some((ak, av)) = decode_record(src, ars..are) {
                attached_records.push((ak, av, ar));
            }
        }
    }
    catalog.read_entity_for_catalog(&kind, values, rec, attached_records);
    let mut used_layers = Vec::new();
    let mut used_lts = Vec::new();
    if let Ok(ls) = core::str::from_utf8(&raw_layer)
        && catalog.has_layer(ls)
    {
        used_layers.push(catalog.intern_layer(ls));
    }
    if let Some(ltb) = &raw_lt
        && let Ok(lts) = core::str::from_utf8(ltb)
        && !lts.is_empty()
        && !lts.eq_ignore_ascii_case("BYLAYER")
        && !lts.eq_ignore_ascii_case("BYBLOCK")
        && catalog.has_line_type(lts)
    {
        used_lts.push(catalog.intern_line_type(lts));
    }
    usages.push(NonSliceUsage {
        record: rec,
        layers: used_layers,
        line_types: used_lts,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 線を並べた小さい DXF
    fn source(handles: &[String], seed: &str) -> Vec<u8> {
        let mut text = format!(
            "  0
SECTION
  2
HEADER
  9
$HANDSEED
  5
{seed}
  0
ENDSEC
  0
SECTION
  2
ENTITIES
"
        );
        for (at, handle) in handles.iter().enumerate() {
            text += &format!(
                "  0
LINE
  5
{handle}
  8
0
 10
{at}.0
 20
0.0
 11
{at}.5
 21
1.0
"
            );
        }
        text += "  0
ENDSEC
  0
EOF
";
        text.into_bytes()
    }

    fn named(handles: &[&str]) -> Vec<String> {
        handles.iter().map(|handle| (*handle).to_owned()).collect()
    }

    #[test]
    fn handles_far_apart_are_found() {
        assert_eq!(size_of::<Row>(), 32);
        let src = source(&named(&["2A", "FFFFF", "400"]), "100000");
        let document = Document::open(src, 1).expect("開く");
        assert_eq!(document.row_of(0x2A), Some(0));
        assert_eq!(document.row_of(0xFFFFF), Some(1));
        assert_eq!(document.row_of(0x400), Some(2));
        assert_eq!(document.row_of(0x2B), None);
        assert_eq!(document.row_of(0), None);
        assert!(document.duplicate_handles.is_empty());
    }

    #[test]
    fn a_repeated_handle_marks_duplicate_handles_and_excludes_collision() {
        let src = source(&named(&["2A", "2A", "2B"]), "100000");
        let document = Document::open(src, 1).expect("開く");
        assert!(!document.duplicate_handles.is_empty());
        assert_eq!(document.row_of(0x2A), None);
        assert_eq!(document.row_of(0x2B), Some(2));

        let src = source(&named(&["2A", "FFFFF", "FFFFF"]), "100000");
        let document = Document::open(src, 1).expect("開く");
        assert!(!document.duplicate_handles.is_empty());
        assert_eq!(document.row_of(0xFFFFF), None);
        assert_eq!(document.row_of(0x2A), Some(0));
    }

    #[test]
    fn a_seed_not_above_the_largest_handle_does_not_lock_the_document() {
        for seed in ["2C", "2D"] {
            let src = source(&named(&["2A", "2B", "2C"]), seed);
            let document = Document::open(src, 1).expect("開く");
            assert!(document.duplicate_handles.is_empty());
            assert_eq!(document.live_max_handle(), Some(0x2C));
        }
    }

    #[test]
    fn opening_does_not_depend_on_the_number_of_parts() {
        let handles: Vec<String> = (0x100..0x100 + 3000)
            .map(|handle| format!("{handle:X}"))
            .collect();
        let src = source(&handles, "FFFF");
        let whole = Document::open(src.clone(), 1).expect("開く");
        assert_eq!(whole.rows.len(), 3000);
        for parts in [2, 3, 7, 16] {
            let scanned = scan_split(&src, parts).expect("読む");
            assert!(scanned.parts.len() > 1, "{parts} 個に分かれない");
            let mut split = Document::assemble(src.clone(), scanned).expect("まとめる");
            split.build_display();
            split.build_index();
            assert!(whole.same_content(&split), "{parts}");
            assert!(whole.display == split.display, "{parts}");
            assert_eq!(whole.boxes, split.boxes, "{parts}");
            assert!(whole.grid == split.grid, "{parts}");
            assert_eq!(whole.counts, split.counts, "{parts}");
        }
    }

    #[test]
    fn compressed_source_scanning_matches_slice_opening() {
        let handles: Vec<String> = (0x100..0x100 + 3000)
            .map(|handle| format!("{handle:X}"))
            .collect();
        let src = source(&handles, "FFFF");
        let whole = Document::open(src.clone(), 1).expect("開く");
        for threads in [1, 4] {
            let comp = SourceBytes::compress_slice(&src, threads);
            let scanned = scan_source(comp, threads).expect("圧縮元を読む");
            let mut from_comp = Document::assemble_scanned(scanned).expect("まとめる");
            from_comp.build_display();
            from_comp.build_index();
            assert!(whole.same_content(&from_comp), "{threads}");
            assert!(whole.display == from_comp.display, "{threads}");
            assert_eq!(whole.boxes, from_comp.boxes, "{threads}");
            assert!(whole.grid == from_comp.grid, "{parts}", parts = threads);
            assert_eq!(whole.counts, from_comp.counts, "{threads}");
            assert_eq!(from_comp.save(), src);
        }
    }
}

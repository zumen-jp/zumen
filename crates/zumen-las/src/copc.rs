//! COPC 1.0（Cloud Optimized Point Cloud、Hobu, Inc. が <https://copc.io/> で公開）の LAZ を、要る八分木の節の範囲だけで
//! 読む（#473、決定 0232）。
//!
//! COPC は LAZ 1.4 の点の形式 6〜8 で、最初の VLR（利用者の ID `copc`、レコードの ID 1）に根の立方体・点の間隔・階層の
//! 位置を持つ。階層（EVLR の `copc` の 1000）は 32 バイトの項の頁の並びで、項は節の番号（段・X・Y・Z）・位置・長さ・
//! 点の数を持つ。点の数が -1 の項は子の頁を、正の項は節の点の LAZ のチャンクを指す。
//!
//! 読むのは、初めの 589 バイト（ヘッダー・情報の VLR）、VLR の範囲、要求の範囲と許容差に掛かる節の頁とチャンクだけで、
//! 足りない範囲は [`Missing::Need`] で求める。節は、そのまま点群の領域モデルの八分木の節にする。COPC の X は東、Y は北
//! なので、領域モデルの番号（北・東・標高）では X と Y を入れ替える。

use std::collections::BTreeSet;

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_pointcloud::{Key, MAX_LEVEL, Node, Octree, PointCloud, Query};
use zumen_sdk::ranged::{Missing, Source};
use zumen_sdk::{Cx, Error};

use crate::{HEADER_1_4, Header, LasFile, VLR_HEADER, at, f64_at, header, records, u16_at, u64_at};

/// 情報と階層のレコードの利用者の ID
const USER: &[u8] = b"copc";
/// 情報の VLR のレコードの ID
const INFO_RECORD: u16 = 1;
/// 情報の VLR の中身の長さ
const INFO_SIZE: u64 = 160;
/// 階層の頁の項の長さ
const ENTRY_SIZE: u64 = 32;
/// 初めの範囲（ヘッダー・情報の VLR の見出しと中身）。ここまでで COPC と見分け、情報を読む
pub const PREFIX: u64 = HEADER_1_4 as u64 + VLR_HEADER + INFO_SIZE;

/// 情報の VLR
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    /// 根の立方体の中心（X・Y・Z）
    pub center: [f64; 3],
    /// 根の立方体の辺の半分
    pub halfsize: f64,
    /// 根の段の点の間隔。段ごとに半分になる
    pub spacing: f64,
    /// 根の頁の位置
    pub root_offset: u64,
    /// 根の頁の長さ
    pub root_size: u64,
    /// GPS 時刻の最小と最大
    pub gps_time: [f64; 2],
}

/// 読んだ節
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadNode {
    /// 領域モデルの八分木の番号（北・東・標高）
    pub key: Key,
    /// 点の LAZ のチャンクの範囲
    pub chunk: ByteRange,
    /// 点の数
    pub points: u64,
}

/// 範囲だけを読んだ COPC
#[derive(Clone, Debug, PartialEq)]
pub struct Copc {
    /// ヘッダー。点の数は、読んだ節の点の数の和
    pub header: Header,
    /// 情報の VLR
    pub info: Info,
    /// 読んだ節の点。八分木の節は読んだ節だけを持つ
    pub cloud: PointCloud,
    /// 読んだ節。番号の順
    pub nodes: Vec<ReadNode>,
    /// 要求の上限のために読まなかった節がある
    pub truncated: bool,
}

/// 入力の初めが COPC か。初めの [`PREFIX`] バイトを渡す
pub fn is_copc(prefix: &[u8]) -> bool {
    let first = usize::from(HEADER_1_4);
    prefix.get(..4) == Some(b"LASF")
        && prefix.get(24..26) == Some(&[1, 4])
        && prefix.get(first + 2..first + 18).is_some_and(|user| {
            user.strip_prefix(USER)
                .is_some_and(|rest| rest.iter().all(|b| *b == 0))
        })
        && u16_at(prefix, first + 18) == Some(INFO_RECORD)
}

fn invalid(cx: &mut Cx<'_>, reason: &'static str, at: Option<(u64, u64)>) -> Missing {
    let mut diagnostic = Diagnostic::new(codes::LAS_COPC_INVALID).with("reason", reason);
    if let Some((start, end)) = at {
        diagnostic = diagnostic.at(Ref::at_bytes(start, end));
    }
    cx.report(diagnostic);
    Error::Invalid.into()
}

/// 情報の VLR の中身を読む
fn info(bytes: &[u8]) -> Option<Info> {
    let f = |offset| f64_at(bytes, offset);
    let info = Info {
        center: [f(0)?, f(8)?, f(16)?],
        halfsize: f(24)?,
        spacing: f(32)?,
        root_offset: u64_at(bytes, 40)?,
        root_size: u64_at(bytes, 48)?,
        gps_time: [f(56)?, f(64)?],
    };
    let finite = info.center.iter().all(|v| v.is_finite());
    let positive = |v: f64| v.is_finite() && v > 0.0;
    (finite
        && positive(info.halfsize)
        && positive(info.spacing)
        && info.root_size > 0
        && info.root_size.is_multiple_of(ENTRY_SIZE))
    .then_some(info)
}

/// 範囲 `[start, start + size)`。足し算があふれるか、入力の外なら `None`
fn span(start: u64, size: u64, total: u64) -> Option<(u64, u64)> {
    let end = start.checked_add(size)?;
    (end <= total).then_some((start, end))
}

/// 根の立方体の八分木。点の間隔の段は、根の間隔を辺の長さで割った数を分ける数にした近似
fn frame(info: &Info) -> Octree {
    let size = info.halfsize * 2.0;
    // 正の有限の値を四捨五入する。`as` は範囲の外を端の値にする
    let span = ((size / info.spacing + 0.5) as u32).max(1);
    Octree {
        origin: [
            info.center[1] - info.halfsize,
            info.center[0] - info.halfsize,
            info.center[2] - info.halfsize,
        ],
        size,
        span,
        nodes: Vec::new(),
    }
}

/// 頁の項。節の番号が範囲の外なら `None`
fn entry(bytes: &[u8]) -> Option<(Key, u64, i32, i32)> {
    let i = |offset| at::<4>(bytes, offset).map(i32::from_le_bytes);
    let (level, x, y, z) = (i(0)?, i(4)?, i(8)?, i(12)?);
    let level = u8::try_from(level).ok().filter(|l| *l <= MAX_LEVEL)?;
    let cells = 1i64 << level;
    let index = |v: i32| u32::try_from(v).ok().filter(|v| i64::from(*v) < cells);
    // COPC の X は東、Y は北。領域モデルの番号は北・東・標高
    let key = Key {
        level,
        x: index(y)?,
        y: index(x)?,
        z: index(z)?,
    };
    Some((key, u64_at(bytes, 16)?, i(24)?, i(28)?))
}

/// 要求の範囲と許容差に掛かる節か。`zumen_pointcloud::select` と同じ条件
fn wanted(tree: &Octree, key: Key, query: &Query) -> bool {
    let fine_enough = query
        .spacing
        .is_none_or(|spacing| key.level == 0 || tree.spacing(key.level - 1) > spacing);
    let overlaps = query.bounds.is_none_or(|[low, high]| {
        let [a, b] = tree.cube(key);
        (0..3).all(|i| a.get(i) <= high.get(i) && low.get(i) <= b.get(i))
    });
    fine_enough && overlaps
}

/// 節の立方体が要求の範囲にすべて入るか。範囲がなければ入る
fn contained(tree: &Octree, key: Key, query: &Query) -> bool {
    query.bounds.is_none_or(|[low, high]| {
        let [a, b] = tree.cube(key);
        (0..3).all(|i| low.get(i) <= a.get(i) && b.get(i) <= high.get(i))
    })
}

/// 要求 `query` の範囲（北・東・標高）・許容差・上限に掛かる節だけを読む。足りない範囲は [`Missing::Need`] で求める
///
/// 上限は、範囲にすべて入る節の点の数の和が上限に届いたところで、それより後の節を読まない。範囲に一部だけ掛かる節は、
/// どれだけの点が範囲に入るかを展開する前に知れないので、上限に数えない。
pub fn read(src: &Source<'_>, query: &Query, cx: &mut Cx<'_>) -> Result<Copc, Missing> {
    let head = src
        .get(0, PREFIX)
        .map_err(|range| Missing::Need(vec![range]))?;
    let Some(mut header) = header(head) else {
        cx.report(Diagnostic::new(codes::LAS_FILE_INVALID));
        return Err(Error::Invalid.into());
    };
    if !is_copc(head) {
        return Err(invalid(cx, "info-missing", None));
    }
    if !header.compressed || !(6..=8).contains(&header.point_format) {
        return Err(invalid(cx, "point-format", None));
    }
    let info_at = u64::from(HEADER_1_4) + VLR_HEADER;
    let Some(info) = head
        .get(usize::from(HEADER_1_4) + VLR_HEADER as usize..)
        .and_then(info)
    else {
        return Err(invalid(cx, "info", Some((info_at, PREFIX))));
    };
    let point_start = u64::from(header.point_offset);
    // 点の初めの 8 バイトは LAZ のチャンクの表の位置。チャンクはその後ろから、EVLR の前まで
    let chunks_start = point_start.saturating_add(8);
    let chunks_end = if header.evlr_count > 0 && header.evlr_start >= chunks_start {
        header.evlr_start.min(src.size)
    } else {
        src.size
    };
    let Some(root) = span(info.root_offset, info.root_size, src.size) else {
        return Err(invalid(cx, "hierarchy-range", Some((info_at, PREFIX))));
    };
    let vlr_range = (u64::from(header.header_size), point_start);
    if vlr_range.1 > src.size {
        cx.report(Diagnostic::new(codes::LAS_FILE_INVALID));
        return Err(Error::Invalid.into());
    }
    let mut needs = Vec::new();
    let vlr_bytes = src
        .get(vlr_range.0, vlr_range.1)
        .map_err(|r| needs.push(r))
        .ok();

    // 階層の頁をたどる。同じ頁を二度読まない
    let mut tree = frame(&info);
    let mut seen = BTreeSet::from([root.0]);
    let mut pages = vec![root];
    let mut found: Vec<(Key, (u64, u64), u64)> = Vec::new();
    let mut keys = BTreeSet::new();
    let mut next = 0;
    while let Some(&(start, end)) = pages.get(next) {
        next += 1;
        let page = match src.get(start, end) {
            Ok(page) => page,
            Err(range) => {
                needs.push(range);
                continue;
            }
        };
        cx.check(Kind::Records, (end - start) / ENTRY_SIZE)?;
        for (index, item) in page
            .as_chunks::<{ ENTRY_SIZE as usize }>()
            .0
            .iter()
            .enumerate()
        {
            let item_at = start + index as u64 * ENTRY_SIZE;
            let item_span = Some((item_at, item_at + ENTRY_SIZE));
            let Some((key, offset, size, count)) = entry(item) else {
                return Err(invalid(cx, "hierarchy-key", item_span));
            };
            if !wanted(&tree, key, query) {
                continue;
            }
            let size = u64::try_from(size).ok();
            match count {
                -1 => {
                    let Some(child) = size
                        .filter(|s| *s > 0 && s.is_multiple_of(ENTRY_SIZE))
                        .and_then(|s| span(offset, s, src.size))
                    else {
                        return Err(invalid(cx, "hierarchy-range", item_span));
                    };
                    if seen.insert(child.0) {
                        pages.push(child);
                    }
                }
                0 => {}
                count if count > 0 => {
                    let Some(chunk) = size
                        .filter(|s| *s > 0)
                        .and_then(|s| span(offset, s, chunks_end))
                        .filter(|(s, _)| *s >= chunks_start)
                    else {
                        return Err(invalid(cx, "chunk-range", item_span));
                    };
                    if !keys.insert(key) {
                        return Err(invalid(cx, "hierarchy-duplicate", item_span));
                    }
                    found.push((key, chunk, count as u64));
                }
                _ => return Err(invalid(cx, "hierarchy-count", item_span)),
            }
        }
    }
    if !needs.is_empty() {
        return Err(Missing::Need(needs));
    }

    // 浅い段の節から、上限に届くまで選ぶ
    found.sort_by_key(|(key, ..)| *key);
    let mut taken = Vec::new();
    let mut sure = 0u64;
    let mut truncated = false;
    for (key, chunk, count) in found {
        if sure >= query.limit {
            truncated = true;
            break;
        }
        if contained(&tree, key, query) {
            sure = sure.saturating_add(count);
        }
        taken.push((key, chunk, count));
    }
    for (_, (start, end), _) in &taken {
        if let Err(range) = src.get(*start, *end) {
            needs.push(range);
        }
    }
    if !needs.is_empty() {
        return Err(Missing::Need(needs));
    }

    let vlr_bytes = vlr_bytes.unwrap_or_default();
    let read_bytes = taken
        .iter()
        .map(|(_, (s, e), _)| e - s)
        .chain(pages.iter().map(|(s, e)| e - s))
        .fold(PREFIX + vlr_bytes.len() as u64, u64::saturating_add);
    cx.check(Kind::InputBytes, read_bytes)?;
    let vlrs = records(
        vlr_bytes,
        u64::from(header.header_size),
        header.vlr_count,
        false,
        cx,
    )?;
    let total = taken.iter().map(|(.., count)| *count).sum::<u64>();
    header.point_count = total;
    let mut points = Vec::new();
    let mut nodes = Vec::new();
    let mut read = Vec::new();
    let mut first = 0u64;
    for (key, (start, end), count) in taken {
        let chunk = src
            .get(start, end)
            .map_err(|range| Missing::Need(vec![range]))?;
        let Ok(n) = usize::try_from(count) else {
            return Err(invalid(cx, "chunk-range", Some((start, end))));
        };
        let decoded = crate::laz::decompress_chunk(&header, &vlrs, chunk, n, cx)?;
        points.extend_from_slice(&decoded);
        let last = first + count;
        let (Ok(from), Ok(to)) = (u32::try_from(first), u32::try_from(last)) else {
            return Err(invalid(cx, "too-large", None));
        };
        nodes.push(Node {
            key,
            points: (from..to).collect(),
        });
        read.push(ReadNode {
            key,
            chunk: ByteRange { start, end },
            points: count,
        });
        first = last;
    }
    let file = LasFile {
        header: header.clone(),
        vlrs,
        points,
    };
    let columns = crate::lift::parts(&file, cx)?;
    let (crs, vertical, unsupported_vertical_epsg) = crate::lift::crs(&file, cx);
    if columns.len() as u64 != total {
        return Err(invalid(cx, "chunk-range", None));
    }
    tree.nodes = nodes;
    Ok(Copc {
        header,
        info,
        cloud: PointCloud {
            crs,
            vertical,
            unsupported_vertical_epsg,
            points: columns,
            octree: tree,
            source: Ref::at_bytes(point_start, chunks_end),
        },
        nodes: read,
        truncated,
    })
}

//! COPC の階層を八分木の節にし、条件に掛かる節の範囲だけを読むことを確かめる（#473）。
//!
//! 入力 `COPC` は `copc_make.py` が lazrs 0.9.0 で作った、点の形式 6 の 10 点の合成の COPC。根の立方体は
//! 中心 (1000, 2000, 50)・辺の半分 8、根の間隔は 2。根の頁に根と (1,0,0,0) の節と、子の頁を指す
//! (1,1,1,0) の項を置き、子の頁に (1,1,1,0) と (2,3,2,0) の節を置く。座標系は WKT の EPSG:6677。

#![cfg(test)]

use zumen_base::{Budget, ByteRange, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_crs::{Crs, Zone};
use zumen_las::Las;
use zumen_las::copc::{self, Copc, PREFIX};
use zumen_pointcloud::{Key, PointCloud, Query};
use zumen_sdk::ranged::{Missing, Source};
use zumen_sdk::{Codec, Error, Lift};

const COPC: &str = concat!(
    "4c415346000011000000000000000000000000000000000001047a756d656e20636f7063207465737400000000000000",
    "00000000000000000000636f70635f6d616b652e707900000000000000000000000000000000000000000100ea077701",
    "5703000003000000861e000000000000000000000000000000000000000000000000007b14ae47e17a843f7b14ae47e1",
    "7a843f7b14ae47e17a843f0000000000408f400000000000409f4000000000000000000000000000788f400000000000",
    "088f4000000000005c9f400000000000249f400000000000804c40000000000080454000000000000000001405000000",
    "000000010000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000636f7063000000",
    "0000000000000000000100a0000000000000000000000000000000000000000000000000000000000000000000000000",
    "0000408f400000000000409f400000000000004940000000000000204000000000000000405005000000000000600000",
    "000000000000000000000059400000000000405b40000000000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000006c61737a697020656e636f6465640000bc56280000000000000000000000000000",
    "00000000000000000000000000000000000000030000000202000000000000ffffffffffffffffffffffffffffffffff",
    "ffffff01000a001e00030000004c4153465f50726f6a656374696f6e0040087600000000000000000000000000000000",
    "000000000000000000000000000000000050524f4a43535b224a474432303131202f204a6170616e20506c616e652052",
    "656374616e67756c6172204353204958222c47454f4743535b224a474432303131222c415554484f524954595b224550",
    "5347222c2236363638225d5d2c415554484f524954595b2245505347222c2236363737225d5d00000500000000000044",
    "fdffff44fdffffcc100000010011000200000001000000000000005940030000000d0000000700000000000000000000",
    "00050000000000000000000000000000001100000020b50495973ccdcee5fb0000005aa07139af000007c00000004a77",
    "9bdee400020bff271e00000100000025feffffa2feffff30110000040011000200000001000000000000c05940020000",
    "0008000000050000000000000000000000040000000000000000000000000000000a0000002096c1b3680000004d5f00",
    "0000080000004a779bdee40001000000640000006400000094110000060011000200000001000000000000405a400300",
    "00000c0000000700000000000000000000000500000000000000000000000000000011000000208b04373592357a6600",
    "00004b1a340400000007c00000004a779bdee400020bff271e000001000000f401000096000000fe1000000900110002",
    "00000001000000000000005b400200000008000000050000000000000000000000040000000000000000000000000000",
    "000a00000020867a18520000004292000000080000004a779bdee40001000000000000000400000013d8f4f7165e8606",
    "9c0000000000636f7063000000000000000000000000e803a00000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000000000000000000000000000000000005f030000000000007000000003000000",
    "01000000000000000000000000000000cf03000000000000610000000200000001000000010000000100000000000000",
    "b00500000000000040000000ffffffff0100000001000000010000000000000030040000000000006f00000003000000",
    "020000000300000002000000000000009f040000000000006100000002000000",
);

/// COPC の情報の VLR の中身の始まり
const INFO: usize = 375 + 54;
/// 階層の項の大きさ
const ENTRY: usize = 32;

fn input() -> Vec<u8> {
    COPC.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

/// 点の始まり（ヘッダーの offset to point data）
fn point_offset(bytes: &[u8]) -> u64 {
    u64::from(u32::from_le_bytes(bytes[96..100].try_into().unwrap()))
}

/// 根の頁の始まり（COPC の情報の root_hier_offset）
fn pages_at(bytes: &[u8]) -> usize {
    usize::try_from(u64_at(bytes, INFO + 40)).unwrap()
}

const ALL: Query = Query {
    bounds: None,
    spacing: None,
    limit: u64::MAX,
};

/// 求められた範囲だけを渡して読む。読めるまで、求められた範囲を足して繰り返す。求められた範囲を順に返す
fn read_by_ranges(
    bytes: &[u8],
    query: &Query,
    diagnostics: &mut Diagnostics,
) -> (Result<Copc, Error>, Vec<Vec<ByteRange>>) {
    let mut given: Vec<ByteRange> = Vec::new();
    let mut asked = Vec::new();
    for _ in 0..10 {
        given.sort_by_key(|r| (r.start, r.end));
        let parts: Vec<(u64, &[u8])> = given
            .iter()
            .map(|r| (r.start, &bytes[r.start as usize..r.end as usize]))
            .collect();
        let src = Source {
            size: bytes.len() as u64,
            parts: &parts,
        };
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, diagnostics, &cancel);
        match copc::read(&src, query, &mut cx) {
            Ok(copc) => return (Ok(copc), asked),
            Err(Missing::Need(needs)) => {
                assert!(!needs.is_empty());
                given.extend(needs.iter().copied());
                asked.push(needs);
            }
            Err(Missing::Failed(error)) => return (Err(error), asked),
        }
    }
    panic!("範囲の要求が終わらない: {asked:?}");
}

/// 全体を渡して読む
fn whole(bytes: &[u8], query: &Query, budget: &mut Budget) -> (Result<Copc, Missing>, Diagnostics) {
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let parts = [(0u64, bytes)];
    let src = Source {
        size: bytes.len() as u64,
        parts: &parts,
    };
    let result = {
        let mut cx = Cx::new(budget, &mut diagnostics, &cancel);
        copc::read(&src, query, &mut cx)
    };
    (result, diagnostics)
}

fn key(level: u8, x: u32, y: u32, z: u32) -> Key {
    Key { level, x, y, z }
}

/// 点（東・北・標高）
fn coordinates(cloud: &PointCloud) -> Vec<(f64, f64, f64)> {
    let p = &cloud.points;
    p.easting
        .iter()
        .zip(&p.northing)
        .zip(&p.height)
        .map(|((&e, &n), &h)| (e, n, h))
        .collect()
}

/// `copc_make.py` が書いた点。節の順
const EXPECTED: [(f64, f64, f64); 10] = [
    (993.0, 1993.0, 43.0),
    (1007.0, 2007.0, 57.0),
    (1000.5, 2000.5, 50.0),
    (995.25, 1996.5, 44.0),
    (999.0, 1999.0, 49.0),
    (1001.0, 2001.0, 45.0),
    (1003.5, 2002.25, 48.5),
    (1002.0, 2003.0, 46.0),
    (1005.0, 2001.5, 43.5),
    (1006.75, 2003.5, 45.0),
];

#[test]
fn reads_hierarchy_as_octree_nodes() {
    let bytes = input();
    assert!(copc::is_copc(&bytes[..PREFIX as usize]));
    let mut budget = Budget::default();
    let (copc, diagnostics) = whole(&bytes, &ALL, &mut budget);
    let copc = copc.unwrap_or_else(|_| panic!("{diagnostics:?}"));
    assert!(!diagnostics.has_errors(), "{diagnostics:?}");
    assert_eq!(copc.info.center, [1000.0, 2000.0, 50.0]);
    assert_eq!(copc.info.halfsize, 8.0);
    assert_eq!(copc.info.spacing, 2.0);
    assert_eq!(copc.header.point_count, 10);
    assert!(!copc.truncated);
    let cloud = &copc.cloud;
    assert_eq!(cloud.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    assert_eq!(coordinates(cloud), EXPECTED);
    // COPC の X（東）と Y（北）を、領域モデルの番号では北・東の順にする。COPC の (2,3,2,0) は北 2・東 3
    let tree = &cloud.octree;
    assert_eq!(tree.origin, [1992.0, 992.0, 42.0]);
    assert_eq!(tree.size, 16.0);
    assert_eq!(tree.spacing(0), 2.0);
    let nodes: Vec<(Key, Vec<u32>)> = tree
        .nodes
        .iter()
        .map(|n| (n.key, n.points.clone()))
        .collect();
    assert_eq!(
        nodes,
        vec![
            (key(0, 0, 0, 0), vec![0, 1, 2]),
            (key(1, 0, 0, 0), vec![3, 4]),
            (key(1, 1, 1, 0), vec![5, 6, 7]),
            (key(2, 2, 3, 0), vec![8, 9]),
        ]
    );
    // 節の点は節の立方体に入る
    for node in &tree.nodes {
        let [low, high] = tree.cube(node.key);
        for &i in &node.points {
            let (e, n, h) = EXPECTED[i as usize];
            let p = [n, e, h];
            assert!(
                (0..3).all(|a| low[a] <= p[a] && p[a] <= high[a]),
                "{:?} {p:?}",
                node.key
            );
        }
    }
    // 全体を LAZ として読んだ点（ファイルの順）と同じ値
    let mut budget = Budget::default();
    let mut d = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut d, &cancel);
    let file = Las::read(&bytes, &mut cx).unwrap();
    let laz: PointCloud = file.lift(&mut cx).unwrap();
    assert_eq!(coordinates(&laz), EXPECTED);
    assert_eq!(laz.points.time, cloud.points.time);
    assert_eq!(laz.points.intensity, cloud.points.intensity);
    assert_eq!(laz.crs, cloud.crs);
}

#[test]
fn asks_only_for_ranges_of_wanted_nodes() {
    let bytes = input();
    let size = bytes.len() as u64;
    let start = point_offset(&bytes);
    let root_at = pages_at(&bytes) as u64;
    let root = ByteRange {
        start: root_at,
        end: root_at + 96,
    };
    let child = ByteRange {
        start: root_at + 96,
        end: root_at + 160,
    };
    let first = [
        vec![ByteRange {
            start: 0,
            end: PREFIX,
        }],
        vec![
            ByteRange {
                start: 375,
                end: start,
            },
            root,
        ],
    ];

    // 全体: 子の頁を読んでから、4 つの節のチャンクを求める
    let mut d = Diagnostics::new();
    let (copc, asked) = read_by_ranges(&bytes, &ALL, &mut d);
    let copc = copc.unwrap();
    assert_eq!(asked[..2], first);
    assert_eq!(asked[2], vec![child]);
    let chunks: Vec<ByteRange> = copc.nodes.iter().map(|n| n.chunk).collect();
    assert_eq!(asked[3], chunks);
    assert_eq!(asked.len(), 4);
    assert_eq!(coordinates(&copc.cloud), EXPECTED);
    assert!(chunks.iter().all(|c| c.start >= start + 8 && c.end <= size));

    // 南西の箱（北・東・標高）: 子の頁の (1,1,1,0) は箱の外なので、子の頁も読まない
    let south_west = Query {
        bounds: Some([[1992.0, 992.0, 42.0], [1999.5, 999.5, 58.0]]),
        spacing: None,
        limit: u64::MAX,
    };
    let (copc, asked) = read_by_ranges(&bytes, &south_west, &mut d);
    let copc = copc.unwrap();
    assert_eq!(asked[..2], first);
    assert_eq!(asked[2], chunks[..2]);
    assert_eq!(asked.len(), 3);
    let keys: Vec<Key> = copc.nodes.iter().map(|n| n.key).collect();
    assert_eq!(keys, vec![key(0, 0, 0, 0), key(1, 0, 0, 0)]);
    assert_eq!(coordinates(&copc.cloud), EXPECTED[..5]);

    // 許容差 1.5: 深さ 2 の節（間隔 0.5）は読まない。深さ 1 の (1,1,1,0) は子の頁にあるので、子の頁は読む
    let coarse = Query {
        bounds: None,
        spacing: Some(1.5),
        limit: u64::MAX,
    };
    let (copc, asked) = read_by_ranges(&bytes, &coarse, &mut d);
    let copc = copc.unwrap();
    assert_eq!(asked[2], vec![child]);
    assert_eq!(asked[3], chunks[..3]);
    assert_eq!(coordinates(&copc.cloud), EXPECTED[..8]);

    // 上限 3: 根の節の 3 点で届くので、ほかの節のチャンクを読まない
    let few = Query {
        bounds: None,
        spacing: None,
        limit: 3,
    };
    let (copc, asked) = read_by_ranges(&bytes, &few, &mut d);
    let copc = copc.unwrap();
    assert_eq!(asked.last().unwrap(), &chunks[..1]);
    assert!(copc.truncated);
    assert_eq!(coordinates(&copc.cloud), EXPECTED[..3]);
    assert!(!d.has_errors(), "{d:?}");
}

/// 壊れた入力で `las.copc.invalid` の理由を返す
fn reason(bytes: &[u8]) -> Option<String> {
    let mut budget = Budget::default();
    let (result, diagnostics) = whole(bytes, &ALL, &mut budget);
    assert!(
        matches!(result, Err(Missing::Failed(Error::Invalid))),
        "{result:?}"
    );
    let found = diagnostics
        .iter()
        .find(|d| d.code() == codes::LAS_COPC_INVALID)?;
    Some(format!("{:?}", found.param("reason")?))
}

fn has_reason(bytes: &[u8], expected: &str) {
    let found = reason(bytes).unwrap_or_default();
    assert!(found.contains(expected), "{found} に {expected} がない");
}

#[test]
fn rejects_broken_hierarchy() {
    let bytes = input();
    let pages = pages_at(&bytes);
    let len = bytes.len() as u64;
    let set = |at: usize, value: &[u8]| {
        let mut b = bytes.clone();
        b[at..at + value.len()].copy_from_slice(value);
        b
    };

    // 根の頁の位置が入力の外
    has_reason(&set(INFO + 40, &len.to_le_bytes()), "hierarchy-range");
    // チャンクの範囲が入力の外
    has_reason(&set(pages + 16, &(len - 4).to_le_bytes()), "chunk-range");
    // チャンクが点の初めの 8 バイト（チャンクの表の位置）に掛かる
    has_reason(
        &set(pages + 16, &point_offset(&bytes).to_le_bytes()),
        "chunk-range",
    );
    // 深さ 1 に X = 2 の節はない
    has_reason(
        &set(pages + ENTRY + 4, &2i32.to_le_bytes()),
        "hierarchy-key",
    );
    // 負の段
    has_reason(&set(pages, &(-1i32).to_le_bytes()), "hierarchy-key");
    // 点の数が -1 より小さい
    has_reason(&set(pages + 28, &(-2i32).to_le_bytes()), "hierarchy-count");
    // 子の頁に、根と同じ節を置く
    has_reason(
        &set(pages + 3 * ENTRY, &bytes[pages..pages + ENTRY]),
        "hierarchy-duplicate",
    );
    // 情報の辺の半分が 0
    has_reason(&set(INFO + 24, &0f64.to_le_bytes()), "info");
    // 点の形式が 6〜8 でない
    has_reason(&set(104, &[0x80 | 1]), "point-format");
    // 最初の VLR が COPC の情報でない
    has_reason(&set(375 + 2, b"x"), "info-missing");
}

#[test]
fn follows_each_page_once() {
    // 子の頁を指す項が根の頁を指しても、同じ頁を二度読まずに終わる
    let mut b = input();
    let pages = pages_at(&b);
    let pointer = pages + 2 * ENTRY;
    b[pointer + 16..pointer + 24].copy_from_slice(&(pages as u64).to_le_bytes());
    b[pointer + 24..pointer + 28].copy_from_slice(&96i32.to_le_bytes());
    let mut budget = Budget::default();
    let (copc, diagnostics) = whole(&b, &ALL, &mut budget);
    let copc = copc.unwrap_or_else(|_| panic!("{diagnostics:?}"));
    let keys: Vec<Key> = copc.nodes.iter().map(|n| n.key).collect();
    assert_eq!(keys, vec![key(0, 0, 0, 0), key(1, 0, 0, 0)]);
}

#[test]
fn stops_on_budget() {
    let bytes = input();
    for kind in [
        Kind::Records,
        Kind::InputBytes,
        Kind::DecodedBytes,
        Kind::Work,
    ] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(4)));
        let (result, _) = whole(&bytes, &ALL, &mut budget);
        let Err(Missing::Failed(Error::Stopped(stop))) = result else {
            panic!("{kind:?} で止まらない: {result:?}")
        };
        assert_eq!(stop.to_diagnostic().code(), kind.exceeded_code());
    }
}

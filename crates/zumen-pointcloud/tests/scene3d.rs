//! 点群の 3D シーンを、同じ Query の select と元の列に照らす（#475）。
#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "合成した正常な入力の試験"
)]
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref};
use zumen_pointcloud::{Octree, PointCloud, Points, Query, scene3d, select};
use zumen_scene3d::{Error, encode, encode_with_points, layout as l, read};

fn with_cx<T>(limits: Limits, operation: impl FnOnce(&mut Cx) -> T) -> T {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    operation(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
}

/// 同じ座標を重ねて深い段の節を作る。大きな原点で f32 の丸めも起こす
fn cloud(attributes: bool) -> PointCloud {
    let mut northing = Vec::new();
    let mut easting = Vec::new();
    let mut height = Vec::new();
    for i in 0..40_u32 {
        let k = f64::from(i % 10);
        northing.push(-120_000.25 + k * 0.5);
        easting.push(30_000.125 + k * 0.75);
        height.push(12.5 + f64::from(i % 2));
    }
    let n = northing.len() as u16;
    let points = Points {
        northing,
        easting,
        height,
        intensity: attributes.then(|| (0..n).map(|i| i * 1000 + 7).collect()),
        classification: attributes.then(|| (0..n).map(|i| (i % 7) as u8).collect()),
        color: attributes.then(|| (0..n).map(|i| [i * 1500, 65535 - i, 256 + i]).collect()),
        returns: None,
        time: None,
    };
    let octree = with_cx(Limits::default(), |cx| Octree::build(&points, cx).unwrap());
    PointCloud {
        points,
        octree,
        source: Ref::default(),
        ..PointCloud::default()
    }
}

fn all() -> Query {
    Query {
        bounds: None,
        spacing: None,
        limit: u64::MAX,
    }
}

/// シーンの点を ZMS3 に書いて読み、元の座標と属性へ戻して select の順と比べる
fn check(cloud: &PointCloud, query: &Query) -> scene3d::Scene {
    let scene = with_cx(Limits::default(), |cx| {
        scene3d::build(cloud, query, cx).unwrap()
    });
    let selection = with_cx(Limits::default(), |cx| select(cloud, query, cx).unwrap());
    assert_eq!(scene.points.positions.len(), selection.points.len());
    assert_eq!(scene.truncated, selection.truncated);
    assert_eq!(scene.level, selection.level);
    let bytes = with_cx(Limits::default(), |cx| {
        encode_with_points(&scene.data, &scene.points, cx).unwrap()
    });
    with_cx(Limits::default(), |cx| {
        let reader = read(&bytes, cx).unwrap();
        assert_eq!(reader.minor, l::MINOR);
        assert_eq!(reader.sections as usize, l::SECTIONS.len());
        let p = &cloud.points;
        let mut row = 0_usize;
        let mut keys = Vec::new();
        for chunk in 0..u64::from(reader.chunks) {
            let points = reader.points(chunk).unwrap();
            keys.push(points.node.unwrap());
            let origin = scene.data.chunks[chunk as usize].origin;
            for i in 0..points.positions.rows() {
                let index = selection.points[row] as usize;
                let source = [p.easting[index], p.northing[index], p.height[index]];
                for (axis, expected) in source.iter().enumerate() {
                    let local = points.positions.f32(i * 3 + axis as u64).unwrap();
                    let back = origin[axis] + f64::from(local);
                    assert!((back - expected).abs() < 1e-3, "{back} {expected}");
                }
                match &p.color {
                    Some(color) => {
                        let [r, g, b] = color[index];
                        let got: Vec<u16> = (0..4)
                            .map(|k| points.rgba.u16(i * 4 + k).unwrap())
                            .collect();
                        assert_eq!(got, [r, g, b, u16::MAX]);
                    }
                    None => assert_eq!(points.rgba.rows(), 0),
                }
                match &p.intensity {
                    Some(v) => assert_eq!(points.intensity.u16(i), Some(v[index])),
                    None => assert_eq!(points.intensity.rows(), 0),
                }
                match &p.classification {
                    Some(v) => assert_eq!(
                        points.classification.bytes().get(i as usize).copied(),
                        Some(v[index])
                    ),
                    None => assert_eq!(points.classification.rows(), 0),
                }
                row += 1;
            }
        }
        assert_eq!(row, selection.points.len());
        let mut sorted = keys.clone();
        sorted.sort_unstable_by_key(|&[level, x, y, z]| (level, x, y, z));
        sorted.dedup();
        assert_eq!(sorted, keys, "節は元のキーの順に 1 回ずつ並ぶ");
    });
    scene
}

#[test]
fn every_point_keeps_order_position_and_attributes() {
    let scene = check(&cloud(true), &all());
    assert_eq!(scene.points.positions.len(), 40);
    assert!(!scene.truncated);
    assert!(scene.level > 0, "重ねた点は深い段の節に入る");
    assert!(scene.data.chunks.len() > 1);
    let [low, high] = scene.extent.unwrap();
    assert!((low[0] - 30_000.125).abs() < 1e-3);
    assert!((high[1] - (-120_000.25 + 4.5)).abs() < 1e-3);
    // 属性のない点群は空の列を返す
    check(&cloud(false), &all());
}

#[test]
fn bounds_spacing_and_limit_follow_select() {
    let cloud = cloud(true);
    // Query は北・東・高さ
    let bounds = Some([[-120_000.0, 30_000.0, 13.0], [-119_998.0, 30_010.0, 14.0]]);
    let spacing = Some(cloud.octree.spacing(0) * 0.99);
    for query in [
        Query { bounds, ..all() },
        Query { spacing, ..all() },
        Query { limit: 3, ..all() },
        Query { limit: 0, ..all() },
        Query {
            bounds,
            spacing,
            limit: 5,
        },
    ] {
        check(&cloud, &query);
    }
    let limited = check(&cloud, &Query { limit: 3, ..all() });
    assert!(limited.truncated);
    assert_eq!(limited.points.positions.len(), 3);
}

#[test]
fn mesh_only_scenes_keep_the_old_layout_and_points_are_deterministic() {
    let cloud = cloud(true);
    let scene = with_cx(Limits::default(), |cx| {
        scene3d::build(&cloud, &all(), cx).unwrap()
    });
    let old = with_cx(Limits::default(), |cx| encode(&scene.data, cx).unwrap());
    assert_eq!(&old[4..8], &[1, 0, 0, 0], "点がなければ 1.0 のまま");
    assert_eq!(u32::from_le_bytes(old[12..16].try_into().unwrap()), 20);
    let encode = || {
        with_cx(Limits::default(), |cx| {
            encode_with_points(&scene.data, &scene.points, cx).unwrap()
        })
    };
    assert_eq!(encode(), encode());
}

#[test]
fn broken_octree_references_are_rejected() {
    let mut duplicated = cloud(true);
    let first = duplicated.octree.nodes[0].points[0];
    duplicated
        .octree
        .nodes
        .last_mut()
        .unwrap()
        .points
        .push(first);
    let mut missing = cloud(true);
    missing.octree.nodes.last_mut().unwrap().points.pop();
    let mut short = cloud(true);
    short.points.intensity.as_mut().unwrap().pop();
    for broken in [duplicated, missing, short] {
        let result = with_cx(Limits::default(), |cx| scene3d::build(&broken, &all(), cx));
        assert!(matches!(result, Err(Error::Invalid(_))), "{result:?}");
    }
    let zero = Query {
        spacing: Some(0.0),
        ..all()
    };
    let result = with_cx(Limits::default(), |cx| {
        scene3d::build(&cloud(true), &zero, cx)
    });
    assert!(matches!(result, Err(Error::Invalid("selection"))));
}

#[test]
fn every_budget_either_completes_identically_or_stops_without_output() {
    let cloud = cloud(true);
    let full = with_cx(Limits::default(), |cx| {
        scene3d::build(&cloud, &all(), cx).unwrap()
    });
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Elements] {
        let mut completed = false;
        for limit in 0..20_000_u64 {
            let limits = Limits::default().with(kind, Limit::Finite(limit));
            match with_cx(limits, |cx| scene3d::build(&cloud, &all(), cx)) {
                Ok(scene) => {
                    assert_eq!(scene, full);
                    completed = true;
                    break;
                }
                Err(Error::Stopped(_)) => {}
                Err(other) => panic!("{other:?}"),
            }
        }
        assert!(completed, "{kind:?}");
    }
}

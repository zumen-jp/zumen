//! 最小の列から、型・参照・大きな原点・破損・予算停止を確かめる。
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_scene3d::{
    Chunk, Color, Data, Element, Error, Instance, Mesh, encode, layout as l, read,
};

fn with_cx<T>(limits: Limits, operation: impl FnOnce(&mut Cx) -> T) -> T {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    operation(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
}

fn scene() -> Data {
    let identity = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.];
    Data {
        meshes: vec![
            Mesh {
                vertices: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
                triangles: vec![[0, 1, 2]],
                edges: vec![[0, 1], [1, 2], [2, 0]],
            },
            Mesh {
                vertices: vec![[0., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 0., 0.]],
                triangles: vec![[0, 1, 2], [1, 2, 3]],
                edges: vec![[0, 1]],
            },
        ],
        instances: vec![
            Instance {
                mesh: 0,
                element: 0,
                color: 0,
                transform: identity,
            },
            Instance {
                mesh: 0,
                element: 0,
                color: 1,
                transform: [-2., 0., 0., 3., 0., 1., 0., 0., 0., 0., 0.5, 0.],
            },
            Instance {
                mesh: 1,
                element: 1,
                color: 1,
                transform: identity,
            },
        ],
        chunks: vec![
            Chunk {
                origin: [1e12, -1e12, 123.5],
                bounds: [0., 0., 0., 3., 1., 0.],
                instances: 0,
            },
            Chunk {
                origin: [0., 0., 0.],
                bounds: [0., 0., 0., 1., 1., 1.],
                instances: 2,
            },
        ],
        elements: vec![
            Element {
                id: "wall-id".into(),
                class: "IfcWall".into(),
                name: "壁".into(),
                storey: Some(2),
            },
            Element {
                id: "door-id".into(),
                class: "IfcDoor".into(),
                name: "扉".into(),
                storey: Some(2),
            },
            Element {
                id: "floor-id".into(),
                class: "IfcBuildingStorey".into(),
                name: "1階".into(),
                storey: None,
            },
        ],
        colors: vec![
            Color {
                rgba: [0.2, 0.5, 0.9, 0.6],
                unresolved: false,
            },
            Color {
                rgba: [0., 0., 0., 1.],
                unresolved: true,
            },
        ],
    }
}

#[expect(clippy::unwrap_used, reason = "合成した正常な入力だけを符号化する")]
fn bytes() -> Vec<u8> {
    with_cx(Limits::default(), |cx| encode(&scene(), cx).unwrap())
}

#[test]
fn columns_keep_mesh_sharing_transforms_origins_text_and_ids() {
    let bytes = bytes();
    assert_eq!(bytes, self::bytes());
    with_cx(Limits::default(), |cx| {
        let view = read(&bytes, cx).unwrap();
        assert_eq!(view.chunks, 2);
        assert_eq!(view.column(l::CHUNK_ORIGINS).unwrap().f64(0), Some(1e12));
        assert_eq!(view.column(l::CHUNK_ORIGINS).unwrap().f64(1), Some(-1e12));
        assert_eq!(view.element(0).unwrap().id, "wall-id");
        assert_eq!(view.element(0).unwrap().name, "壁");
        assert_eq!(view.element(0).unwrap().storey, Some(2));
        assert_eq!(view.element(2).unwrap().class, "IfcBuildingStorey");
        assert_eq!(view.element(2).unwrap().storey, None);
        assert_eq!(
            view.instance(0).unwrap().mesh,
            view.instance(1).unwrap().mesh
        );
        assert_eq!(view.instance(1).unwrap(), scene().instances[1]);
        assert_eq!(view.mesh(0).unwrap().vertices.rows(), 3);
        assert_eq!(view.mesh(1).unwrap().vertices.rows(), 4);
        assert_eq!(view.mesh(1).unwrap().triangles.rows(), 2);
        assert_eq!(view.mesh(1).unwrap().triangles.u32(5), Some(3));
        assert_eq!(view.column(l::COLOR_FLAGS).unwrap().bytes(), [0, 1]);
        assert_eq!(view.column(l::COLOR_RGBA).unwrap().f32(3), Some(0.6));
        assert!(view.mesh(2).is_err());
        assert!(view.instance(3).is_err());
        assert!(view.element(3).is_err());
        assert!(view.column(usize::MAX).is_none());
        assert!(view.text(u32::MAX, 1).is_err());
        assert!(view.string(l::VERTEX_XYZ, 0).is_err());
        assert!(view.column(l::VERTEX_XYZ).unwrap().f32(u64::MAX).is_none());
    });
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "合成した列の既知の位置を読む"
)]
fn get_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
#[expect(clippy::indexing_slicing, reason = "合成した列の既知の位置を変える")]
fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn start(bytes: &[u8], section: usize) -> usize {
    get_u32(bytes, 16 + section * 8) as usize
}

#[test]
fn malformed_headers_columns_references_and_nonfinite_values_are_rejected() {
    let original = bytes();
    for end in 0..original.len() {
        assert!(with_cx(Limits::default(), |cx| read(&original[..end], cx).is_err()));
    }
    type Mutation = Box<dyn Fn(&mut Vec<u8>)>;
    let mutations: Vec<Mutation> = vec![
        Box::new(|b| b[0] = b'!'),
        Box::new(|b| b[4] = 2),
        Box::new(|b| put_u32(b, 12, u32::MAX)),
        Box::new(|b| put_u32(b, 16, 0)),
        Box::new(|b| put_u32(b, 20, 1)),
        Box::new(|b| {
            let at = start(b, l::MESH_VERTICES);
            put_u32(b, at, 1);
        }),
        Box::new(|b| {
            let at = start(b, l::TRIANGLE_VERTICES);
            put_u32(b, at, 3);
        }),
        Box::new(|b| {
            let at = start(b, l::INSTANCE_MESHES);
            put_u32(b, at, 2);
        }),
        Box::new(|b| {
            let at = start(b, l::INSTANCE_ELEMENTS);
            put_u32(b, at, 3);
        }),
        Box::new(|b| {
            let at = start(b, l::INSTANCE_ELEMENTS) + 8;
            put_u32(b, at, 0);
        }),
        Box::new(|b| {
            let at = start(b, l::INSTANCE_COLORS);
            put_u32(b, at, 2);
        }),
        Box::new(|b| {
            let at = start(b, l::ELEMENT_STOREYS);
            put_u32(b, at, 3);
        }),
        Box::new(|b| {
            let at = start(b, l::VERTEX_XYZ);
            put_u32(b, at, f32::NAN.to_bits());
        }),
        Box::new(|b| {
            let at = start(b, l::COLOR_RGBA);
            put_u32(b, at, 1.5_f32.to_bits());
        }),
        Box::new(|b| {
            let at = start(b, l::COLOR_FLAGS);
            b[at] = l::color_flag::UNRESOLVED;
        }),
        Box::new(|b| {
            let at = start(b, l::INSTANCE_TRANSFORMS);
            b[at..at + 48].fill(0);
        }),
        Box::new(|b| {
            let at = start(b, l::STRINGS);
            b[at] = 255;
        }),
        Box::new(|b| {
            let at = start(b, l::ELEMENT_IDS);
            let id = b[at..at + 8].to_vec();
            b[at + 8..at + 16].copy_from_slice(&id);
        }),
        Box::new(|b| {
            let at = start(b, l::CHUNK_BOUNDS);
            put_u32(b, at, 4_f32.to_bits());
        }),
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert!(with_cx(Limits::default(), |cx| matches!(
            read(&changed, cx),
            Err(Error::Invalid(_))
        )));
    }
}

#[test]
fn a_later_minor_can_append_an_unknown_section() {
    let original = bytes();
    let count = l::SECTIONS.len();
    let old_table_end = 16 + count * 8;
    let extra_start = (original.len() + 8 + 7) & !7;
    let mut extended = vec![0; extra_start + 3];
    extended[..old_table_end].copy_from_slice(&original[..old_table_end]);
    extended[old_table_end + 8..original.len() + 8].copy_from_slice(&original[old_table_end..]);
    extended[6] = 1;
    put_u32(&mut extended, 12, count as u32 + 1);
    for index in 0..count {
        put_u32(
            &mut extended,
            16 + index * 8,
            get_u32(&original, 16 + index * 8) + 8,
        );
    }
    put_u32(&mut extended, old_table_end, extra_start as u32);
    put_u32(&mut extended, old_table_end + 4, 3);
    extended[extra_start..].copy_from_slice(&[255, 254, 253]);
    with_cx(Limits::default(), |cx| {
        let view = read(&extended, cx).unwrap();
        assert_eq!(view.sections, count as u32 + 1);
        assert_eq!(view.minor, 1);
        assert_eq!(view.element(0).unwrap().name, "壁");
        assert_eq!(view.instance(2).unwrap().mesh, 1);
    });
}

#[test]
fn encoding_validation_and_budgets_return_no_partial_buffer() {
    let mut invalid = scene();
    invalid.instances[2].element = 0;
    assert!(with_cx(Limits::default(), |cx| matches!(
        encode(&invalid, cx),
        Err(Error::Invalid("element_chunk"))
    )));
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let limits = Limits::default().with(kind, Limit::Finite(0));
        assert!(with_cx(limits, |cx| matches!(
            encode(&scene(), cx),
            Err(Error::Stopped(_))
        )));
    }
    let bytes = bytes();
    let limits = Limits::default().with(Kind::InputBytes, Limit::Finite(bytes.len() as u64 - 1));
    assert!(with_cx(limits, |cx| matches!(
        read(&bytes, cx),
        Err(Error::Stopped(_))
    )));
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    assert!(matches!(
        encode(&scene(), &mut cx),
        Err(Error::Stopped(Stop::Cancelled))
    ));
    assert!(matches!(
        read(&bytes, &mut cx),
        Err(Error::Stopped(Stop::Cancelled))
    ));
    with_cx(Limits::default(), |cx| {
        let empty = encode(&Data::default(), cx).unwrap();
        let view = read(&empty, cx).unwrap();
        assert_eq!(view.chunks, 0);
        assert_eq!(view.column(l::ELEMENT_IDS).unwrap().rows(), 0);
    });
}

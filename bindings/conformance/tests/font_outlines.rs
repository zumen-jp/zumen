//! #564 の合成フォントが、空の字形や代替の字形でなく、指定した直線と曲線を描くことを確かめる。
//! 外形は生成器の長方形と、3 次 Bézier 曲線の制御点から求める。ハッシュを正しさの根拠にしない。

use std::cell::RefCell;
use std::rc::Rc;

use zumen::scene::{Column, Scene, read_scene};
use zumen_conformance::{Direct, Engine, cases, run};

struct Outlines {
    engine: Direct,
    blobs: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl Engine for Outlines {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        self.engine.session_new(options)
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        self.engine.push(name, data)
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        self.engine.call(request)
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let result = self.engine.take(blob);
        if let Some(bytes) = &result.1 {
            self.blobs.borrow_mut().push(bytes.clone());
        }
        result
    }
}

fn indices<'a>(scene: &'a Scene, name: &str) -> &'a [u32] {
    match scene.column_named(name) {
        Some(Column::U32(values)) => values,
        _ => panic!("{name} の列がない"),
    }
}

#[test]
fn cff_outlines_and_the_second_collection_font_are_drawn() {
    let case = cases()
        .unwrap()
        .into_iter()
        .find(|case| case.name == "scene_fonts_cff")
        .unwrap();
    let blobs = Rc::new(RefCell::new(Vec::new()));
    let report = run(&[case], || {
        Box::new(Outlines {
            engine: Direct::default(),
            blobs: Rc::clone(&blobs),
        })
    });
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let blobs = blobs.borrow();
    assert_eq!(blobs.len(), 4);
    // 単独の CFF・CFF2 と、TTC の番号 0・1 は、それぞれ同じシーンを返す。
    // CFF と CFF2 の H の幅は異なるので、番号を無視すると一致しなくなる。
    assert_eq!(blobs[0], blobs[2]);
    assert_eq!(blobs[1], blobs[3]);
    assert_ne!(blobs[0], blobs[1]);
    for (blob, width) in blobs.iter().zip([400.0, 700.0, 400.0, 700.0]) {
        let scene = read_scene(blob).unwrap();
        assert_eq!(indices(&scene, "glyph_shapes"), [0, 1]);
        assert_eq!(indices(&scene, "shape_rings"), [0, 1]);
        assert_eq!(indices(&scene, "outline_vertices"), [0, 4]);
        let Some(Column::F32(xy)) = scene.column_named("outline_xy") else {
            panic!("字形の頂点の列がない");
        };
        let (points, rest) = xy.as_chunks::<2>();
        assert!(rest.is_empty());
        assert_eq!(
            &points[..4],
            &[[0.0, 0.0], [width, 0.0], [width, 700.0], [0.0, 700.0]]
        );
        // g: (100, 0)・(100, 400)・(400, 400)・(400, 0) の曲線。
        // y の最大は t = 1/2 の 300 で、両端を結ぶ線だけになっていない。
        let curve = &points[4..];
        assert!(curve.len() > 4);
        assert_eq!(curve.first(), Some(&[100.0, 0.0]));
        assert_eq!(curve.last(), Some(&[400.0, 0.0]));
        assert!(curve.contains(&[250.0, 300.0]));
        assert!(
            curve
                .iter()
                .all(|[x, y]| { (100.0..=400.0).contains(x) && (0.0..=300.0).contains(y) })
        );
    }
}

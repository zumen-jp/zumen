//! ZMS3 のヘッダーだけでなく、メッシュ・配置・文字列の参照まで通す最小入力。
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_scene3d::{Chunk, Color, Data, Element, Instance, Mesh};

pub(super) fn seed() -> Result<Vec<u8>, String> {
    let data = Data {
        chunks: vec![Chunk {
            origin: [1e12, 0., 0.],
            bounds: [0., 0., 0., 1., 1., 0.],
            instances: 0,
        }],
        meshes: vec![Mesh {
            vertices: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            triangles: vec![[0, 1, 2]],
            edges: vec![[0, 1], [1, 2], [2, 0]],
        }],
        instances: vec![Instance {
            mesh: 0,
            element: 0,
            color: 0,
            transform: [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.],
        }],
        elements: vec![Element {
            id: "synthetic".into(),
            class: "IfcWall".into(),
            name: "壁".into(),
            storey: None,
        }],
        colors: vec![Color {
            rgba: [0., 0., 0., 1.],
            unresolved: true,
        }],
    };
    zumen_scene3d::encode(
        &data,
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .map_err(|error| format!("3D のシーンの種を作れない: {error:?}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn seed_has_mesh_and_element_references() {
        let bytes = super::seed().unwrap();
        let scene = zumen_scene3d::read(
            &bytes,
            &mut zumen_base::Cx::new(
                &mut zumen_base::Budget::default(),
                &mut zumen_base::Diagnostics::new(),
                &zumen_base::CancelToken::new(),
            ),
        )
        .unwrap();
        assert_eq!(scene.element(0).unwrap().name, "壁");
        assert_eq!(scene.instance(0).unwrap().element, 0);
        assert_eq!(scene.mesh(0).unwrap().triangles.rows(), 1);
    }
}

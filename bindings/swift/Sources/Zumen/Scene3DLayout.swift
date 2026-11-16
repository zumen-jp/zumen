// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。

/// 3D シーンの配置
public enum Scene3DLayout {
    public static let major: UInt16 = 1
    public static let minor: UInt16 = 0
    public static let none = UInt32.max
    public static let sections: [Constants.SceneSection] = [
        // 塊の原点。プロジェクト空間の文書単位の X・Y・Z
        Constants.SceneSection(name: "chunk_origins", type: .f64, width: 3),
        // 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
        Constants.SceneSection(name: "chunk_bounds", type: .f32, width: 6),
        // 塊のインスタンスの始まり。1 要素を複数の塊に分けない
        Constants.SceneSection(name: "chunk_instances", type: .u32, width: 1),
        // メッシュの頂点の始まり。vertex_xyz の行
        Constants.SceneSection(name: "mesh_vertices", type: .u32, width: 1),
        // メッシュの三角形の始まり。triangle_vertices の行
        Constants.SceneSection(name: "mesh_triangles", type: .u32, width: 1),
        // メッシュの稜線の始まり。edge_vertices の行
        Constants.SceneSection(name: "mesh_edges", type: .u32, width: 1),
        // メッシュの局所座標。文書単位の X・Y・Z
        Constants.SceneSection(name: "vertex_xyz", type: .f32, width: 3),
        // 三角形の頂点。所属するメッシュの中の頂点番号
        Constants.SceneSection(name: "triangle_vertices", type: .u32, width: 3),
        // 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
        Constants.SceneSection(name: "edge_vertices", type: .u32, width: 2),
        // 配置するメッシュ。mesh_vertices の行
        Constants.SceneSection(name: "instance_meshes", type: .u32, width: 1),
        // 配置の元の要素。element_ids の行
        Constants.SceneSection(name: "instance_elements", type: .u32, width: 1),
        // 配置の色。color_rgba の行
        Constants.SceneSection(name: "instance_colors", type: .u32, width: 1),
        // メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
        Constants.SceneSection(name: "instance_transforms", type: .f32, width: 12),
        // 建物モデルの要素の ID。strings の始まりと長さ
        Constants.SceneSection(name: "element_ids", type: .u32, width: 2),
        // 要素のクラス。strings の始まりと長さ
        Constants.SceneSection(name: "element_classes", type: .u32, width: 2),
        // 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
        Constants.SceneSection(name: "element_names", type: .u32, width: 2),
        // 所属する階。element_ids の行。なければ番号の「なし」
        Constants.SceneSection(name: "element_storeys", type: .u32, width: 1),
        // 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
        Constants.SceneSection(name: "color_rgba", type: .f32, width: 4),
        // 色のフラグ。color_flag の値の和
        Constants.SceneSection(name: "color_flags", type: .u8, width: 1),
        // ID・クラス・名前の UTF-8 のバイト列
        Constants.SceneSection(name: "strings", type: .u8, width: 1),
    ]
    public static let color_flag_unresolved: UInt32 = 1
}

// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。
package dev.zumen;

/** 3D シーンの配置 */
public final class Scene3DLayout {
    private Scene3DLayout() {}
    public static final int MAJOR = 1;
    public static final int MINOR = 0;
    public static final int NONE = -1;
    public static final Constants.SceneSection[] SECTIONS = {
        // 塊の原点。プロジェクト空間の文書単位の X・Y・Z
        new Constants.SceneSection("chunk_origins", Constants.ColumnType.F64, 3),
        // 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
        new Constants.SceneSection("chunk_bounds", Constants.ColumnType.F32, 6),
        // 塊のインスタンスの始まり。1 要素を複数の塊に分けない
        new Constants.SceneSection("chunk_instances", Constants.ColumnType.U32, 1),
        // メッシュの頂点の始まり。vertex_xyz の行
        new Constants.SceneSection("mesh_vertices", Constants.ColumnType.U32, 1),
        // メッシュの三角形の始まり。triangle_vertices の行
        new Constants.SceneSection("mesh_triangles", Constants.ColumnType.U32, 1),
        // メッシュの稜線の始まり。edge_vertices の行
        new Constants.SceneSection("mesh_edges", Constants.ColumnType.U32, 1),
        // メッシュの局所座標。文書単位の X・Y・Z
        new Constants.SceneSection("vertex_xyz", Constants.ColumnType.F32, 3),
        // 三角形の頂点。所属するメッシュの中の頂点番号
        new Constants.SceneSection("triangle_vertices", Constants.ColumnType.U32, 3),
        // 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
        new Constants.SceneSection("edge_vertices", Constants.ColumnType.U32, 2),
        // 配置するメッシュ。mesh_vertices の行
        new Constants.SceneSection("instance_meshes", Constants.ColumnType.U32, 1),
        // 配置の元の要素。element_ids の行
        new Constants.SceneSection("instance_elements", Constants.ColumnType.U32, 1),
        // 配置の色。color_rgba の行
        new Constants.SceneSection("instance_colors", Constants.ColumnType.U32, 1),
        // メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
        new Constants.SceneSection("instance_transforms", Constants.ColumnType.F32, 12),
        // 建物モデルの要素の ID。strings の始まりと長さ
        new Constants.SceneSection("element_ids", Constants.ColumnType.U32, 2),
        // 要素のクラス。strings の始まりと長さ
        new Constants.SceneSection("element_classes", Constants.ColumnType.U32, 2),
        // 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
        new Constants.SceneSection("element_names", Constants.ColumnType.U32, 2),
        // 所属する階。element_ids の行。なければ番号の「なし」
        new Constants.SceneSection("element_storeys", Constants.ColumnType.U32, 1),
        // 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
        new Constants.SceneSection("color_rgba", Constants.ColumnType.F32, 4),
        // 色のフラグ。color_flag の値の和
        new Constants.SceneSection("color_flags", Constants.ColumnType.U8, 1),
        // ID・クラス・名前の UTF-8 のバイト列
        new Constants.SceneSection("strings", Constants.ColumnType.U8, 1),
    };
    public static final int COLOR_FLAG_UNRESOLVED = 1;
}

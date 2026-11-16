// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。
import 'constants.dart';

/// 3D シーンの配置
abstract final class Scene3DLayout {
  static const major = 1;
  static const minor = 0;
  static const none = 0xffffffff;
  static const sections = <SceneSection>[
    // 塊の原点。プロジェクト空間の文書単位の X・Y・Z
    SceneSection('chunk_origins', ColumnType.f64, 3),
    // 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
    SceneSection('chunk_bounds', ColumnType.f32, 6),
    // 塊のインスタンスの始まり。1 要素を複数の塊に分けない
    SceneSection('chunk_instances', ColumnType.u32, 1),
    // メッシュの頂点の始まり。vertex_xyz の行
    SceneSection('mesh_vertices', ColumnType.u32, 1),
    // メッシュの三角形の始まり。triangle_vertices の行
    SceneSection('mesh_triangles', ColumnType.u32, 1),
    // メッシュの稜線の始まり。edge_vertices の行
    SceneSection('mesh_edges', ColumnType.u32, 1),
    // メッシュの局所座標。文書単位の X・Y・Z
    SceneSection('vertex_xyz', ColumnType.f32, 3),
    // 三角形の頂点。所属するメッシュの中の頂点番号
    SceneSection('triangle_vertices', ColumnType.u32, 3),
    // 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
    SceneSection('edge_vertices', ColumnType.u32, 2),
    // 配置するメッシュ。mesh_vertices の行
    SceneSection('instance_meshes', ColumnType.u32, 1),
    // 配置の元の要素。element_ids の行
    SceneSection('instance_elements', ColumnType.u32, 1),
    // 配置の色。color_rgba の行
    SceneSection('instance_colors', ColumnType.u32, 1),
    // メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
    SceneSection('instance_transforms', ColumnType.f32, 12),
    // 建物モデルの要素の ID。strings の始まりと長さ
    SceneSection('element_ids', ColumnType.u32, 2),
    // 要素のクラス。strings の始まりと長さ
    SceneSection('element_classes', ColumnType.u32, 2),
    // 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
    SceneSection('element_names', ColumnType.u32, 2),
    // 所属する階。element_ids の行。なければ番号の「なし」
    SceneSection('element_storeys', ColumnType.u32, 1),
    // 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
    SceneSection('color_rgba', ColumnType.f32, 4),
    // 色のフラグ。color_flag の値の和
    SceneSection('color_flags', ColumnType.u8, 1),
    // ID・クラス・名前の UTF-8 のバイト列
    SceneSection('strings', ColumnType.u8, 1),
  ];
  static const colorFlagUnresolved = 1;
}

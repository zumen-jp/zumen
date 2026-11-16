// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。
package zumen

const Scene3DMajor uint16 = 1
const Scene3DMinor uint16 = 1
const Scene3DNone uint32 = 0xffffffff

var Scene3DSections = []SceneSection{
	// 塊の原点。プロジェクト空間の文書単位の X・Y・Z
	{Name: "chunk_origins", Type: ColumnF64, Width: 3},
	// 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
	{Name: "chunk_bounds", Type: ColumnF32, Width: 6},
	// 塊のインスタンスの始まり。1 要素を複数の塊に分けない
	{Name: "chunk_instances", Type: ColumnU32, Width: 1},
	// メッシュの頂点の始まり。vertex_xyz の行
	{Name: "mesh_vertices", Type: ColumnU32, Width: 1},
	// メッシュの三角形の始まり。triangle_vertices の行
	{Name: "mesh_triangles", Type: ColumnU32, Width: 1},
	// メッシュの稜線の始まり。edge_vertices の行
	{Name: "mesh_edges", Type: ColumnU32, Width: 1},
	// メッシュの局所座標。文書単位の X・Y・Z
	{Name: "vertex_xyz", Type: ColumnF32, Width: 3},
	// 三角形の頂点。所属するメッシュの中の頂点番号
	{Name: "triangle_vertices", Type: ColumnU32, Width: 3},
	// 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
	{Name: "edge_vertices", Type: ColumnU32, Width: 2},
	// 配置するメッシュ。mesh_vertices の行
	{Name: "instance_meshes", Type: ColumnU32, Width: 1},
	// 配置の元の要素。element_ids の行
	{Name: "instance_elements", Type: ColumnU32, Width: 1},
	// 配置の色。color_rgba の行
	{Name: "instance_colors", Type: ColumnU32, Width: 1},
	// メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
	{Name: "instance_transforms", Type: ColumnF32, Width: 12},
	// 建物モデルの要素の ID。strings の始まりと長さ
	{Name: "element_ids", Type: ColumnU32, Width: 2},
	// 要素のクラス。strings の始まりと長さ
	{Name: "element_classes", Type: ColumnU32, Width: 2},
	// 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
	{Name: "element_names", Type: ColumnU32, Width: 2},
	// 所属する階。element_ids の行。なければ番号の「なし」
	{Name: "element_storeys", Type: ColumnU32, Width: 1},
	// 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
	{Name: "color_rgba", Type: ColumnF32, Width: 4},
	// 色のフラグ。color_flag の値の和
	{Name: "color_flags", Type: ColumnU8, Width: 1},
	// ID・クラス・名前の UTF-8 のバイト列
	{Name: "strings", Type: ColumnU8, Width: 1},
	// 塊の点の始まり。point_xyz の行。点を含むシーンで塊ごとに持つ
	{Name: "chunk_points", Type: ColumnU32, Width: 1},
	// 元の八分木のノード。段・北の位置・東の位置・高さの位置。段は0〜20。点のない塊は4値ともなし
	{Name: "chunk_nodes", Type: ColumnU32, Width: 4},
	// 塊の原点からの点の東・北・高さ。メートル
	{Name: "point_xyz", Type: ColumnF32, Width: 3},
	// 点の赤・緑・青・不透明度。0〜65535。元の16ビットの色を保つ。属性がなければ空の列
	{Name: "point_rgba", Type: ColumnU16, Width: 4},
	// 点の元の強度。属性がなければ空の列
	{Name: "point_intensity", Type: ColumnU16, Width: 1},
	// 点の元の分類。属性がなければ空の列
	{Name: "point_classification", Type: ColumnU8, Width: 1},
}

const Scene3DColorFlagUnresolved uint32 = 1

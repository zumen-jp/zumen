// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。
using System;
using System.Collections.Generic;

namespace Zumen {
    /// <summary>3D シーンの配置</summary>
    public static class Scene3DLayout {
        /// <summary>ZMS3 の識別子</summary>
        public const uint Magic = 0x33534d5a;
        /// <summary>メジャーバージョン</summary>
        public const int Major = 1;
        /// <summary>マイナーバージョン</summary>
        public const int Minor = 0;
        /// <summary>参照なし</summary>
        public const uint None = 0xffffffff;
        /// <summary>セクションの定義</summary>
        public static readonly IReadOnlyList<SceneSection> Sections = Array.AsReadOnly(new SceneSection[] {
            // 塊の原点。プロジェクト空間の文書単位の X・Y・Z
            new SceneSection(0, "chunk_origins", typeof(double), 3),
            // 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
            new SceneSection(1, "chunk_bounds", typeof(float), 6),
            // 塊のインスタンスの始まり。1 要素を複数の塊に分けない
            new SceneSection(2, "chunk_instances", typeof(uint), 1),
            // メッシュの頂点の始まり。vertex_xyz の行
            new SceneSection(3, "mesh_vertices", typeof(uint), 1),
            // メッシュの三角形の始まり。triangle_vertices の行
            new SceneSection(4, "mesh_triangles", typeof(uint), 1),
            // メッシュの稜線の始まり。edge_vertices の行
            new SceneSection(5, "mesh_edges", typeof(uint), 1),
            // メッシュの局所座標。文書単位の X・Y・Z
            new SceneSection(6, "vertex_xyz", typeof(float), 3),
            // 三角形の頂点。所属するメッシュの中の頂点番号
            new SceneSection(7, "triangle_vertices", typeof(uint), 3),
            // 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
            new SceneSection(8, "edge_vertices", typeof(uint), 2),
            // 配置するメッシュ。mesh_vertices の行
            new SceneSection(9, "instance_meshes", typeof(uint), 1),
            // 配置の元の要素。element_ids の行
            new SceneSection(10, "instance_elements", typeof(uint), 1),
            // 配置の色。color_rgba の行
            new SceneSection(11, "instance_colors", typeof(uint), 1),
            // メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
            new SceneSection(12, "instance_transforms", typeof(float), 12),
            // 建物モデルの要素の ID。strings の始まりと長さ
            new SceneSection(13, "element_ids", typeof(uint), 2),
            // 要素のクラス。strings の始まりと長さ
            new SceneSection(14, "element_classes", typeof(uint), 2),
            // 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
            new SceneSection(15, "element_names", typeof(uint), 2),
            // 所属する階。element_ids の行。なければ番号の「なし」
            new SceneSection(16, "element_storeys", typeof(uint), 1),
            // 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
            new SceneSection(17, "color_rgba", typeof(float), 4),
            // 色のフラグ。color_flag の値の和
            new SceneSection(18, "color_flags", typeof(byte), 1),
            // ID・クラス・名前の UTF-8 のバイト列
            new SceneSection(19, "strings", typeof(byte), 1),
        });
        /// <summary>色が未解決。読み手が代替色を決める</summary>
        public const uint ColorFlagUnresolved = 1;
    }
}

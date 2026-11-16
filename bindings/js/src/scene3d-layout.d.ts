// 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。

export declare const SCENE3D_MAGIC: "ZMS3";
export declare const SCENE3D_MAJOR: 1;
export declare const SCENE3D_MINOR: number;
export declare const SCENE3D_NONE: 4294967295;

export type Scene3DSectionName =
  | "chunk_origins"
  | "chunk_bounds"
  | "chunk_instances"
  | "mesh_vertices"
  | "mesh_triangles"
  | "mesh_edges"
  | "vertex_xyz"
  | "triangle_vertices"
  | "edge_vertices"
  | "instance_meshes"
  | "instance_elements"
  | "instance_colors"
  | "instance_transforms"
  | "element_ids"
  | "element_classes"
  | "element_names"
  | "element_storeys"
  | "color_rgba"
  | "color_flags"
  | "strings"
;

export interface Scene3DSection {
  readonly name: Scene3DSectionName;
  readonly type: "u8" | "u32" | "f32" | "f64";
  readonly width: number;
}

export declare const SCENE3D_SECTIONS: readonly Scene3DSection[];
export declare const SCENE3D_VALUES: {
  readonly color_flag: {
    readonly unresolved: 1;
  };
};

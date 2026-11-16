import type { Bytes } from "./index.js";
import type { Scene3DSectionName } from "./scene3d-layout.js";

/** コアの予算とは別の、読み手の上限。 */
export interface Scene3DReadLimits {
  /** 既定 512 MiB */
  maxBytes?: number;
  /** コピーと検査用の索引・文字列の論理量。既定 256 MiB */
  maxAllocatedBytes?: number;
  /** 列の値と参照を検査する作業量。既定 100000000 */
  maxWork?: number;
}
export interface Scene3DElement { readonly id: string; readonly class: string; readonly name: string; readonly storey: number | null; }
export interface Scene3DInstance { readonly mesh: number; readonly element: number; readonly color: number; readonly transform: Float32Array; }
export interface Scene3DMesh { readonly vertices: Float32Array; readonly triangles: Uint32Array; readonly edges: Uint32Array; }
export interface Scene3DColor { readonly rgba: Float32Array; readonly unresolved: boolean; }
export interface Scene3DChunk { readonly origin: Float64Array; readonly bounds: Float32Array; readonly begin: number; readonly end: number; }
export type Scene3DColumns = {
  readonly chunk_origins: Float64Array; readonly chunk_bounds: Float32Array; readonly chunk_instances: Uint32Array;
  readonly mesh_vertices: Uint32Array; readonly mesh_triangles: Uint32Array; readonly mesh_edges: Uint32Array;
  readonly vertex_xyz: Float32Array; readonly triangle_vertices: Uint32Array; readonly edge_vertices: Uint32Array;
  readonly instance_meshes: Uint32Array; readonly instance_elements: Uint32Array; readonly instance_colors: Uint32Array; readonly instance_transforms: Float32Array;
  readonly element_ids: Uint32Array; readonly element_classes: Uint32Array; readonly element_names: Uint32Array; readonly element_storeys: Uint32Array;
  readonly color_rgba: Float32Array; readonly color_flags: Uint8Array; readonly strings: Uint8Array;
};
/** 検査済みの列。参照中の blob・型付き配列は変更・転送しない。 */
export declare class Scene3D {
  private constructor();
  readonly major: number; readonly minor: number; readonly chunks: number; readonly sections: number;
  readonly columns: Scene3DColumns;
  rows(name: Scene3DSectionName): number;
  text(start: number, length: number): string;
  string(name: "element_ids" | "element_classes" | "element_names", row: number): string;
  element(row: number): Scene3DElement;
  elementById(id: string): number | undefined;
  instance(row: number): Scene3DInstance;
  mesh(row: number): Scene3DMesh;
  color(row: number): Scene3DColor;
  chunk(row: number): Scene3DChunk;
}
export declare function readScene3D(input: Bytes, limits?: Scene3DReadLimits): Scene3D;

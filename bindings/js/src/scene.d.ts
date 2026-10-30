// シーンの blob の読み手の型（決定 0137・0194）。区画の型は constants.d.ts に生成する。

import type { SceneColumns, SceneSectionName } from "./constants.js";

/** 読んだシーン */
export declare class Scene {
  private constructor();
  /** 大版 */
  readonly major: number;
  /** 小版 */
  readonly minor: number;
  /** 塊の数 */
  readonly chunks: number;
  /** blob にある区画の数。この包みが知らない区画も数える */
  readonly sections: number;
  /** 区画ごとの列。行ごとに width 個の値を並べる。ない区画は空の列 */
  readonly columns: SceneColumns;
  /** 区画の行の数 */
  rows(name: SceneSectionName): number;
  /** strings の区画から、始まりと長さ（バイト）で文字列を引く */
  text(start: number, length: number): string;
}

/** シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない */
export declare function readScene(input: ArrayBuffer | ArrayBufferView): Scene;

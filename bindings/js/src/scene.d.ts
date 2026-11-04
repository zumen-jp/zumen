// シーンの blob のリーダーの型（決定 0137・0194）。セクションの型は constants.d.ts に生成する。

import type { SceneColumns, SceneSectionName } from "./constants.js";

/** 読んだシーン */
export declare class Scene {
  private constructor();
  /** メジャーバージョン */
  readonly major: number;
  /** マイナーバージョン */
  readonly minor: number;
  /** 塊の数 */
  readonly chunks: number;
  /** blob にあるセクションの数。このラッパーが知らないセクションも数える */
  readonly sections: number;
  /** セクションごとの列。行ごとに width 個の値を並べる。ないセクションは空の列 */
  readonly columns: SceneColumns;
  /** セクションの行の数 */
  rows(name: SceneSectionName): number;
  /** strings のセクションから、始まりと長さ（バイト）で文字列を引く */
  text(start: number, length: number): string;
}

/** シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない */
export declare function readScene(input: ArrayBuffer | ArrayBufferView): Scene;

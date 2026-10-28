# 0137: 2D のシーンは 1 つの ArrayBuffer に列を並べる独自の配置で渡す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-28 |
| 状態 | 採用 |

## 背景

[#6](https://github.com/zumen-jp/zumen/issues/6) で、Worker とページの間で大きなデータを渡す形式を決める。いまの制約は次のとおり。

- 表の結果は Arrow で返す。2D・3D のシーンの配置は #6 で測って決め、定義を `schema/` に置いて `zm_take` で渡す（[決定 0012](0012-boundary-functions-and-messages.md)）。
- バイナリの結果は、wasm のメモリから 1 度だけ写し、ArrayBuffer のまま、ページへ移す（[決定 0013](0013-boundary-in-four-forms.md)）。
- 表示用のシーンは、塊ごとに f64 の原点と、そこからの f32 の相対座標で持つ（[決定 0019](0019-coordinate-systems-and-gsi-data.md)）。

旧版の決定 0016 の測定用の図面（22.7 万図形の SFC）は、手元の試料にない。そこで、同じ規模の合成データで 4 つの形式を測った。測定の道具は `bench/transfer` にある（コアのワークスペースの外）。

## 選択肢

1. JSON。旧版の決定 0016 のように、図形ごとのオブジェクトを並べた文字列を送る。
2. 独自の列指向の配置。1 つのバッファーに、見出し、区画の表、列（型付き配列）を 8 バイトの境界で並べる。
3. FlatBuffers。1 つの表に、2 と同じ列をベクターで持つ。
4. Arrow IPC。折れ線の表と、塊の原点の表の 2 つのストリームを並べる。

## 測定

合成データは、1 km 四方（平面直角座標の 10⁵ m 級）に 2〜8 点の折れ線を 22.7 万本、頂点を 113.5 万点置いた。塊は 32 m 四方で、222 個ある。折れ線ごとに画層・色・幅を持つ。32 m の範囲の f32 の刻みは約 4 µm になる。

ページ側の処理は、どの形式でも、原点（f64）・頂点の始まり（u32）・相対座標（f32）の型付き配列を得るまでとした。JSON はさらに、解析した結果を型付き配列に詰める。4 つの形式で、得た配列が一致することを確かめた。Apple M2 Pro で測り、7 回の中央値を示す。

| 形式 | 大きさ | 符号化（Rust） | 送る | ページ側（Node 24） | 合計（Node 24） | 合計（Chromium 152） | メモリの増分（Node 24） | wasm の増分（gzip） |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| JSON | 30.0 MB | 140 ms | 11 ms | 124 ms（うち解析 111 ms） | 135 ms | 455〜577 ms | 229 MB | 32 KB（12 KB） |
| 独自の列指向 | 12.7 MB | 2.0 ms | 1.9 ms | 0.0 ms | 1.9 ms | 3.6〜3.9 ms | 50 MB | 5 KB（2 KB） |
| FlatBuffers | 12.7 MB | 2.9 ms | 1.9 ms | 0.0 ms | 2.0 ms | 2.7〜3.0 ms | 50 MB | 9 KB（3 KB） |
| Arrow IPC | 14.1 MB | 3.0 ms | 2.3 ms | 2.6 ms | 5.5 ms | 3.9〜5.4 ms | 53 MB | 562 KB（142 KB） |

- 「送る」は、Worker が結果を写して（JSON は文字列にして）送り始めてから、ページが受け取るまで。
- Chromium は、デスクトップアプリの組み込みのブラウザーで 2 回測った範囲。ペインを隠した状態だったので、値は目安とする。
- メモリの増分は、Worker が結果を持った後の、プロセスの RSS の最大の増分。Worker が読んだ 12.7〜30 MB を含む。
- wasm の増分は、合成データを作るだけの wasm（17.6 KB）との差。JSON の増分の多くは浮動小数の書式で、コアはすでに `zumen-json` を持つので、境界の wasm では増えない。

## 決定

選択肢 2 を採る。2D のシーンを 1 つの ArrayBuffer に列で並べ、ページでは型付き配列のビューを作るだけにする。

- **JSON を使わない。** 22.7 万本で、ページ側の時間が 2 桁以上長く、メモリも 4 倍を超える。JSON は、メタデータ、マークアップ、操作、診断のような小さいものに限る。
- **FlatBuffers を採らない。** 時間と大きさは 2 とほぼ同じで、違いは表の仕組み（vtable）の分だけになる。列を並べるだけなら FlatBuffers のスキーマ言語と生成器は要らず、定義は `schema/` と `cargo xtask codegen` で持てる。
- **シーンに Arrow IPC を使わない。** ページ側の時間は短いが、wasm が 562 KB（gzip で 142 KB）増える。表の結果を Arrow で返すこと（決定 0012）は変えない。Python などへシーンを渡すときは、同じ列から Arrow の表を作れる。
- **配置。** 見出しは `ZMSC`、大版（u16）、小版（u16）、数（u32）、区画の数（u32）。続けて区画ごとに始まりと長さ（u32）を置き、各区画を 8 バイトの境界に置く。数はリトルエンディアン。
- **版と互換。** 読み手は、同じ大版で、小版が新しいものを読む。知らない区画は、区画の表で読み飛ばす。区画の意味を変えるときは大版を上げる。
- **大きな座標。** 塊ごとに f64 の原点と f32 の相対座標を持つ（決定 0019）。塊の大きさの決め方は、表示の許容差（[決定 0018](0018-geometry-layers-and-tolerances.md)）と合わせて、実際の図面で決める。

## 影響

- 合成データの測定から採った。旧版の決定 0016 の図面と、BIM ソフトが書き出した IFC（zumen-jp/zumen-legacy#128）での測定は、塊の大きさを決めるときに #219 で行う。測定用の図面は Git の外の `ZUMEN_SAMPLES` で扱う。
- 3D のインスタンス（同じ形状の使い回し）と LOD の持ち方は、まだ決めていない。3D のメッシュの扱い（zumen-jp/zumen-legacy#126）と合わせて、3D のシーンで測る（#219）。
- 区画の定義を `schema/` に置き、`zm_take` が返すシーンの種類として足す（#219）。

## 参照

- [決定 0012](0012-boundary-functions-and-messages.md)、[決定 0013](0013-boundary-in-four-forms.md)、[決定 0018](0018-geometry-layers-and-tolerances.md)、[決定 0019](0019-coordinate-systems-and-gsi-data.md)、[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)
- [旧版の決定 0016](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0016-send-worker-scenes-as-json.md)
- [bench/transfer](../../bench/transfer/README.md)
- [Arrow の IPC の形式](https://arrow.apache.org/docs/format/Columnar.html#serialization-and-interprocess-communication-ipc)、[FlatBuffers の内部の形式](https://flatbuffers.dev/internals/)
- #6

# 0256: PDF のページは hayro-interpret を版を固定して解釈し、ファイルモデルは自前でオブジェクトに区切り、wasm では別のモジュールにする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-03 |
| 状態 | 採用 |

## 背景

PDF は #1 の 2 章の対象で、台帳では `public`（ISO 32000-2:2020）である。旧版は、hayro-interpret でページを解釈し、線・文字・画像をシーンに読んだ（[旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md)）。ブラウザーでは、PDF を別の wasm のモジュールにした（[旧版の決定 0030](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0030-pdf-in-a-webassembly-module-of-its-own.md)）。新しい層（[決定 0008](0008-model-layers-and-codecs.md)）で読むには、次のことを決める必要があった（#506）。

- ページの内容（内容のストリーム・フォント・CMap・色空間・フィルター）を、自前で解釈するか、依存に任せるか。依存にするなら、`AGENTS.md` の依存の規則（ライセンス・wasm で動くこと・[決定 0007](0007-single-rust-implementation.md) の決定性）を満たすか。
- ファイルモデル（L1）のレコードを、どう区切るか。
- 図面モデルの要素の ID（[決定 0156](0156-drawing-stable-ids-and-preconditions.md)）を、何から作るか。
- wasm のモジュール（[決定 0221](0221-wasm-modules.md)・[0253](0253-wasm-civil-and-pointcloud-modules.md)）のどれに入れるか。

## 選択肢

### ページの解釈

1. 自前で書く。依存は増えないが、内容のストリームの演算子のほかに、フォント（TrueType・CFF・Type 1・Type 3）・CMap・圧縮のフィルター・画像の符号化を読む必要があり、大きい。
2. hayro-interpret を使う（旧版と同じ）。Rust で書かれ、wasm で動き、描く先を `Device` の trait で受けるので、線・塗り・文字・画像をラスターにせずに受け取れる。
3. pdf.js など、ほかの言語の実装を使う。Rust の 1 つの実装（決定 0007）に反し、ネイティブと wasm で同じ結果を返せない。

### L1 の区切り

1. hayro-syntax が読んだオブジェクトを使う。hayro-syntax は、オブジェクトのバイト列の範囲を返さないので、書き戻しにも、診断の場所にも使えない。
2. 自前の走査で、ファイルの字句（7.2〜7.5 節）からオブジェクトの範囲を区切る。

### wasm のモジュール

1. 図面のモジュール（`drawing`）に入れる。
2. PDF だけの `pdf` のモジュールに分け、PDF を開くときだけ読む（旧版の決定 0030 と同じ）。

## 決定

ページの解釈は選択肢 2、L1 は選択肢 2、wasm のモジュールは選択肢 2 を採る。

### 依存

- `hayro-interpret` を `=0.9.0` で固定し、既定の機能（埋め込みのないフォントの代わりと、定義済みの CMap を同梱する）で使う。PDF の crate（`zumen-pdf`）の機能 `drawing` だけが使い、L1 だけを使う利用者は外せる。
- ライセンス: 2026-11-03 に、依存の木（hayro-syntax・hayro-font・hayro-cmap・hayro-jbig2・hayro-jpeg2000・hayro-ccitt・hayro-postscript・skrifa・read-fonts・moxcms・kurbo・zune-jpeg・flate2・zlib-rs・brotli ほか）のライセンスを `cargo tree` で確かめた。どれも MIT・Apache-2.0・BSD-3-Clause・Zlib・Unicode-3.0 か、それらを選べるもので、`deny.toml` の一覧に入る。
- wasm: `wasm32-unknown-unknown` で build と clippy が通る。hayro-interpret は、ファイルの外（ファイルシステム・時刻・環境変数）を読まない。
- 決定性: hayro の結果のうち、環境で値が変わりうるものは使わない。
  - 色: hayro は DeviceCMYK と ICC の色を moxcms で sRGB にする。浮動小数で色を管理する計算は、版と環境で値が変わりうるので、hayro の色の値は使わず、色空間の系統と成分から自前の式で色にする。hayro は系統を公開していないので、色空間の `Debug` の表記の頭で見分ける。版を固定するのは、このためでもある。
  - 座標: hayro に渡す装置の空間を 1/1024 mm にする。2 の冪なので、mm に戻すときに丸めが入らない。
  - 角度と長さ: 自前のコードの三角関数と `hypot` は `libm` を使う。hayro の中には、std の数学関数を使う計算がある。ネイティブと wasm で同じ結果になることは、境界につなぐとき（#518）に適合試験で確かめる。
  - 順序: 要素は hayro が描いた順に並べ、自前の表は `BTreeMap` だけにする。hayro は表に `FxHashMap` を使い、そのハッシュの値は 32 ビットと 64 ビットの環境で違いうる。読んだかぎりでは、引くことと、順に依らない処理（拡張グラフィックス状態の辞書の項目を当てるなど）にだけ使い、Type 3 の字形の名前は並べ替えてから番号を振っている。
- 版を上げるのは読み取りの変更として扱い、ID の一覧の試験（`crates/zumen-pdf/tests/ids/pdf.txt`）・色の試験・適合試験で確かめる。
- 予算（[決定 0010](0010-budget-by-work.md)）: 自前のコードは、パス・グリフの列・画像ごとに `Work` を、要素ごとに `Elements` を、内容のストリームの展開を `DecodedBytes` で数える。hayro の中の作業（ストリームの展開・フォントの読み・曲線の平坦化）は予算の外にあり、hayro-syntax 0.9.0 の展開は出力の大きさに上限を持たない。予算で止める方法は #521 で決める。

### ファイルモデル（L1）

- レコードは、見出し・間接オブジェクト（`N G obj … endobj`）・相互参照表・トレーラー・`startxref`・コメント・読めないバイト列とする。オブジェクトのストリームと相互参照のストリームの中は分けず、1 つのオブジェクトにする。
- 書き戻しは、レコードのバイト列をそのまま並べ、読んだバイト列と同じものを返す。
- 図面モデルへの移しは、hayro-syntax が読んだ文書を使い、L1 のレコードは ID と診断の場所に使う。同じ番号のオブジェクトが何度も出るときは、最後のレコードを使う（増分の更新、7.5.6 節）。

### 要素の ID

- ページの要素の ID は、ページの最初の内容のストリームのレコードから、決定 0156 の規則で作る（`sheet/{ページ}/r{レコード}`、2 つ目から `/1`・`/2`…）。内容のストリームがなければ、ページのオブジェクトのレコードにする。
- hayro の版を上げて、描く順や数が変わると、ID が変わる。版を上げる PR は、その差を書く。

### wasm のモジュール

PDF の読み取りだけを入れた wasm（`zumen-pdf` と hayro と図面モデル。境界の関数は入れない。`wasm32-unknown-unknown` の release、名前の区画を除いたもの、brotli は Node.js の `zlib.brotliCompressSync` の既定の水準）を、2026-11-03 に測った。

| モジュール | バイト | brotli |
| --- | ---: | ---: |
| 図面（`drawing`、決定 0253） | 5,889,456 | 1,170,885 |
| PDF の読み取りだけ | 4,297,107 | 1,316,581 |

PDF を図面のモジュールに入れると、転送量がおよそ 2 倍になる。図面だけを開くアプリが、使わない PDF の分を読むことになるので、`pdf` のモジュールに分ける。すべてを入れたモジュールとネイティブ・CLI には、PDF を入れる。境界につなぐこと（`open` の判定・モジュールの表・適合試験）は #518 で行う。

## 影響

- 台帳の PDF の項目に、この決定と形式の文書（`docs/formats/pdf.md`）を書く。移しの規則と、移せないものの診断は、形式の文書に書く。
- `Cargo.lock` に、hayro とその依存の crate が加わる。どれも `zumen-pdf` の機能 `drawing` の下にある。
- hayro が描かないもの（隠した表示グループの中身）と、`Device` に渡さないもの（XObject の表示グループ、グリフごとの位置）は、移せない。診断で返し、#520 で追う。
- 外部の実装（pdf.js）との照合と公開サンプルは #519 で足す。

## 参照

- ISO 32000-2:2020（7.2〜7.5 節、8 章、9 章、14.11 節など）
- [決定 0007](0007-single-rust-implementation.md)・[0008](0008-model-layers-and-codecs.md)・[0010](0010-budget-by-work.md)・[0156](0156-drawing-stable-ids-and-preconditions.md)・[0221](0221-wasm-modules.md)・[0253](0253-wasm-civil-and-pointcloud-modules.md)、旧版の決定 0020・0030
- #506・#518・#519・#520・#521

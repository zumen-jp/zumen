# 0012: 各言語への境界を、数個の関数と、スキーマで定めたメッセージにする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版の結合は、言語ごとに手で書いた。Python の層は約 5,400 行あり（[0011](0011-bindings-through-ffi.md)）、wasm の結合は、形式の判定・シーンの読み取り・埋め込みファイルの取り出しの 3 つの関数だけだった。解析・変換・編集はブラウザーから使えず（[zumen-jp/zumen-legacy#97](https://github.com/zumen-jp/zumen-legacy/issues/97)・[zumen-jp/zumen-legacy#118](https://github.com/zumen-jp/zumen-legacy/issues/118)）、AI 向けの道具は Python にしかなかった（[旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md)・[旧版の決定 0011](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0011-tool-functions-for-agents.md)）。
- [0011](0011-bindings-through-ffi.md) で、各言語からは C ABI のライブラリと wasm のモジュールを呼び、言語の側は、手書きの小さな包みと、スキーマから生成した型だけにすると決めた。
- [0008](0008-model-layers-and-codecs.md) で、API の低・中・高の 3 つの水準を、どれも境界のメッセージとして出すと決めた。[0009](0009-diagnostics-and-catalog.md) は文言を組み立てる `zm_describe` の形を、[0010](0010-budget-by-work.md) は予算の設定の渡し方を、ここに残した。
- 呼ぶ側には、次の制約がある。
  - VBA は、構造体を値で渡せない。
  - コールバック（ホストの関数をコアに渡すこと）は、IL2CPP・VBA・wasm・別のスレッドで壊れやすい。
  - VBA・BAT・PowerShell は、JSON を扱いにくい。
  - JavaScript の数は、2^53 を超える整数を正しく持てない。
- 入力は 1 つのファイルとは限らない。外部参照、電子納品のフォルダー、SHX のフォント、比べる 2 つの版のように、名前のついた複数のファイルを一緒に読む。

## 選択肢

### 境界の形

1. **言語ごとに API を作る**（旧版）。言語の数だけ差と保守が増え、版ごとに機能がずれる。
2. **細かい C の関数を多く出す**（要素ごとに関数を呼ぶ）。言語の側の包みが大きくなり、境界を越える回数も増える。
3. **数個の関数と、版のあるメッセージにする。** 機能はメッセージとして足し、関数は増やさない。

### ハンドルが持つもの

1. **1 つのファイルを開いた文書**（issue の素案の `zm_open`）。2 つ以上のファイルを使う操作に、別の関数か、ハンドルをまたぐ操作が要る。
2. **名前のついた入力の集まりと、そこから開いた文書を持つセッション。** 1 つのファイルでも複数でも、同じ関数で扱える。

### メッセージを定める言語

1. **JSON Schema。** MCP の道具の定義（2020-12 が既定）にそのまま使え、どの言語にも検証の道具がある。ただし書くと長く、表せることが多すぎて、そのままでは型に写しにくい。
2. **独自の定義の言語。** 短く書けるが、AI の道具の定義や外部の検証には、JSON Schema への変換が要る。
3. **Rust の型を元にし、スキーマを生成する。** Rust では書きやすいが、ほかの言語の型が Rust の書き方に引きずられ、スキーマの変更がコードの変更に埋もれる。

## 決定

境界の形は 3、ハンドルは 2 を選ぶ。定める言語は 1 を選び、型に写せる部分だけを使う。

### 関数

```c
typedef int32_t zm_status;                            /* ZM_OK など。下の表 */
typedef struct zm_session zm_session;
typedef struct { uint8_t *ptr; size_t len; } zm_buf;

uint32_t  zm_abi_version(void);
zm_status zm_session_new(const uint8_t *options, size_t options_len, zm_session **session);
zm_status zm_push(zm_session *session, const uint8_t *name, size_t name_len,
                  const uint8_t *data, size_t data_len);
zm_status zm_call(zm_session *session, const uint8_t *request, size_t request_len,
                  zm_buf *response);
zm_status zm_take(zm_session *session, uint32_t blob, zm_buf *out);
zm_status zm_take_arrow(zm_session *session, uint32_t blob, struct ArrowArrayStream *out);
void      zm_cancel(zm_session *session);
void      zm_session_close(zm_session *session);
void      zm_free(zm_buf *buf);
```

| 関数 | 役割 |
| --- | --- |
| `zm_abi_version` | ABI の大版を返す。この関数の形は変えない（[0015](0015-add-only-boundary-and-version-floors.md)） |
| `zm_session_new` | セッションを作る。設定（実行の形など、[0013](0013-boundary-in-four-forms.md)）は JSON で渡す |
| `zm_push` | 名前のついた入力に、バイト列を足す。大きなファイルは、何回かに分けて渡せる |
| `zm_call` | 要求のメッセージを渡し、応答のメッセージを受け取る。問い合わせ・変更・書き出し・描画・文言は、すべてこれで行う |
| `zm_take` | 直前の `zm_call` が作ったバイナリの結果（書き出したファイル、画像、シーン、Arrow の IPC など）を取り出す |
| `zm_take_arrow` | 表の結果を、Arrow の C Stream Interface で取り出す。ネイティブだけ |
| `zm_cancel` | 動いている呼び出しを止める。別のスレッドから呼べる |
| `zm_session_close` | セッションを閉じる |
| `zm_free` | コアが返したバッファーを返す |

- 読める形式の一覧（素案の `zm_capabilities`）と、診断の文（素案の `zm_describe`）は、関数ではなく op にする。op にすれば、プロセスの外・HTTP・MCP の形（[0013](0013-boundary-in-four-forms.md)）にも、手を加えずに出る。
- wasm のモジュールは、同じ関数を export し、メモリを確保して返す `zm_alloc`・`zm_dealloc` を足す（[0011](0011-bindings-through-ffi.md)）。`zm_take_arrow` は出さず、表は Arrow の IPC で `zm_take` から渡す。
- 関数を足すときは新しい名前にし、今ある関数の引数と意味は変えない。

### 関数の約束

- **構造体を値で渡さない。** 引数は、整数・ポインター・長さだけにする。結果は、呼び出し側が用意した `zm_buf` などに、ポインターで書く。
- **`bool` を使わない。** 真偽は整数で表す。VBA の Boolean は 2 バイトで、真が -1 になる。
- **文字列は UTF-8。** 要求と応答の JSON、入力の名前が対象。
  - UTF-16 の入口は置かない。.NET・Java は標準の変換で、VBA は包みの中の Windows の API で UTF-8 にする。
  - CP932 などの元の文字コードは、境界に出さない。図面の文字は Unicode にして返す。元のバイト列が要るときは、低水準の op で取り出す。
- **パスを渡さない。** コアはファイルシステムを読まない（[0007](0007-single-rust-implementation.md)）。ホストがファイルを読み、名前とバイト列を `zm_push` で渡す。
  - 入力の名前は、`/` で区切った相対の名前にする。外部参照や電子納品のフォルダーは、この名前で解決する。
  - `..`・絶対パス・ドライブ名・NUL を含む名前は受け付けない（[0010](0010-budget-by-work.md) の ZIP の項目の名前と同じ規則）。
- **コールバックを受け取らない。** 進み具合や途中の結果は、呼び出しを分けて受け取る。Arrow の C Stream の `release` のように、コアが渡してホストが呼ぶ関数は、この規則に入らない。
- **メモリ**
  - 入力のバッファーは、呼び出しの間だけ借りる。コアは、要るものを写す。
  - コアが返したバッファーは、`zm_free` まで有効にする。大きな結果を、写さずにその場で読める。
  - 取り出さなかったバイナリの結果は、次の `zm_call` で捨てる。
  - 取り出した結果は、セッションを閉じたあとも使える。
- **状態コード**は少なく保ち、理由は応答の診断（[0009](0009-diagnostics-and-catalog.md)）で返す。

  | 状態 | 意味 |
  | --- | --- |
  | `ZM_OK` | 成功した |
  | `ZM_FAILED` | 操作が失敗した。応答に `error` の診断がある |
  | `ZM_INVALID` | 引数が使えない（NULL、UTF-8 でない名前など）。応答は書かない |
  | `ZM_BUSY` | 別のスレッドが、同じセッションを使っている |
  | `ZM_PANICKED` | コアの不具合で止まった。セッションは閉じるしかない |

- **パニック。** ネイティブでは、どの関数も `catch_unwind` で止め、`internal.panic.caught` の診断を返す。wasm では trap になり、ホストがインスタンスを作り直す（[0010](0010-budget-by-work.md)）。
- **スレッド**
  - 1 つのセッションは、一度に 1 つのスレッドだけが使う。重ねて呼ぶと `ZM_BUSY` を返し、未定義の動作にはしない。
  - `zm_cancel` は、どのスレッドからも呼べる。動いている呼び出しを止め、その呼び出しは `call.request.cancelled` を返す。動いている呼び出しがなければ、何もしない。
  - wasm では、呼び出しの途中に `zm_cancel` は届かない。ホストの手段で止める（[0010](0010-budget-by-work.md)）。
  - 別々のセッションは、別々のスレッドで同時に使える。
- **粒度。** 呼び出しを粗くし、図形 1 つごとに境界を越えない。一覧を返す op は、件数の上限と、続きを読む位置を持つ（[旧版の決定 0011](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0011-tool-functions-for-agents.md) の `limit`・`next_offset` を続ける）。

### メッセージ

```json
{"v": 1, "op": "entities", "doc": "a.sfc", "view": "A3-平面図", "region": [100, 50, 200, 120], "fields": ["id", "kind", "layer", "bbox"], "limit": 500}
```

```json
{"v": 1, "result": {"items": [], "total": 1234, "next_offset": 500}, "diagnostics": [], "blobs": []}
```

- **要求**は、版 `v`、操作 `op`、op ごとの欄からなる。どの op にも置ける欄は次のとおり。
  - `budget`: 予算の単位ごとの上限。上限をなくす単位には `"unlimited"` と書き、書かない単位は既定値にする（[0010](0010-budget-by-work.md)）。
  - `partial`: 予算で止まったとき、止まった所までの結果を求める（[0010](0010-budget-by-work.md)）。
  - `max_places`: 診断ごとに返す場所の数（[0009](0009-diagnostics-and-catalog.md)）。
  - `time_limit_ms`: 時間の上限。ネイティブの境界が時計を見て、中断の合図で伝える。wasm では受け付けない（[0010](0010-budget-by-work.md)）。
- **応答**は、結果 `result`、診断 `diagnostics`、バイナリの結果の一覧 `blobs`（番号・種類・大きさ）からなる。失敗したときも、同じ形で診断を返す。
- **文書。** `open` の op で入力を読み、文書を作る。文書は、セッションの中の名前で指す。形式の判定（[0007](0007-single-rust-implementation.md) のシグネチャ）と外部参照の解決は、セッションの入力の中で行う。
- **名前。** キー・op の名前・列挙の値は、英小文字・数字・`_` にする。診断の code は、[0009](0009-diagnostics-and-catalog.md) の形のまま。
- **数**
  - 整数は ±(2^53 − 1) の中に限る。超えうる値と ID は、文字列にする。
  - 浮動小数は、読み直すと同じ値に戻る最短の表記で書く。NaN と無限大は書かない。入力にあれば、読むときに診断にする。
- **決まったバイト列。** キーの順はスキーマの順にし、空白を入れない。同じ要求には、どの環境でも、同じバイト列の応答を返す（[0007](0007-single-rust-implementation.md)）。
- **知らない欄。** 要求に知らない欄があれば、失敗にする。古いコアが、新しい欄を黙って無視しないように。応答の知らない欄は、包みが無視する（[0015](0015-add-only-boundary-and-version-floors.md)）。
- **図面の中身はデータ。** 図面に記録された文字・名前・属性は、データとして返す。AI の道具の説明には、それを指示として扱わないように書く（[旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md)）。

### 最初の op

op は、[0008](0008-model-layers-and-codecs.md) の 3 つの水準ごとに、層を作るときに足す。最初の版には、次のものを置く。

| op | 中身 |
| --- | --- |
| `capabilities` | ABI とスキーマの版、使える op、読み書きできる形式と版、形式ごとの ID の規則の版 |
| `open` | 入力を読んで、文書を作る |
| `close` | 文書と入力を捨てる |
| `describe` | 診断・要素の種類・属性・列挙の値の文を、カタログから組み立てる |

`describe` に使うカタログは、ほかの入力と同じく `zm_push` で渡す。カタログがなければ、code と値をそのまま返す。数の整形も、ここで行う（[0009](0009-diagnostics-and-catalog.md)）。

### 表と、大きなデータ

- 表の形の結果は、要求の `format` で、Arrow・JSON・TSV のどれでも返せるようにする。
  - Arrow: ネイティブでは `zm_take_arrow` で取り出す。pyarrow・polars・DuckDB・R・.NET の Arrow が、写さずに受け取れる。wasm と JavaScript では、IPC のバイト列を `zm_take` で取り出す。
  - TSV: 1 行目を列のキーにし、タブ・改行・`\` をエスケープする。JSON を扱いにくい VBA・BAT・PowerShell のための形で、VBA の包みは 2 次元の配列にしてワークシートへ渡す。
- 2D・3D のシーンのような大きなデータのバイナリの配置は、[#6](https://github.com/zumen-jp/zumen/issues/6) で測って決める。その定義も `schema/` に置き、`zm_take` で渡す。

### スキーマと生成

- 要求と応答は、op ごとに JSON Schema（2020-12）で定め、`schema/` に置く。
- 使うのは、どの言語の型にも素直に写せる部分に限る。xtask の生成器と検査で、この範囲を守らせる。
  - object（`properties`・`required`。知らない欄は許さない）、array、string、integer、number、boolean
  - 文字列の `enum`、`$ref`
  - `const` の欄を目印にした `oneOf`（タグつきの共用体）
- op と欄の説明は日本語で書く（[0005](0005-japanese-first.md)）。この説明が、文書、生成した型のコメント、AI の道具の説明になる。
- 生成器（[0011](0011-bindings-through-ffi.md)）は、スキーマから次のものを作る。
  - コアの側の、Rust の要求と応答の型
  - 各言語のメッセージの型と、op ごとのメソッド（Python の `session.entities(view=…)`、C# の `session.Entities(…)` など）
  - C のヘッダー
  - AI 向けの op の、MCP の道具の定義
- 列挙の型は、知らない値も受け取れるように生成する。スキーマは足すだけなので、古い包みが、新しいコアから新しい値を受け取ることがある（[0015](0015-add-only-boundary-and-version-floors.md)）。

### 適合試験

- 言語に依存しない試験を、`bindings/conformance/` に置く。1 件は、入力・要求の並び・期待する結果の組にする。
  - 入力は、テストキットで作る合成のファイルにする。xtask で作って、各言語の試験に渡す。受け取った図面は使わない。
  - 期待する結果は、状態コードと、応答の一部の値（JSON Pointer で指す）にし、仕様から書く。
  - すべての応答とバイナリの結果について、ハッシュの一覧を作る。
- 同じ組を、Rust から直接・各言語の結合・すべての形（[0013](0013-boundary-in-four-forms.md)）で流し、ハッシュの一覧が、Rust から直接呼んだものと一致することを確かめる（[0007](0007-single-rust-implementation.md)）。階層 1 の言語は PR ごと、階層 2 は毎晩に流す（[0006](0006-language-tiers.md)）。
- 言語を足す PR のチェックリストの「適合試験に通る」（[0006](0006-language-tiers.md)）は、これを指す。

## 影響

- 機能を足しても、境界の関数は増えない。op とスキーマが増え、新しい op の PR には、生成した全言語の差分が並ぶ（[0011](0011-bindings-through-ffi.md)）。
- 1 つのファイルを開くだけでも、セッションを作る・入力を渡す・`open` を呼ぶ、の 3 段になる。各言語の包みに、これをまとめた入口を置く。
- issue の素案から、次のものを変えた。
  - `zm_open` は、セッション・`zm_push`・`open` の op に分けた。複数の入力を扱うためと、読み始める前から中断できるようにするため
  - `zm_view` は `zm_take` にした。書き出したファイルや画像も、同じ形で受け取る
  - `zm_arrow` は `zm_take_arrow` にした。表を作る要求も `zm_call` で送り、診断を同じ形で返す
  - `zm_capabilities`・`zm_describe` は op にした
  - UTF-16 の入口は置かない
- 入りきらない大きな入力（点群など）は、コアが要る範囲を応答で求め、ホストがその範囲を `zm_push` で渡す形を、点群の系統を作るときに決める。その場合も、コールバックは使わない。
- 境界を作る crate は、FFI のために `unsafe` を使う。ワークスペースの `unsafe_code = "deny"` は、その crate でだけ、理由を書いて外す。
- [0009](0009-diagnostics-and-catalog.md) の「`zm_describe` の引数と、カタログをコアに渡す方法」と、[0010](0010-budget-by-work.md) の「予算の設定を受け渡す形」は、この記録で決めた。
- [#13](https://github.com/zumen-jp/zumen/issues/13) の完了の条件のうち、境界のヘッダーとスキーマの最初の版は、境界の crate を作る PR で入れる。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「7. あらゆる言語から使う」、[#13](https://github.com/zumen-jp/zumen/issues/13)
- [0005](0005-japanese-first.md)、[0006](0006-language-tiers.md)、[0007](0007-single-rust-implementation.md)、[0008](0008-model-layers-and-codecs.md)、[0009](0009-diagnostics-and-catalog.md)、[0010](0010-budget-by-work.md)、[0011](0011-bindings-through-ffi.md)、[0013](0013-boundary-in-four-forms.md)、[0015](0015-add-only-boundary-and-version-floors.md)
- [旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md)・[旧版の決定 0011](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0011-tool-functions-for-agents.md)
- [zumen-jp/zumen-legacy#97](https://github.com/zumen-jp/zumen-legacy/issues/97)・[zumen-jp/zumen-legacy#118](https://github.com/zumen-jp/zumen-legacy/issues/118)
- [Arrow の C Stream Interface](https://arrow.apache.org/docs/format/CStreamInterface.html)、[JSON Schema 2020-12](https://json-schema.org/draft/2020-12)

## 後の記録

- 2026-10-24: [0016](0016-boundary-message-details.md) で、メッセージの JSON の読み書きの細部を決め、スキーマに 3 つの書き方（`allOf` による共通の欄の取り込み、名前から値への表、スカラーの共用体）を足した。読めない要求は、`ZM_INVALID` ではなく、`ZM_FAILED` と理由の診断で返す。

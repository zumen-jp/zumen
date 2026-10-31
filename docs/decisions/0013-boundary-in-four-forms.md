# 0013: 同じ境界を、ネイティブ・wasm・プロセスの外・HTTP の 4 つの形で出し、セッションごとに選ぶ

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 境界は、数個の関数と、版のあるメッセージにした（[0012](0012-boundary-functions-and-messages.md)）。各言語は、C ABI のライブラリか wasm のモジュールを呼ぶ（[0011](0011-bindings-through-ffi.md)）。
- 呼ぶ側の事情は、場面ごとに違う。
  - ホストアプリ（Revit・AutoCAD・Excel など）の中で落ちると、作業中のモデルやブックを失う。
  - 32 bit の Excel は、使えるアドレス空間が 2〜4 GiB しかない。
  - 依存の中の処理は、予算で止めきれないことがある（[旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md) の hayro）。
  - 外部のライブラリを読み込めない環境がある。Power Automate・Office Scripts・Google Apps Script のようなクラウドの低コードや、FileMaker。
  - iOS のアプリは、ほかのプロセスを起こせない。
- 旧版は、ブラウザーでは wasm を Worker の中で動かし、時間の上限では Worker ごと止めた（[0010](0010-budget-by-work.md) の背景）。wasm のモジュールは、図面の形式のものと PDF のものに分けた（[旧版の決定 0030](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0030-pdf-in-a-webassembly-module-of-its-own.md)。brotli で、図面のものが約 0.58 MB、PDF だけのものが約 1.43 MB）。
- 旧版は、AI 向けの道具を、Python のパッケージの MCP サーバーで出した。MCP の公式の SDK を使い、HTTP は、認証と利用者ごとの権限が要るので出さなかった（[旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md)）。
- 道具の状況（2026-10-24 に確かめた）
  - MCP は、2026-07-28 の版で、状態を持たない形に大きく変わった。それより前の版のクライアントに応えるには、前の版の初期化も受ける必要がある。公式の Rust SDK（rmcp、Apache-2.0）は両方に応える。ただし tokio などの依存が多く、2026 年に大版を 3 回上げた。
  - HTTP のサーバーの crate は、hyper（MIT）が軽い。tiny_http は、2022 年から版が出ていない。
  - wasmtime の各言語の結合（Python・.NET）は、64 bit の環境にしかない。32 bit の Windows は、wasmtime 自身でも Tier 3（本番には勧めない）。
  - 安定版の Rust（1.99.1）では、スレッドを使う wasm32 のモジュールを作れない（nightly の `-Z build-std` が要る）。
- [0007](0007-single-rust-implementation.md) は wasm のモジュールの分け方を、[0011](0011-bindings-through-ffi.md) は MCP の SDK と HTTP の道具、1 つの言語の中で形を選べるようにするかを、ここに残した。

## 選択肢

### 出す形

1. **C ABI のライブラリと wasm だけを出す。** ほかの形は、利用者が自分で作る。
2. **C ABI のライブラリ・wasm・プロセスの外・HTTP の 4 つを、同じメッセージで出す。**

### ホストアプリの中で、受け取ったファイルを読む形

1. **プロセスの中のネイティブ。** 速い。ただし、コアの不具合やメモリの不足で、ホストごと落ちうる。
2. **プロセスの中の wasm の砂場。** trap はホストに響かず、メモリの上限と fuel・epoch で止められる。ただし、言語ごとに wasm のランタイムが要り、32 bit の Windows で勧められるランタイムがない。
3. **プロセスの外。** どの失敗も呼び出し元に響かず、時間を過ぎたら止めて捨てられる。プロセスを起こす手間と、データを送る手間がかかる。

### MCP と HTTP の道具

1. **公式の SDK（rmcp）と hyper を使う。** MCP の版の変化に追いつきやすい。依存が増え、SDK の大版を上げるたびに直す。
2. **自前で書く。** 依存は少ない。MCP の版の変化（2026-07-28 の、状態を持たない形への変更など）に、自分で追いつく。

## 決定

出す形は 2 を選ぶ。ホストアプリの中で受け取ったファイルを読むときは、3 を勧める。道具は 1 を選ぶ。

### 4 つの形

| 形 | 中身 | 主に使う所 |
| --- | --- | --- |
| ネイティブのライブラリ | C ABI の動的ライブラリ（.dll・.so・.dylib）と静的ライブラリ | Python・.NET・Java・Swift・Dart・VBA・C・C++ |
| wasm | [0012](0012-boundary-functions-and-messages.md) の関数を export するコアモジュール。import を持たない | ブラウザー・Node.js・Go |
| プロセスの外 | CLI の `zumen`。サブコマンド、標準入出力の JSON-RPC 2.0（`zumen serve --stdio`）、MCP（`zumen mcp`） | 階層 3 の言語、ホストアプリの中での分離、AI のアプリ |
| HTTP | 同じ JSON-RPC を HTTP で受ける（`zumen serve --http`）。コンテナーのイメージにする | クラウドの低コード、社内のサーバー |

- どの形でも、同じ op と同じメッセージを使う。JSON-RPC のメソッドは、[0012](0012-boundary-functions-and-messages.md) の関数と op に、1 対 1 で対応させる。
- どの形でも、応答のメッセージを書き換えずに運ぶ。[0012](0012-boundary-functions-and-messages.md) の適合試験をすべての形で流し、ハッシュの一覧が一致することを確かめる。
- プロセスの外と HTTP のサーバーは、コアではなく、ホストにあたる。
  - 標準入出力では、入力を、パスか、base64 のバイト列で渡せる。
  - HTTP では、送られたバイト列だけを扱い、サーバーのファイルは読まない。

### 形をセッションごとに選ぶ

- 形は、`zm_session_new` の設定の `engine` で選ぶ。各言語の包みは、セッションを作るときの引数として出す。

  | `engine` | 中身 | 使える所 |
  | --- | --- | --- |
  | `native` | プロセスの中で、コアを直接動かす | ネイティブのライブラリを呼ぶ言語 |
  | `process` | 子プロセスの CLI に、同じメッセージを送る | プロセスを起こせる所（iOS・Android・ブラウザーを除く） |
  | `wasm` | プロセスの中で、wasm のモジュールを動かす | ブラウザー・Node.js・Go |

- **既定は `native` にする**（ブラウザー・Node.js・Go では `wasm`）。
- **`process` は、C ABI のライブラリの中に作る。** ライブラリが CLI を子プロセスとして起こし、標準入出力の JSON-RPC で、同じメッセージを送る。各言語の包みは、子プロセスとのやり取りを書かない。VBA や C++ からも、同じ設定で使える。
  - 子プロセスの実行ファイルは、設定でパスを渡すか、ライブラリと同じフォルダーから探す。
  - 時間の上限を過ぎたら、子プロセスを止めて捨てる。依存の中の、止めきれない処理も止まる。
  - 子プロセスのメモリには、OS の仕組み（Windows のジョブオブジェクトなど）で上限を付ける。
  - 32 bit のホストからも、64 bit の子プロセスを使える。32 bit の Excel でも、大きな図面を読める。
  - 子プロセスが落ちたら、その呼び出しは診断を返して失敗する。セッションは、作り直すしかない。
  - Node.js と Go では、包みの中に `process` を作る。標準入出力の JSON-RPC は公開の約束なので、同じものを使う。
- **ホストアプリの中で、受け取ったファイルを読むときは、`process` を勧める。** ホストアプリ向けの例（[0004](0004-repositories.md) の案の `examples`）とテンプレートは、`process` で書く。wasm の砂場より勧める理由は、次のとおり。
  - どの言語からも、同じ設定で使える。
  - 32 bit のホストでも使え、メモリがホストのプロセスから分かれる。
  - 言語ごとに wasm のランタイムを足さずに済む。
  - 依存の中の処理も含めて、確実に止められる。
- ネイティブの言語の中で wasm を砂場として動かす形は、当面は作らない。需要を見て、記録を書いてから足す。候補は、Java の Chicory（Apache-2.0、Java だけで動く）と、wasmtime の各言語の結合。

### wasm のモジュール

- 重い依存ごとに、モジュールを分ける。

  | モジュール | 中身 |
  | --- | --- |
  | 図面 | DXF・DWG・JWW・JWC・JWS・SXF、レビュー、電子納品の管理ファイル |
  | PDF | PDF の読み取り（[旧版の決定 0030](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0030-pdf-in-a-webassembly-module-of-its-own.md) を続ける） |
  | BIM | IFC と、3D の幾何カーネル |
  | 土木・地理 | J-LandXML・SIMA・CityGML など、座標系の変換 |
  | 点群 | LAS・LAZ・E57 など |

- どのモジュールも、同じ関数を export する。モジュールにない op は、使えないことを診断で返す。
- 形式の判定の規則（[0007](0007-single-rust-implementation.md) のシグネチャ）と、どのモジュールがどの形式と op を持つかの表を、データから TypeScript に生成する。JavaScript の包みは、形式を判定してから、要るモジュールだけを読み込む。
- モジュールをまたぐ処理（IFC から切り出した図面を DXF に書く、など）は、L2 をバイナリ（[#6](https://github.com/zumen-jp/zumen/issues/6)）で受け渡し、包みがつなぐ。
- ネイティブのライブラリと CLI は、すべての公開の形式を 1 つに入れる。大きさが問題になる所（モバイルなど）のために、Cargo の feature で形式を減らせるようにする。
- モジュールごとの大きさ（brotli で圧縮したもの）を CI で測って記録し、分け方を見直す材料にする。
- wasm32 のメモリは 4 GiB までで、ブラウザーではさらに小さいことがある。点群のような大きなデータは、範囲を区切って扱う（[0012](0012-boundary-functions-and-messages.md) の影響）。memory64 は、Safari と Rust の対応がそろうまで使わない。

### ブラウザー

- JavaScript の包みは、既定で Worker の中で wasm を動かす。止めるときは Worker ごと止め、作り直す（[0010](0010-budget-by-work.md)）。
- バイナリの結果は、wasm のメモリから 1 度だけ写し、ArrayBuffer のまま、ページへ移す。
- wasm のスレッドは、安定版の Rust で作れるようになるまで使わない。並列にするときは、Worker ごとにインスタンスを作って仕事を分け、結果を入力の順に集める（[0007](0007-single-rust-implementation.md)）。

### CLI・MCP・HTTP

- CLI は、1 つの実行ファイル `zumen` にする。よく使う op（形式の判定、変換、描画など）をサブコマンドにし、`serve --stdio`・`mcp`・`serve --http` を置く。
- **標準入出力の JSON-RPC** は、自前で書く。MCP の stdio と同じく、1 行に 1 つのメッセージにする。
- **MCP**
  - 公式の Rust SDK（rmcp）を使い、大版を固定する。大版を上げるときは、サーバーを見直す（[旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md) と同じ）。この項は [0197](0197-cli-and-mcp-server.md) で、依存を足さずに自前で書くことに改めた。
  - 道具は、スキーマの AI 向けの op から生成する（[0012](0012-boundary-functions-and-messages.md)）。道具の一覧は、決まった順に返す。
  - 読めるフォルダーを `--root` で限り、入力の大きさ・返す件数・画像の大きさに上限を置く（[旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md)・[旧版の決定 0011](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0011-tool-functions-for-agents.md) を続ける）。
  - 図面の文字・名前・属性は、データであって指示ではないことを、道具の説明とサーバーの案内に書く。
  - 当面は、標準入出力だけで出す。HTTP で出すには、認証と、利用者ごとの権限が要る。
- **HTTP**
  - hyper と tokio（どちらも MIT）で作る。この項は [0200](0200-cli-http-process-engine-and-mcp-resources.md) で、標準ライブラリで書くことに改めた。
  - 既定は、127.0.0.1 だけで待ち受ける。TLS と認証は持たない。ほかのアドレスで待ち受けるときは、認証のある前段（社内のプロキシなど）の後ろに置く。
  - セッションは、推測できない ID で指し、使われない時間が続いたら捨てる。セッションの数とメモリにも、上限を置く。
  - 大きな入力と結果は、base64 にせず、本文のまま送れるようにする。
- tokio・rmcp・hyper は、CLI だけが使う。コアと C ABI のライブラリには入れない。子プロセス用に配る CLI は、MCP と HTTP を外して作れるようにする（Cargo の feature）。

## 影響

- 各言語の包みは、形を意識しない。同じメソッドが、どの形でも同じ結果を返す。
- `process` を使うには、CLI の実行ファイルが要る。配り方は [0014](0014-platforms-signing-and-distribution.md) で決める。
- iOS・Android・ブラウザーでは、`process` を使えない。
- 標準入出力の base64 は、大きなファイルで遅くなる。測って、要るなら、バイト列をそのまま送る枠を足す。
- MCP の SDK は、大版が速く上がる。上げるたびに、道具の定義と案内を見直す。
- [0007](0007-single-rust-implementation.md) の「モジュールの分け方」と、[0011](0011-bindings-through-ffi.md) の「1 つの言語の中で形を選べるようにするか」「MCP の SDK と HTTP の道具」は、この記録で決めた。
- [#14](https://github.com/zumen-jp/zumen/issues/14) は、この記録で完了する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「5. 層をまたぐ仕組み」の「ブラウザーでの実行」と「7. あらゆる言語から使う」、[#14](https://github.com/zumen-jp/zumen/issues/14)
- [0004](0004-repositories.md)、[0007](0007-single-rust-implementation.md)、[0010](0010-budget-by-work.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)
- [旧版の決定 0004](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0004-mcp-server-in-the-python-package.md)・[旧版の決定 0011](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0011-tool-functions-for-agents.md)・[旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md)・[旧版の決定 0030](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0030-pdf-in-a-webassembly-module-of-its-own.md)
- [JSON-RPC 2.0](https://www.jsonrpc.org/specification)、[MCP 2026-07-28 の変更点](https://modelcontextprotocol.io/specification/2026-07-28/changelog)、[rmcp](https://github.com/modelcontextprotocol/rust-sdk)、[hyper](https://crates.io/crates/hyper)
- [Wasmtime の対応するプラットフォーム](https://docs.wasmtime.dev/stability-platform-support.html)、[Chicory](https://github.com/dylibso/chicory)

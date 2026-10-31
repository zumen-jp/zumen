# 0200: serve --http は標準ライブラリで書き、process は C ABI のライブラリが CLI を起こし、MCP にリソースと進み具合の通知を足す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- [0197](0197-cli-and-mcp-server.md) は、CLI と MCP を依存を足さずに書き、`serve --http`・子プロセスの形（`engine` の `process`）・画像の大きさの上限・MCP の `resources` と進み具合の通知を残した。HTTP（hyper・tokio）は `serve --http` を作るときに決めるとした。
- [0013](0013-boundary-in-four-forms.md) の HTTP の形: 同じ JSON-RPC を HTTP で受ける。既定は 127.0.0.1 だけで待ち受け、TLS と認証は持たない。送られたバイト列だけを扱い、サーバーのファイルは読まない。セッションは推測できない ID で指し、使われない時間が続いたら捨て、数とメモリに上限を置く。大きな入力と結果は、base64 にせず本文のまま送れるようにする。
- [0013](0013-boundary-in-four-forms.md) の `process`: C ABI のライブラリが CLI を子プロセスとして起こし、標準入出力の JSON-RPC で同じメッセージを送る。実行ファイルは設定でパスを渡すか、ライブラリと同じフォルダーから探す。時間の上限を過ぎたら止めて捨て、メモリには OS の仕組みで上限を付ける。落ちたら、その呼び出しは診断を返して失敗する。Node.js と Go は包みの中に作る。
- MCP の仕様を、2026-10-31 に公開の文書で確かめた。
  - リソース: `resources` の能力を宣言し、`resources/list`・`resources/templates/list`・`resources/read` に応える。一覧は、つながりや、ほかの要求の副作用で変えてはならない。ないリソースは `-32602` を返す（前の版は `-32002`）。
  - キャッシュ: `server/discover`・`tools/list`・`resources/list`・`resources/templates/list`・`resources/read` の結果に、`ttlMs` と `cacheScope` を書かなければならない。
  - 進み具合: 要求の `_meta` の `progressToken` に、`notifications/progress` を送ってよい。`progress` は増やし続け、`total` は省いてよい。2025-11-25 も同じ形。
- この版の op には画像を返すものがない（blob の種類 `image` を作る op がない）。

## 選択肢

### HTTP のサーバー

1. **hyper と tokio を使う（[0013](0013-boundary-in-four-forms.md) の記述）。** HTTP/1.1 と HTTP/2、つながりの再利用、非同期の多重化が手に入る。依存の木が大きく、大版の上げに追いつく。
2. **標準ライブラリ（`std::net`）で書く。** 1 つのつながりで 1 つの要求を受けて閉じる、`Content-Length` だけの HTTP/1.1 に絞る。ローカルの前段の後ろに置く用途には足りる。

### `process` を置く所

1. **C ABI のライブラリ（`zumen-abi`）のセッションの中に置く（[0013](0013-boundary-in-four-forms.md) の記述）。** Python・.NET・C・C++ の包みは、設定の `engine` を変えるだけで使える。
2. **包みごとに書く。** 言語ごとに子プロセスとのやり取りを書き直す。

### MCP のリソースに出すもの

1. **セッションの blob を一覧に並べる。** 一覧が op の呼び出しで変わり、仕様の「副作用で変えない」に反する。
2. **一覧は決まったもの（メッセージのスキーマとカタログ）にし、blob は URI の型で読ませる。**

## 決定

HTTP は 2、`process` は 1、リソースは 2 を選ぶ。依存は足さない。[0013](0013-boundary-in-four-forms.md) の「HTTP は hyper と tokio で作る」は、この記録で改める。HTTP/2 やつながりの再利用が要るようになったら、そのときに hyper を見直す。

### `zumen serve --http [--listen アドレス:ポート]`

- 既定は `127.0.0.1:24180`。待ち受けたアドレスを、標準出力の 1 行目に `http://<アドレス>/` で書く。ループバックでないアドレスでは、認証のある前段の後ろに置くよう、標準エラーに書く。
- `POST /`（`application/json`）で、JSON-RPC 2.0 の 1 つのメッセージを受ける。メソッドは `serve --stdio` と同じ `session_new`・`push`・`call`・`take`・`cancel` と、`session_close`。`session_new` は推測できない `session`（OS の乱数で鍵を決めた SipHash による 128 bit）を返し、ほかのメソッドは `session` で指す。`cancel` は、そのセッションで動いている `call` を止める。
- 本文のままの入出力: `POST /sessions/<id>/inputs/<名前>`（`application/octet-stream`、名前は `%XX` で書ける）で入力を足し、`GET /sessions/<id>/blobs/<番号>` でバイナリの結果を本文で返す。
- サーバーのファイルは読まない。`push` の `path` は `-32602` で断る（[0013](0013-boundary-in-four-forms.md)）。`--root` は持たない。
- ループバックで待ち受けるときは、`Host` がループバックの名前（`localhost`・`127.0.0.1`・`[::1]`）の要求だけを受け、DNS の付け替えを防ぐ。`Origin` のある要求（ブラウザー）は 403 で断る。`Content-Type` を確かめ、ブラウザーが前もって確かめずに送れる形を受けない。
- 上限: セッションは 16、入力の合計は 1 GiB、要求の本文は 256 MiB、要求の行と見出しは 16 KiB、同時のつながりは 32、要求を読む時間は 60 秒。使われない時間が 10 分続いたセッションは捨てる。`Transfer-Encoding` は受けず、`Content-Length` を求める。同じセッションで動いている呼び出しがあれば `busy` を返す。

### 実行の形 `process`

- `session_options` に `process_path`（文字列）を足す。`engine` が `process` で、ネイティブのライブラリなら、`process_path` か、ライブラリと同じフォルダーの `zumen` を `serve --stdio` で起こす。ライブラリの場所は、Unix では `dladdr` で求める。起こせなければ `zm_session_new` は `failed` を返す。wasm のモジュールでは、これまでどおり `failed`。
- `push`・`call`・`take` を子プロセスに送り、応答の JSON は書き換えずに返す。中断の合図を受けたら `cancel` を送る。要求の `time_limit_ms` を 1 秒過ぎても答えがなければ止めて捨て、`budget.time.exceeded` で失敗する。中断の合図のあと 1 秒答えがなければ、止めて捨て、`call.request.cancelled` で失敗する。子プロセスが落ちたか、答えが読めなければ、新しい診断 `call.process.caught` で失敗する。止めたあとのセッションは使えず、作り直す。
- 終わった子プロセスには書かない。書くと、SIGPIPE を無視しないホストは止まる。
- Python・.NET・C・C++ の包みは、セッションの設定で `engine` と `process_path` を渡せば使える。
- 適合試験の組 `session` の `process` の行は、ない実行ファイルを `process_path` に書いて `failed` を確かめる形にした。ライブラリと同じフォルダーに CLI があるかどうかで、結果が変わらないようにするため。`bindings/cli/tests/conformance.rs` は、設定に `process` とこの CLI のパスを足して、すべての組を流す。

### MCP

- `resources` の能力を宣言する。`resources/list` は、メッセージのスキーマ（`zumen://schema/messages.json`）と、カタログ（`zumen://catalog/ja.json`・`zumen://catalog/en.json`）を返す。`resources/templates/list` は、blob の型 `zumen://{session}/blobs/{blob}` を返す。`take` の埋め込みも、この URI を使う。
- `resources/read` は、決まったものを文字で、blob を base64 で返す。一度取り出した blob は、そのセッションの次の op まで、`take` とリソースで何回でも読める（埋め込みの上限の 16 MiB まで）。ないものは `-32602` と `data.uri` で返す。
- キャッシュの手がかり: 一覧・型の一覧・決まったリソース・`server/discover` は `ttlMs` 3600000 と `public`、blob は `ttlMs` 0 と `private`。
- 進み具合: `tools/call` の `_meta` に `progressToken`（文字列か整数）があれば、始まりに `progress` 0 を送り、道具が終わるまで 5 秒ごとに 1 ずつ増やして送る。コアは進み具合を返さないので、`total` は書かない。長い呼び出しで、クライアントが時間切れにしないためのもの。

### 画像の大きさの上限

- 作らない。画像を返す op がまだないため。描画の op を足すときに、要求に画素の数の上限を持たせ、MCP では `image` の内容として、その上限と埋め込みの上限で返す。

### CLI でシーンの blob を読む

- `zumen scene-info <ファイル | ->`: シーンの blob の見出しを確かめ、大版・小版・塊の数と、区画ごとの名前・行の数・バイト数を JSON で書く。知らない区画（後の小版で足したもの）は、名前を書かずに番号とバイト数だけを書く。範囲の外を指す区画と、行の大きさで割り切れない区画は、誤りにする。

## 影響

- 4 つの形のうち、HTTP がそろう。適合試験を `serve --stdio`・`serve --http`・MCP・`process` で流し、ハッシュの一覧が一致することを確かめる。
- [0013](0013-boundary-in-four-forms.md) の「子プロセス用に配る CLI は、MCP と HTTP を外して作れるようにする（Cargo の feature）」は、依存を足さないので要らない。
- 残り:
  - `process` のメモリの上限（Unix の `setrlimit`、Windows のジョブオブジェクト）。OS ごとの関数を結ぶので、別に作る。
  - Windows でライブラリの場所を探すこと（`GetModuleFileNameW`）。それまでは `process_path` で渡す。
  - Node.js の包みの `process`。今の JavaScript の包みは同期の API で、子プロセスとのやり取りは非同期になるので、非同期のセッションの形を決めてから作る。Go の包みは、まだない。
  - 画像の大きさの上限（描画の op と一緒に作る）。
- 公開するまで、HTTP の道筋・メソッド・上限の値は変えてよい。

## 参照

- [#355](https://github.com/zumen-jp/zumen/issues/355)
- [0010](0010-budget-by-work.md)、[0013](0013-boundary-in-four-forms.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0197](0197-cli-and-mcp-server.md)
- [RFC 9112（HTTP/1.1）](https://www.rfc-editor.org/rfc/rfc9112)、[MCP 2026-07-28 のリソース](https://modelcontextprotocol.io/specification/2026-07-28/server/resources)、[キャッシュ](https://modelcontextprotocol.io/specification/2026-07-28/server/utilities/caching)、[進み具合](https://modelcontextprotocol.io/specification/2026-07-28/basic/utilities/progress)

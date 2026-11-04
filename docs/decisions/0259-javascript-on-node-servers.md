# 0259: npm のパッケージに Node.js の入口を足し、trap のあとのインスタンスを使えなくし、ライセンスの文を入れる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-03 |
| 状態 | 採用 |

## 背景

- 利用者は、最初の公開で、まず Web（Next.js のバックエンドなど）から使えることを求めた（#531）。
- ラッパーの README は、Node.js で wasm を `import.meta.resolve` で探して読む例を示していた（[0194](0194-typescript-javascript-wrapper.md)・[0205](0205-release-and-publishing.md)）。
- Next.js 16.4.4 で `npm pack` したパッケージを API ルートから使い、公開サンプルの DXF と DWG を読ませた。

  | wasm の読み込み方 | Turbopack | webpack（standalone） | `serverExternalPackages` あり |
  | --- | --- | --- | --- |
  | README の `import.meta.resolve` | 500 | 500 | 500 |
  | `process.cwd()` から `node_modules` を読む | 動く | ENOENT | 動く |
  | パッケージの中で `new URL("../zumen_abi1.wasm", import.meta.url)` | 動く | 失敗 | 動く |

  - webpack は、パッケージの中の `new URL(…, import.meta.url)` を、ブラウザー向けの `/_next/` の URL に書き換え、standalone で組んだ出力にも wasm を入れない。
  - 同じバージョンで、route に `runtime = "edge"` を書いて試すと、`?module` で読み込んだモジュールが別の realm のもので、`load` の `instanceof WebAssembly.Module` が弾いた。Next.js の文書は、Edge のランタイムを Proxy で使うものとしている。Vercel の文書は、Next.js 16.3 から route と page の `runtime = 'edge'` は使えず、Node.js で動くとしている。
- wasm では、パニックとメモリ不足が trap になり、ホストがインスタンスを作り直す（[0010](0010-budget-by-work.md)、`crates/zumen-abi/src/ffi.rs`）。ラッパーにはその扱いがなく、trap のあとも同じインスタンスを呼べた。
- wasm のメモリは縮まない。300 MB の入力を渡すと 602 MiB になり、セッションを閉じても戻らなかった。手元の Node.js では、コンパイルしたモジュールからインスタンスを作るのは 1 回 0.6 ms ほどだった。
- [0014](0014-platforms-signing-and-distribution.md) は、どの成果物にも依存のライセンスの文（`THIRD_PARTY_LICENSES`、cargo-about）を入れると決めたが、リリースのワークフローは作っていなかった。npm のパッケージには、MIT と Apache-2.0 の本文もなかった。wasm には、14 の第三者の crate が入る。
- cargo-about は、crate の本文を見分けられないと、SPDX の定型の文で代え、`--fail` を付けても止まらない。libm は、著作権の表示のない MIT の定型の文になり、laz は、作者と移植元の表示を落とした Apache-2.0 の標準の本文になった。

## 選択肢

### Node.js で wasm を読み込む形

1. **今のまま、利用者が wasm を探して読む。** bundler ごとに書き方が変わり、README の例は Next.js で動かない。
2. **`load` を、引数がなければパッケージの wasm を読む形にする。** 入口が 1 つで済むが、ブラウザー向けの入口に `node:fs` が入り、ブラウザーの bundler が組めなくなる。
3. **Node.js の入口（`@zumen-jp/zumen/node`）を足し、パッケージの中から `new URL("../zumen_abi1.wasm", import.meta.url)` で読む。** Node.js はそのまま読み、Turbopack はこの形を辿って、組んだ出力に入れる。webpack では、パッケージを `serverExternalPackages` に入れる設定が 1 行要る。
4. **wasm を base64 の文字列にして、JavaScript に埋め込む。** どの bundler でも設定は要らないが、パッケージが約 1.33 倍になり、読み込むたびに文字列の解析と写しで時間とメモリを使う。

### trap のあと

1. **ラッパーは何もせず、利用者が `WebAssembly.RuntimeError` を見て作り直す。** 利用者が知らなければ、壊れたインスタンスを使い続ける。
2. **wasm の関数から例外が出たら、ラッパーがインスタンスを使えないものにする。** その呼び出しと、それからの呼び出しを `panicked` の `ZumenError` にする。

## 決定

- Node.js で読み込む形は、選択肢 3 を採る。
  - `@zumen-jp/zumen/node` に `loadBundled()` を置く。コンパイルは 1 度だけにして覚え、呼ぶたびに新しいインスタンスを返す。読み込みに失敗したら覚えたものを捨て、次に呼んだときにやり直す。
  - `@zumen-jp/zumen/node` は、`@zumen-jp/zumen` の名前をすべて出し直す。
- trap のあとは、選択肢 2 を採る。
  - wasm の関数から例外が出たら（trap・スタックのあふれなど）、そのインスタンスを使えないものにし、`Zumen.broken` を `true` にする。読み込むときに呼ぶ関数（`zm_abi_version`・`zm_plugin_init`）も同じにする。
  - その呼び出しと、それからの呼び出しは、状態が `panicked` の `ZumenError` にし、元の例外を `cause` に入れる。
  - `Session.close` と `cancel` は、使えないインスタンスでは何もしない。`finally` の中で呼んでも、先に出た例外を隠さない。
  - API が `failed` などを返したときは、インスタンスは使えるまま。
  - 分けたモジュールの `Modules`（`loadModules`）は、使えなくなったインスタンスを捨て、次に引いたときに読み込み直す。読み込みに失敗したときも覚えず、次に引いたときにやり直す（`loadBundled` と同じ）。
- `load`・`loadSync` などは、型を `Object.prototype.toString` の名前で調べ、別の realm で作った `WebAssembly.Module`・`ArrayBuffer`・`Response` も受け取る。
- サーバーでは、要求ごとに `loadBundled` で新しいインスタンスを使うよう README で勧める。trap が要求の外へ及ばず、wasm のメモリも要求が終われば回収される。
- ライセンスの文:
  - リポジトリの直下の `about.toml` と `about.hbs` から、cargo-about 0.9.2 で `THIRD_PARTY_LICENSES` を作る。対象は `zumen-abi` の wasm32 の依存で、build と dev の依存と、このリポジトリの crate は載せない。許すライセンスは `deny.toml` と合わせる。
  - cargo-about が本文を見分けられない crate は、`about.toml` の clarify で本文のファイルを指す。第三者の crate が SPDX の定型の文のままのときと、cargo-about が警告を出したとき（clarify で指したファイルが変わったときなど）は、リリースのワークフローを止める。
  - cargo-about は、配るものを作る job と分けた job で動かす。cargo-about とその依存の build script が、配るものに触れないようにする。
  - リリースのワークフローは、npm のパッケージに `zumen_abi1.wasm`・`THIRD_PARTY_LICENSES`・`LICENSE-MIT`・`LICENSE-APACHE` を入れ、GitHub の Release にも `THIRD_PARTY_LICENSES` を置く。

## 影響

- Node.js の利用者は、wasm の道筋を書かずに済む。webpack で組む Next.js の利用者は、`serverExternalPackages` を書く。
- Edge のランタイム（Proxy など）でも、`load` に `?module` のモジュールを渡せるが、テストはしない。
- テストは、PR ごとの `bindings/js/test/server.mjs` と、毎晩の `bindings/js/test/nextjs`（Next.js・React のバージョンは、その `package.json` で固定する）にある。
- Next.js の任意の依存の sharp は、LGPL の libvips をテストの環境に入れる。テストのツールにだけ使い、配る成果物には入らない（Playwright の WebKit と同じ扱い）。
- crate のバージョンを上げて、clarify で指したファイルが変わると、cargo-about は警告を出して元の扱い（定型の文や標準の本文）に戻り、リリースのワークフローは、この警告で止まる。中身を見て、`about.toml` を直す。
- ほかの成果物（wheel・NuGet・CLI の圧縮・ネイティブのライブラリ）のライセンスの文は、#536 で入れる。cargo-about のバージョンを上げるときは、`about.toml` の書き方の変化を確かめる（0.9 で ClearlyDefined の設定がなくなった）。
- crates.io に出す一覧（`CRATES`）の足し忘れは、`cargo xtask codegen --check` が確かめる。並びは確かめない。Cargo 1.90 から、複数の crate を渡した `cargo publish` は、渡した順によらず依存される側から出す。
- 同期の呼び出しはイベントループを止めるので、大きな図面を受けるサーバーには `worker_threads` で動かす手順が要る（#532）。入力を wasm のメモリに 2 回写すことは #533、サーバーで画像を描く op は #534、分けた wasm のモジュールの配布は #535 で扱う。

## 参照

- [0010](0010-budget-by-work.md)・[0014](0014-platforms-signing-and-distribution.md)・[0194](0194-typescript-javascript-wrapper.md)・[0205](0205-release-and-publishing.md)
- #317・#531
- [Vercel Functions Limits](https://vercel.com/docs/functions/limitations)
- [Edge Runtime（Vercel）](https://vercel.com/docs/functions/runtimes/edge)・[Edge Runtime（Next.js）](https://nextjs.org/docs/app/api-reference/edge)

# 0259: npm の包みに Node.js の入口を足し、trap のあとのインスタンスを使えなくし、ライセンスの文を入れる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-03 |
| 状態 | 採用 |

## 背景

- 利用者は、最初の公開で、まず Web（Next.js のバックエンドなど）から使えることを求めた（#531）。
- 包みの README は、Node.js で wasm を `import.meta.resolve` で探して読む例を示していた（[0194](0194-typescript-javascript-wrapper.md)・[0205](0205-release-and-publishing.md)）。
- Next.js 16.4.4 で `npm pack` した包みを API ルートから使い、公開サンプルの DXF と DWG を読ませた。

  | wasm の読み込み方 | Turbopack | webpack（standalone） | `serverExternalPackages` あり |
  | --- | --- | --- | --- |
  | README の `import.meta.resolve` | 500 | 500 | 500 |
  | `process.cwd()` から `node_modules` を読む | 動く | ENOENT | 動く |
  | 包みの中で `new URL("../zumen_abi1.wasm", import.meta.url)` | 動く | 失敗 | 動く |

  - webpack は、包みの中の `new URL(…, import.meta.url)` を、ブラウザー向けの `/_next/` の URL に書き換え、standalone の配備にも wasm を入れない。
  - Edge のランタイムでは、`?module` で読み込んだモジュールが別の realm のもので、`load` の `instanceof WebAssembly.Module` が弾いた。Edge のランタイムは、Next.js 16.4 でも Vercel でも非推奨になった。
- wasm では、パニックとメモリ不足が trap になり、ホストがインスタンスを作り直す（[0010](0010-budget-by-work.md)、`crates/zumen-abi/src/ffi.rs`）。包みにはその扱いがなく、trap のあとも同じインスタンスを呼べた。
- wasm のメモリは縮まない。300 MB の入力を渡すと 602 MiB になり、セッションを閉じても戻らなかった。手元の Node.js では、コンパイルしたモジュールからインスタンスを作るのは 1 回 0.6 ms ほどだった。
- [0014](0014-platforms-signing-and-distribution.md) は、どの成果物にも依存のライセンスの文（`THIRD_PARTY_LICENSES`、cargo-about）を入れると決めたが、リリースのワークフローは作っていなかった。npm の包みには、MIT と Apache-2.0 の本文もなかった。wasm には、14 の第三者の crate が入る。

## 選択肢

### Node.js で wasm を読み込む形

1. **今のまま、利用者が wasm を探して読む。** bundler ごとに書き方が変わり、README の例は Next.js で動かない。
2. **`load` を、引数がなければ包みの wasm を読む形にする。** 入口が 1 つで済むが、ブラウザー向けの入口に `node:fs` が入り、ブラウザーの bundler が組めなくなる。
3. **Node.js の入口（`@zumen-jp/zumen/node`）を足し、包みの中から `new URL("../zumen_abi1.wasm", import.meta.url)` で読む。** Node.js はそのまま読み、Turbopack はこの形を辿って配備に入れる。webpack では、包みを `serverExternalPackages` に入れる設定が 1 行要る。
4. **wasm を base64 の文字列にして、JavaScript に埋め込む。** どの bundler でも設定は要らないが、包みが約 1.33 倍になり、読み込むたびに文字列の解析と写しで時間とメモリを使う。

### trap のあと

1. **包みは何もせず、利用者が `WebAssembly.RuntimeError` を見て作り直す。** 利用者が知らなければ、壊れたインスタンスを使い続ける。
2. **wasm の関数から例外が出たら、包みがインスタンスを使えないものにする。** その呼び出しと、それからの呼び出しを `panicked` の `ZumenError` にする。

## 決定

- Node.js で読み込む形は、選択肢 3 を採る。
  - `@zumen-jp/zumen/node` に `loadBundled()` を置く。コンパイルは 1 度だけにして覚え、呼ぶたびに新しいインスタンスを返す。読み込みに失敗したら覚えたものを捨て、次に呼んだときにやり直す。
  - `@zumen-jp/zumen/node` は、`@zumen-jp/zumen` の名前をすべて出し直す。
- trap のあとは、選択肢 2 を採る。
  - wasm の関数から例外が出たら（trap・スタックのあふれなど）、そのインスタンスを使えないものにし、`Zumen.broken` を `true` にする。
  - その呼び出しと、それからの呼び出しは、状態が `panicked` の `ZumenError` にし、元の例外を `cause` に入れる。
  - `Session.close` と `cancel` は、使えないインスタンスでは何もしない。`finally` の中で呼んでも、先に出た例外を隠さない。
  - 境界が `failed` などを返したときは、インスタンスは使えるまま。
- `load`・`loadSync` などは、型を `Object.prototype.toString` の名前で調べ、別の realm で作った `WebAssembly.Module`・`ArrayBuffer`・`Response` も受け取る。
- サーバーでは、要求ごとに `loadBundled` で新しいインスタンスを使うよう README で勧める。trap が要求の外へ及ばず、wasm のメモリも要求が終われば回収される。
- ライセンスの文:
  - リポジトリの直下の `about.toml` と `about.hbs` から、cargo-about 0.9.2 で `THIRD_PARTY_LICENSES` を作る。対象は `zumen-abi` の wasm32 の依存で、build と dev の依存と、このリポジトリの crate は載せない。許すライセンスは `deny.toml` と合わせる。
  - リリースのワークフローは、npm の包みに `zumen_abi1.wasm`・`THIRD_PARTY_LICENSES`・`LICENSE-MIT`・`LICENSE-APACHE` を入れ、GitHub の Release にも `THIRD_PARTY_LICENSES` を置く。
- 試験:
  - PR ごとに、Node.js の入口・別の realm のモジュール・trap のあとを、`bindings/js/test/server.mjs` で確かめる。
  - 毎晩、`bindings/js/test/nextjs` で、`npm pack` した包みを Next.js の Turbopack（設定なし）と webpack（`serverExternalPackages`）で standalone に組み、組んだサーバーの API ルートで図面を開く。Next.js・React の版は、その `package.json` で固定する。

## 影響

- Node.js の利用者は、wasm の道筋を書かずに済む。webpack で組む Next.js の利用者は、`serverExternalPackages` を書く。
- Edge のランタイムでも、`load` に `?module` のモジュールを渡せるが、試験はしない。
- Next.js の任意の依存の sharp は、LGPL の libvips を試験の環境に入れる。試験の道具にだけ使い、配る成果物には入らない（Playwright の WebKit と同じ扱い）。
- ほかの成果物（wheel・NuGet・CLI の圧縮・ネイティブのライブラリ）のライセンスの文は、#536 で入れる。cargo-about の版を上げるときは、`about.toml` の書き方の変化を確かめる（0.9 で ClearlyDefined の設定がなくなった）。
- 同期の呼び出しはイベントループを止めるので、大きな図面を受けるサーバーには `worker_threads` で動かす手順が要る（#532）。入力を wasm のメモリに 2 回写すことは #533、サーバーで画像を描く op は #534、分けた wasm のモジュールの配布は #535 で扱う。

## 参照

- [0010](0010-budget-by-work.md)・[0014](0014-platforms-signing-and-distribution.md)・[0194](0194-typescript-javascript-wrapper.md)・[0205](0205-release-and-publishing.md)
- #317・#531
- [Vercel Functions Limits](https://vercel.com/docs/functions/limitations)

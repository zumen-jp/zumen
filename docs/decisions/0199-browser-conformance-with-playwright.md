# 0199: TypeScript・JavaScript の包みの適合試験を、版を固定した Playwright でブラウザーの中で流す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- TS・JS の包みは、Node.js では適合試験に通っている。ブラウザーでの試験は、道具を選ぶときに足すことにしていた（[0194](0194-typescript-javascript-wrapper.md)）。
- [docs/support.md](../support.md) は、ブラウザーを PR ごとに Chromium、毎晩に Firefox と WebKit で確かめると約束している。
- 包みは実行時と開発の npm の依存を持たない（[0194](0194-typescript-javascript-wrapper.md)）。npm の道具は、使う場所ごとに `package.json` と `package-lock.json` で版を固定している（表記の検査は根、サイトは `site/`。[0021](0021-notation-and-terms.md)）。
- 適合試験の手順の読み方（`bindings/conformance/cases.mjs`）は、`node:fs` でファイルを読むので、そのままではブラウザーで読み込めない。

## 選択肢

### ブラウザーを動かす道具

1. **Playwright。** 1 つの API で Chromium・Firefox・WebKit を動かせる。ブラウザーは道具の版ごとに決まった build を取り、Linux の CI で WebKit も動く。npm の依存は `playwright` と `playwright-core` の 2 つで、どちらも Apache-2.0。
2. **WebdriverIO か Selenium。** 標準の WebDriver で動かせるが、Linux で WebKit を動かすには WebKitGTK の driver を別に用意する。依存も多い。
3. **Puppeteer。** Chromium と Firefox だけで、WebKit を動かせない。

### npm の依存の置き場所

1. **`bindings/js/package.json` の devDependencies。** 包みが開発の依存を持たない、という [0194](0194-typescript-javascript-wrapper.md) の決定と食い違う。
2. **根の `package.json`。** 表記の検査の道具の置き場所で、`npm ci` のたびにブラウザーの道具も入る。
3. **ブラウザーの試験のための `bindings/js/test/browser/package.json`。** サイトと同じく、使う場所に版を固定した依存を置く。

## 決定

道具は 1（Playwright）、置き場所は 3 を選ぶ。

- `bindings/js/test/browser/package.json`（`"private": true`）の devDependencies に `playwright` を版を固定して入れ（今は 1.64.1）、`package-lock.json` で `playwright-core` まで固定する。テスト・ランナー（`@playwright/test`）は使わず、ライブラリだけを使う。
- ブラウザーは Playwright の版に合うものを、CI が `npx playwright install --with-deps` で取る。リポジトリにも成果物にも入れない。道具を上げると、試すブラウザーの版も上がる。
- **試験の形**
  - 手順の読み方のうちファイルを読まない部分を `bindings/conformance/steps.mjs` に分け、`cases.mjs` はそれを出し直す。包みで組を流す本体を `bindings/js/test/run.mjs` に分け、Node.js の `conformance.mjs` とブラウザーの `browser/run.mjs` が共有する。ファイルの読み方と SHA-256 の計算は引数で渡す（ブラウザーでは `fetch` と `crypto.subtle`）。
  - `browser/run.mjs` は、Playwright が `http://localhost:24190` への要求を横取りして、包み・本体・公開サンプル・wasm だけを返す。サーバーは立てない。ページの中で wasm を `fetch` の応答から `load` で読み込んで組を流し、主スレッドで非同期にコンパイルしたモジュールで `loadSync` も試す。ハッシュの一覧は、ページの外で `hashes.txt` と比べる。
- **CI**
  - PR と main への push: `ci.yml` の wasm32 の job で、Node.js の試験のあとに Chromium（headless shell）で流す。
  - 毎晩: 新しい `nightly.yml`（`schedule` と `workflow_dispatch`、権限は `contents: read` だけ）で、Firefox と WebKit で流す。

## 影響

- Firefox と WebKit で落ちたことは、PR ではなく毎晩の結果で分かる。
- Playwright の WebKit と Firefox は、Playwright が手を入れて作ったもので、Safari と Firefox の製品そのものではない。下限の版（Safari 15・Firefox 79 など）も試さない。最新の build で、wasm の機能と包みが動くことを確かめる。
- Playwright の版を上げるときは、`package.json` と `package-lock.json` を一緒に直す。版は、公開から 2 週間たったものにする。
- 適合試験の手順の読み方を直すときは、ファイルを読まない部分を `steps.mjs` に置く。

## 参照

- [#354](https://github.com/zumen-jp/zumen/issues/354)、[#349](https://github.com/zumen-jp/zumen/issues/349)
- [0021](0021-notation-and-terms.md)、[0194](0194-typescript-javascript-wrapper.md)、[docs/support.md](../support.md)

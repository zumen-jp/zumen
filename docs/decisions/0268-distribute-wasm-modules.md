# 0268: 全体版と分割版の wasm を同じ npm パッケージと Release に入れ、ブラウザー向けに静的な URL の一覧を出す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-06 |
| 状態 | 採用 |

## 背景

[決定 0221](0221-wasm-modules.md) と [決定 0253](0253-wasm-civil-and-pointcloud-modules.md) は、ブラウザーが必要な機能だけを読み込めるように wasm を分けた。リリースのワークフローは全体版と 4 つの分割版を作るが、npm のパッケージと GitHub の Release へコピーする一覧は全体版だけだった。

`loadModules` は読み込む関数を受け、初めて必要になったモジュールを取得する。npm の中の wasm を bundler が出力へ移し、アプリからその URL を参照できる入口が要る（#535）。

## 選択肢

### 配布するパッケージ

1. **既存の `@zumen-jp/zumen` に全体版と 4 つの分割版を入れる。** インストールする量は増えるが、ラッパーと wasm のバージョンがそろい、取得するモジュールはブラウザー側で選べる。
2. **分割版ごとに npm のパッケージを作る。** インストールする量も選べるが、名前の確保と公開先が増え、ラッパーと各パッケージのバージョンを合わせる手順が要る。

### ブラウザーから wasm を指す方法

1. **ファイルの exports だけを足す。** 直接ファイルを取り出せるが、ブラウザー向けの URL の作り方は bundler ごとに利用者が調べる。
2. **exports と、パッケージの中の静的な `new URL(..., import.meta.url)` を出す。** ファイルの場所をパッケージ側にまとめ、既存の `load`・`loadModules` へ渡せる。

## 決定

- 配布は 1、URL は 2 を採る。全体版と `drawing`・`bim`・`civil`・`pointcloud` を同じ npm パッケージに入れ、同じ 5 ファイルを GitHub の Release に置く。プラグインの土台は利用者が独自のモジュールを作るためのもので、配布する一覧へは足さない。
- `package.json` の `files` と `exports` に 4 つの分割版を明示する。リリースの既存のコピー元の一覧にも同じ 4 つを足す。
- `@zumen-jp/zumen/wasm` に `wasmUrls` を出す。キーは `all`・`drawing`・`bim`・`civil`・`pointcloud`、値は URL の文字列とし、一覧は凍結する。静的な相対パスを `new URL` に渡し、ブラウザー向けの bundler が wasm を探して URL を書き換えられるようにする。import では取得もコンパイルもしない。
- `loadModules` の loader は、利用者が `() => fetch(wasmUrls.drawing)` などで渡す。キャッシュと再試行は既存の `loadModules` が担う。Node.js の `loadBundled` は全体版を読み続ける。
- ブラウザー向けの production build は Vite で確かめる。Next.js の既存のテストはサーバー向けで、ブラウザーが分割版を遅延取得することを試さない。Vite のバージョンはサイトの lockfile と同じ 8.3.5 に固定する。[公式のライセンス](https://github.com/vitejs/vite/blob/v8.3.5/LICENSE)は MIT。
- Vite は [決定 0199](0199-browser-conformance-with-playwright.md) の private な `bindings/js/test/browser` の devDependencies に置く。Playwright は既存の 1.64.1 を使う。公開するラッパーや Rust の依存には入らず、wasm の実行や決定性には関わらない。
- Vite の依存の [Lightning CSS は MPL-2.0](https://github.com/parcel-bundler/lightningcss/blob/v1.33.0/LICENSE) である。テストのアプリを組むツールとしてだけ入れ、配る wasm・npm のパッケージには入れない。これは [決定 0259](0259-javascript-on-node-servers.md) の Next.js の検査用の依存と同じ扱いにする。Vite が求めるため、この検査の Node.js は 22.12 以降にする。ラッパーの下限は 22 のまま。
- PR ごとの CI で、`npm pack` したパッケージを一時のアプリへ入れて Vite で組み、出力だけを Chromium に返す。5 つのファイルの exports・内容・URL、ルート以外の配信先、必要になるまで取得しないこと、各モジュールの op と再利用を確かめる。

## 影響

- npm のインストールと bundler の出力に 5 つの wasm が入る。ブラウザーで取得してコンパイルする量は、呼ぶ loader で決まる。インストール時の量も減らす必要が生じたら、パッケージを分ける判断を別に行う。
- この URL の入口はブラウザー向けで、Vite の SSR は対象にしない（[公式の説明](https://vite.dev/guide/assets.html#new-url-url-import-meta-url)）。Node.js のサーバーは `@zumen-jp/zumen/node` を使える。ファイルの exports も残すので、独自の配信先や読み込み方を選べる。
- 新しい npm の名前、公開先、認証、ワークフローの権限・環境・発火条件は増えない。公開 job の変更は既存のモジュールの一覧への追加に限る。

## 参照

- #535・#531
- [決定 0205](0205-release-and-publishing.md)・[決定 0221](0221-wasm-modules.md)・[決定 0253](0253-wasm-civil-and-pointcloud-modules.md)・[決定 0259](0259-javascript-on-node-servers.md)

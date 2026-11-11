# zumen.dev のサイト

<https://zumen.dev> に置く案内のサイトです。[Astro](https://astro.build/) で作り、静的な HTML として公開します。図面を手元で開く `/viewer/` だけで JavaScript を動かします。

## 見た目の考え方

役所・自治体・企業の担当者が見慣れた、[デジタル庁デザインシステム](https://design.digital.go.jp/)（DADS）に沿って組みます。

- 色と角丸は、DADS のトークン（`@digital-go-jp/design-tokens`、MIT License）の変数を使う。主の色は青（key-900）、文字は濃い灰色にする。
- 文字は Noto Sans JP を、このサイトから配る（`@fontsource-variable/noto-sans-jp`、SIL Open Font License）。本文は 16px、行の高さは 1.7 にする。
- ボタン・リンク・フォーカス・お知らせの帯・表・開け閉めする質問は、DADS の仕様に合わせて `src/styles/global.css` に書く。DADS の部品の CSS は写さない。
- 並びは、日本の企業・製品のサイトの定番に合わせる。トップにメインビジュアル・お知らせ・3 つの特長・早見表・よくある質問を置き、下の階層ではパンくずを見出しの上に置く。
- メインビジュアルは、通り芯と寸法を入れた平面図にする。
- デジタル庁のロゴや名前は飾りに使わない。
- アクセス解析・クッキーを使わない。

## 手元で見る

Node.js 22.23.4 以降を使います（Astro 7.4 の下限）。CI は Node.js 24 で確かめます。

```bash
cd site
npm ci
npm run dev
```

<http://localhost:24150/> を開きます。`npm run check` で型を、`npm run build` で `site/dist` への出力を確かめます。CI の「サイト」でも同じことを確かめます。

`/viewer/` も見るときは、先に `npm run build` を流し、`npm run preview` で開きます。Astro の開発サーバーでは、別に組むビューアーは表示しません。

## 図面ビューアー

`npm run build` は、案内の HTML のあと、既存の `playground/` を Vite で `dist/viewer/` に組みます。wasm とラッパーは npm の `@zumen-jp/zumen@0.2.0` から取り、非公開の表示部品は `packages/` を使います。Rust と手元の `target/` は使いません。

- wasm・Worker・CSS・公開サンプルは、同一オリジンから配ります。CSP は `public/_headers` にあり、ページと Worker の外部への通信を制限します。案内ページは JavaScript を読み込みません。これをビルドの出力で毎回検査します。
- `Cache-Control` の `no-transform` で、配信側による解析スクリプトなどの自動注入を止めます。`max-age=0, must-revalidate` で更新時の再検証も保ちます。これは [Cloudflare Web Analytics の公式の方法](https://developers.cloudflare.com/web-analytics/get-started/)で、サイトのビルドの出力に含めます。本番で注入を見つけたら、CSP や通信検査を緩めずに調べます。
- 案内ページと 404 ページは、HTML の charset の直後に meta CSP を置き、スクリプトと `fetch`・解析 POST などの通信を拒否します。CSS・フォント・画像と通常のリンクは使えます。[CSP3 の 3.3 節](https://www.w3.org/TR/CSP3/#meta-element)に従い、資産を参照する `link` より前にあることもビルドで検査します。meta より前の資産の先読みまで防ぐものではありません。
- [Pages は見つからないパスに `404.html` を返します](https://developers.cloudflare.com/pages/configuration/serving-pages/)。2026-11-10 の実配信では、この 404 応答に `_headers` の CSP と `no-transform` が付かず、解析の script タグが加わりました。404 で注入がなくなったとは扱わず、meta CSP で読み込みと実行を拒否します。ヘッダーの扱いは観測した結果であり、Pages の仕様として断定しません。
- サンプルの出所・ライセンス・必要な本文は `viewer/samples.md` に、JavaScript に組み込む依存のライセンスは `viewer/licenses.md` に出します。公開済みでもログイン名を含む 2 件の DWG はサイトに入れません。
- wasm に入る依存の本文は `viewer/THIRD_PARTY_LICENSES` に、コアのライセンスは `viewer/LICENSE-MIT`・`viewer/LICENSE-APACHE` に、SBOM は `viewer/sboms/` に保ちます。出力した wasm のハッシュと SBOM の一致も検査します。
- 図面は、公開サンプルか、利用者が手元で選ぶファイルをブラウザーの中だけで読みます。不具合と非公開の報告先もビューアーに案内します。
- `npm run preview` は `_headers` を HTTP ヘッダーにはしません。CSP の検査では、検証用のサーバーで同じヘッダーを付けてブラウザーを動かし、公開後には実際のレスポンスでも確認します。

ビルド後に `npx playwright install chromium` と `npm run test:browser` を流すと、CSP を付けたローカルのサーバー（24189）で確かめます。Windows の Edge なら、`npm run test:browser -- msedge` でも確かめられます。公開後は `npm run test:browser -- msedge https://zumen.dev` のように URL を渡せます。使う入力は公開サンプルと合成データだけで、`ZUMEN_SAMPLES` は読みません。予期しない通信は送信前に止め、検査の失敗として数えます。

同じブラウザー検査で、全案内ページと存在しないパスへの 404 も開きます。ローカルでは HTTP の CSP がない 404 に、インライン・外部の通常のスクリプト・モジュールのスクリプトを加えて配ります。各ページで同一オリジンのスクリプトと解析 POST の拒否も確かめます。CSP で止まった要求は Playwright の `request` イベントに現れることがあるため、送信前の `route` に届かなかったことと CSP の違反イベントで判定します。実配信でも同じ検査を使い、404 の HTML に残る script タグの数と、通信の有無を分けて記録します。

サイトの検査は `site/package.json` の Playwright 1.65.0 を使い、CI では Chromium、毎晩は Firefox・WebKit で流します。JS・React・View2D の `bindings/js/test/browser` の検査環境とは分けています。

## 置き場所

| 場所 | 中身 |
| --- | --- |
| `src/pages/` | ページ。ファイル名が URL になる |
| `src/layouts/Base.astro` | ヘッダー・ナビ・開発中の知らせの帯・フッター |
| `src/components/` | 見出し・お知らせ・対応形式の表・よくある質問などの部品 |
| `src/data/site.ts` | お知らせ・対応形式・ホストアプリ・よくある質問の中身 |
| `src/styles/global.css` | 見た目 |

## 公開

Cloudflare Pages で、このリポジトリの `main` から公開する想定です。

- ルートのディレクトリ: `site`
- ビルドのコマンド: `npm ci && npm run build`
- 出力のディレクトリ: `dist`

## 直すとき

- 文は [表記の基準](../docs/style.md) に従い、「です・ます」で書く。
- お知らせ・対応形式は `src/data/site.ts` を直す。お知らせには、リリースと大きな節目だけを書く。対応形式は、[形式と権利の一覧](../docs/formats/README.md) と食い違わないようにする。
- 公開版の案内は `releaseVersion` と公開レジストリの実体で確かめます。対応形式と使い方のリンクは同じバージョンのタグに固定し、main だけの機能を混ぜません。OS の対象の下限と配布物の制約も区別します。v0.2.0 の GNU Linux の制約と修正は [#789](https://github.com/zumen-jp/zumen/issues/789) にあります。
- 未公開のものを「使える」と書かない。

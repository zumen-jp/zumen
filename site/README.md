# zumen.dev のサイト

<https://zumen.dev> に置く案内のサイトです。[Astro](https://astro.build/) で作り、静的な HTML として公開します。図面を手元で開く `/viewer/` だけで JavaScript を動かします（[決定 0296](../docs/decisions/0296-site-viewer-and-content-security-policy.md)）。

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
- サンプルの出所・ライセンス・必要な本文は `viewer/samples.md` に、JavaScript に組み込む依存のライセンスは `viewer/licenses.md` に出します。公開済みでもログイン名を含む 2 件の DWG はサイトに入れません。
- wasm に入る依存の本文は `viewer/THIRD_PARTY_LICENSES` に、コアのライセンスは `viewer/LICENSE-MIT`・`viewer/LICENSE-APACHE` に、SBOM は `viewer/sboms/` に保ちます。出力した wasm のハッシュと SBOM の一致も検査します。
- 図面は、公開サンプルか、利用者が手元で選ぶファイルをブラウザーの中だけで読みます。不具合と非公開の報告先もビューアーに案内します。
- `npm run preview` は `_headers` を HTTP ヘッダーにはしません。CSP の検査では、検証用のサーバーで同じヘッダーを付けてブラウザーを動かし、公開後には実際のレスポンスでも確認します。

ビルド後に `npx playwright install chromium` と `npm run test:browser` を流すと、CSP を付けたローカルのサーバー（24189）で確かめます。Windows の Edge なら、`npm run test:browser -- msedge` でも確かめられます。公開後は `npm run test:browser -- msedge https://zumen.dev` のように URL を渡せます。使う入力は公開サンプルと合成データだけで、`ZUMEN_SAMPLES` は読みません。予期しない通信は送信前に止め、検査の失敗として数えます。

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
- 未公開のものを「使える」と書かない。

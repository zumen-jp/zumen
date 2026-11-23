# zumen.dev のサイト

<https://zumen.dev> に置く案内のサイトです。[Astro](https://astro.build/) で作り、JavaScript を読み込まない静的な HTML として公開します。ブラウザーで図面を試す入口は、別リポジトリで公開している [zumen-editor-web](https://editor.zumen.dev/) へ案内します。

## 見た目の考え方

役所・自治体・企業の担当者が見慣れた、[デジタル庁デザインシステム](https://design.digital.go.jp/)（DADS）に沿って組みます。

- 色と角丸は、DADS のトークン（`@digital-go-jp/design-tokens`、MIT License）の変数を使う。主の色は青（key-900）、文字は濃い灰色にする。
- 文字は Noto Sans JP を、このサイトから配る（`@fontsource-variable/noto-sans-jp`、SIL Open Font License）。本文は 16px、行の高さは 1.75 にする。
- ボタン・リンク・フォーカス・お知らせの帯・表・開け閉めする質問は、DADS の仕様に合わせて `src/styles/global.css` に書く。DADS の部品の CSS は写さない。
- 並びは、日本の企業・製品のサイトの定番に合わせる。トップにメインビジュアル・3 つの入口（zumen-editor-web・AI プラグイン・zumen-editor 先行版）・30 秒の紹介動画・お知らせ・3 つの特長・早見表・ホストアプリ・よくある質問を置き、下の階層ではパンくずを見出しの上に置く。
- メインビジュアルは、通り芯と寸法を入れた平面図にする。平面図と下の階層の図面は、共通の二重枠と表題欄でまとめる。
- 下の階層の見出しは方眼の地にし、ページの中の節へのリンクと図面を添える。図面は、トップの平面図と同じ描き方で、zumen-editor 構成図（zumen-editor）・AI 連携系統図（AI プラグイン）・立面図（特長）・縦断図（対応形式）・配置図（対応環境）・工程表（お知らせ）・擁壁の断面図（よくある質問）を描く。飾りなので、狭い画面と印刷では出さない。
- 下の階層のページの終わりには、ほかのページへの案内と GitHub への誘いを置く。
- デジタル庁のロゴや名前は飾りに使わない。
- アクセス解析・クッキーを使わない。

## 手元で見る

Node.js 24 以降を使います。

```bash
cd site
npm ci
npm run dev
```

<http://localhost:24150/> を開きます。`npm run check` で型を、`npm run build` で `site/dist` への出力を確かめます。CI の「サイト」でも同じことを確かめます。

## セキュリティと転送設定

- すべての案内ページは JavaScript を読み込まず、ビルドの出力で毎回検査します。
- `Cache-Control` の `no-transform` で、配信側による解析スクリプトなどの自動注入を止めます。`max-age=0, must-revalidate` で更新時の再検証も保ちます。これは [Cloudflare Web Analytics の公式の方法](https://developers.cloudflare.com/web-analytics/get-started/)で、サイトのビルドの出力に含めます。本番で注入を見つけたら、CSP や通信検査を緩めずに調べます。
- 案内ページと 404 ページは、HTML の charset の直後に meta CSP を置き、スクリプトと `fetch`・解析 POST などの通信を拒否します。CSS・フォント・画像と通常のリンクは使えます。[CSP3 の 3.3 節](https://www.w3.org/TR/CSP3/#meta-element)に従い、資産を参照する `link` より前にあることもビルドで検査します。meta より前の資産の先読みまで防ぐものではありません。
- [Pages は見つからないパスに `404.html` を返します](https://developers.cloudflare.com/pages/configuration/serving-pages/)。2026-11-10 の実配信では、この 404 応答に `_headers` の CSP と `no-transform` が付かず、解析の script タグが加わりました。404 で注入がなくなったとは扱わず、meta CSP で読み込みと実行を拒否します。ヘッダーの扱いは観測した結果であり、Pages の仕様として断定しません。
- 以前の簡易ビューアー（`/viewer/`）は廃止し、`public/_redirects` で <https://editor.zumen.dev/> へ 301 転送します。
- `npm run preview` は `_headers` や `_redirects` を再現しません。CSP と転送の検査では、検証用のサーバーで同じヘッダーと転送を付けてブラウザーを動かし、公開後には実際のレスポンスでも確認します。

ビルド後に `npx playwright install chromium` と `npm run test:browser` を流すと、ヘッダーと転送を付けたローカルのサーバー（24189）で全案内ページ・404・CSP による通信の拒否を確かめます。Windows の Edge なら、`npm run test:browser -- msedge` でも確かめられます。公開後は `npm run test:browser -- msedge https://zumen.dev` のように URL を渡せます。予期しない通信は送信前に止め、検査の失敗として数えます。

サイトの検査は `site/package.json` の Playwright 1.65.0 を使い、CI では Chromium、毎晩は Firefox・WebKit で流します。JS・React・View2D の `bindings/js/test/browser` の検査環境とは分けています。

## 置き場所

| 場所 | 中身 |
| --- | --- |
| `src/pages/` | ページ。ファイル名が URL になる |
| `src/layouts/Base.astro` | ヘッダー・ナビ・知らせの帯・フッター |
| `src/layouts/Page.astro` | 下の階層のページの枠。見出し・ほかのページへの案内・GitHub への誘い |
| `src/components/` | 見出し・お知らせ・対応形式の表・よくある質問などの部品 |
| `src/components/drawings/` | 下の階層の見出しに添える図面 |
| `src/data/site.ts` | ページの名前と説明・お知らせ・対応形式・ホストアプリ・よくある質問の中身 |
| `src/styles/global.css` | 見た目 |

## 公開

Cloudflare Pages で、このリポジトリの `main` から公開する想定です。

- ルートのディレクトリ: `site`
- ビルドのコマンド: `npm ci && npm run build`
- 出力のディレクトリ: `dist`

Windows 11 x64 用 zumen-editor の配布先は [配布専用リポジトリ](https://github.com/zumen-jp/zumen-editor-releases/releases) です。更新時は MSI と SHA-256 を公開し、`src/data/site.ts` の `editorVersion`・`editorBytes`・`editorSha256` をその版に合わせます。

## 直すとき

- 文は [表記の基準](../docs/style.md) に従い、「です・ます」で書く。
- お知らせ・対応形式は `src/data/site.ts` を直す。お知らせには、リリースと大きな節目だけを書く。対応形式は、[形式と権利の一覧](../docs/formats/README.md) と食い違わないようにする。
- 公開版の案内は `releaseVersion` と公開レジストリの実体で確かめます。対応形式と使い方のリンクは同じバージョンのタグに固定し、main だけの機能を混ぜません。OS の対象の下限と配布物の制約も区別します。v0.2.0 の GNU Linux の制約と、v0.3.0 での修正は [#789](https://github.com/zumen-jp/zumen/issues/789) にあります。
- 公開版を切り替えるときは、対象の Git タグを取得し、`site` で `node scripts/check-formats.mjs --tag` を実行します。対応表の各行を同じ版の形式台帳と照らし、公開 npm パッケージで ST-Bridge の読み取りの可否を調べます。さらに線分 1 本の DXF を表示している各出力先に変換し、別のセッションで開いて両端の座標を照合します。配布物の検査はサイトのビルドでも実行します。
- 未公開のものを「使える」と書かない。

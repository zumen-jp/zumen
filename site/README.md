# zumen.dev のサイト

<https://zumen.dev> に置く案内のサイトです。[Astro](https://astro.build/) で作り、静的な HTML として公開します。ページの中で JavaScript を動かしません。

## 見た目の考え方

役所・自治体・企業の担当者が見慣れた、[デジタル庁デザインシステム](https://design.digital.go.jp/)（DADS）に沿って組みます。

- 色と角丸は、DADS のトークン（`@digital-go-jp/design-tokens`、MIT License）の変数を使う。主の色は青（key-900）、文字は濃い灰色にする。
- 文字は Noto Sans JP を、このサイトから配る（`@fontsource-variable/noto-sans-jp`、SIL Open Font License）。本文は 16px、行の高さは 1.7 にする。
- ボタン・リンク・フォーカス・お知らせの帯・表・開け閉めする質問は、DADS の仕様に合わせて `src/styles/global.css` に書く。DADS の部品の CSS は写さない。
- 並びは、日本の企業・製品のサイトの定番に合わせる。トップにメインビジュアル・お知らせ・3 つの特長・早見表・よくある質問を置き、下の階層ではパンくずを見出しの上に置く。
- メインビジュアルは、通り芯と寸法を入れた平面図にする。
- 国や自治体のサイトと間違えられないように、デジタル庁のロゴや名前を飾りに使わず、フッターに「国や自治体の事業ではない」と書く。
- アクセス解析・クッキーを使わない。

## 手元で見る

Node.js 22.23.4 以降を使います（Astro 7.4 の下限）。CI は Node.js 24 で確かめます。

```bash
cd site
npm ci
npm run dev
```

<http://localhost:24150/> を開きます。`npm run check` で型を、`npm run build` で `site/dist` への出力を確かめます。CI の「サイト」でも同じことを確かめます。

## 置き場所

| 場所 | 中身 |
| --- | --- |
| `src/pages/` | ページ。ファイル名が URL になる |
| `src/layouts/Base.astro` | ヘッダー・ナビ・開発中のお知らせの帯・フッター |
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
- お知らせ・対応形式は `src/data/site.ts` を直す。対応形式は、[形式と権利の一覧](../docs/formats/README.md) と食い違わないようにする。
- 未公開のものを「使える」と書かない。

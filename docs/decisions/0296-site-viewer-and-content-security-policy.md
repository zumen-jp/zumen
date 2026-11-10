# 0296: サイトの viewer だけで JavaScript を動かし、公開済みの wasm と共通のプレイグラウンドを同一オリジンから配る

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-10 |
| 状態 | 採用 |

## 背景

サイトは静的な案内で、ページの中で JavaScript を動かしていない。#691 は、図面をブラウザーの中だけで開くデモを `zumen.dev/viewer/` に置く。表示部品とプレイグラウンドは、決定 0267・0268 の構成ですでにある。Cloudflare Pages のビルドの設定を変えず、公開済みの npm 0.2.0 を使う。

## 選択肢

1. サイトに別のビューアーを作り、Rust もビルドする。表示のコードと検査が重なり、Pages の環境にも Rust が要る。
2. 公開済みの wasm を npm から取り、サイトのビルドで既存の `playground/` を組む。案内は Astro の静的な HTML のままとし、`/viewer/` だけが JavaScript を使う。
3. 別のオリジンにプレイグラウンドを置き、サイトからリンクする。ビルドは分かれるが、公開先と管理する設定が増える。

## 決定

2 を採る。表示のコードを共用でき、既存の Pages のビルドの設定（`site` で `npm ci && npm run build`、出力 `dist`）で配れるためである。

- `site/package.json` と lockfile で `@zumen-jp/zumen` を公開済みの `0.2.0` に固定する。`drawing` の wasm とラッパーは、このパッケージから同じバージョンを使う。ブラウザーが CDN やレジストリへ取りに行く形にはしない。
- wasm に入る依存の `THIRD_PARTY_LICENSES`、コアのライセンス、`drawing` の SBOM を同じパッケージから写す（決定 0269・0279）。出力した wasm の SHA-256 が SBOM と合うことも、ビルドのたびに確かめる。
- `@zumen-jp/react`・`@zumen-jp/view2d` は、非公開のままリポジトリのコードを使う。React・React DOM と Vite は、既存のプレイグラウンドと同じバージョン（19.3.1・8.3.5）を使う。React・React DOM・Vite は MIT License で、Rust の依存は増えない。Vite のツールの依存の扱いは決定 0268 と同じにする。Vite が生成した JavaScript に入った依存のライセンスを `viewer/licenses.md` に出す。
- Astro のビルドのあと、`playground/` の同じエントリ・CSS・Worker を Vite で `dist/viewer` に組む。サイト用の解決先は公開済みの npm パッケージに固定し、隣にある `bindings/js` や開発用の wasm の有無に依存しない。サイトのビルドで Rust は起動しない。
- JavaScript を動かす範囲は `/viewer/` だけにする。ビルドのたびに、ほかのページの HTML に `script` がないことと、ビューアーの wasm・Worker・CSS・ライセンスの出力を検査する。
- CI のサイトの job でもブラウザーの検査を流す。Playwright は既存の決定 0199 と同じ 1.64.1（Apache-2.0）を開発用の依存に置き、配る JavaScript には入れない。
- [Cloudflare Pages の `_headers`](https://developers.cloudflare.com/pages/configuration/headers/) で `/viewer/*` に CSP を付ける。Worker の JavaScript にも同じ規則を付ける。`default-src 'none'` を起点に、スクリプト・Worker・通信・CSS・画像を同一オリジンの `'self'` に限る。wasm の実行には `script-src` の `'wasm-unsafe-eval'` を使い、JavaScript の `eval` とインラインのスクリプトは許さない（[CSP の script-src](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/script-src)）。`base-uri`・`form-action`・`frame-ancestors`・`object-src` は `'none'` にする。外部のフォント・解析・クッキーは使わない。
- 通常のリンクでのページの移動と、手元でのダウンロードは残す。CSP は同一オリジンの通信を許すので、CSP だけで図面が送信されないことを証明したとはしない。図面を開く操作と外部参照を持つ図面をブラウザーで流し、ページと Worker の要求が同一オリジンの資産への GET だけであることも確かめる。
- 出所とライセンスは共通の台帳から生成する。サイトに同梱する DWG は、対になる DXF の `$LASTSAVEDBY` が空の R2000 の用紙のサンプルに絞る。公開済みでもログイン名を含む R2004 の格子の配置と R2018 の切り抜きのサンプルは、サイトには入れない。元の公開サンプルは変えない。
- 画面に使っているコアのバージョンを示す。公開済みの 0.2.0 は、公開サンプルの ANSI_1252 の文字を扱わない。サンプルは残し、要素がなければ表示できる要素がないと知らせ、`dxf.code-page.unsupported` の診断を示す。対応は、使っているバージョンの診断として示す。
- 不具合の知らせ方をビューアーに置く。受け取った図面や顧客の情報を公開の issue に貼らないことと、落ちる・終わらない・メモリを使いすぎる入力の非公開の報告先を、決定 0283 と `SECURITY.md` に沿って案内する。

## 影響

サイトのインストールに、公開済みの wasm と React の依存が加わる。コアのビルドと独立しているため、コアを変えた PR でも、サイトは明示して更新するまで 0.2.0 を使う。更新するときは、公開済みのバージョンを lockfile とともに替え、表示と通信を確かめる。

Pages の設定・公開先・認証・ワークフローの権限は変えない。`main` の既存の公開でビューアーも配る。公開後は実際のレスポンスの CSP と、公開サンプル・手元で選ぶ合成の図面での通信を別に確認する。

## 参照

- #691・#687・#689・#690
- [決定 0267](0267-view2d-on-scene-columns.md)・[0268](0268-distribute-wasm-modules.md)・[0283](0283-fuzz-reporting-after-release.md)

# 0263: 文書だけを変えた PR では fuzz を流さず、種や実装に使う Markdown はパスを明示する

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-05 |
| 状態 | 採用 |

## 背景

[0258](0258-fuzz-seeds-and-runs.md) は、文書だけの PR では fuzz を流さないとした。一方、決定のパス一覧は `crates/**`・`fuzz/**`・`samples/**` を含むので、その中の README だけを変えても流れる（#556）。[0261](0261-fuzz-outside-required-checks.md) に従って結果を待つ必要もある。

`cargo xtask fuzz-seeds` は、台帳に載せた公開サンプル、適合テストの JSON の合成の入力と要求、Rust で作る攻撃の形と格子を読む。Markdown は含まれていない。crate・xtask・fuzz の `include_str!`・`include_bytes!` にも、Markdown を読むものはない。

## 選択肢

1. 今のパスを保つ。種や実装の変更を拾えるが、文書だけの PR でも実行を待つ。
2. 今のパスから Markdown を外す。コードと種の変更は引き続き拾い、文書だけの変更を省く。
3. 実行するファイルの拡張子を列挙する。設定や新しい形式のサンプルを追加するとき、一覧への追加を忘れると流れない。

## 決定

選択肢 2 にする。0258 の PR のパス一覧を、次のように補う。

- `.github/workflows/fuzz.yml` の `pull_request.paths` の末尾に `!**/*.md` を置く。先に含めたパスのうち Markdown を除く。
- コード・種の元・target・ワークフローなど、Markdown 以外の既存の対象は変えない。文書とコードを一緒に変えた PR でも流す。
- 今後、種や実装に Markdown を使うときは、そのファイルのパスを除外条件の後ろに足し、実行の対象に戻す。Markdown を読むコードか台帳を足す PR で、一緒に確かめる。
- 毎晩と手で流す実行、実行する秒数と target、権限は変えない。

## 影響

- `fuzz/README.md`・crate の README・公開サンプルの説明だけを直す PR では、fuzz を待たずに済む。文書の表記の検査は CI で続ける。
- 実行した fuzz の結果を確かめてからマージする条件（0261）は変わらない。
- 0258 は元の記録を残し、この決定へのリンクを足す。

## 参照

- #556、[0258](0258-fuzz-seeds-and-runs.md)・[0261](0261-fuzz-outside-required-checks.md)
- [GitHub Actions のパス条件](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#onpushpull_requestpull_request_targetpathspaths-ignore)
- `xtask/src/fuzz.rs`・`fuzz/README.md`・`.github/workflows/fuzz.yml`

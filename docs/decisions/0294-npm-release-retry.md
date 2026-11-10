# 0294: npm の公開だけを、元のタグと配布物を検証して main のワークフローから再試行する

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-10 |
| 状態 | 採用 |

## 背景

[決定 0205](0205-release-and-publishing.md) は、タグを push して各レジストリに公開し、手動実行では公開しないとした。`v0.2.0` の公開では、GitHub の Release・PyPI・NuGet は成功したが、npm は `npm publish packages/*.tgz` の引数が Git のリポジトリの省略形として扱われて失敗した（#767）。ローカルのパッケージを渡すには、`./packages/` から始める必要がある。

`v0.2.0` は、コミット `ce01f02dc6725914f43627b3cdb353007dbd83dc` に付けて公開済みである。タグを移さず、公開済みの配布物を使って npm だけを回復したい。元の実行を再実行しても、元のコミットとワークフローを使うので、main の修正は反映されない。

利用者は 2026-11-10 に、回復の方針として `CIで再試行できるように直す（provenanceを保つ）` を選んだ。方針の選択と、具体的なワークフローの差分・GitHub の設定変更の承認は分ける。この決定に基づく変更は、下の承認の手順を満たしてから適用する。

## 選択肢

1. 同じ `release.yml` に、npm だけを再試行する手動入力と job を足す。Release にあるパッケージと元の attestation を検証し、npm の trusted publishing で公開する。
2. npm の再試行だけを行う別のワークフローを作る。npm の trusted publisher に、別のワークフローのファイル名を登録する必要がある。
3. main の修正したワークフローで、元のタグを再ビルドする。配布済みのパッケージと同じものかをあらためて確かめる必要があり、今回の回復には不要である。

## 決定

選択肢 1 を採る。[決定 0205](0205-release-and-publishing.md) の「手で走らせたワークフローは出さない」に、ここで定める npm の再試行だけの例外を設ける。

### 実行の経路

- `release.yml` の `workflow_dispatch` に、空文字を既定とする `npm_retry_tag` を足す。
- 入力が空なら、作るところと `cargo publish --dry-run` までを確かめ、公開しない。手動でタグを指定して実行した場合も同じにする。
- 通常の公開を担う 5 つの job（`release`・`crates-io`・`npm`・`pypi`・`nuget`）は、`push` でタグを受けた場合だけ動く。通常の npm の公開も、パッケージのパスを `./packages/` から始める。
- 入力が空でない場合は、通常のバージョン確認・ビルド・梱包・公開を動かさず、`npm-retry` だけを動かす。`main` 以外からの再試行は断る。
- 再試行も、同じ `release.yml` と `release` 環境を使う。GitHub-hosted runner で動かし、job の権限は `contents: read`・`attestations: read`・`id-token: write` にする。

### 配布物の検証

再試行を許す組は、Python のスクリプトに固定する。今回は次の 1 組だけとし、ほかの組を足すときも、差分をレビューする。

| 項目 | 値 |
| --- | --- |
| タグ | `v0.2.0` |
| コミット | `ce01f02dc6725914f43627b3cdb353007dbd83dc` |
| Release の asset | `zumen-jp-zumen-0.2.0.tgz` |
| SHA-256 | `da0e5b5701bd06b6f023167aa763e082b1e6d7c76a7765168c38a24f2b2bcaa0` |

- 正規のリポジトリ `zumen-jp/zumen` のタグを読み、注釈付きのタグなら参照先をたどったコミットが、固定したものと同じことを確かめる。
- そのタグの Release が下書きでもプレリリースでもないことと、asset の名前と digest を確かめる。取得したファイルの SHA-256 も、固定した値と照らす。
- tarball を展開せずに `package/package.json` を読み、名前が `@zumen-jp/zumen`、バージョンが `0.2.0`、リポジトリが `zumen-jp/zumen` であることを確かめる。パッケージのコードやスクリプトは実行しない。
- `gh attestation verify` で、ファイルと元の attestation を検証する。`--source-ref` は `refs/tags/v0.2.0`、`--source-digest` と `--signer-digest` は固定したコミット、`--signer-workflow` は `zumen-jp/zumen/.github/workflows/release.yml` にする。`--deny-self-hosted-runners` も指定する。
- npm に同じバージョンがすでにあれば断る。通信などの失敗を、バージョンがないこととみなさない。
- 検証した単一のファイルを、`npm publish ./packages/zumen-jp-zumen-0.2.0.tgz --ignore-scripts --access public --provenance` で公開する。

### 来歴の証明

npm が自動生成する provenance は、再試行した main のワークフローと、そのコミットを参照する。元のタグを参照する証明とはしない。パッケージが元のタグの配布物と同じことは、固定した digest と、元の GitHub attestation の検証で確かめる。この 2 つの証明を分けて記録する。

既存の GitHub attestation は、npm が求める package の purl と SHA-512 を持つ単一の subject ではないので、`--provenance-file` として流用しない。来歴を示すために、`GITHUB_SHA` などの実行環境の値を書き換えることもしない。

### 承認と環境の設定

2026-11-10 に読み取った `release` 環境の設定では、`v*` のタグだけが公開元として許されている。承認後に、`main` ブランチの許可を 1 件足す。既存のタグの許可・承認者・ほかの保護の規則は維持する。

次は、[決定 0254](0254-parallel-agents-and-cross-review.md) と `AGENTS.md` の「人を待つもの」に当たる。

- 公開 job の手順と、手動入力による公開の発火条件を変えること。
- GitHub の環境の設定に、`main` ブランチの許可を足すこと。
- 利用者が決めた、決定 0205 の手動実行の方針に例外を設けること。
- npm にパッケージを公開すること。

push の前に、ブランチ・push するコミット・差分と、上の各項の中身を示し、利用者に会話で認めてもらう。承認前にそのブランチのワークフローを手で実行せず、GitHub の設定も変えない。承認の記録は、認めた日・head・範囲・利用者の言葉とともに PR に書く。

## 影響

- 公開済みのタグと配布物を保ったまま、npm の失敗だけを再試行できる。GitHub の Release と、ほかのレジストリには出し直さない。
- 入力なしの手動実行は、タグを指定した場合も、公開せずに検査する経路になる。
- 再試行の入力と、検証するタグ・コミット・asset・digest の不一致、npm に存在するバージョンを断るテストを行う。公開しない検査と、承認後に行う実際の公開の結果を分けて記録する。

## 参照

- [決定 0205](0205-release-and-publishing.md)・[決定 0254](0254-parallel-agents-and-cross-review.md)
- [npm の trusted publishing](https://docs.npmjs.com/trusted-publishers/)
- [npm の provenance](https://docs.npmjs.com/generating-provenance-statements/)
- [npm の provenance の生成と検証の実装](https://github.com/npm/cli/blob/latest/workspaces/libnpmpublish/lib/provenance.js)
- [GitHub Actions の再実行](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)
- [`gh attestation verify`](https://cli.github.com/manual/gh_attestation_verify)
- #767

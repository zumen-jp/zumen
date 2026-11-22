# zumen で図面を操作する

zumen を AI エージェントから使うと、CAD 図面のレイヤや文字を調べ、図面の一部を画像で確認し、指定した図形を編集して別名で保存できます。利用できる操作は[対応形式](https://zumen.dev/formats/)によります。

GitHub から導入する場合は Node.js 24 以降と `tar` を使います。初回に [zumen v0.3.0 の公式 CLI](https://github.com/zumen-jp/zumen/releases/tag/v0.3.0) を取得し、圧縮ファイルと実行ファイルの SHA-256 を照合します。次回からは利用者のホームにある `.cache/zumen/plugins` の同じ版を使います。デスクトップ用の配布ファイルには CLI を同梱します。

## Claude Code

```sh
claude plugin marketplace add zumen-jp/zumen
claude plugin install zumen@zumen-jp
```

Claude Code で `/plugin configure zumen@zumen-jp` を開き、図面を読み書きするフォルダーを選んでください。`/mcp` で接続を確認できます。

## Codex

```sh
codex plugin marketplace add zumen-jp/zumen
codex plugin add zumen@zumen-jp
```

新しいチャットを開いてください。初回起動時にプラグインのデータフォルダーに `drawings` を作ります。そこに図面を置いてください。PowerShell では次のコマンドで場所を確認できます。

```powershell
$dataDir = (codex mcp get zumen --json | ConvertFrom-Json).transport.env.PLUGIN_DATA
Join-Path $dataDir drawings
```

別のフォルダーを使う場合は、データフォルダーの `drawings-root.txt` にその絶対パスを UTF-8 で 1 行だけ書き、Codex を再起動してください。このリポジトリで受け取った図面を扱うときは、リポジトリ外の `ZUMEN_SAMPLES` 配下を選びます。

## Claude Desktop

拡張機能の設定で「拡張機能をインストール」から `zumen-plugin-0.1.0.mcpb` を選び、図面フォルダーを指定してください。Windows と macOS の実行ファイルを含みます。[Claude Desktop の拡張機能](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop)

## GitHub Copilot CLI

```sh
copilot plugin marketplace add zumen-jp/zumen
copilot plugin install zumen@zumen-jp
```

PowerShell では、次のコマンドで図面を置く場所を確認できます。別のフォルダーを使う場合は Codex と同じように、データフォルダーの `drawings-root.txt` にその絶対パスを書き、Copilot CLI を再起動してください。[Copilot CLI のプラグイン](https://docs.github.com/ja/copilot/reference/copilot-cli-reference/cli-plugin-reference)

```powershell
$dataDir = (copilot mcp get zumen --json | ConvertFrom-Json).zumen.env.PLUGIN_DATA
Join-Path $dataDir drawings
```

## Antigravity

```sh
agy plugin install https://github.com/zumen-jp/zumen/tree/main/plugins/zumen
```

Antigravity の `/plugin` で Installed を開き、zumen を有効にして「図面フォルダー」に絶対パスを入力してください。手元にこのリポジトリがある場合は、ルートで `agy plugin install ./plugins/zumen` も使えます。[Antigravity のプラグイン](https://antigravity.google/docs/plugins?tab=cli)

## Gemini CLI

```sh
gemini extensions install https://github.com/zumen-jp/zumen --ref=main
```

図面のあるフォルダーで `gemini` を起動します。`/mcp` で zumen を確認できます。読み書きの対象は、そのワークスペースの中です。[Gemini CLI の拡張機能](https://geminicli.com/docs/extensions/)

## Cursor

Cursor で図面のあるフォルダーを開いてください。手元で試すには、このリポジトリの `plugins/zumen` から `.cursor-plugin/`、`server/`、`assets/`、`skills/` を `~/.cursor/plugins/local/zumen/` にコピーします。`Developer: Reload Window` を実行して Customize で zumen の MCP サーバーを確認してください。読み書きの対象は、開いたワークスペースの中です。

Teams・Enterprise では、チームのマーケットプレースの「Import from Repo」に `https://github.com/zumen-jp/zumen` を指定して配布できます。[Cursor のプラグイン](https://cursor.com/docs/plugins)

## 図面で試す

公開サンプルの [text_alignments.dxf](https://github.com/zumen-jp/zumen/blob/main/samples/dxf/text_alignments.dxf) を選んだフォルダーに置き、次のように頼めます。

> zumen で text_alignments.dxf を開いて、文字の位置と図面全体のプレビューを見せてください。

> 確認用というレイヤを 1 つ追加して、checked.dxf として保存してください。保存したファイルを開き直し、追加したレイヤを確認してください。

zumen は図面を手元で読みます。ツールが返す文字列・座標・プレビューはエージェントの会話に渡ります。会話データの扱いは使うエージェントの設定と契約に従います。

With zumen, you can inspect drawing layers and text, preview a sheet, and save edits to a new file. Choose the folder that contains the drawings you want to work with. Ask your agent to reopen the saved drawing and check the changed elements. The agent receives tool results and any previews it requests.

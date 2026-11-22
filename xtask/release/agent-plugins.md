# 図面プラグインの配布

GitHub から導入するときの配布元は、このリポジトリの `plugins/zumen` です。プラグインの版と、プラグインが起動する CLI の版は別に管理します。`plugins/zumen/server/release.json` に CLI の版と、各 OS の圧縮ファイルと実行ファイルの SHA-256 を記録します。

## CLI 同梱版を作る

リポジトリのルートで次のコマンドを実行します。公開済みのバイナリから、Claude Desktop の拡張機能と OpenAI のデスクトップ向け ZIP を作ります。

```sh
gh release download v0.3.0 --pattern 'zumen-0.3.0-win-*.zip' --pattern 'zumen-0.3.0-osx-*.tar.gz' --pattern 'zumen-0.3.0-linux-*.tar.gz' --dir target/agent-plugin/upstream --skip-existing
python xtask/release/agent_plugins.py target/agent-plugin/upstream target/agent-plugin/dist
```

実行ファイルとともに、対象環境のライセンス、依存の許諾、SBOM（ソフトウェア部品表）を入れます。配布ファイルを展開し、公開 DXF へのレイヤ追加、別名保存、再読み込みを確かめます。CLI を更新するときは新しいリリースの資産とハッシュを確認し、プラグインの各マニフェストの版も更新します。

## 公式ディレクトリへの提出

- OpenAI のローカル MCP は提出組織の事前承認が必要です。承認を受けた組織で CLI 同梱の `-desktop.zip` をアップロードし、審査と公開を進めます。[OpenAI の提出手順](https://developers.openai.com/plugins/deploy/desktop-only)
- Claude のプラグインはポータルで GitHub の配布元と `plugins/zumen` を指定します。Claude Desktop のローカル拡張機能は別のフォームに `.mcpb` を提出します。[プラグインの提出手順](https://claude.com/docs/plugins/submit)・[デスクトップ拡張機能の提出窓口](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop)
- Cursor は公開リポジトリの URL を提出し、審査を受けます。[Cursor Marketplace の提出窓口](https://cursor.com/marketplace/publish)
- Gemini CLI のギャラリーは、公開リポジトリのトピック `gemini-cli-extension` とルートのマニフェストから収録します。[ギャラリーの収録条件](https://geminicli.com/docs/extensions/releasing/)
- Antigravity はパートナー向けの申請フォームを使います。[Antigravity の掲載案内](https://antigravity.google/docs/build-plugins#publish-to-the-antigravity-marketplace)

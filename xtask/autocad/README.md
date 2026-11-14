# AutoCAD で公開サンプルを作るスクリプト

`samples/dwg/` の公開サンプル（DWG と対の ASCII DXF）を、自作の AutoLISP スクリプトと `accoreconsole` で作り直す手順です（[Autodesk の製品を一括で動かす](../../CONTRIBUTING.md#autodesk-の製品を一括で動かす)）。書き戻しの照合手順は [検証キット](../verify/README.md#autocad-で開いて照らす)にあります。

| ファイル | 役割 |
| --- | --- |
| [samples.lsp](samples.lsp) | 主題ごとに図面を描き、各バージョンの DWG と ASCII DXF に保存するスクリプト |
| [make.sh](make.sh) | SSH 先の Windows で `accoreconsole` を流し、`samples/dwg/` に回収する |
| [run.ps1](run.ps1) | Windows 側で `accoreconsole` を時間制限つきで動かす補助スクリプト |

## 注意

- AutoCAD が保存したファイルには、Windows の利用者名や端末名（DXF の `$LASTSAVEDBY`、DWG の要約情報など）が入ることがあります。リポジトリに入れる前にこれらの情報が含まれていないことを確かめてください。
- 公開サンプルを更新・追加するときは、[公開サンプルの条件](../../samples/README.md)に従って `samples/ledger.toml` を更新し、`cargo xtask codegen --check` を通します。

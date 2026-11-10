# Autodesk Build の BCF 2.1 の検証キット

#686 のため、公開 IFC に合う指摘を BCF 2.1 に書き戻し、製品で確認するキットです（作成は #787）。Python 3.11 以降の標準ライブラリと zumen の CLI を使います。外部のサービスへ送る処理はありません。

## キットを作る

リポジトリの根で流します。出力先は、Git の作業ツリーの外の新しいフォルダーか空のフォルダーを指定してください。既存の内容は削除しません。

```powershell
cargo build -p zumen-cli
python -I xtask/verify/acc/make.py make target/debug/zumen.exe ../acc-check
python -I -m unittest discover -s xtask/verify/acc -p "test_*.py"
```

Windows 以外では実行ファイルの `.exe` を外してください。別の checkout の中に worktree を作った場合は、`..` がその checkout の中になることがあるため、出力先にリポジトリの外の絶対パスを指定してください。

| ファイル | 中身 |
| --- | --- |
| `model.ifc` | `samples/ifc/text2bim/sample_model.ifc` のバイト列をそのままコピーしたもの。Text2BIM の公開 IFC、MIT License |
| `seed.bcf` | このスクリプトが作った変更前の BCF 2.1。指摘 4 件・コメント・視点・画像 |
| `for-acc.bcf` | zumen の `bcf_write` が書き戻した BCF 2.1。製品にはこれを取り込む |
| `snapshot.png` | 青枠と赤い中央十字の合成画像。モデルの画面ではなく、画像の保持を確かめるためのもの |
| `edited-review.json`・`reread-review.json` | 編集したレビューの文書と、出力を読み直した文書 |
| `expected.json` | 変更のレシピから決めた題・状態・コメント・選択要素・カメラと、製品で確認する状態 |
| `api.json`・`manifest.json` | 呼んだ API と診断、入力の出所・ライセンス・ハッシュ・出力のハッシュ・オフラインの比較結果 |
| `licenses/` | 公開 IFC の MIT License と、このリポジトリの MIT License・Apache License 2.0 |
| `report.md` | 利用者が製品で確認して記入する様式 |

公開 IFC の SHA-256 をサンプル台帳と照合し、選ぶ壁が `building_elements` で読めることを確かめます。元の BCF を `bcf_read` で読み、レビューの題・説明・コメントを書き換え、`bcf_write` に `original` と `version=2.1` を渡します。場所は変えず、元の Snapshot を保つ経路です。書き戻した値・要素のハッシュ・画像のバイト列を照合し、BCF の往復や選択する要素の確認で診断があれば止まります。IFC を開くときの既存の `ifc.unit.missing`・`ifc.entity.dropped` 等の診断は、`api.json` と `manifest.json` に残します。建物モデルに移せた範囲と、製品が IFC を表示できるかを同じ確認とは扱いません。

## 期待する結果

| 指摘 | 選択要素 | BCF の状態 | 製品で確認する状態 |
| --- | --- | --- | --- |
| 検証 01: 選択要素あり・未解決 | 壁 1 件 | Open | Open |
| 検証 02: 選択要素あり・解決済み | 同じ壁 1 件 | Resolved | Open |
| 検証 03: 選択要素あり・終了 | 同じ壁 1 件 | Closed | Closed |
| 検証 04: 選択要素なし・未解決 | なし | Open | Open。取り込むか飛ばすかを選ぶ |

各指摘には `公開 IFC の BCF 検証 01` などの説明と、`書き戻したコメント 01` などのコメントがあります。作者は検証用の `zumen-verification@example.invalid`、日時は `2026-11-10T00:00:00Z` です。画像は全件とも青枠と赤い中央十字です。

選ぶ要素は GlobalId `13v5x2IG0HxwZGA6irHSbP` の壁（IFC の `#1732`、`IfcWallStandardCase`）です。公開ファイルの押し出し形状と配置から、範囲は X が −0.1〜0.1 m、Y と Z が 0〜3 m です。カメラは `[-5, 1.5, 1.5]` m から `[1, 0, 0]` を向き、上方向 `[0, 0, 1]`、水平の視野角は 60° です。壁の中央を正面から向く検証用の位置で、製品でのピンの置き方は別に記録します。

## 利用者の許可後に製品で確かめる

この作業は #686 の開始条件を満たしてから行います。利用者が Autodesk Build のプロジェクトを用意し、この公開 IFC と BCF を外部のサービスへ送ることを許してください。キットを作っただけでは、この許可を得たことにも、製品で確認したことにもなりません。

以下は [Autodesk の取り込みのヘルプ](https://help.autodesk.com/cloudhelp/ENU/Build-Issues/files/Issues_BCF_Import.html)と[書き出しのヘルプ](https://help.autodesk.com/cloudhelp/ENU/Build-Issues/files/Issues_Export.html)を 2026-11-10 に確認した手順です。現在のヘルプの本文は Autodesk Forma の表記です。画面の名前と実際に使った製品・版を報告に記録してください。

1. 指摘の Manage 以上の権限がある検証用のプロジェクトで、`model.ifc` を Files に置き、処理が終わるまで待つ。
2. その単一の IFC をビューアーで開き、壁を表示できることを確かめる。別のモデルや RVT に取り込まない。
3. ビューアーの指摘の一覧から More → Import from BCF を選び、`for-acc.bcf` を指定する。対応するのは BCF 2.1、1 ファイル 100 件までで、このキットは 4 件。
4. 要素のない指摘では Include issues を選ぶ。飛ばすほうも調べる場合は別の検証用の状態で行い、検証 04 がないことを記録する。両方を同じ一覧に重ねて取り込まない。
5. 製品で必須の種類・担当者を指定し、確認して取り込む。検証 02 の Resolved が Open になるのはヘルプにある扱い。状態の変化と、文字・コメント・要素・画像の欠落を分けて記録する。
6. 4 件それぞれで題・説明・状態・コメント・画像を開く。選択ありの 3 件は同じ壁に置かれるか、選択なしの 1 件はカメラから置かれるかを記録する。画像がモデルの画面に差し替わった場合も、その結果を記録する。
7. 指摘の一覧でこの 4 件だけを選び、Export as BCF で書き出す。1 回 250 件まで。メールで届くリンクから、リポジトリの外のキットのフォルダーに `exported.bcf` として保存する。
8. 取り込んだ値と、書き出した値を次のコマンドで照合する。

```powershell
python -I xtask/verify/acc/make.py observe target/debug/zumen.exe ../acc-check ../acc-check/exported.bcf
```

`observed/` に、読み直したレビュー・API の応答と診断・項目ごとの比較を置きます。同じ出力先への上書きはしません。比較用の BCF は入力・展開後の合計とも 64 MiB、ZIP の項目は 4,096 件までです。題で指摘を対応させ、製品が GUID を付け直しても比較します。文字、コメント、選択要素、カメラの値、画像のハッシュを照らし、差があった項目はそのまま残します。カメラの値が丸められた場合も違いとして記録するため、その違いを人が評価してください。検証 04 を飛ばした場合は、件数とその指摘がないという違いが出ます。

製品の出力や画面にはアカウント・プロジェクトなどの情報が入ることがあります。出力・画面・`observed/` の JSON をそのまま Git・公開 issue・PR に貼らないでください。`report.md` の項目ごとの結果と、診断の code・件数・ハッシュだけを共有します。入力の読み取りが落ちる・終わらないなどのときは、`AGENTS.md` と決定 0283 に従って入力と詳細を非公開で扱います。

## 確かめた範囲

キットの生成と zumen による往復の比較は `synthetic` です。ネイティブ CLI で流し、BCF 2.1 の値と画像を比較します。wasm との一致や Autodesk Build の読みをこの検査で確かめたとは扱いません。製品での結果は #686 に記録し、`docs/formats/bcf.md` の「確かめた度合い」を更新します。

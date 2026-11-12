# IFC の書き戻しの検証キット

#526 のうち、IFC4・IFC2X3 を `building_apply` で編集し、BIM ソフトで開いて確かめるキットです。Python 3.11 以降の標準ライブラリと既存の zumen CLI を使います。アプリの操作や外部への送信は行いません。

## キットを作る

リポジトリの根で流します。出力先は Git の作業ツリーの外の、新しいフォルダーか空のフォルダーにします。既存のファイルは削除しません。

```powershell
cargo build -p zumen-cli
python -I xtask/verify/ifc/make.py make target/debug/zumen.exe ../ifc-edit-check
$env:ZUMEN_CLI = (Resolve-Path target/debug/zumen.exe).Path
python -I -m unittest discover -s xtask/verify/ifc -p "test_*.py"
```

Windows 以外では `.exe` を外し、テストには `ZUMEN_CLI=target/debug/zumen python -I -m unittest discover -s xtask/verify/ifc -p 'test_*.py'` を使います。worktree が別の checkout の中にある場合は、出力先にリポジトリの外の絶対パスを指定してください。

| ファイル | 中身 |
| --- | --- |
| `ifc4/input.ifc` | GNI BIM Dataset の `model_9.ifc`（CC BY 4.0）のコピー |
| `ifc2x3/input.ifc` | Text2BIM の `sample_model.ifc`（MIT License）のコピー |
| 各フォルダーの `edited.ifc` | CLI の `building_apply` が書き戻したファイル。BIM ソフトでこれを開く |
| 各フォルダーの `request.json` | 実際に呼んだ変更の要求。対象は読んだ ID とハッシュで指す |
| 各フォルダーの `expected.json` | 再読み込みの値からではなく、変更のレシピから決めた期待値 |
| 各フォルダーの `before.json`・`after.json` | 編集前と再読み込み後の要素・名前・説明・配置・プロパティ |
| `api.json` | 呼んだ API・応答・診断 |
| `manifest.json` | 入力の出所・ライセンス・帰属表記・改変内容・ハッシュ、CLI の版、比較結果 |
| `licenses/` | Text2BIM の MIT License と、このリポジトリの MIT License・Apache License 2.0 |
| `report.md` | 利用者が製品で確認して記入する様式 |

入力は `samples/ledger.toml` のバイト数と SHA-256 を確かめてからコピーします。GNI の出所・帰属表記と CC BY 4.0 のリンク、Text2BIM の出所・元サンプルのヘッダーの変更記録も `manifest.json` に引き継ぎます。GNI の編集済みファイルを再配布する場合は、その帰属表記・ライセンスのリンクと、名前・説明・プロパティを zumen で編集したことを一緒に示してください。

書き出しは、同じ形式の編集（L2 の変更セット → `apply` → L1 の触れたインスタンスだけを書き換える）です。新しい IFC の作成や、別の形式への変換の確認には使いません。

## 対象と期待する値

| 項目 | IFC4 | IFC2X3 |
| --- | --- | --- |
| 編集対象の GlobalId | `2q2CBrfQTCKPY0zxCdZwK$` | `13v5x2IG0HxwZGA6irHSbP` |
| 元の STEP インスタンス | `#628`（IfcWall） | `#1732`（IfcWallStandardCase） |
| 元の名前 | `Basic Wall:Standard 20.0:480832` | `Exterior Concrete Wall` |
| 編集後の名前 | `検証526 IFC4 '壁' \ A` | `検証526 IFC2X3 '壁' \ A` |
| 変更しない対照の GlobalId | `2q2CBrfQTCKPY0zxCdZwHh` | `13v5wuIG0HxwZGA6irHSbP` |
| 対照の STEP インスタンス | `#691` | `#1610` |

両方の編集対象で、次の値を確認します。

| 場所 | 編集後の値 |
| --- | --- |
| Description | `名前・説明・プロパティの書き戻しを確認する壁` |
| `Pset_WallCommon.IsExternal` | false（編集前は true） |
| `Zumen_Check526.Note` | `検証用の備考`（IfcLabel） |
| `Zumen_Check526.Ratio` | 1.25（IfcReal） |
| `Zumen_Check526.Count` | 7（IfcInteger） |

IFC4 の対象と対照は `IsExternal` の実体を共有します。対象だけが false になり、対照は true のままかを確認します。IFC2X3 の対象は単独のプロパティを持ち、対照は別のプロパティを持ちます。対照の名前・説明を含むほかの要素と、編集対象のほかのプロパティ・配置も比較します。

キットの生成時には、名前・プロパティの変更を許した元の行以外がバイト単位で変わらないことも確かめます。形の参照と形そのものを含む行が比較対象です。これだけで、BIM ソフトが同じ形を表示したと判断はしません。

## BIM ソフトで開く

この段階は、利用者がソフトを使える環境を用意し、そのセッションでアプリの利用を許してからです。クラウドへ置く必要があるソフトでは、外部のサービスへの送信も別に許可を得てください。

1. IFC4 と IFC2X3 を開ける検証用の BIM ソフトを用意する。実施日・製品・バージョンを `report.md` に記入する。
2. `input.ifc` を先に開き、開くときの誤り・警告の件数を記録する。編集対象と対照を GlobalId で探し、元の名前と `IsExternal = true` を確認する。名前だけで選ぶと、同名の別の壁と取り違えることがある。
3. 別の文書として `edited.ifc` を開く。元の文書を上書きしない。同じ GlobalId の壁で、表の名前・説明・既存プロパティ・新しいセットの値を確認する。日本語・引用符・バックスラッシュの表示も記録する。
4. 対照の壁の値、壁の位置と形、開口、周囲の要素が変わらないか、入力と同じ視点で比べる。表示できない形や確認できなかった項目は、未確認として残す。
5. 入力より誤り・警告が増えたかを記録する。元のファイルにもある指摘と、書き戻したファイルだけにある指摘を分ける。
6. ソフトが IFC を書き出せる場合は、編集後の文書を同じバージョンの IFC として、Git の作業ツリーの外に保存する。再書き出しができない場合は、次の比較を未実施として記録する。

製品が書いたファイルを読み直すときは、次を流します。最後の引数は、実際に保存したファイルの道筋に替えてください。`observed/ifc4/` と `observed/ifc2x3/` は、それぞれ新しいフォルダーである必要があります。

```powershell
python -I xtask/verify/ifc/make.py observe target/debug/zumen.exe ../ifc-edit-check ifc4 ../ifc4-exported.ifc
python -I xtask/verify/ifc/make.py observe target/debug/zumen.exe ../ifc-edit-check ifc2x3 ../ifc2x3-exported.ifc
```

`observed/` に読み直したファイル・応答・診断・比較結果を保存します。IFC のバージョンと単位、GlobalId の集合、対象の値、ほかの要素・プロパティ・配置、診断の種類と件数を比べ、違いがあれば終了コード 1 を返します。製品が要素やプロパティを補い・省いた場合や、表記・単位・ハッシュが変わった場合も違いになります。変更の意図どおりか、製品による変換かを、人が結果と分けて評価してください。読み込むファイルは 64 MiB までです。

製品の出力や画面には、アカウント・プロジェクトなどの情報が入ることがあります。ファイル・画面・`observed/` の JSON をそのまま Git・公開 issue・PR に貼らず、`report.md` の項目ごとの結果と診断の code・件数・ハッシュを共有します。読み取りの失敗の入力と詳細は、決定 0283 の対象かを確かめるまで非公開で扱います。

## 確かめた範囲

このキットの生成と CLI による再読み込みは `synthetic` です。ネイティブと wasm の一致、IfcOpenShell との外部照合、BIM ソフトでの `application` の確認を、この検査で済んだとは扱いません。

既存の外部照合は `crates/zumen-ifc/tests/edit.rs` と `tests/ifcopenshell/check.py` にあり、テストで作った IFC4・IFC2X3 の名前・説明・単一値プロパティの編集を調べています。このキットは、その操作を既存 CLI から公開モデルに当て、実機で確認するためのものです。キットだけでは台帳の段を上げず、製品での結果を #526 に記録してから、形式の文書と台帳を更新します。

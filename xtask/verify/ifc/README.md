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
| `external/results.json` | 別に実行した IfcOpenShell の照合結果。入力・出力・比較器・CLI のハッシュ、比較数、元からある指摘と編集後の指摘 |
| `licenses/` | Text2BIM の MIT License と、このリポジトリの MIT License・Apache License 2.0 |
| `report.md` | 利用者が製品で確認して記入する様式 |

入力は `samples/ledger.toml` のバイト数と SHA-256 を確かめてからコピーします。GNI の出所・帰属表記と CC BY 4.0 のリンク、Text2BIM の出所・元サンプルのヘッダーの変更記録も `manifest.json` に引き継ぎます。GNI の編集済みファイルを再配布する場合は、その帰属表記・ライセンスのリンクと、名前・説明・プロパティを zumen で編集したことを一緒に示してください。

書き出しは、同じ形式の編集（L2 の変更セット → `apply` → L1 の触れたインスタンスだけを書き換える）です。新しい IFC の作成や、別の形式への変換の確認には使いません。

## IfcOpenShell と照らす

キットを作った後、固定した外部実装でその出力を直接読みます。IfcOpenShell はテストのツールで、製品の依存にはしません。ソースや IFC スキーマを読んだり同梱したりせず、固定バージョンの配布済みパッケージと公開 API を使います。

```sh
python -m pip install --only-binary :all: ifcopenshell==0.9.0.post1 pytest==9.2.0
python -X utf8 -I xtask/verify/ifc/external/check.py <キット> --cli <キットを作ったzumen>
python -X utf8 -I xtask/verify/ifc/external/test_check.py <キット>
```

Windows では Python 3.13、毎晩の CI では Python 3.12 を使います。uv を使うときも `--no-python-downloads --no-build --python 3.13 --with ifcopenshell==0.9.0.post1 --with pytest==9.2.0` でバージョンを固定できます。外部照合はバージョンごとの子プロセスを直列に動かし、1 件 180 秒で止めます。入力は台帳にある上記 2 件とハッシュが同じものに限り、編集後と期待値も manifest・固定したレシピと照らします。既存の `external/` は上書きしません。

比較するものは次のとおりです。

- 対象の名前・説明と、4 プロパティの値・IFC 型・付随属性。未変更のプロパティと数量、共有値を持つ対照を含む非対象のプロパティも照らす。名前の重複を隠さず、bool・整数・実数、未指定のセット名と文字列を区別する。
- 元の全 STEP インスタンスの属性と参照。変更を許すのは、IFC4 の `#628` の Name/Description と `#638` の HasProperties、IFC2X3 の `#1732` の Name/Description と `#1956` の NominalValue だけ。配置・形・単位を含むほかの値は同じでなければ失敗する。
- 元のオブジェクトと GlobalId の保持、追加した GlobalId の重複がないこと。追加したセット・単一値プロパティ・関係が対象に結び付き、対象からたどれない追加や、対照への誤った関連付けがないこと。
- IfcOpenShell のスキーマ・EXPRESS 規則の検査。指摘の種類・インスタンス・属性・文の先頭行のハッシュ・件数を編集前後で比べ、新しい指摘や件数の増加を失敗にする。元からある指摘を 0 件と扱わず、規則本文を含むメッセージは成果物に保存しない。

これは属性と参照の比較です。形を計算した体積・描画・見え方や、ネイティブと wasm のバイト一致には広げません。比較器の検査は、実際の公開モデルをメモリ内で改変し、値・型・共有値・座標・参照・ID・欠落・重複・関連付けの食い違いを検出します。

毎晩の CI でもキットを作って外部実装と照らし、CLI 比較器 15 本と外部比較器 9 本を検査します。外部照合が成功したときだけ、帰属表記・許諾本文・実測用の様式を含むキットを `verify-ifc-edit` に保存します。公開モデルを BIM ソフトで開いた結果は [#526](https://github.com/zumen-jp/zumen/issues/526) にあります。

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

このキットの生成と CLI による再読み込みは `synthetic` です。外部照合は `external/check.py` を別に実行し、成功した固定出力のハッシュと比較範囲を `external/results.json` に記録します。ネイティブと wasm の一致や、BIM ソフトでの `application` の確認を、この検査で済んだとは扱いません。

既存の合成入力の外部照合は `crates/zumen-ifc/tests/edit.rs` と `tests/ifcopenshell/check.py` にあります。このキットは、その操作を既存 CLI から公開モデルに当て、外部実装と実機で確認するためのものです。FreeCAD で両版、FZKViewer で IFC4 を確認した範囲は、[形式の文書](../../../docs/formats/ifc.md#確かめた度合い)にあります。

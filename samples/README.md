# 公開サンプル

再配布を認めるライセンスで公開された図面を、取得したままの形で置きます。CI は、これらの図面で適合テストを流します。サンプルのテストでの利用は #310 で進めます。

**ここのファイルは、リポジトリのライセンス（MIT OR Apache-2.0）に従いません。** それぞれ、[`ledger.toml`](ledger.toml) に書いたライセンスに従います。配る crate とパッケージには入れません。

受け取った図面と顧客の図面は、公開されていても入れません。Git に入れられない図面は、今までどおりリポジトリの外のフォルダーを `ZUMEN_SAMPLES` に指定して使います。

## 入れてよいもの

目的を問わず、再配布・改変・商用の利用を、条件なしか表示だけを条件に認めるライセンスのものです。台帳の `license` には、次のどれかを書きます。

| `license` | ライセンス |
| --- | --- |
| `パブリックドメイン` | パブリックドメイン |
| `CC0-1.0` | CC0 1.0 |
| `CC-BY-2.0`・`CC-BY-2.1-JP`・`CC-BY-2.5`・`CC-BY-3.0`・`CC-BY-4.0` | CC BY |
| `公共データ利用規約（第 1.0 版）` | デジタル庁の公共データ利用規約（PDL1.0） |
| `政府標準利用規約（第 2.0 版）` | 各府省の政府標準利用規約 |
| `MIT`・`BSD-2-Clause`・`BSD-3-Clause`・`Apache-2.0`・`ISC`・`Zlib` | 寛容なライセンス |

次のものは入れません。

- ライセンスや利用条件が書かれていないもの。ダウンロードできることは、再配布の許諾ではありません
- 非営利に限るもの（NC）、改変を禁じるもの（ND）、同じ条件での公開を求めるもの（SA）
- 利用規約で再配布を禁じるもの。ソフトウェアの製品に付くサンプルで、使用許諾が再配布を認めないものも含みます
- 個人の氏名・住所・連絡先が読めるもの
- 1 つで 5 MB を超えるもの。`samples/` の全体は 100 MB までです

ライセンスがはっきりしないものは入れず、見送った理由を issue に書きます。どうしても必要なときは、権利者への問い合わせを利用者に頼みます。

## 足し方

1. 取得したままのファイルを `samples/<形式>/` に置きます（例: `samples/dxf/`・`samples/jww/`・`samples/sxf/`）。Git は、このフォルダーのファイルの改行を変えません（`.gitattributes`）。
2. `ledger.toml` に、ファイルごとに `[[samples]]` を足します。

   ```toml
   [[samples]]
   path = "dxf/example.dxf"              # samples/ からの道筋
   sha256 = "…"                          # SHA-256 の 16 進の小文字
   bytes = 12345
   title = "図面の題"
   publisher = "公開した者"
   url = "https://example.com/example.dxf"  # 出所
   retrieved = "2026-10-30"              # 取得した日
   license = "CC-BY-4.0"
   license-url = "https://creativecommons.org/licenses/by/4.0/legalcode.ja"
   attribution = "表示の文"
   changes = "変えた内容"                 # 変えていなければ書かない
   ```

3. `cargo xtask codegen --check` で、ファイルと台帳の一対一・ハッシュとバイト数・ライセンス・大きさを確かめます。
4. ライセンスが本文や著作権表示の同梱を求める場合は、その条件を満たすように本文を `licenses/` に置き、台帳の表示の文から参照します。
5. PR に、ライセンスの本文の URL と、再配布を認める箇所を書きます。

## 外し方

ライセンスに疑いが出たときなどに、サンプルを外す手順です。

1. ファイルと、`ledger.toml` のその `[[samples]]` を消します。台帳から参照しなくなった `licenses/` の本文も消します。
2. PR に、外した理由を書きます。

fuzz が育てた種（actions/cache）のキーは、台帳のハッシュを含みます。台帳が変わると、外す PR の実行も、マージの後の毎晩の実行も、前の種を戻しません。そのため、その後の実行の種と artifact に、外したサンプルとそれを変えた入力は入りません（[fuzz の手順](../fuzz/README.md)・[`fuzz/README.md`](../fuzz/README.md)）。

前の種のキャッシュは、7 日使われなければ GitHub が消します。早く消すときは、外す PR をマージし、流れている fuzz の実行が終わってから、次のコマンドで消します。消した後に、`gh cache list --key fuzz-corpus-` で何も出ないことを確かめます。

```bash
for id in $(gh cache list --key fuzz-corpus- --limit 100 --json id --jq '.[].id'); do gh cache delete "$id"; done
```

## AutoCAD で作った図面

`dwg/zumen-*` は、zumen-jp が AutoCAD 2027 で作った図面です（#316）。ライセンスは CC0 1.0 です。主題ごとに 1 つの図面を作り、バージョンごとに DWG と、同じ図面の DXF（`_ascii.dxf`）を対で置きます。テストは、この対の表と図面モデルを比べます。

作り直すときは、`xtask/autocad/make.sh` を使います。SSH でつなぐ Windows の AutoCAD の accoreconsole で `xtask/autocad/samples.lsp` を流し、できた図面を `dwg/` に置きます。そのあと、台帳のハッシュとバイト数を直します。AutoCAD は保存のたびに、Windows のログイン名を図面の DWGPROPS に書きます。

## PLY

`ply/pdal-mesh-fixed.ply` は PDAL の公開サンプルです（#809）。[追加コミット](https://github.com/PDAL/PDAL/commit/f65c92beb523de0d70119c1fab2dc295933e9398)の `PlyWriterTest.cpp` は、テストの中で座標を決めた 4 頂点・2 面を、小数点以下 3 桁の ASCII で書き、このファイルと比較します。実測の点群ではありません。同コミットの `LICENSE.txt` は、別記のない全ファイルを BSD とし、このファイルへの別条件はありません。出所・帰属とライセンスの全文は台帳と `licenses/pdal-2017.txt` にあります。取得した 277 バイトを変更していません。

`crates/zumen-ply/tests/external.rs` は ply-rs の読みと全頂点の座標を比較し、面を移さない診断と予算の停止も確認します。`bindings/conformance/cases/pointcloud_ply_sample.json` は同じ公開サンプルの応答・診断をネイティブと wasm で照らします。確認した範囲と残りは [PLY の文書](../docs/formats/ply.md#確かめた度合い)にあります。

## IFC

`ifc/` は、再配布を認める公開の IFC です（#412）。出所・ライセンス・表示の文・変えた内容は、台帳にあります。

| フォルダー | 公開した者 | ライセンス | 書き出したソフト | バージョン |
| --- | --- | --- | --- | --- |
| `rinya/` | 林野庁 | 公共データ利用規約（第 1.0 版） | Autodesk Revit 2024 日本語版（公開元が IfcOpenShell で保存） | IFC4X3 |
| `nakano/` | 中野区 | CC BY 4.0 | Autodesk Civil 3D 2024 日本語版 | IFC2X3 |
| `gni/` | Technical University of Munich, Georg Nemetschek Institute（GNI BIM Dataset） | CC BY 4.0 | Autodesk Revit 2024・2025、Archicad 28 | IFC2X3・IFC4 |
| `bim-whale/` | André Wisén（BIM Whale） | MIT | Autodesk Revit 2021 | IFC2X3 |
| `text2bim/` | dcy0577（Text2BIM） | MIT | Vectorworks Architect 2024 | IFC2X3 |

- ヘッダーに、作った人の名前・手元の機械の名前・手元のフォルダーの道筋があるものは、その値だけを置き換えました。データのセクションは、取得したままのバイト列です。取得したファイルの大きさと SHA-256 は、台帳の `changes` にあります。
- `text2bim/` の GlobalId はバージョン 1 の UUID で、作った機械の識別子（MAC アドレスと推定）を含みます。氏名・住所・連絡先ではなく、元のファイルも公開されているので、データのセクションを変えずに残しました。
- `nakano/` の形は点群から作った Brep で、要素はすべて `IfcBuildingElementProxy` です。`rinya/` と `gni/` は、公開元が書き出した後に保存し直しています。どちらも、書き出したソフトがそのまま書いたファイルとしては扱いません。
- `crates/zumen-ifc/tests/samples.rs` が、すべてを読み、建物モデルに写した要約を `samples.txt` と比べます。毎晩の CI は、写した値を IfcOpenShell で読んだ値と照らします（`crates/zumen-ifc/tests/ifcopenshell/samples.py`）。

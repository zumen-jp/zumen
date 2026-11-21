# 表の合成入力

`table-r2000.dwg`・`table-r2004.dwg`・`table-r2010.dwg`・`table-r2013.dwg`・`table-r2018.dwg` は、自作の 2 行 2 列の表と線分です。旧版のセルの文字は `A00` から `A11`、R2010 以降は `表00` から `表11`、表示用ブロックは罫線と文字だけを持ちます。受領図面や製品の付属ファイルは使っていません。入力を作るコードと図面には、リポジトリの MIT OR Apache-2.0 を適用します。

`xtask/verify/acadsharp/TableFixtures.cs` から、別に入れた ACadSharp 3.8.0（MIT）で作ります。

`table-styled-r2010.dwg`・`table-styled-r2013.dwg`・`table-styled-r2018.dwg` は、文書に登録した表スタイルと文字スタイルを参照する別の入力です。表・データ・見出し・題のセルスタイルに文字スタイルを結び、生成器が読み戻して参照が正しいことと、読取通知が 0 件であることを検査します。既存の `table-r*.dwg` は、表スタイルを指さない最小入力として残します。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-table-fixtures <生成先>
cargo run -p zumen-acad --example legacy_table_fixture -- <生成先>
```

ACadSharp 3.8.0 は R2000・R2004 の表を書かないため、旧版だけは表示用ブロックと線を外部実装で作り、ODS 20.4.96.1 に従う自作のレコードで線を表に置き換えます。`legacy_table_fixture.rs` は、生成器が出した `.meta` の型番号とハンドルを使います。旧版の文字にはコードページによらない ASCII を使います。5 入力と、表の位置を変えた 10 出力を ACadSharp の読み手で別に照らします。

`table-add-r2004.dwg` は、追加の試験用に Standard の文字高を 2.5、行の位置合わせを 5 にした同じ生成器の入力です。元の `table-r2004.dwg` は、文字高 0 のスタイルを参照する新規表を拒む試験にも使います。

日時と単位を固定し、著者と最終保存者は空にします。GUID は外部ライブラリが決めるので、再生成ではファイルのハッシュも確認します。ネイティブと wasm の一致は、記録した同じ入力バイトを使って確かめます。

生成先で内容を確かめ、DWG だけをこのフォルダーにコピーします。再生成後は `bindings/conformance/cases/apply_dwg_table.json`、スタイルを参照する入力は `apply_styled_dwg_table.json` の同名の `push.hex` も更新し、適合テストの手順でハッシュを作り直します。Rust の試験は、この入力が同じバイト列であることも確かめます。

表の移動で、セル・書式・表示用ブロックの元の値を保つ試験に使います。AutoCAD での監査と表示の確認は含みません。

## 個別 DSTYLE を持つ寸法の合成入力

`dim-dstyle-r2010.dwg`・`dim-dstyle-r2013.dwg`・`dim-dstyle-r2018.dwg` は、`xtask/verify/acadsharp/DimensionFixtures.cs` から ACadSharp 3.8.0（MIT）で作る平行寸法の合成入力です（#905）。名前付き寸法スタイル `CaseBase`（文字高 2.5・矢印長さ 1.25・小数桁 2）、図形の `ACAD`/`DSTYLE` 上書き（41 = 3.625・140 = 4.75・271 = 3）、拡張辞書の `VERIFY_VALUES`（1 = `A-19`・70 = 321・40 = 6.125）、`UpdateBlock()` が作った初期匿名ブロック（補助 `POINT` 4 個を含む）を持ちます。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-dim-dstyle-fixtures <生成先>
```

移動後に DWG の匿名ブロックが実効の文字高・矢印長さ・小数桁数を反映し、補助 `POINT` を除いた表示部品（`LINE`・`SOLID`・`MTEXT`）だけになることと、測点・測定値・拡張辞書・`DSTYLE` 上書きを保つ回帰試験に使います。

## R2010 以降の表スタイルとセル書式の合成入力

`table-style-r2010.{dwg,dxf}`・`table-style-r2013.{dwg,dxf}`・`table-style-r2018.{dwg,dxf}` と `table-format-cases-r2018.dwg` は、`xtask/verify/acadsharp/TableFormatFixtures.cs` から ACadSharp 3.8.0（MIT）で作る表スタイル・セル書式・色見本帳の合成入力です（#906）。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-table-format-fixtures <生成先>
```

- `table-style-r2010/r2013/r2018.{dwg,dxf}`: 色見本帳（`SamplePocket$Harbor`、RGB 13, 97, 184）を指す円（`#49` → `DBCOLOR #48`）と、名前付き表スタイル `SampleFormat`（`#4A`、データ行 ACI 2・文字高 1.75、見出し・表題行の文字高 1.75、位置合わせ 5）を指す 1 行 2 列の表（`#4E`、セルごとに文字高 2.5／3.5・ACI 3／4 を上書き）。対照 DXF は ACadSharp が出さない `ACAD_TABLE` と `TABLESTYLE` の行値を同じ定義から補完し、生成時に ACadSharp で DWG・DXF の双方を再読して照合します。
- `table-format-cases-r2018.dwg`: 対応済みのフィールド（表題・見出し・データ行の文字スタイル・文字高・ACI 色）だけを持つ `CleanFormat` と、セル未設定・文字高のみ上書き・文字色のみ上書き（ByBlock）・内容書式（`CellContent.Format`）での文字高と文字色（ByLayer）上書きの 4 セルを持つ表 1、およびセルに RGB 真の色の上書きを持つ表 2 を並べ、未対応フィールドを持つ表 2 だけに `dwg.table-format.unsupported` が出ることを確かめます。

## GEODATA の合成入力

`geodata-r2004-v1.dwg`・`geodata-r2010.dwg`・`geodata-r2013.dwg`・`geodata-r2018.dwg` は、`xtask/verify/acadsharp/GeoDataFixtures.cs` から ACadSharp 3.8.0（MIT）で作る地理位置（`GEODATA`）の合成入力です（#685）。`*Model_Space` の拡張辞書の `ACAD_GEOGRAPHICDATA` に `GEODATA` を置き、生成時に ACadSharp で再読して読取通知が 0 件であることを確かめます。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-geodata-fixtures <生成先>
```

- `geodata-r2010.dwg`・`geodata-r2013.dwg`・`geodata-r2018.dwg`: 版 2／3、局所の格子、図面単位 mm（倍率 0.001）、北を 30 度回した向き、架空名 `ZUMEN-TEST-IX` に `EPSG:6677` の別名を付けた座標系 XML を持ち、4 点を写した座標が `ezdxf 1.4.5` の `GeoData.get_crs_transformation` と合うことを確かめます。
- `geodata-r2004-v1.dwg`: 版 1 の `GEODATA` を持ち、未対応の版として `crs.geodata.unsupported`（`reason = "version"`）とバイト範囲の出所を返すことを確かめます。

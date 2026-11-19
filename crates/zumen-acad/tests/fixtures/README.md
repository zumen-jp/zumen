# 表の合成入力

`table-r2000.dwg`・`table-r2004.dwg`・`table-r2010.dwg`・`table-r2013.dwg`・`table-r2018.dwg` は、自作の 2 行 2 列の表と線分です。旧版のセルの文字は `A00` から `A11`、R2010 以降は `表00` から `表11`、表示用ブロックは罫線と文字だけを持ちます。受領図面や製品の付属ファイルは使っていません。入力を作るコードと図面には、リポジトリの MIT OR Apache-2.0 を適用します。

`xtask/verify/acadsharp/TableFixtures.cs` から、別に入れた ACadSharp 3.8.0（MIT）で作ります。

`table-styled-r2010.dwg`・`table-styled-r2013.dwg`・`table-styled-r2018.dwg` は、文書に登録した表スタイルと文字スタイルを参照する別の入力です。表・データ・見出し・題のセルスタイルに文字スタイルを結び、生成器が読み戻して参照が正しいことと、読取通知が 0 件であることを検査します。既存の `table-r*.dwg` は、表スタイルを指さない最小入力として残します。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-table-fixtures <生成先>
cargo run -p zumen-acad --example legacy_table_fixture -- <生成先>
```

ACadSharp 3.8.0 は R2000・R2004 の表を書かないため、旧版だけは表示用ブロックと線を外部実装で作り、ODS 20.4.96.1 に従う自作のレコードで線を表に置き換えます。`legacy_table_fixture.rs` は、生成器が出した `.meta` の型番号とハンドルを使います。旧版の文字にはコードページによらない ASCII を使います。5 入力と、表の位置を変えた 10 出力を ACadSharp の読み手で別に照らします。

日時と単位を固定し、著者と最終保存者は空にします。GUID は外部ライブラリが決めるので、再生成ではファイルのハッシュも確認します。ネイティブと wasm の一致は、記録した同じ入力バイトを使って確かめます。

生成先で内容を確かめ、DWG だけをこのフォルダーにコピーします。再生成後は `bindings/conformance/cases/apply_dwg_table.json`、スタイルを参照する入力は `apply_styled_dwg_table.json` の同名の `push.hex` も更新し、適合テストの手順でハッシュを作り直します。Rust の試験は、この入力が同じバイト列であることも確かめます。

表の移動で、セル・書式・表示用ブロックの元の値を保つ試験に使います。AutoCAD での監査と表示の確認は含みません。

## 個別 DSTYLE を持つ寸法の合成入力

`dim-dstyle-r2010.dwg`・`dim-dstyle-r2013.dwg`・`dim-dstyle-r2018.dwg` は、`xtask/verify/acadsharp/DimensionFixtures.cs` から ACadSharp 3.8.0（MIT）で作る平行寸法の合成入力です（#905）。名前付き寸法スタイル `CaseBase`（文字高 2.5・矢印長さ 1.25・小数桁 2）、図形の `ACAD`/`DSTYLE` 上書き（41 = 3.625・140 = 4.75・271 = 3）、拡張辞書の `VERIFY_VALUES`（1 = `A-19`・70 = 321・40 = 6.125）、`UpdateBlock()` が作った初期匿名ブロック（補助 `POINT` 4 個を含む）を持ちます。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-dim-dstyle-fixtures <生成先>
```

移動後に DWG の匿名ブロックが実効の文字高・矢印長さ・小数桁数を反映し、補助 `POINT` を除いた表示部品（`LINE`・`SOLID`・`MTEXT`）だけになることと、測点・測定値・拡張辞書・`DSTYLE` 上書きを保つ回帰試験に使います。

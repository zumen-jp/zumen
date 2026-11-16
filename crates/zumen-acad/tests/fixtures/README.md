# 表の合成入力

`table-r2000.dwg`・`table-r2004.dwg`・`table-r2010.dwg`・`table-r2013.dwg`・`table-r2018.dwg` は、自作の 2 行 2 列の表と線分です。旧版のセルの文字は `A00` から `A11`、R2010 以降は `表00` から `表11`、表示用ブロックは罫線と文字だけを持ちます。受領図面や製品の付属ファイルは使っていません。入力を作るコードと図面には、リポジトリの MIT OR Apache-2.0 を適用します。

`xtask/verify/acadsharp/TableFixtures.cs` から、別に入れた ACadSharp 3.8.0（MIT）で作ります。

```powershell
dotnet run --project xtask/verify/acadsharp -c Release -- --make-table-fixtures <生成先>
cargo run -p zumen-acad --example legacy_table_fixture -- <生成先>
```

ACadSharp 3.8.0 は R2000・R2004 の表を書かないため、旧版だけは表示用ブロックと線を外部実装で作り、ODS 20.4.96.1 に従う自作のレコードで線を表に置き換えます。`legacy_table_fixture.rs` は、生成器が出した `.meta` の型番号とハンドルを使います。旧版の文字にはコードページによらない ASCII を使います。5 入力と、表の位置を変えた 10 出力を ACadSharp の読み手で別に照らします。

日時と単位を固定し、著者と最終保存者は空にします。GUID は外部ライブラリが決めるので、再生成ではファイルのハッシュも確認します。ネイティブと wasm の一致は、記録した同じ入力バイトを使って確かめます。

生成先で内容を確かめ、DWG だけをこのフォルダーにコピーします。再生成後は `bindings/conformance/cases/apply_dwg_table.json` の同名の `push.hex` も更新し、適合テストの手順でハッシュを作り直します。Rust の試験は、この入力が同じバイト列であることも確かめます。

表の移動で、セル・書式・表示用ブロックの元の値を保つ試験に使います。AutoCAD での監査と表示の確認は含みません。

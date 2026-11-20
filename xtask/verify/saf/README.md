# SAF 付き P2Z・SFZ の照合（#494）

`make.py` は自作の線分と円、SAF の属性を ZIP にまとめ、公開 API の `apply` で属性のない円を移動します。8 組すべての入力と編集後を外部実装で読み、非対象の線分・表・属性とその結び先が保たれることを確かめます。許可していない SXF ブラウザは起動しません。

```powershell
cargo build --locked -p zumen-cli
cargo build --locked -p zumen-sxf --example verify_saf
uv run --no-python-downloads --python 3.14 --with ezsxf==0.3.7 python -I -m unittest discover -s xtask/verify/saf -p test_make.py
uv run --no-python-downloads --python 3.14 --with ezsxf==0.3.7 python -I xtask/verify/saf/make.py --cli target/debug/zumen.exe --probe target/debug/examples/verify_saf.exe --out ../zumen-verify-saf
```

ビルドと生成は同じチェックアウトで実行します。`CARGO_TARGET_DIR` を指定した場合は、`--cli` と `--probe` もその場所に合わせます。出力先は Git の外の新しいフォルダーか空のフォルダーに限ります。既存のキットは上書きしません。生成や照合が途中で失敗したフォルダーは完成したキットとして扱わず、新しい出力先で再実行します。成功した場合だけ最後に `manifest.json` を書きます。

## 入力と比較先

`fixtures.py` の自作 SFC・P21・SAF を使います。SFZ と P2Z、SAF 名の同名幹と拡張子付き、無圧縮と deflate の直積が 8 組です。入力は小さく固定し、製品に付属する資源や受け取った図面を混ぜません。ZIP の名前・順序・CRC・非対象メンバーは Python の `zipfile`、XML の構造・名前・型・単位・文字列は `ElementTree` で照合します。

図形と画層は ezsxf 0.3.7 で照合します。既存の `check_ezsxf.py` は P21 の円を種類だけ比べるため、このキットは ezsxf の円・配置・中心の参照をたどり、中心と半径を補って照らします。変更の期待値は定数 (55, 22)、半径 10 で、zumen の再読から作りません。入力と変更を繰り返し、出力バイト列が一致することも検査します。

公開 API の `lift` が返す図面モデルを `verify_saf` で読み、2 属性の名前・フィールド・対象 ID・有効性を取り出します。外部 XML の値と自作入力の期待値に結び、円の変更後も線分に結ばれていることを確かめます。`verify_saf` は図面モデルを読むための検証用の例で、別の SAF 実装ではありません。XML の解析が比較先です。

## 成果物と残件

入力・編集後のアーカイブ、展開したメンバー、公開 API の要求と結果、図面の再読、属性の読取、外部照合結果、生成器・照合器と実行ファイルの SHA-256 を保存します。[キットに含める期待値表](kit-readme.md)と `report-template.json` が実機確認のスクリプトです。

この自動検査はネイティブ CLI が対象です。別 OS・wasm との一致、SAF DTD の適合性、SXF ブラウザの表示・印刷は確認していません。DynaCAD（ビューアー 9.00.00）では、同名幹の SAF を持つ SFZ の 2 組で、編集前後とも線分に結び付いた 2 属性が一致しました。拡張子付きの SAF は同じビューアーで表示されませんでした。P21 は用紙と図面版・タイトルを結び、用紙の枠と表示寸法が同じ A3 横の枠を指すようにしました。[実機での記録](https://github.com/zumen-jp/zumen/issues/494#issuecomment-6969769269)・[用紙の参照の例（OCF）](https://ocf.or.jp/pdf/2001kickoff2.pdf)。

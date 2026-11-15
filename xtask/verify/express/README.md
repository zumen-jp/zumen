# 公開 IFC EXPRESS の宣言の照合

buildingSMART が公開する同じ EXPRESS を、`Express::read` と IfcOpenShell 0.9.0.post1 で読みます（[#663](https://github.com/zumen-jp/zumen/issues/663)）。取得元は公式の [Standards-builds](https://github.com/buildingSMART/Standards-builds/tree/dcf43f7f167b2cdf8007c1ea1ceec32de772f44f) の固定コミットです。版ごとのパス・サイズ・SHA-256 を `check.py` で固定します。

スキーマは実行時に OS の一時領域へ取得し、照合後に除きます。全宣言の観測値と外部実装のキャッシュも同じ一時領域に置きます。Git・パッケージ・テストデータ・CI の成果物へスキーマや全宣言を同梱しません。結果には取得元・ハッシュ・件数・成否・比較した範囲だけを残します。

## 実行

出力先には、リポジトリ外の新規フォルダーを指定してください。

```sh
cargo test --locked -p zumen-ifc --lib --no-run
uv run --no-python-downloads --no-build --python 3.13 --with ifcopenshell==0.9.0.post1 python -I -m unittest discover -s xtask/verify/express -p test_check.py
uv run --no-python-downloads --no-build --python 3.13 --with ifcopenshell==0.9.0.post1 python -I xtask/verify/express/check.py --out ../zumen-express-verify
```

`results.json` の `complete` が true で全件の `verified` が true のときだけ、照合が完了した成功記録です。終了コードは一致なら 0、不一致なら 1、実行の失敗なら 2 です。途中の記録も残しますが、成功の証拠には使いません。入力・結果を上書きしません。

IfcOpenShell は同じスキーマを子プロセスで読み、[公開 API](https://docs.ifcopenshell.org/autoapi/ifcopenshell/express/index.html) の `parse` と [宣言の問い合わせ](https://docs.ifcopenshell.org/ifcopenshell-python/schema_querying.html)を使います。観測後に `disown()` で所有権を明示し、子プロセスの正常終了も必須にします。外部実装のソースは読みません。

## 確認した範囲

2026-11-16 の Windows で、次の宣言が一致しました。

| スキーマ | 実体 | 自分で宣言した明示属性 | 評価しない規則の数（zumen） |
| --- | ---: | ---: | ---: |
| IFC2X3 TC1 | 653 | 1320 | 382 |
| IFC4 ADD2 TC1 | 776 | 1491 | 683 |
| IFC4X3 ADD2 | 876 | 1644 | 783 |

比較するのは、実体の名前・抽象指定・上位型、明示属性の名前と順序・省略可否・直接指す型名・単純型、集まりの種別と上下限です。計 2305 実体・4455 属性が対象です。小さな自作スキーマでも外部の読みと正常終了を確かめ、属性・型・上限・名前・抽象指定を変えた比較結果が不一致になることを検査します。

別名の型の定義、列挙と選択の全項目、導出・逆属性、継承属性の再宣言、集まりの項目の省略可否・UNIQUE は比較していません。精度と幅の指定も比較範囲外です。規則の数は zumen の未評価件数であり、外部実装との一致や規則の評価結果を表しません。公開 IFC の `validate`、WHERE・UNIQUE・RULE の評価差、別 OS・wasm の一致は残件です。

毎晩の CI の `ifcopenshell` ジョブでも取得と照合を流します。保存する成果物は集計の `results.json` だけです。台帳の段は、残る `validate` の照合と範囲の整理を終えるまで変更しません。

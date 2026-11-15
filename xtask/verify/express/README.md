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

別名の型の定義、列挙と選択の全項目、導出・逆属性、継承属性の再宣言、集まりの項目の省略可否・UNIQUE は宣言の比較に含めません。精度と幅の指定も比較範囲外です。規則の数は zumen の未評価件数であり、外部実装との一致や規則の評価結果を表しません。

宣言の照合は、Windows の手元と [Linux の毎晩用 CI](https://github.com/zumen-jp/zumen/actions/runs/43745904688)で通過しました。wasm での同じ入力の照合は含めません。

## 公開 IFC の validate

`validate.py` はネイティブ CLI の公開 `validate` を呼び、[公開試料の台帳](../../../samples/ledger.toml)にある IFC 12 件を、IfcOpenShell の[検査 API](https://docs.ifcopenshell.org/autoapi/ifcopenshell/validate/index.html)と照らします。スキーマは上記と同じ固定版を取得し、サンプルも台帳の SHA-256 を検査します。

```sh
cargo build --locked -p zumen-cli --bin zumen
uv run --no-python-downloads --no-build --python 3.13 --with ifcopenshell==0.9.0.post1 --with pytest==9.2.0 python -I -m unittest discover -s xtask/verify/express -p 'test_*.py'
uv run --no-python-downloads --no-build --python 3.13 --with ifcopenshell==0.9.0.post1 --with pytest==9.2.0 python -I xtask/verify/express/validate.py --cli target/debug/zumen --out ../zumen-express-validation
```

Windows では CLI の名前に `.exe` を付けます。2026-11-16 の Windows で次を確認しました。表の左の指摘は、インスタンス番号・実体の型・属性・理由・重複数まで一致した、明示属性の欠落です。右の 3 列は zumen が評価しない検査を外部実装が指摘した数で、一致した指摘へ加算しません。

| 公開サンプル（samples/ifc からの道筋） | 一致した指摘 | 逆属性 | ヘッダー | 追加の規則 |
| --- | ---: | ---: | ---: | ---: |
| bim-whale/LargeBuilding.ifc | 1 | 19 | 0 | 0 |
| bim-whale/TallBuilding.ifc | 0 | 11 | 0 | 0 |
| gni/model_125.ifc | 0 | 0 | 0 | 0 |
| gni/model_18.ifc | 0 | 0 | 0 | 2 |
| gni/model_9.ifc | 0 | 0 | 0 | 1 |
| nakano/nakano_20F_Restaurant.ifc | 0 | 0 | 1 | 66 |
| nakano/nakano_6F_chapel.ifc | 0 | 0 | 1 | 212 |
| nakano/nakano_ConcertHall.ifc | 0 | 0 | 1 | 84 |
| nakano/nakano_Outside.ifc | 0 | 0 | 1 | 41 |
| nakano/nakano_entrance.ifc | 0 | 0 | 1 | 192 |
| rinya/wood-office-architecture.ifc | 52 | 0 | 0 | 37 |
| text2bim/sample_model.ifc | 0 | 3 | 0 | 4 |
| 合計 | 53 | 33 | 5 | 639 |

逆属性は外部スキーマの逆属性宣言で分類します。ヘッダー5 件は `FILE_NAME.name` の省略で、インスタンスの明示属性の検査と分けます。規則評価の有無で、明示属性・逆属性・ヘッダーの指摘が増減しないことも必須です。外部の規則の種別を集計しますが、WHERE・UNIQUE・RULE の全規則を網羅したり、評価結果が正しいことを保証したりする検査ではありません。

IfcOpenShell は同じ版名の組み込みスキーマを使います。上記の宣言照合の、同じ `.exp` を解析する経路とは分けます。林野庁の入力の版名 `IFC4X3` は、既存の公開サンプル照合と同じく、外部実装に渡す一時の写しのヘッダーだけを `IFC4X3_ADD2` に変えます。元の入力と変更した写しのハッシュを記録し、DATA が変わっていないことを検査します。zumen は元の入力を読み、版名の違いを `express.schema.mixed` で報告します。元ファイルは変えません。

この 12 件で外部照合した明示属性の不備は欠落だけです。ほかの型不正・集まりの上限違反などを含む全条件の照合には広げません。未知の外部指摘、指摘の場所や理由の違い、ページの取り残し、入力の改変は失敗にします。検査結果が一致することは、公開サンプルに不備がないという意味ではありません。

毎晩の CI の `ifcopenshell` ジョブで宣言と公開 IFC の照合を流します。保存する成果物は両方の集計 `results.json` だけです。全宣言・スキーマ・一時の IFC・規則の式・メッセージ全文は残しません。台帳の `external` は、ここで示した範囲の証拠です。

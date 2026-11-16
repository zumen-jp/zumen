# IDS の名前の要件を外部実装と照らす

公開の Text2BIM の IFC2X3 に、自分で定めた IDS 1.0 の名前の要件を当て、公開操作の `ids_check` と IfcTester 0.9.0 の判定を比べます（#522）。コアの実装ソースを比較の根拠にしません。

## 入力と期待する値

IFC は [サンプル台帳](../../../samples/ledger.toml)の `ifc/text2bim/sample_model.ifc` を使います。MIT の公開ファイルで、既存の台帳どおりにヘッダーの端末の道筋を除いたものです。258,353 バイト、SHA-256 は `f3e6b444dfc08bea7b1a8c74746ce4ad4752951fd2912df067f1bf65dff72797` です。入力と MIT の本文をキットへ写し、元を変更しません。

`cases.py` に要件と期待する全 GlobalId を固定します。IfcTester の公開 API で IDS を書き、UTF-8・BOM 付き UTF-16LE・BOM 付き UTF-16BE の 3 組を作ります。buildingSMART の例や XSD を写したものではありません。

| 仕様 | 直接のクラス | 名前の要件 | 対象 | 不適合 |
| --- | --- | --- | ---: | ---: |
| `wall-name-required` | IFCWALL | 必須 | 7 | 1 |
| `wall-name-exact` | IFCWALL | `Exterior Concrete Wall` と一致 | 7 | 1 |
| `wall-name-wrong` | IFCWALL | 自作の一致しない値 | 7 | 7 |
| `wall-name-prohibited` | IFCWALL | 存在を禁止 | 7 | 6 |
| `standard-name-required` | IFCWALLSTANDARDCASE | 必須 | 2 | 0 |
| `standard-name-wrong` | IFCWALLSTANDARDCASE | 自作の一致しない値 | 2 | 2 |

IFCWALL は直接のクラスの 7 要素で、IFCWALLSTANDARDCASE の 2 要素を含めません。各文字コードで 6 仕様、延べ 32 対象、17 指摘を比べます。全体では 18 仕様の結果です。同じ IFC 要素を複数の仕様で評価するので、対象や指摘の合計は固有の要素数ではありません。

## 実行

Python 3.13 の専用環境へ、`requirements.txt` の固定版を wheel だけで入れます。IfcTester が宣言する依存のうち、今回呼ばない報告書と BCF の依存は入れません。`bcf-client`・odfpy・pystache・flask はこの検査に不要です。IfcTester と IfcOpenShell は LGPL の外部プログラムとして別に動かし、Cargo の依存や配布物には入れません。公開 API の説明だけを参照し、実装ソースを読みません。

```sh
python -m pip install --only-binary :all: --no-deps -r xtask/verify/ids/requirements.txt
cargo build --locked -p zumen-cli --bin zumen
python -I xtask/verify/ids/check.py --cli target/debug/zumen --out ../zumen-ids-check
python -I xtask/verify/ids/test_check.py ../zumen-ids-check
```

Windows では CLI を `target/debug/zumen.exe` に替えます。出力先は Git の外の新しいフォルダーに限り、既存の内容を消しません。終了値は、一致が 0、不一致が 1、入力・ツール・実行の失敗が 2 です。毎晩の CI の `ids` ジョブも同じ検査を流し、`verify-ids` にキットを残します。

## 比べ方と記録

外部実装の対象・不適合の全 GlobalId を、固定した期待値と照らします。コアの成否・対象数・不適合数・未対応数・指摘総数、各指摘の仕様名・code・重大度・件数・場所・パラメーターを別に照らします。指摘の順序は問いませんが、重複を消さず、欠落・余分な指摘・次ページの取り残しも失敗にします。数値の代わりの真偽値も受け付けません。

同じプロセスで各要求を 2 回呼び、整形した JSON 応答が同一であることも確かめます。生の転送バイト列や環境間の一致を示す検査ではありません。`test_check.py` は保存した実測値を改変する反例と正常な列挙順の変更、出力先の保護を含む 25 テストです。

`results.json` は完了状態、比較結果、入力と CLI の SHA-256、チェックアウトのコミット、比較スクリプトと成果物のハッシュ、外部ツールの全固定版を記録します。コミットだけで渡された CLI のビルド元を証明できるわけではないため、上記のビルドを同じチェックアウトで実行してください。`api.json` に要求と応答、`expected.json` に期待値、各文字コードのファイルに外部観測・コア応答・差を残します。失敗時も未完了を記録し、外部実装の例外本文やソース行は写しません。

2026-11-16 に Windows x64・Python 3.13.17 で 18 結果と比較器の 25 テストが通りました。Linux の結果は CI の実行記録で確認します。

## 範囲の限界

自作 IDS の entity と attribute（Name）のみを対象にします。公開 IDS の再配布可能な例、IFC4/4.3、property・classification・material・partOf・制約式・仕様の全組み合わせは未確認です。自分で定めた要件の照合なので、IDS の読みの段は `synthetic` のままにします。実機の表示、ネイティブと wasm の一致、CAD の実機確認率はこの結果に含めません。

buildingSMART/IDS の `development/LICENSE` は CC BY-ND 4.0 でした（2026-11-16、Git blob `18f0e4d4846260986bd05a14a8988e2342567385`）。個々の例の別の許諾は未確認のため、例と XSD は同梱していません。公開 API は [IfcTester の使い方](https://docs.ifcopenshell.org/ifctester.html)と [API の説明](https://docs.ifcopenshell.org/autoapi/ifctester/ids/index.html)を参照しました。

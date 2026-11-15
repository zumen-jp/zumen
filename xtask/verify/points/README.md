# 点の書き戻しの検証キット

[#342](https://github.com/zumen-jp/zumen/issues/342) の JWW・SFC・P21 を、固定版 ezjww 0.7.6・ezsxf 0.3.7 で照らし、実機に渡す入力・出力・期待値表を作ります。自作の合成入力だけを使い、Jw_cad や SXF ビューアーは起動しません。実機の操作は利用者待ちです。

```sh
cargo build --locked -p zumen-cli
python -m pip install --only-binary :all: ezjww==0.7.6 ezsxf==0.3.7
python -X utf8 -I xtask/verify/points/make.py target/debug/zumen <リポジトリ外の新規フォルダー>
python -X utf8 -I xtask/verify/points/test_make.py <点のキット>
```

Windows では実行ファイルを `zumen.exe` にします。既存の入力は上書きしません。Python 3.12 以降を使い、ソースから依存をビルドしません。手元で uv を使う場合も `--no-python-downloads --no-build --python 3.13 --with ezjww==0.7.6 --with ezsxf==0.3.7` でバージョンを固定できます。

## 確かめる範囲

既存の `make.py` の点追加を再利用し、合成入力 4 件だけで短く再実行できます。JWW には点 2 個と線・円・円弧、SFC・P21 には点 2 個を足します。入力と出力の SHA-256、CLI・検査器・サンプルの指紋、要求・期待値・診断を残します。非対象の要素の ID・ハッシュ・出所・値と表は、CLI の読みで比較します。

| 入力 | 外部比較 | 状態と限界 |
| --- | --- | --- |
| `conformance-open_jww-a.jww` | 追加5要素の形・レイヤ・ペン。点の線種1/100。元の1要素はezjwwが返す全フィールドを保持 | `verified`。Jw_cadで打った普通の点の既定線種と一致するかは実機で確かめる |
| `conformance-open_sxf-a.sfc` | 出所と要素の種類 | `partial`。ezsxfが型で読まないので、点の値の一致には数えない |
| `conformance-open_sxf-c.p21` | 点2個の位置・角度・倍率・レイヤ、用紙への所属 | `partial`。マーカーの種類と色は比較器の範囲外 |
| `made-features.sfc` | 点2個の位置・角度・種類・倍率・色・レイヤ、用紙への所属 | `verified`。非対象要素の全属性を外部実装で照らしたものではない |

外部の監査・警告も入力と比較します。通常の比較器と同じく、SFC/P21 では警告の新しい種類を検出し、件数の増加は別に記録します。編集の候補なし、期待値の欠落、比較した変更数の減少は成功にしません。外部比較の件数は 9 変更で、全 11 変更の確認済みとは扱いません。4 件とも実機は未実行です。このコアの検査だけで、製品の wasm や必須配置変種、実機確認率の分子を増やしません。

## キットと実機への引き継ぎ

`README.md` に出力ハッシュ・要素 ID・座標・レイヤ・マーカーを持つ期待値表と操作手順を、`report.json` に未実行の記録様式を作ります。`results.json` の `complete` は 4 件の処理が完了したことです。各件の `status`・`verified`・`unverified` で確認範囲を判断します。`manifest.json` に要求と出所、`zumen/` に入力と出力の CLI の読み、`licenses/` に自作サンプルの許諾本文を残します。

毎晩の CI でも生成・比較器の改変検査を行い、`verify-bundle` の `342/points-kit/` に保存します。実機では対象と範囲の許可を受けてから、その出力を開きます。保存し直すファイル・画面・ログに利用者のパスが含まれる可能性があるため、Git や issue へ写す前に公開できる範囲を確認します。

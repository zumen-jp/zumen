# POLYLINE の共通属性の照合

自作の 2D POLYLINE 1 件（2 頂点）と、非対象の LINE 1 件を、公開 API と ezdxf 1.4.5 で照らします（[#885](https://github.com/zumen-jp/zumen/issues/885)）。ASCII/binary DXF の R2000・R2004・R2007・R2010・R2013・R2018 を対象に、実寸の空間・用紙（`Layout1`）・名前付きブロック（`QA_BLOCK`、モデル空間に INSERT を 1 つ置く）の 3 つの置き場所の各版で、次の 5 操作を確認します。

| 操作 | 親と各頂点の期待値 | 非対象の保持 |
| --- | --- | --- |
| 同じ色を指定 | ACI 2、画層 `0`、線種 `Continuous` | 入力と同じ |
| 色だけ変更 | ACI 6 | 画層・線種と形を保持 |
| 画層だけ変更 | `QA_TARGET` | 色・線種と形を保持 |
| 線種だけ変更 | 自作の `QA_DASH` | 色・画層と形を保持 |
| 3 属性を変更 | ACI 6、`QA_TARGET`、`QA_DASH` | 形を保持 |

すべての操作で、親・頂点のハンドル、頂点の順序・個数・所有者、変更を要求していない属性、LINE と画層・線種の表を比較します。用紙とブロックでは、置き場所の外のモデル空間の要素（ブロックの INSERT）も比べます。SEQEND の画層は元の値か指定先だけを許し、残る属性を比較します。AUDIT と同じ修復を自前で行い、成功扱いにする処理はありません。

## 実行

同じチェックアウトで CLI をビルドしてから流します。出力先には、リポジトリ外の空のフォルダーを指定してください。

```sh
cargo build -p zumen-cli
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I -m unittest discover -s xtask/verify/polyline -p test_check.py
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I xtask/verify/polyline/check.py --cli target/debug/zumen --out ../zumen-polyline-verify
```

Windows では CLI に `.exe` を付けます。終了コードは、全件一致なら 0、不一致があれば 1、実行できなければ 2 です。不一致の条件も最後まで流し、全 180 件を `results.json` に残します。既存の記録は上書きしません。

wasm では、`cargo build -p zumen-cli --release --target wasm32-wasip1` で作った `zumen.wasm` を `--cli` の代わりに `--wasi` で渡します。Node.js 22 以降の WASI（`bindings/conformance/wasi.mjs`）の上の `serve --stdio` で流します。`--same-as` に先に流したネイティブの `results.json` を渡すと、各条件の出力の SHA-256 が同じことも確かめ、違えば `same-as` の不一致にします。

```sh
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I xtask/verify/polyline/check.py --wasi target/wasm32-wasip1/release/zumen.wasm --out ../zumen-polyline-verify-wasi --same-as ../zumen-polyline-verify/results.json
```

各条件のフォルダーに入力・出力の DXF、公開 API の要求、期待値と監査前の実測値、SHA-256、監査の件数を保存します。同じ入力と変更を独立した 2 セッションへ渡し、出力バイトの一致も確かめます。入力・表・線種の模様は ezdxf の公開 API で自作し、製品に付属する図面や線種を使いません。

ezdxf の `audit` は修復件数へ数えずに子の画層を親へそろえるため、比較する値は監査前に取得します。監査・修復が 0 件でも、頂点の属性が期待値と違えば失敗です。親だけを変更してしまう出力を、8 テストで検出することも確認します。

## 確認した範囲

2026-11-16、`749b2ee5261b5ec7fa023b43d7464608f15b2c60` のネイティブ CLI では、同じ色を指定する 12 件は一致し、単独変更と複合変更の 48 件が不一致でした。各頂点に元の共通属性が残り、単独変更は 2 項目、複合変更は 6 項目が不一致です。監査の誤り・修復は全件 0、非対象の LINE と表は保持され、同じ入力と編集の出力バイトも一致しました。未修正のため終了コードは 1 で、通常の CI の成功条件には追加していません。

この最小入力の R2000 ASCII を AutoCAD 2027 で直列に確認した結果は、次のとおりです。自作 dump が示す頂点の属性の不一致と、AUDIT の件数が対応します。自作の正常図面 0 件・異常図面 2 件の対照も成立しました。

| 操作 | 入力の AUDIT | 出力の AUDIT | 自作 dump の不一致 |
| --- | ---: | ---: | --- |
| 同じ色を指定 | 0 | 0 | なし |
| 色だけ変更 | 0 | 2 | 2 頂点の色 |
| 画層だけ変更 | 0 | 2 | 2 頂点の画層 |
| 線種だけ変更 | 0 | 2 | 2 頂点の線種 |
| 3 属性を変更 | 0 | 6 | 2 頂点の色・画層・線種 |

2026-11-18、POLYLINE の restyle で続く VERTEX に同じ変更を書き、SEQEND の画層をそろえる修正の後は、実寸の空間の 60 件すべてが一致しました（終了コード 0）。AutoCAD 2027 でも、R2000 ASCII の 5 操作と R2018 binary の 3 属性の変更の出力が AUDIT 0 件で、親・頂点・SEQEND の画層と、親・頂点の線種・色が期待値と一致しました。

このスクリプト自体は AutoCAD を起動しません。実機で照合するときは、[一括実行の条件](../../../CONTRIBUTING.md#autodesk-の製品を一括で動かす)と[自作 dump の手順](../README.md#autocad-で開いて照らす)に従い、生成した入力と出力を 1 つずつ開いて保存せずに比較します。製品のメッセージ全文・画面・付属資源は記録しません。

同じ日に、用紙と名前付きブロックを足した 180 件を、`9427ef89` にこの記録の変更を載せて組み立てたネイティブ CLI と wasm32-wasip1 の `zumen.wasm`（Node.js 24 の WASI）で流しました。どちらも 180 件が一致し（終了コード 0）、wasm の出力は全件でネイティブの出力と同じバイトでした（`--same-as`）。AutoCAD 2027 でも、用紙とブロックの 120 件の出力と 24 件の入力が AUDIT 0 件で、親・頂点の画層・線種・色と SEQEND の画層が期待値と一致しました。

最小キットは開いた 2D POLYLINE を対象とします。wasm は WASI の CLI で確かめ、ブラウザーの wasm（`zumen_abi1.wasm`）は適合テストに任せます。DWG・3D・メッシュ・曲線近似や画面・印刷へ結果を広げません。

公開資料は、[DXF Reference の POLYLINE](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-ABF6B778-BE20-4B49-9B58-A94E64CEFFF3.htm)・[VERTEX](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-0741E831-599E-4CBF-91E1-8ADBCFD6556D.htm)と、[ezdxf の公開 API](https://ezdxf.readthedocs.io/en/stable/dxfentities/polyline.html)です。

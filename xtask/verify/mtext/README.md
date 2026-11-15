# MTEXT の DXF 角度の照合

自作の MTEXT 1 件と非対象の LINE 1 件を、公開 API と ezdxf 1.4.5 で照らします（[#882](https://github.com/zumen-jp/zumen/issues/882)）。ASCII/binary DXF の R2000・R2004・R2007・R2010・R2013・R2018 を対象に、各版で次の 4 操作を確認します。

| 入力・操作 | 期待する角度（度） |
| --- | ---: |
| group 50 に 35 を持つ MTEXT を読む | 35 |
| 公開 API で回転 35 の MTEXT を追加 | 35 |
| 回転 0 の MTEXT を回転 35 へ置き換え | 35 |
| 回転 0 の MTEXT に行列 `[0,-1,0,1,0,0]` を適用 | 90 |

## 実行

同じチェックアウトで CLI をビルドしてから流します。出力先には、リポジトリ外の空のフォルダーを指定してください。

```sh
cargo build -p zumen-cli
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I -m unittest discover -s xtask/verify/mtext -p test_check.py
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I xtask/verify/mtext/check.py --cli target/debug/zumen --out ../zumen-mtext-verify
```

Windows では CLI に `.exe` を付けます。終了コードは、全件一致なら 0、不一致があれば 1、実行できなければ 2 です。不一致がある版も最後まで流し、全 48 件を `results.json` に残します。既存の記録は上書きしません。

入力・出力の DXF、公開 API の要求、度数の期待値と両実装の実測値、SHA-256、監査の件数を保存します。同じ入力と変更を独立した 2 セッションへ渡し、出力バイトの一致も確かめます。期待値は操作で決め、自前の再読の値から作りません。図面内の日付や GUID は ezdxf の生成値なので、別に生成した入力同士のバイト一致は確認しません。

## 確認した範囲

2026-11-16、`5de2691cadfa39591f15cb43a23286c57083e07b` のネイティブ CLI では、読み・置き換え・変換の 36 件が不一致、追加の 12 件は一致しました。監査の誤り・修復は全件 0、非対象の LINE は保持され、同じ入力と編集の出力も一致しました。修正完了までは、終了コード 1 が未解消の不一致を表します。通常の CI の成功条件には追加していません。

[Autodesk の公開 DXF リファレンス](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-5E5DB93B-F8D3-4433-ADF7-E92E250D2BAB.htm) の group 50 の単位表記と、ファイルを読んだ実測値は区別します。[ezdxf の公開文書](https://ezdxf.readthedocs.io/en/stable/dxfentities/mtext.html) は度数としています。追加は方向ベクトルを使うため、group 50 を使う読み・置き換えの対照になります。

このスクリプトは AutoCAD を起動しません。製品資源、画面、フォント、印刷を確認した意味ではありません。wasm・別 OS との一致、用紙と名前付きブロックの照合も別の記録で示します。実機で照合するときは、[一括実行の条件](../../../CONTRIBUTING.md#autodesk-の製品を一括で動かす)に従います。

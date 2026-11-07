# AutoCAD の書式を確かめる（#316）

文字列の色と段落、文字スタイルの太字・斜体、マルチ引出線の線ごとの上書きについて、AutoCAD の画面で設定した値と、同じ図面に保存された DXF の値を照らします。毎晩の CI の成果物 `verify-bundle` の `316/controls/` に、変更前の合成図面 `neutral.dxf`、その出所と SHA-256（`source.json`）、比較用の `observe.py` が入ります。フォントは名前で参照します。

キットを手元で作るには、Python の仮想環境に `ezdxf==1.4.5` を入れ、リポジトリのルートで次を実行します。出力先には新しいフォルダーを指定します。

```sh
python -I xtask/autocad/check316/make.py target/controls-316
```

## 基準にする図面

1. AutoCAD を使える環境で、このキットの `neutral.dxf` だけを開きます。`AUDIT` で修正の問いに「いいえ」と答え、誤りの数を記録します。文字スタイル 4 つが Arial の標準書体で、左・右の矢が 1 つの注記につながることを画面で確かめます。
2. R2018 の `before.dwg` として保存し、開き直します。その図面を R2018 の ASCII DXF でも保存し、`before.dxf` とします。これで AutoCAD が読み書きしたときに加わる既定値を基準にできます。
3. 下の各項目は、同じ `before.dwg` の新しいコピーで試します。`Z316_CONTROL` の文字・画層・図形の色は保ちます。編集した DWG を開き直して設定値を画面で読み、同じ R2018 の ASCII DXF に保存します。
4. キットのフォルダーで、下の例のように変更前後と項目名を渡します。両方のファイルはこの合成図面から作ったものを使います。

```sh
python -I observe.py before.dxf after-color.dxf color
```

### MTEXT の文字列の色（`color`）

文字の編集画面で、`Z316_RGB_A` の文字列全体に True Color の RGB（17, 34, 51）、`Z316_RGB_B` に RGB（51, 34, 17）を設定します。図形の色は ByLayer のままにします。`observe.py` は文字列にかかっている小文字の `\c` の整数を読みます。上のバイトから見て `0x332211`・`0x112233` なら BGR、逆なら RGB と報告します。マーカーより前に終わった書式や大文字の `\C` で上書きされた色は数えません。

### MTEXT の段落（`paragraph`）

高さ 2.5 mm の `Z316_PARA_25` と 5 mm の `Z316_PARA_50` を、それぞれ文字の編集画面で選びます。両方の段落を「中央揃え」、左インデントを 5 mm にします。文字の高さと図形の配置は保ちます。比較では `\p` の引数をそのまま返します。中央揃えの `qc` と、左インデントの値が 2 と 1 なら文字の高さの倍数、5 と 5 なら図面の単位という読みと合います。[Autodesk の段落の設定](https://help.autodesk.com/cloudhelp/2022/ENU/AutoCAD-Core/files/GUID-E4DC3A14-3F0A-46AE-9503-6BBEE8DAF916.htm)

### STYLE の太字と斜体（`styles`）

文字スタイル管理（`STYLE`）で、4 つともフォントは Arial のままにして書体を指定します。各スタイル名を使う TEXT が図面に 1 つずつあります。再度開いた画面で、4 つのスタイルの書体を個別に記録します。

| スタイル | 指定する書体 | 1071 のビット 25、24 の予想 |
| --- | --- | --- |
| `Z316_REGULAR` | 標準 | 0、0 |
| `Z316_BOLD` | 太字 | 1、0 |
| `Z316_ITALIC` | 斜体 | 0、1 |
| `Z316_BOTH` | 太字と斜体 | 1、1 |

比較には STYLE の拡張データのうち、アプリケーション名が `ACAD` の 1000（書体名）と 1071 を使います。標準書体の値を基準に、上のビット以外も同じかを調べます。Autodesk の STYLE のリファレンスは 1071 に複数の属性が入るとしています。太字・斜体の指定は個別に記録でき、ビット番号はこの実験で確かめます。[STYLE の DXF グループコード](https://help.autodesk.com/cloudhelp/2017/ENU/AutoCAD-DXF/files/GUID-EF68AF7C-13EF-45A1-8175-ED6CE66C8FC9.htm)・[SetFont の Bold と Italic](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-ActiveX-Reference/files/GUID-DB668114-2395-43C6-858C-2F2514C4BF46.htm)

### MULTILEADER の左右の枝

`Z316_LEADER` 画層には、矢の先が左（−30, 0）と右（30, 0）にあるマルチ引出線が 1 つあります。マルチ引出線を選び、Ctrl キーを使って左の線だけを選択します。下の各行を 1 回ずつ、基準のコピーから始めます。個別に選べない設定は、その版では実施できなかったと記録します。右の線とマルチ引出線全体の設定も、保存の前後で照らします。[Autodesk の線ごとの色の設定](https://help.autodesk.com/cloudhelp/2026/ENU/OARX-ManagedRefGuide/files/OARX-ManagedRefGuide-Autodesk_AutoCAD_DatabaseServices_MLeader_SetLeaderLineColor_int_Color.html)

| 比較する項目名 | 左の線に指定する値 | 線の中に予想するグループコード | 93 の予想値 |
| --- | --- | --- | --- |
| `leader-type` | スプライン | 170 = 2 | 1 |
| `leader-color` | True Color の RGB（17, 34, 51） | 92 = `0xC2112233` | 2 |
| `leader-linetype` | キットの線種 `Z316_DASH` | 340 がその線種を指す | 4 |
| `leader-weight` | 線の太さ 0.50 mm | 171 = 50 | 8 |
| `leader-size` | 矢印サイズ 4.5 | 40 = 4.5 | 16 |
| `leader-arrow` | 矢印の種類「点」（Dot） | 341 がブロック `_DOT` を指す | 32 |

線ごとのフィールドは `LEADER_LINE{` の中から取り出します。比較には左右の矢の座標を使い、線種と矢印はハンドルの参照先で照らします。

## 結果の記録

`observe.py` は対象の値と 2 つの SHA-256 を JSON に出します。終了コード 0 は対照を保って表の予想のいずれかに合ったこと、1 は値が予想と違うこと、2 は対照の変化などで比較できないことを示します。1 も実測結果として残してください。合成入力を使った自動テストの結果は、実機で設定した結果とは別に記録します。

[#316](https://github.com/zumen-jp/zumen/issues/316) には、AutoCAD の製品名と版、項目名、開き直した画面の設定、`AUDIT` の誤りの数、比較結果を記録します。実施できなかった項目と理由も書きます。ほかの図面や利用者の環境のパスは記録に含めません。

点の印刷と MINSERT の再読み込みには、同じ成果物のルートの `README.md` に個別の手順と期待値があります。点は `input/zumen-points-a-r2018.dwg` を PDF に印刷して、画層 0 と記号内の点、印刷しない 2 画層の点を照らします。MINSERT は `316/` の R2000・R2004・R2018 の編集済み DWG を開き、格子と属性、監査結果を確かめます。[既存キットのファイルと期待値](https://github.com/zumen-jp/zumen/issues/316#issuecomment-6610769586)

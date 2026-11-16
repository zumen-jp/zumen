# 建物の作図の慣習（2026-11）

`zumen-building-drawing` は、建物から切り出した図面へ当てる zumen の選択です。出典の数値を参照し、線の割り当て、記号の寸法と形、階名への対応を選んでいます。JIS や国土交通省の基準への適合を保証するプロファイルではありません。確認申請のチェックは別の `mlit-bim-drawing` で扱います。

元は [profile.toml](profile.toml)、配布する値は `cargo xtask codegen` が作る [profile.json](profile.json) です。値を変える場合は既存の `extends` の規則で語彙全体を置き換えます。

## 出典と採用した値

出典は国土交通省大臣官房官庁営繕部「[建築工事設計図書作成基準](https://www.mlit.go.jp/common/001157950.pdf)」（令和 2 年 9 月 30 日改定）です。[掲載ページ](https://www.mlit.go.jp/gobuild/gobuild_tk2_000017.html)と PDF の本文を 2026-11-16 に確認しました。

| 語彙 | 参照した項 | このプロファイルの選択 |
| --- | --- | --- |
| 線幅 | 3.3(2) の組合せ2 | 0.5・0.25・0.13 mm を切断・見え掛かり・隠れ等へ割り当てる。建具の切断は 0.25 mm |
| 線種・レイヤ | 3.3(1)、3.5(6)、4.1(7) | 実線・破線・一点鎖線を使い、紙上の線分と間の長さ、黒の RGB、レイヤ名を独自に選ぶ |
| 材料の模様 | 3.3(3) | 平行線は線幅の 3 倍、交差する線は 4 倍以上の間隔。材料の表は写さず、名前への対応・角度・間隔を独自に選ぶ |
| 軸の記号 | 3.2、3.5(7) | 元の軸の名前を両端の円に置く。円の半径・離す距離・文字の高さは選んだ紙上 mm |
| 切る高さ・深さ・用紙 | 3.4 | 縮尺 1:100、A1 横、基点から 1.2 m 上、面から下へ 3 m を選ぶ。階の基点は呼び出し側が渡す |

開き戸と引き戸は buildingSMART の [IfcDoor](https://ifc43-docs.standards.buildingsmart.org/IFC/RELEASE/IFC4x3/HTML/lexical/IfcDoor.htm) と [IfcDoorTypeOperationEnum](https://ifc43-docs.standards.buildingsmart.org/IFC/RELEASE/IFC4x3/HTML/lexical/IfcDoorTypeOperationEnum.htm) の自然文の意味を参照しています。図を写さず、閉じた戸の幅を局所の X=0〜1、開く側を +Y とし、線と 90 度の弧、移動方向の矢印を自作しました。IFC の左右は日本語の呼び方から推測しません。描く幅には明記された `OverallWidth` を使います。

階名は[公開サンプルで確認した名前の記録](https://github.com/zumen-jp/zumen/blob/f6d038e54e49997c8e2f6644688f634cdd0e6104/docs/decisions/0260-kernel-and-building-items-from-samples.md)の 3 を参照しました。`level_kind` は元の名前から種類を返すだけで、モデルの名前・高さ・種類を変更しません。

## 権利の確認

確認日は 2026-11-16 です。国土交通省の [利用規約](https://www.mlit.go.jp/link.html)は[公共データ利用規約第1.0版](https://www.digital.go.jp/resources/open_data/public_data_license_v1.0)を採用し、出典と加工の明示を求めています。同規約 1.7(3) は CC BY 4.0 による利用も認めています。[別のルールを適用する一覧](https://www.mlit.go.jp/page/kanbo01_hy_003657.html)に、この基準の指定はありません。

このデータは同基準の数値と節を参照して zumen が加工・作成したものです。規約 1.2 の第三者の権利に注意し、基準の図・表・文章、JIS の記号表は転載していません。数値の選択と独自の対応・記号だけを [CC BY 4.0](../../LICENSE.md) で提供します。公的機関が作成・承認したデータとは表示しません。IFC のスキーマ・生成物は同梱しません。

## 適用の範囲

- 線の役割と要素の元のクラスを照合し、クラスを指定した行、役割だけの行の順で選びます。同じ優先度で複数が一致すれば失敗にします。
- 材料名は完全一致です。要素、次に型の材料を使います。構成する材料が別の模様に対応する場合、未対応の材料、開いた・分岐した境界は診断し、模様を補いません。各形の穴を保ち、重なる形は和にしてから模様を置きます。
- 建具は元の Plan の曲線を優先します。既定の記号は `SINGLE_SWING_LEFT`・`SINGLE_SWING_RIGHT`・`SLIDING_TO_LEFT`・`SLIDING_TO_RIGHT` に限ります。割り当てた型の `OperationType` を優先し、不明な型の値を要素の値で補いません。幅が不明なら形から測って補いません。窓の `PartitioningType` は開閉形式として使いません。
- 通り芯の記号は、連続する一直線と名前を確認できる軸だけに足します。曲がった軸を直線に置き換えません。
- 元の建物と線の ID は保ちます。模様・通り芯の記号には別の派生 ID と、元の要素への対応を持たせます。記号の省略は `profile.building-drawing.unsupported` に種類・理由・要素の ID を付けます。

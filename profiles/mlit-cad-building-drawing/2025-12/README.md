# CAD製図基準に合わせる建物の切り出しプロファイル（2025-12）

`mlit-cad-building-drawing` は、建物から切り出した平面図・断面図を提出向けの表記で書き出し、`mlit-cad`（2025-12）の規則でチェックするための zumen の選択です。`mlit-cad` を `extends` で継承するため、同じプロファイル組を `DrawingProfile::load` と `zumen_profile::check` の両方に渡せます。

元は [profile.toml](profile.toml)、配布する値は `cargo xtask codegen` が作る [profile.json](profile.json) です。

## 出典と採用した値

出典は国土交通省「[CAD製図基準](https://www.cals-ed.go.jp/mg/wp-content/uploads/cad8.pdf)」および「[CAD製図基準 解説](https://www.cals-ed.go.jp/mg/wp-content/uploads/cad8_manu.pdf)」（令和 7 年 12 月）、ならびに国土交通省大臣官房官庁営繕部「[建築工事設計図書作成基準](https://www.mlit.go.jp/common/001157950.pdf)」（令和 2 年 9 月 30 日改定）です。いずれも 2026-11-19 に公開ページと PDF を確認しました。

| 語彙 | 参照した項 | このプロファイルの選択 |
| --- | --- | --- |
| レイヤ名 | CAD製図基準 1-5-7（図 1-10、表 1-4） | 責任主体 `D`（設計）と作図対象 `STR`・`BYP`・`BMK`・`DOC`・`DCR` に利用者定義部を続けた `D-STR-CUT` 等を割り当てる |
| 色・線種 | CAD製図基準 1-5-9、1-5-10（表 1-6） | 黒（`#000000`）と `continuous`・`dashed`・`chain` を使い、紙上の線分と間の長さを正の線分長で独自に選ぶ |
| 線幅 | CAD製図基準 1-5-10（3、表 1-7） | A1 の太線 0.5 mm、細線 0.25 mm、寸法・引出線 0.13 mm を役割ごとに割り当てる |
| 軸の記号と文字 | CAD製図基準 1-5-11（2）、建築工事設計図書作成基準 3.2・3.5(7) | 文字高さ 3.5 mm、円の半径 4.0 mm、離す距離 6.0 mm、フォント名 `MS Gothic` を選ぶ |
| 切る高さ・深さ・用紙 | CAD製図基準 解説 1-4-1（解説表 1-1）・1-4-5、建築工事設計図書作成基準 3.4 | A1 横（841 × 594 mm）、縮尺 1:100、平面は基点から 1.2 m 上、深さ 3.0 m を選ぶ |
| 材料の模様 | 建築工事設計図書作成基準 3.3(3) | 線幅の 3 倍（交差する線は 4 倍）以上の間隔とし、材料名への対応・角度・間隔を独自に選ぶ |

建具の記号と階名の種類は [zumen-building-drawing（2026-11）](../../zumen-building-drawing/2026-11/README.md) と同じ選択です。

## 権利の確認

確認日は 2026-11-19 です。国土交通省の[利用規約](https://www.mlit.go.jp/link.html)と電子納品に関する要領・基準の公開ページは[公共データ利用規約第1.0版](https://www.digital.go.jp/resources/open_data/public_data_license_v1.0)を採用し、同規約 1.7(3) により CC BY 4.0 と互換です。

このデータは公開基準の数値・符号の規則を参照して zumen が加工・作成したものであり、基準の本文・表・図や JIS の記号表は転載していません。公的機関が作成・承認したデータとは表示せず、[CC BY 4.0](../../LICENSE.md) で提供します。

## 書き出し形式ごとの扱い

- DXF（`lower_to_dxf`）には `Drawing::at_view_scale` で紙上 mm の円・文字・線種の列を実寸へ直してから渡し、線・円弧・ハッチング・通り芯の円と文字を落とさずに書きます。
- SXF P21（`lower_to_p21`）は最初のビューで用紙へ写し、`drawing-grid-symbol` の `font` から `Standard` 文字スタイルのフォント名を解決して通り芯の文字まで書きます。ハッチングは `convert.element.unsupported`（`hatch`）として元要素の派生 ID 付きで診断します。
- JWW（`lower_to_jww`）は線・円弧・通り芯の円を実寸 mm で書きます。ハッチングと、字幅を推測しないと左下基点へ移せない中央揃えの通り芯文字は `convert.element.unsupported` に派生 ID を付けて診断し、要素を持たない未使用レイヤは変換報告で `Dropped` にします。

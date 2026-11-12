# PLY

PLY（台帳の `ply`）の点群を読み、点群の領域モデルに移す規則を記録します。実装は `crates/zumen-ply` です。持ち主の決めた仕様はないので、広く使われる書き方（台帳の根拠）に従います。

## API

- `open` は、入力の最初の行が `ply` なら PLY として開く。`pointcloud` の op で、要約と点を返す（[LAS・LAZ](las.md) と同じ）。
- 書き出しはしない。

## ヘッダーと本体

- ヘッダーは `format`（`ascii`・`binary_little_endian`・`binary_big_endian`）・`element`・`property`（`list` を含む）を `end_header` まで読む。`comment` と `obj_info` は飛ばす。
- 値の型は `char`・`uchar`・`short`・`ushort`・`int`・`uint`・`float`・`double` と、`int8`〜`float64` の別名を読む。
- ASCII の `float`・`float32` は f32 で読んでからモデルの f64 に広げる。同じ値を入れたバイナリと同じ精度にする（#809）。
- 本体は、ヘッダーの要素の順に、要素の数だけ読む。列の性質（`list`）は、長さの分を飛ばす。本体がヘッダーの数より短いか、値を読めなければ、`ply.file.invalid` を返して開かない。

## 点

- `vertex` の要素を点にする。ほかの要素（`face` など）は移さず、`ply.element.dropped` を返す。
- `x` を東、`y` を北、`z` を標高にする。`x`・`y`・`z` のどれかがなければ、`ply.file.invalid` を返す。
- 色は `red`・`green`・`blue`（`diffuse_` の別名を含む）を読み、8 ビットの値は 257 倍して 16 ビットにする。強度は `intensity`、分類は `classification` を読む。CloudCompare がつける `scalar_` の接頭辞は除いて読む。浮動小数の強度が 0〜1 に収まれば、65535 倍する。
- ほかの性質（法線など）と列の性質は移さず、性質ごとに `ply.property.dropped` を返す。
- PLY は座標系を書かないので、座標系を不明にして `ply.crs.missing` を返す。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ply` | 読み | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-ply)にあります。

- `external` の根拠は、PDAL が作った `samples/ply/pdal-mesh-fixed.ply` 1 件だけ。ASCII、座標は float64 の 4 頂点で、色等の属性は持たない。`crates/zumen-ply/tests/external.rs` は ply-rs 0.1.3 で全頂点の順と全座標をビットで比較する。面 2 件を移さない診断と、座標系が不明なことも確認する。
- 自前の合成は、ASCII・両バイト順の座標・8 ビットの色・16 ビットの強度・分類・落とす法線を照合する。`float` と `float32` は、小数・負のゼロ・丸めの境界・小さな値を、同じ値の ASCII・両バイト順で比較する。合成は公開サンプルの実績に数えない。
- 公開サンプルの入力・要素・作業の予算と、テストキットの停止位置・レコードの順・決定性を確認する。点ごとの ID は返していない。公開 API の適合テストは、公開サンプルと float32 の合成の応答・診断をネイティブと wasm で照らす。これは外部の実装での値の照合とは別に確かめる。
- ply-rs は MIT の dev の依存で、コアが読むための依存ではない。wasm のテストも組み立てられる。列名と頂点の順で比較し、HashMap の列挙順を使わない。build の skeptic と間接の依存も寛容なライセンスで、配布物の実装にリンクしない。
- 実測の点群、属性を持つ公開サンプル、公開サンプルのバイナリ、法線や面のモデル化は未確認。CloudCompare・MeshLab で開いた結果との照合は #504 に残る。受け取った図面と `ZUMEN_SAMPLES` は使っていない。

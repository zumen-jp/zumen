# 0186: CFF・CFF2 の字形の命令をたどって正確な外形を求め、カーニングは GPOS か kern の表から読む

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

[決定 0180](0180-read-truetype-metrics-without-dependency.md) で、TrueType・OpenType のフォントの寸法を依存なしで読んだ。外形が CFF・CFF2 のフォントは `Unread::Cff` で断り、カーニングの表も読まなかった。[決定 0181](0181-formatted-text-layout.md) で `Glyphs::kerning` を足したが、`font` は 0 を返していた（#336、#340）。

CFF の外形は、字形の見出しに外形の値がなく、字形の命令（Type 2 charstring）をたどって求めるしかない。命令は副ルーチンを入れ子に呼べるので、受け取ったファイルでは作業量が入れ子の深さの指数で増えうる（決定 0180 で `ttf-parser` を採らなかった理由の 1 つ）。

## 選択肢

1. 外形を、曲線の制御点まで含めた外形にする。計算は足し算だけで済むが、曲線の膨らみの分だけ外形が大きくなる。
2. 外形を、曲線の極値まで含めた正確な外形にする。曲線ごとに 2 次方程式を解く。
3. CFF は引き続き断り、TrueType の外形のフォントだけを使う。

カーニングは、`GPOS` の pair adjustment と、古い `kern` の表のどちらを読むか、両方あればどちらを使うかを決める。

## 決定

外形は選択肢 2 を採る。TrueType の `glyf` の見出しの外形は、フォントの作者が輪郭から求めた正確な外形なので、CFF もそれに揃える。

- **置き場所。** `zumen_drawing::font` の下に `cff`（字形の命令）と `kern`（カーニング）を置く。依存は足さない。
- **CFF の表。** Adobe の Technical Note #5176（CFF）・#5177（Type 2 Charstring）と、OpenType の仕様の CFF2 の節に従って読む。CID の CFF と CFF2 は、FDSelect（形式 0・3・4）で字形の Font DICT を引き、その Private DICT の局所の副ルーチンを使う。字形の命令が Type 2 でないフォントと、Font DICT が 256 を超えるフォントは `Unread::Cff` で断る。CFF の表が壊れていれば `Unread::Table("CFF ")`（CFF2 は `"CFF2"`）を返す。
- **単位と送り。** 字形の座標は `head` の unitsPerEm の単位とし、FontMatrix は読まない（OpenType の CFF は、座標の単位を unitsPerEm に合わせる）。送りは TrueType と同じく `hmtx` から読み、字形の命令の幅の引数は読み飛ばす。
- **外形。** 線の両端と、曲線の両端と、曲線の各軸の極値（微分が 0 になる 0 < t < 1 の点）を含める。移動だけで何も描かない字形は、外形を持たない。計算は浮動小数の四則と平方根だけで、どの環境でも同じ値になる（[決定 0007](0007-single-rust-implementation.md)）。
- **CFF2。** 可変フォントの既定の形を読む。blend は既定の値だけを残して差分を捨て、差分の数は vstore（ItemVariationStore）の vsindex の領域の数で決める。
- **上限。** 副ルーチンの入れ子は 10 段、引数のスタックは CFF で 48、CFF2 で 513（どちらも仕様の上限）までにする。1 つの字形で読む数と演算子は 65536 までにする。上限を超えた字形・命令の壊れた字形は、フォントにない字形として扱い、`.notdef` で置く。`.notdef` が読めなければフォントを読まない。入れ子は再帰でなく、呼んだ所を配列に積んでたどる。位置の計算は `u64` で行う（決定 0180 と同じ）。
- **予算。** `Glyphs` は予算を受け取らないので、作業量は字形ごとの上限で抑える。配置の作業量は、内容の文字数 × 字形ごとの上限で決まり、内容の長さは読み取りの予算で抑えている（[決定 0010](0010-budget-by-work.md)、[決定 0181](0181-formatted-text-layout.md)）。Font DICT の数（256）と Private DICT の大きさ（64 KiB）も抑え、読むときの作業を表の大きさで決める。
- **読まない命令。** 廃止された seac（endchar の 4 つの引数。標準の符号化の表で字形を引く）と、算術・記憶の演算子（#5177 の 4.5・4.6 節）は読まず、その字形をフォントにないものとする。CFF2 で予約の return・endchar は、CFF と同じ意味に読む。

カーニングは次のとおりにする。

- **GPOS を先に使う。** `GPOS` の `kern` の機能が指す lookup のうち、pair adjustment（type 2 の形式 1・2。extension の type 9 を通したものを含む）があれば、それだけを使い、`kern` の表は使わない。なければ `kern` の表（Microsoft の版 0）の形式 0 の副表のうち、横書きで、最小値・縦の向きでないものを使う。
- **値。** 1 番目の字形の XAdvance だけを、2 文字の間に足す送りにする。2 番目の字形の値と、位置の値（XPlacement など）、デバイスの表は読まない。lookup の中では最初に当たる副表を使い、lookup の間では足す。`kern` の副表は足し、override の副表は置き換える。
- **用字系。** 文字の並びの用字系と言語は分からないので、どの用字系の `kern` の機能の lookup も使う。lookup の flag（記号を飛ばすなど）は見ない。2 文字が隣り合うときだけ当てる。
- **上限。** 表の位置は互いに指し合えるので、ファイルの大きさでは作業量を抑えられない。pair adjustment の副表は 1024、機能から読む lookup の番号は 65536、`kern` の副表は 1024 までにし、超える表は読まない（GPOS を超えれば `kern` の表を使う）。文字の組ごとの作業は、副表の数と二分探索で抑える。
- **読まないもの。** Apple の `kern` の表（版 1）、`kern` の形式 2、GPOS の文脈つきの lookup（type 7・8）と FeatureVariations は読まない。

## 影響

- 外形が CFF・CFF2 のフォント（多くの OpenType の日本語のフォントなど）で、文字を置けるようになる。合成の CFF・CFF2 のフォントで、外形・副ルーチン・FDSelect・blend・入れ子と作業量とスタックの上限を試験し、バイト列を切り詰めたり 1 バイトずつ変えたりしてもパニックしないことを確かめた。
- 配置の送りに、フォントのカーニングが効く。表示（AutoCAD など）がカーニングを当てるかは、まだ比べていない。
- 実在のフォントとの照合（外形の値・カーニングの値）は、表示との照合と合わせて行う。

## 参照

- [決定 0007](0007-single-rust-implementation.md)、[決定 0010](0010-budget-by-work.md)、[決定 0176](0176-text-layout-with-glyph-metrics.md)、[決定 0180](0180-read-truetype-metrics-without-dependency.md)、[決定 0181](0181-formatted-text-layout.md)
- Adobe Technical Note #5176（The Compact Font Format Specification）、#5177（The Type 2 Charstring Format）
- [OpenType の仕様（Microsoft）](https://learn.microsoft.com/en-us/typography/opentype/spec/) の CFF2・GPOS・kern・Common Table Formats・Item variation store の節
- #336、#340

# 0188: シーンの op でフォントを受け取り、文字を字形の形と変換で返す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

フォントのバイト列を読むこと（[決定 0180](0180-read-truetype-metrics-without-dependency.md)・[決定 0186](0186-read-cff-outlines-and-kerning.md)）と、文字の配置（[決定 0176](0176-text-layout-with-glyph-metrics.md)・[決定 0181](0181-formatted-text-layout.md)・[決定 0187](0187-vertical-text-stacks-and-kinsoku.md)）はできたが、境界でフォントを渡す方法がなかった。[決定 0172](0172-scene-sections-and-op.md) は、字形の輪郭をシーンに返すことを「フォントを渡せるようにしてから」とし、[決定 0184](0184-scene-annotations-and-approximations.md) は、線種に添える形と、高さの意味（`text_flag` の `height_frame`・`height_em`）の換算を、フォントを受け取れるようになってからとした（#337）。

決めることは、フォントの渡し方、文字スタイルのフォント名とフォントの対応、フォントがないときの既定と診断、シーンに字形の区画を足すかである。

## 選択肢

渡し方は次から選ぶ。

1. `zm_push` でフォントのファイルを入力として渡し、`scene` の要求の欄でその名前を指す。
2. フォントを読む op を足し、セッションに読んだフォントを持たせる。

シーンの字形は次から選ぶ。

1. 置いた字形ごとに、フォントと字形の番号と変換を返す。描く側がフォントを読んで輪郭を描く。
2. 置いた字形ごとに、共有した形（折れ線の輪）と変換を返す。描く側はフォントを読まない。
3. 字形の輪郭を面の区画に足す。

渡し方の 2 は、セッションの状態と op が増える。色の表（`palette`）と同じく入力の名前で指す 1 が、今の境界の流儀に合う。字形の 1 は、Canvas などは番号で字形を描けず、描く側ごとにフォントを読む道具が要る。3 は、同じ字形を置くたびに輪郭を繰り返し、文字の区画との対応も失う。

## 決定

渡し方は 1、シーンの字形は 2 を採る。

- **op の欄。** `scene` の要求に `fonts`（`scene_font` の列。`name`・`input`・`index`）と `heights`（宣言値のキーから `cap_height`・`em`・`frame`）を足す。小版 37 から。`input` は `zm_push` で渡した TrueType・OpenType（CFF・CFF2 を含む）のファイル、`index` はコレクションの中の番号。読めないフォント・コレクションにない番号は `call.value.invalid`（`/fonts/<n>/input`）、同じ名前と 2 つ目の名前のないフォントも `call.value.invalid` にする。結果の `counts` に `glyphs` を足す。
- **名前の対応。** 文字スタイルと書式の区間のフォントの名前（シーンの `font_names` と同じ、ファイルに書かれた元の名前）で引く。同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。拡張子や系統の名前で推し量ることはしない。呼び出し側は、`font_names` を見て、名前ごとにフォントを並べる。
- **既定と診断。** 名前の合うものがなければ、名前のないフォント（既定）で置き、`scene.font.assumed` を返す。既定もなければ、その文字は字形を作らず文字列だけを返し、`scene.font.missing` を返す。書式の区間のフォントが合わなければ、既定か文字のフォントで置き、`scene.font.assumed` を返す。`fonts` を書かなければ、今までどおり文字列だけを返し、字形の診断も返さない。フォントにない文字は字形 0（.notdef）で置き `scene.glyph.missing`、輪郭が読めない字形は形を空にして `scene.glyph-outline.invalid` を返す。
- **高さ。** 宣言値（`HeightMeaning::Declared`）は `heights` で寸法を決める（決定 0176 の「呼び出し側がキーごとに渡す」）。渡さないキーは、シーンが印のない高さに与えている意味（大文字の高さ）とみなし、`scene.text-height.assumed` を返す。字形で置いた文字の高さは、フォントの寸法で大文字の高さに直し、`height_frame`・`height_em` の印を外す（#335 の残り）。
- **符号。** 高さのキーが `dxf.` で始まる文字（DXF・DWG）は、置く前に特殊な符号（`%%c` など）を解く（決定 0181 の `decode`）。下線と上線は描かない。
- **字形の区画。** シーンの配置の小版を 2 にし、`text_glyphs`（文字ごとの字形の始まり）・`glyph_shapes`・`glyph_axes`（変換の線形の部分）・`glyph_xy`（字形の原点、塊の原点から）・`shape_rings`・`outline_vertices`・`outline_xy`（字形の単位の輪）を後ろに足す。字形を持つ文字は、描く側が文字列の代わりに字形を描く。輪は TrueType・CFF と同じく非零の規則で塗る。字形で置いた上下に重ねた文字の線は、線の区画に足す。
- **輪郭。** `zumen_drawing::font::Font::outline` が、`glyf` の単純な字形と複合の字形（入れ子 8 段、点と部品の数 65536 まで。点の番号で合わせる部品は読まない。後に[決定 0189](0189-composite-points-vertical-kerning-and-kana-kinsoku.md) で、点の番号で合わせる部品と SCALED_COMPONENT_OFFSET を読み、縦書きの上下に重ねた文字を置くことにした）と、CFF・CFF2 の字形の命令（決定 0186 と同じ上限）から、直線・2 次・3 次の区間の輪郭を返す。可変フォントの変化は当てない。
- **折れ線。** 許容差を字形の単位に直した値を 2 の冪に切り下げた刻み（2^-8〜2^16）で、曲線の媒介変数を 2 階の差分から決めた数（区間ごとに 64 まで）に等分する。フォント・字形・刻みの組ごとに 1 つの形にして共有する。刻みは浮動小数のビットから決めるので、どの環境でも同じになる。曲線を折れ線にしたら `scene.curve.approximated` に含め、置いた字形と形の点は予算の `segments` で数える。
- **線種に添える文字。** フォントがあれば字形で置く。高さは TEXT の高さと同じ意味（`dxf.TEXT.40`）とみなす（推定）。形のファイル（SHX）の形は、SHX を読むかの判断（#338）を待って描かず、今までどおり `scene.line-type-shape.dropped` を返す。
- **置かないもの。** 縦書きの 1 行の文字（決定 0176 の置き方が扱わない）、縦書きの上下に重ねた文字・測定値の区間（決定 0181・0187 が置かない）、寸法の決まらない文字は、文字列だけを返し `scene.text-layout.unsupported` を返す。寸法値と表のセルのように、シーンが作る文字は字形にしない。

## 影響

- 字形の区画を足したので、シーンの応答のハッシュが変わり、適合試験のハッシュを作り直した。`cases/scene_fonts.json` が、試験の中で組み立てた合成の TrueType で、字形の区画とフォントの誤りを確かめる。実在のフォントはリポジトリに入れない。
- 描く側は、字形を持つ文字では文字列を描かない。字形を持たない文字（フォントがない・置けない）は、今までどおり文字列と書式から描く。
- 残る作業は、SHX の形と字形（#338）、シーンが作る文字（寸法値・表のセル）の字形、縦書きの 1 行の文字、下線・上線、`dxf.TEXT.40` などの寸法の選び方を表示と比べて決めることである。

## 参照

- [決定 0007](0007-single-rust-implementation.md)、[決定 0010](0010-budget-by-work.md)、[決定 0015](0015-add-only-boundary-and-version-floors.md)、[決定 0172](0172-scene-sections-and-op.md)、[決定 0176](0176-text-layout-with-glyph-metrics.md)、[決定 0180](0180-read-truetype-metrics-without-dependency.md)、[決定 0181](0181-formatted-text-layout.md)、[決定 0184](0184-scene-annotations-and-approximations.md)、[決定 0186](0186-read-cff-outlines-and-kerning.md)、[決定 0187](0187-vertical-text-stacks-and-kinsoku.md)
- [OpenType の仕様（Microsoft）](https://learn.microsoft.com/en-us/typography/opentype/spec/)
- #337

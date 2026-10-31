# 0202: AutoCAD の形のフォント（SHX）は読まず、代わりのフォントで置いて診断を返す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- [0180](0180-read-truetype-metrics-without-dependency.md) は SHX を読まないとし、読むかの判断を残した。Autodesk が公開しているのは形の定義の書き方（SHP の文）で、組み立てた SHX の並びは公開されていない。読むなら、公開された資料と推定により、入手先と権利の根拠を `formats/ledger.toml` と同じ扱いで確かめることになる。
- SHX を読めば、AutoCAD の縦書きのフォントと、形を使う線種を、作った環境と同じ形で描ける。
- [0188](0188-scene-fonts-and-glyph-outlines.md) は、文字スタイルのフォントの名前に合うフォントがなければ既定のフォントで置き、`scene.font.assumed` を返す。既定もなければ字形を作らず、`scene.font.missing` を返す。

## 選択肢

1. SHX を読まない。SHX を指す文字スタイルは、代わりのフォントで置いて診断を返す。
2. 公開された SHP の書き方から SHX の並びを推して読む。

## 決定

選択肢 1 を採る。権利と出所のはっきりしない形式を扱わない。

- SHX を指す文字スタイルは、[0188](0188-scene-fonts-and-glyph-outlines.md) のとおり、名前の合う TrueType・OpenType のフォントか既定のフォントで置き、`scene.font.assumed` を返す。呼び出し側は、SHX と同じ名前を付けた代わりのフォントを `fonts` で渡せる。
- `fonts` の入力に SHX のファイルを渡すと、TrueType・OpenType でないので、今までどおり要求の誤り（`/fonts/<番号>/input` の値）にする。
- 線種に添える形（[0150](0150-drawing-line-type-marks.md)）は、今までどおり図面モデルにフォントと番号で持ち、シーンでは描かない。

## 影響

- コードと境界は変わらない。
- 権利と出所を確かめられる資料が出て、利用者の要望があれば、新しい記録で読むかを決め直す。

## 参照

- [0180](0180-read-truetype-metrics-without-dependency.md)、[0188](0188-scene-fonts-and-glyph-outlines.md)、#338

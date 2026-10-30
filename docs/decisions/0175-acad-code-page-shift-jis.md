# 0175: 2007 より前の版の ANSI_932 の文字は、zumen-acad が encoding_rs で Shift_JIS として戻す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-30 |
| 状態 | 採用 |

## 背景

DXF と DWG は、2007（AC1021）より前の版で、文字の値を見出しの `$DWGCODEPAGE` のコード・ページで符号化する。日本の DXF の多くは AC1015 などの版で、コード・ページは `ANSI_932`（Shift_JIS）である。図面モデルへの変換は、ASCII でない文字を `dxf.code-page.unsupported` で落としていたので、日本の図面では文字・画層の名前・線種・文字スタイルの多くが移らなかった。須坂市の都市計画基本図（AC1015・ANSI_932）では、TEXT 1262 個がすべて落ちた。

`zumen-jww` と `zumen-sxf` は、図面モデルへの変換の feature（`drawing`）の依存にした `encoding_rs` で、Shift_JIS を直接戻している（[決定 0121](0121-sxf-sfc-lift-to-drawing.md)、[決定 0134](0134-jww-lift-to-drawing.md)）。

## 選択肢

1. 復号を共通の crate（`zumen-base`）に寄せ、各形式から呼ぶ。
2. `zumen-acad` も、`drawing` の feature の依存に `encoding_rs` を足して直接呼ぶ。
3. Shift_JIS の表を自前で持つ。

戻せないバイト列の扱いには、置き換えの文字（U+FFFD）にして移す案と、その値を読めないものとして診断で返す案がある。

## 決定

選択肢 2 を採る。復号は `encoding_rs::SHIFT_JIS` を呼ぶだけで、共通にしても包む処理がない。`zumen-base` は L1 だけを使う利用者も依存するので、図面モデルに要る依存を足さない。表を自前で持つ理由もない（[決定 0121](0121-sxf-sfc-lift-to-drawing.md) と同じ）。

- **コード・ページ。** 2007 より前の版で、`$DWGCODEPAGE` が `ANSI_932`（大文字と小文字を区別しない）なら、ASCII でない文字の値を WHATWG の Shift_JIS（Windows の 932 と同じ表）として戻す。ほかのコード・ページと、`$DWGCODEPAGE` がないものは、今までどおり ASCII の文字だけを読み、ASCII でない文字を持つものを `dxf.code-page.unsupported` で落とす。ほかの表は、試料が得られたときに足す。
- **戻せないバイト列。** 置き換えの文字にせず、2007 以降の版の UTF-8 でない値と同じく、そのレコードを `dxf.lift.invalid` で落とす。置き換えると、画層や文字スタイルの名前が別の名前と同じになりうるためである。
- **DWG。** R2000〜R2004 の DWG は、文字列をコード・ページのバイトのまま L1 に持つ。見出しのコード・ページの番号を、DXF と同じ名前の `$DWGCODEPAGE` として L1 の見出しの変数に置き、図面モデルへの変換は DXF と同じ経路で読む。番号の順は ODA のコード・ページの列挙による（推定。30 が ANSI_1252 であることは試料で確かめた）。この変数を変えた書き戻しは受け付けない。
- **書き戻し。** 図面モデルから元の形式へ文字を書き戻すときの符号化は、この決定に含めない。

## 影響

- 須坂市の都市計画基本図の TEXT 1262 個と、線種・文字スタイルが図面モデルに移る。
- DWG の L1 の見出しの変数に `$DWGCODEPAGE` が増える。境界の応答（適合試験のハッシュ）は変わらない。
- `dxf.code-page.unsupported` は、対応表のないコード・ページのときだけ返る。

## 参照

- [決定 0007](0007-single-rust-implementation.md)、[決定 0121](0121-sxf-sfc-lift-to-drawing.md)、[決定 0134](0134-jww-lift-to-drawing.md)
- [docs/formats/dxf.md](../formats/dxf.md)、[docs/formats/dwg.md](../formats/dwg.md)
- #324

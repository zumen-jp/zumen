# 0138: 基準と社内標準を、出典つきの版のあるプロファイル（TOML）として持つ

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-28 |
| 状態 | 採用 |

## 背景

[#9](https://github.com/zumen-jp/zumen/issues/9) で、基準・要領・社内標準を、コードとは別の版のあるデータとして持つ形を決める。旧版は、JSON の規則集で照合し、CAD製図基準（令和 7 年 12 月）を `mlit-cad-2025` として組み込んでいた（[旧版の Checks](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/checks.md)）。旧版の規則集には、語彙と照合の規則が混ざり、出典は節の番号だけだった。

これまでの決定は次を求めている。

- プロファイルは、当面 `zumen/profiles/` に置く（[決定 0004](0004-repositories.md)）。データは CC BY 4.0 にし、フォルダーにライセンスを置く（[決定 0002](0002-dual-license.md)）。
- コアは人向けの文を返さない（[決定 0009](0009-diagnostics-and-catalog.md)）。
- 符号表と新規作成の既定値はプロファイルに置き、解決に使ったプロファイルの版を残す（[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0106](0106-drawing-color-line-type-weight.md)）。
- プロファイルの版は、それぞれの記録で決める（[決定 0015](0015-add-only-boundary-and-version-floors.md)）。

旧版の `mlit-cad-2025` の項目は、CAD製図基準の本編と解説（どちらも令和 7 年 12 月）で、節と表を確かめた。機種依存文字の範囲だけは、解説が文字を列挙しないので、旧版で選んだ値になる。

## 選択肢

1. 旧版の JSON の規則集をそのまま移す。
2. 語彙（値の一覧と対応表）と照合の規則を分け、項目ごとに出典を持つ TOML を元にする。配るときは、生成した JSON を使う。
3. JSON Schema で書いた JSON を、元として手で書く。

## 決定

選択肢 2 を採る。1 は語彙を書き出しの既定値や対応表に使えず、出典も節の番号だけになる。3 は、注釈を書けないので、出典の補足や選んだ理由を残しにくい。カタログ（`catalog/*.toml` から JSON を作る）と同じ流れにする。

- **置き場所。** `profiles/<id>/<edition>/profile.toml` を元にし、`cargo xtask codegen` が同じ場所に `profile.json` を作る。`codegen --check` が、参照と値を確かめる。
- **識別。** `id`（基準の系統）、`edition`（基準の版）、`revision`（このデータの直し、1 から）。照合や書き出しの結果には、この 3 つを残す。基準の版はコードの版と独立に上がる。図面に書く「適用要領基準」の値は `standard-code` に置く。
- **出典。** `sources` に文書（名前・発行者・版・URL・確かめた日）を置く。形式の仕様は、権利の台帳（`formats/ledger.toml`）の名前で指す。語彙と規則は `source` に文書・節・項（表や図）を持つ。出典が列挙しない値は `basis = "chosen"` にする。
- **語彙。** `vocabularies` に、値と、形式の符号との対応（SXF の既定義色のコード、線種の名前など）を置く。照合と、書き出しの既定値の両方から引く。
- **規則。** `rules` に、id、照合の種類（`check`）、重要度（`error`・`warning`・`info`）、出典、使う語彙と、種類ごとの欄を置く。説明の文は持たない。人向けの文は、照合の種類ごとにカタログの code から引き、規則の id・プロファイルの識別・出典を値として渡す。
- **版と互換。** `format` は書式の版（いまは 0.1）。読み手は同じ大版を読み、知らない欄は読み飛ばす。欄の意味を変えるときは大版を上げる。
- **利用者のプロファイル。** 利用者は自前のプロファイルを同じ形で書いて渡す。`extends` で別のプロファイル（id と edition）を指し、同じ id の規則を置き換えるか、規則を足す。
- **ライセンス。** `profiles/` のデータは CC BY 4.0 にする。

## 影響

- `mlit-cad` の 2025-12 版を `profiles/mlit-cad/2025-12/` に移した。旧版の 9 つの規則と同じ値を持つ。線の用途ごとの太さ（表 1-7）、延長サイズ（解説表 1-1）、図枠と表題欄、ファイル名は、まだ持たない。
- 照合の実装（図面モデルから値を集めて規則と比べ、診断を返す）は、まだない。作るときに、照合の種類ごとの code をカタログに足し、`extends` の解き方を決める（#220）。
- 電子納品要領、SXF の実装規約、J-LandXML、IFC データ作成基準は、同じ形で足す。IDS は IFC のモデル（#5）と一緒に、取り込み方を決める。
- 別のリポジトリに分ける時期（決定 0004）まで、ここで版を上げていく。

## 参照

- [決定 0002](0002-dual-license.md)、[決定 0004](0004-repositories.md)、[決定 0009](0009-diagnostics-and-catalog.md)、[決定 0015](0015-add-only-boundary-and-version-floors.md)、[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0106](0106-drawing-color-line-type-weight.md)
- [旧版の Checks](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/checks.md)
- [profiles](../../profiles/README.md)
- [CAD製図基準（令和 7 年 12 月）](https://www.cals-ed.go.jp/mg/wp-content/uploads/cad8.pdf)、[同 解説](https://www.cals-ed.go.jp/mg/wp-content/uploads/cad8_manu.pdf)
- #9

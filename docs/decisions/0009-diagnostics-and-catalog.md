# 0009: 診断を code・場所・忠実度・型付きの値で返し、文言はカタログから引く

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版の `Notice` は `code`・`level`・`message`（日本語）・`count` だけで、どの要素か、仕様のどの節かを持たない。
- 旧版の表示の部品は文言と CSS を持たなかった（[旧版の決定 0005](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0005-browser-packages-for-embedding.md)）。ただし読み取りの結果には、日本語の種類名（`type`）・属性名（`label`）と、整形した値（`value`）が入っていた。整形を TypeScript の参照実装とそろえるために、Rust の側で JavaScript の数値の整形まで再現した（`zumen_core::js`、[0007](0007-single-rust-implementation.md)）。
- 変換で落ちたもの（[旧版の決定 0006](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0006-write-through-the-autocad-model.md)・[旧版の決定 0010](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0010-write-sxf-from-the-autocad-model.md) の notices）は、種類ごとの件数だけだった。要素ごとの結果も、書き出す前の見積もりもなかった。
- [0005](0005-japanese-first.md) で、コアは人向けの文を返さず、文言は日本語と英語のカタログに置くと決めた。[0008](0008-model-layers-and-codecs.md) で、lift・lower・apply が要素ごとの出所と忠実度を返すと決めた。

## 選択肢

1. **旧版の形を広げる。** `Notice` に場所を足し、文言はコアが作り続ける。手間は少ないが、文言と整形の規則がコアに残る。
2. **診断を機械向けの値だけにし、文言は各言語の側で作る。** コアは軽くなるが、同じ診断から言語ごとに違う文ができる。
3. **診断を機械向けの値だけにし、文言はカタログと境界の `zm_describe` で作る。** 文の組み立ても Rust の 1 か所で行い、どの言語からも同じ文を返す。カタログはコアと別に配る。

## 決定

3 を選ぶ。

### 診断の形

```rust
pub struct Diagnostic {
    pub code: Code,                 // "sxf.clothoid.approximated" のような変わらない名前。台帳にある
    pub severity: Severity,         // info・warning・error。code ごとに決まる
    pub fidelity: Option<Fidelity>, // 変換の結果: Equivalent・Approximated { tolerance }・Drawn・Dropped
    pub count: u64,                 // まとめた件数
    pub at: Vec<Ref>,               // ID と、L1 のレコード・バイト範囲。入力の順に先頭の数件
    pub cite: Option<SpecRef>,      // 仕様の名前・版・節
    pub params: Params,             // 文言に差し込む型付きの値
}
```

- **`severity`**
  - `info`: 何も失っていない。別の表現に置き換えた、既定値を仮定した、など
  - `warning`: 近似した、見た目だけにした、落とした、壊れた所を直した。結果は使える
  - `error`: 処理が失敗した、または結果が欠けている
- **まとめ方。** 同じ code で、台帳で決めた値が同じ診断は 1 つにまとめ、件数と先頭の数件の場所を持つ。場所の件数は設定で変えられる。
- **順。** 診断は入力の順に並べる（[0007](0007-single-rust-implementation.md) の決定性）。
- **場所。** ID と、L1 のレコードとバイト範囲（[0008](0008-model-layers-and-codecs.md) の出所）。
- **仕様の節。** 仕様の名前・版・節を、機械向けの値で持つ。仕様の一覧は台帳に置き、仕様の本文は写さない。
- **値。** 数・長さ（単位つき）・角度・ID・図面に記録された名前・列挙の値などを、型のまま持つ。人向けに整形した文字列は持たない。図面に記録された名前は翻訳しない。
- **失敗。** 処理を止めた失敗も、同じ形の診断（`severity` が `error`）で返す。

### 忠実度

| 段 | 意味 |
| --- | --- |
| `native` | 同じ意味の要素 |
| `equivalent` | 別の表現で同じ意味 |
| `approximated` | 許容差つきの近似 |
| `drawn` | 見た目だけ |
| `dropped` | 落とした |

- lift・lower・apply は、要素ごとに段を記録し、要素の種類と段の組ごとに数える。これを忠実度の記録と呼ぶ。
- `native` でない要素には、理由を表す診断を付ける。
- 書き出す前に、忠実度の記録と診断だけを返す見積もり（dry-run）を出せるようにする。
- 忠実度の記録を受け渡す形は [#6](https://github.com/zumen-jp/zumen/issues/6) で決める。

### code の名前と台帳

- **名前。** `<範囲>.<対象>.<出来事>` の 3 つの部分にし、英小文字・数字・ハイフンだけを使う。例: `sxf.clothoid.approximated`、`budget.decoded-bytes.exceeded`、`zip.path.unsafe`
  - 範囲は、形式の系統か符号化（`sxf`・`jww`・`dxf`・`dwg` など）、または横断の部品（`budget`・`zip`・`xml`・`text` など）
  - 出来事は、台帳に決めた語から選ぶ（`unsupported`・`equivalent`・`approximated`・`drawn`・`dropped`・`repaired`・`assumed`・`invalid`・`exceeded`・`cancelled` など）
- **使い回さない。** 一度使った code を、別の意味に使わない。意味を変えるときは新しい code を作る。使わなくなった code は、台帳に「廃止」として残す。
- **台帳。** `catalog/codes.toml` に置く。code ごとに次のものを持つ。
  - `severity` と `fidelity`
  - 値の名前と型、まとめるときに比べる値
  - 既定の仕様の節
  - 開発者向けの説明
  - 足した版と、状態（使用中・廃止）
- **生成と検査。** Rust の code の定数を、台帳から生成する。CI で次を確かめる。
  - 使っている code が、すべて台帳にある
  - 台帳の code に、日本語と英語の文言がある
  - 文言に差し込む名前が、台帳の値の名前と合う

### カタログと文言

- コア（読み取り・書き出し・解析）は、人向けの文を作らない。種類名・属性名・診断の文の代わりに、安定したキー（要素の種類、属性のキー、列挙の値、code）と型付きの値を返す。
- カタログは、`catalog/` に日本語（`ja.toml`）と英語（`en.toml`）で置く。中身は次のとおり。
  - 診断の文のひな形
  - 要素の種類名、属性名、列挙の値の名前
  - 仕様の名前
- 日本語の文を正にし、英語はその訳にする（[0005](0005-japanese-first.md)）。どちらも CI で必須にする。
- 文は、境界の `zm_describe`（[#13](https://github.com/zumen-jp/zumen/issues/13)）がカタログから組み立てる。Rust の 1 か所で作るので、どの言語からも同じ文になる。
  - 数の整形は、`zm_describe` がカタログの規則で行う。OS のロケールは使わない。読み取りは数を整形しない。
  - カタログにない code は、code と値をそのまま返す。古いカタログでも落ちない。
- カタログはコアと同じ版で出し、コアとは別のファイルとして配る。
  - 文言が要らないアプリは、読み込まなくてよい。
  - どのパッケージに含めるかは、各言語の包みで決める。ブラウザーでは、要るときに読み込む。
- カタログのライセンスは、コードと同じ MIT OR Apache-2.0 にする。アプリが文言を組み込みやすいように。

## 影響

- 旧版の `Notice`、日本語の `type`・`label`、整形した `value` は移さない。旧版の notice の code（`unit-assumed` など）は、形式を移すときに新しい名前に付け替え、対応を `docs/formats/` に書く。
- 診断を足す PR は、台帳と、日本語・英語の文言を一緒に変える。
- 文言を変えても code は変わらない。アプリと AI は、code で分岐できる。
- `zm_describe` の引数と、カタログをコアに渡す方法は [#13](https://github.com/zumen-jp/zumen/issues/13) で決める。
- [#7](https://github.com/zumen-jp/zumen/issues/7) の完了の条件のうち、台帳の最初の版は、base の crate と生成の仕組みと一緒に入れる。
- AGENTS.md の「コアは人向けの文を返さない」から、この記録を参照する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「5. 層をまたぐ仕組み」の「ID・出所・診断」、[#7](https://github.com/zumen-jp/zumen/issues/7)
- [0005](0005-japanese-first.md)、[0007](0007-single-rust-implementation.md)、[0008](0008-model-layers-and-codecs.md)
- [旧版の決定 0005](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0005-browser-packages-for-embedding.md)・[旧版の決定 0006](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0006-write-through-the-autocad-model.md)・[旧版の決定 0010](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0010-write-sxf-from-the-autocad-model.md)

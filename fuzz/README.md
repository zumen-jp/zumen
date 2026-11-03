# fuzz

受け取ったバイト列を読む所を、cargo-fuzz で試します（#398・#501、[決定 0010](../docs/decisions/0010-budget-by-work.md)・[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)・[決定 0258](../docs/decisions/0258-fuzz-seeds-and-runs.md)）。登録したすべての形式と、L0 の ZIP・XML・ISO 10303-21 に target があります。どの target も、テストキットの fuzz の関数（`survives`・`survives_lift`・`survives_with`）を呼びます。小さい予算の中で止まり、2 回で同じ結果（読めたか・止まった理由・診断・使った量）になることを確かめます。パニック・止まらない繰り返し・メモリの使い過ぎは、libFuzzer が見つけます。

## target

| target | 試すこと |
| --- | --- |
| `zip` | ZIP を読み、項目を展開し、`.zip` の項目は書庫として辿る。展開した量（ZIP 爆弾）と入れ子が予算で止まり、読めた名前が展開先の外を指さない |
| `xml` | XML を読み、文字と属性の値を戻す。入れ子が予算で止まり、文書型宣言の実体を戻さない |
| `step` | ISO 10303-21 を、フィーチャのコメントを読まない書き方と読む書き方で読み、文字列を戻す。入れ子が予算で止まる |
| `dxf`・`dwg`・`sxf`・`jww` | 読んで図面モデルに移す |
| `saf`・`drawing` | SXF の属性ファイルと、図面モデルのバイナリを読む |
| `ifc` | IFC-SPF か ifcZIP を読み、建物モデルと土木の領域モデルに移す |
| `express`・`ids`・`bcf` | EXPRESS のスキーマ・IDS・BCF を読む |
| `landxml` | 読んで土木の領域モデルに移す |
| `citygml`・`fgd`・`extdm` | 読んで地理の領域モデルに移す |
| `isg`・`par` | ジオイド高の格子（ISG）と、座標の補正量の格子（TKY2JGD・PatchJGD の形）を読む |
| `las`・`e57`・`ply`・`xyz` | 読んで点群の領域モデルに移す。LAZ は、laz の中のパニックを直すまで飛ばす（#510） |
| `delivery` | 電子納品の成果品の書庫を読み、照合する |
| `jpeg`・`boring`・`xlsx`・`csv` | 工事写真・ボーリング交換用データ・Excel の表・CSV を読む |

フォント・プロファイル・レビューの文書・要求の JSON と、領域モデルから図や応答を作る所には、まだ target がありません。#517 で足します。

## 名前の付け方

target の名前は、`xtask/src/fuzz.rs` の検査が求める名前にします。crate が `[package.metadata.zumen]` の `formats` で名乗る形式、`schema/boundary.toml` の `[[detection]]` と `[[modules]]` の形式、L0 の `zip`・`xml`・`step` です。どれかに target がないと、`cargo xtask codegen --check` が落ちます。形式を足したら、`fuzz/fuzz_targets/<名前>.rs` を作り、`fuzz/Cargo.toml` の `[[bin]]` に足してください。

検査が求めない読み取りにも、target を置けます。名前は、読むものの短い英語にします（EXPRESS のスキーマの `express`、補正量の格子の `par`）。

## 動かし方

cargo-fuzz は nightly の Rust を使います。Linux か macOS で動かします。Windows の MSVC では、libFuzzer が使う計測の記号がリンクで見つからず、作れません。CI と同じ版は、`.github/workflows/fuzz.yml` の `NIGHTLY` と `CARGO_FUZZ_VERSION` から読みます。

```bash
NIGHTLY=$(sed -n 's/^  NIGHTLY: //p' .github/workflows/fuzz.yml)
CARGO_FUZZ_VERSION=$(sed -n 's/^  CARGO_FUZZ_VERSION: //p' .github/workflows/fuzz.yml)
```

```bash
rustup toolchain install "$NIGHTLY" --profile minimal
```

```bash
cargo install cargo-fuzz --version "$CARGO_FUZZ_VERSION" --locked
```

```bash
cargo xtask fuzz-seeds
```

```bash
cp Cargo.lock fuzz/Cargo.lock
```

```bash
cargo "+$NIGHTLY" fuzz run -O --debug-assertions zip
```

- `cargo xtask fuzz-seeds` は、どの target にも `fuzz/corpus/<target>/` に種を書きます。種にするのは、次の 4 つです（決定 0258）。受け取った図面と、国土地理院のデータは置きません（AGENTS.md、決定 0019）。
  - 公開サンプル（`samples/`）
  - 適合試験の組の合成の入力
  - L0 の攻撃の形（ZIP 爆弾・膨らむ deflate・書庫の入れ子・パスの走査・XML の外部実体と深い入れ子・ISO 10303-21 の深い入れ子）
  - 合成の格子（ISG・TKY2JGD・PatchJGD の形）
- `cp Cargo.lock fuzz/Cargo.lock` で、fuzz の crate の依存を、ワークスペースと同じ版から解きます。`fuzz/Cargo.lock` はリポジトリに入れません。
- `cargo "+$NIGHTLY" fuzz list` で、target の一覧を出せます。
- 見つけた入力は `fuzz/artifacts/` に出ます。直したら、その入力を最小にして、合成の試験に移します。`corpus` と `artifacts` はリポジトリに入れません。
- 見つけた入力は種か種を変えたもので、表示を求めるライセンス（CC BY・MIT・公共データ利用規約）の公開サンプルから来ることがあります。issue などに貼るときは、最小にした入力にし、元の公開サンプルの出所とライセンス（`samples/ledger.toml`）を添えます。元が分からなければ、種を作ったコミットに固定した URL（`https://github.com/zumen-jp/zumen/blob/<コミット>/samples/ledger.toml`）で、台帳の全体を指します。サンプルを外した後も、指す先から表示が消えないようにするためです（決定 0258）。種を作ったコミットは、artifact の `NOTICE.txt` と、log の checkout に出ます。
  - 毎晩の実行なら、その夜の main のコミットです。
  - PR の実行なら、PR と main を合わせた一時のコミットです。このコミットは、後でどの ref からも指されなくなり、残るとは限らないので、件名の「Merge <PR の head> into <main のコミット>」の 2 つのコミットの台帳を、並べて指します。

## CI

`.github/workflows/fuzz.yml` が、毎晩すべての target を 1 つ 180 秒ずつ、4 つずつ並べて流します（決定 0258）。

- 前の夜までに育てた種を actions/cache で引き継ぎます。キャッシュのキーに公開サンプルの台帳（`samples/ledger.toml`）のハッシュを入れ、台帳が変わったら引き継ぎません。
- 残す前に、target ごとに、通る辺を変えずに種を縮めます（libFuzzer の `-merge=1`）。libFuzzer は起動時に種をすべて流してから時間を数えるので、縮めないと、種が増えるほど新しい入力を試す時間が減ります。縮める前と後の種の数も、summary と log に書きます。
- target ごとの結果の表を、実行の summary と log に書きます。表には、試した入力・通った辺・もっとも遅い入力（1 秒から）・使ったメモリ・流した後の種の数を出します。
- パニック、30 秒を超える入力、3 GB を超えるメモリを見つけたら失敗します。
- 見つけた入力と、1 秒以上かかった入力のうち target ごとにそれまでの最長を超えたものを、artifact の `fuzz-artifacts` に 14 日残します。公開サンプルの出所とライセンス（`samples/README.md`・`samples/ledger.toml`・`licenses/`）と、種を作ったコミット（`NOTICE.txt`）を添えます。
- 公開サンプルを台帳から外すと、台帳のハッシュが変わるので、外したサンプルとそれを変えた入力を含む前の種は戻りません。前の種のキャッシュを早く消す手順は、[`samples/README.md`](../samples/README.md) の「外し方」にあります。
- crate・種の元（`samples/`・`bindings/conformance/cases/`）・fuzz の target・この workflow を変えた PR では、1 つ 20 秒ずつ流します。fuzz の crate はワークスペースの外にあるので、ここで組み立てを確かめます。台帳が同じなら main の育てた種も戻すので、種を流す時間の分、20 秒より長くかかります。
- fuzz の job は、main の必須の検査ではありません。PR で流れたら、通ったことを確かめてからマージします。PR の変更と関係のない失敗（同じ入力が main でも落ちるもの）は、見つけた入力を最小にして issue にしてから、マージしてよいです。必須にするかは、#545 で決めます。

## ライセンス

libfuzzer-sys は LLVM の libFuzzer を同梱し、そのライセンスは NCSA です。fuzz の道具にだけ使い、配る成果物には入りません。`deny.toml` の許可リストに理由を書いてあります。

## 普段の試験

fuzz の道具なしでも、`crates/zumen-abi/tests/contracts.rs` が、試料と合成の入力を途中で切ったものに `survives` と `survives_lift`（図面モデルまで）を流します。

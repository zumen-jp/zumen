# fuzz

受け取ったバイト列を読む所を、cargo-fuzz で試します（#398・#501、[決定 0010](../docs/decisions/0010-budget-by-work.md)・[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)）。登録したすべての形式と、L0 の ZIP・XML・ISO 10303-21 に target があります。どの target も、テストキットの fuzz の関数（`survives`・`survives_lift`・`survives_with`）を呼びます。小さい予算の中で止まり、2 回で同じ結果（読めたか・止まった理由・診断・使った量）になることを確かめます。パニック・止まらない繰り返し・メモリの使い過ぎは、libFuzzer が見つけます。

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

target の名前は、形式の台帳（`formats/ledger.toml`）のキーか、L0 の名前にします。crate が `[package.metadata.zumen]` で名乗る形式と、`schema/boundary.toml` の `[[detection]]`・`[[modules]]` の形式に target がないと、`cargo xtask codegen --check` が落ちます（`xtask/src/fuzz.rs`）。形式を足したら、`fuzz/fuzz_targets/<キー>.rs` を作り、`fuzz/Cargo.toml` の `[[bin]]` に足してください。

## 動かし方

cargo-fuzz は nightly の Rust を使います。CI と同じ版は、`.github/workflows/fuzz.yml` の `NIGHTLY` と `CARGO_FUZZ_VERSION` にあります。Linux か macOS で動かします。Windows の MSVC では、libFuzzer が使う計測の記号がリンクで見つからず、作れません。

```bash
cargo install cargo-fuzz --version 0.13.2 --locked
```

```bash
cargo xtask fuzz-seeds
```

```bash
cargo +nightly-2026-10-30 fuzz run -O --debug-assertions zip
```

- `cargo xtask fuzz-seeds` は、公開サンプル（`samples/`）、適合試験の組の合成の入力、L0 の攻撃の形（ZIP 爆弾・書庫の入れ子・パスの走査・XML の外部実体と深い入れ子・ISO 10303-21 の深い入れ子）を、どの target にも `fuzz/corpus/<target>/` の種として書きます。受け取った図面は置きません（AGENTS.md）。
- `cargo +nightly-2026-10-30 fuzz list` で、target の一覧を出せます。
- 見つけた入力は `fuzz/artifacts/` に出ます。直したら、その入力を最小にして、合成の試験に移します。`corpus` と `artifacts` はリポジトリに入れません。

## CI

`.github/workflows/fuzz.yml` が、毎晩すべての target を 1 つ 180 秒ずつ、4 つずつ並べて流します。前の夜までに育てた種を actions/cache で引き継ぎ、target ごとの結果の表を実行の summary に書きます。パニック、30 秒を超える入力、3 GB を超えるメモリを見つけたら失敗し、入力を artifact の `fuzz-artifacts` に残します。fuzz の target・テストキット・この workflow を変えた PR では、1 つ 20 秒ずつ流します。

## ライセンス

libfuzzer-sys は LLVM の libFuzzer を同梱し、そのライセンスは NCSA です。fuzz の道具にだけ使い、配る成果物には入りません。`deny.toml` の許可リストに理由を書いてあります。

## 普段の試験

fuzz の道具なしでも、`crates/zumen-abi/tests/contracts.rs` が、試料と合成の入力を途中で切ったものに `survives` を流します。

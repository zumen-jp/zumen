# fuzz

登録した図面の形式（DXF・DWG・SXF・JWW）の読み取りを、cargo-fuzz で試します（#398、[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)）。どの target も、テストキットの `survives` を呼びます。小さい予算の中で止まり、2 回読んで同じ結果（読めたか・止まった理由・診断・使った量）になることを確かめます。パニックは libFuzzer が見つけます。

## 動かし方

cargo-fuzz は nightly の Rust を使います。

```bash
cargo install cargo-fuzz
```

```bash
cargo +nightly fuzz run dxf
```

- target は `dxf`・`dwg`・`sxf`・`jww` です。`cargo +nightly fuzz list` で一覧を出せます。
- 種にする入力は、公開サンプル（`samples/`）と、適合試験の組の合成のものから選び、`fuzz/corpus/<target>/` に置きます。受け取った図面は置きません（AGENTS.md）。
- 見つけた入力は `fuzz/artifacts/` に出ます。直したら、その入力を合成の試験に移します。`corpus` と `artifacts` はリポジトリに入れません。

## ライセンス

libfuzzer-sys は LLVM の libFuzzer を同梱し、そのライセンスは NCSA です。fuzz の道具にだけ使い、配る成果物には入りません。`deny.toml` の許可リストに理由を書いてあります。

## 普段の試験

fuzz の道具なしでも、`crates/zumen-abi/tests/contracts.rs` が、試料と合成の入力を途中で切ったものに `survives` を流します。

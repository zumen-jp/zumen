# 0011: 各言語からは C ABI と wasm を FFI で呼び、型は自前の生成器で作る

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 各言語へは、10 個ほどの関数と、スキーマで定めたメッセージからなる 1 本の細い境界で届ける（[#13](https://github.com/zumen-jp/zumen/issues/13)）。境界は、C ABI・wasm・プロセスの外・HTTP の形で出す（[#14](https://github.com/zumen-jp/zumen/issues/14)）。言語の段は [0006](0006-language-tiers.md) で決めた。
- 境界は粗い粒度で呼ぶ。図形 1 つごとに境界を越えないので、1 回の呼び出しの費用は問題にならない。
- 旧版の Python の結合は、手書きで約 5,400 行あった。wasm の結合は、2 つの関数しかなかった。
- 使えそうな道具と、そのライセンス（2026-10-24 に crates.io と GitHub で確かめた）:

| 用途 | 道具 | ライセンス |
| --- | --- | --- |
| 言語ごとの Rust の結合 | PyO3・maturin、wasm-bindgen、jni-rs、swift-bridge、ext-php-rs | MIT OR Apache-2.0 |
| 言語ごとの Rust の結合 | napi-rs、csbindgen・interoptopus（C#）、flutter_rust_bridge、magnus、extendr | MIT |
| 多言語の生成器 | Diplomat（C・C++・Dart・JS/TS・.NET・Kotlin・Python） | MIT OR Apache-2.0 |
| 多言語の生成器 | UniFFI（と、その C#・Go の生成器） | MPL-2.0。実行時に使う crate も MPL |
| C のヘッダーの生成 | cbindgen | MPL-2.0。ビルド時の道具 |
| wasm のランタイム | wasmtime | Apache-2.0 WITH LLVM-exception |
| wasm のランタイム | wazero（Go）、Chicory（Java）、jco | Apache-2.0 |
| Java の FFI | JNA | Apache-2.0 OR LGPL-2.1-or-later（選べる） |
| 依存の検査 | cargo-deny | MIT OR Apache-2.0 |

- UniFFI は、実行時の依存に MPL を入れない規則（AGENTS.md、[旧版の決定 0013](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0013-st-bridge-out-of-zumen.md)）に反する。
- WebAssembly のコンポーネントモデル（WIT）は、ホスト側の対応がそろっていない。wasmtime-py は README で対応をうたうが、wasmtime-dotnet・wasmtime-go・wazero・Chicory の README には記載がない（[#15](https://github.com/zumen-jp/zumen/issues/15) の背景）。

## 選択肢

1. **言語ごとに Rust の結合を書く**（PyO3・napi-rs・jni-rs・csbindgen など）。その言語らしい API を作りやすい。ただし、言語ごとにネイティブの成果物とビルドが増え、Rust の側の包みを言語の数だけ保守する。
2. **多言語の生成器を使う。** UniFFI は MPL なので使えない。Diplomat は寛容だが、Rust の型と関数を各言語に写す道具で、スキーマのメッセージを中心にする境界と合わない。どちらも、VBA などには対応しない。
3. **C ABI のライブラリと wasm のモジュールだけを作り、各言語から FFI か wasm のランタイムで呼ぶ。** 言語の側は、境界の関数の手書きの包みと、スキーマから生成する型だけにする。

## 決定

3 を選ぶ。

### 言語ごとの呼び方

| 言語 | 呼ぶもの | 手段 |
| --- | --- | --- |
| Rust | crate | そのまま使う |
| TS・JS | wasm | ブラウザー・Node.js などの WebAssembly の API。wasm-bindgen は使わない |
| Python | C ABI | ctypes（標準ライブラリ。呼び出しの間は GIL を手放す） |
| .NET | C ABI | P/Invoke（`DllImport`。.NET Framework 4.8 でも動く） |
| C・C++ | C ABI | 生成したヘッダー。C++ には RAII の薄い包みを付ける |
| CLI・MCP | crate | Rust のバイナリ |
| Java・Kotlin | C ABI | JNA（Apache-2.0 を選ぶ）。Android でも動く |
| Swift | C ABI | C のモジュール（module map） |
| Go | wasm | wazero。cgo を使わない |
| Dart | C ABI | dart:ffi |
| VBA | C ABI | `Declare PtrSafe`。32 bit の Windows のために、関数を stdcall で出す |
| 段 3 の言語 | C ABI・wasm・CLI・HTTP | 各言語の FFI・wasm のランタイム・プロセスの起動・HTTP の手順を書く |

- ネイティブで呼ぶ言語は、同じ C ABI のライブラリを使う。プラットフォームごとに 1 度作れば、どの言語のパッケージにも同じファイルが入る。
- wasm で呼ぶ言語は、ブラウザーに配るのと同じモジュールを使う。
- 1 つの言語の中で別の形（wasm の砂場、プロセスの外）を選べるようにするかは、[#14](https://github.com/zumen-jp/zumen/issues/14) で決める。そのときも、下のライセンスの規則に従う。MCP の SDK と HTTP の道具も、[#14](https://github.com/zumen-jp/zumen/issues/14) で選ぶ。

### wasm の ABI

- 当面はコアモジュールに、C ABI と同じ形の小さな ABI を置く。バイト列を渡し、バイト列を受け取る。メモリを確保・解放する関数も export する。
- モジュールは import を持たない。ブラウザーでも WASI のランタイムでも、同じモジュールが動く。
- ホストの対応がそろったら、同じ形のまま WIT に移る。移る前に、各ホストの対応を確かめ直す。

### 生成器

- 生成器は、ワークスペースの xtask に、自前のテンプレートで書く。入力は [#13](https://github.com/zumen-jp/zumen/issues/13) で決めるスキーマで、次のものを作る。
  - 各言語のメッセージの型と、op ごとのメソッド
  - C のヘッダー
- 境界の 10 個ほどの関数の包みは、言語ごとに手で書く。
- Diplomat・UniFFI・cbindgen・wasm-bindgen は使わない。手で書く部分が大きくなったら、この記録を置き換えて見直す。
- 生成したコードはコミットする。CI で生成し直し、差があれば失敗にする。新しい op の PR には、全言語の差分が並ぶ。
- 生成したファイルの先頭には、生成したことと、元のスキーマを書く。

### 道具のライセンス

| 対象 | 許すライセンス |
| --- | --- |
| 成果物に入るもの（実行時の依存、リンクする依存、生成したコード、それを作る生成器とテンプレート） | 寛容なものだけ |
| Cargo の依存のすべて（通常・build・dev） | 寛容なものだけ。cargo-deny で検査する |
| Cargo の外の道具で、成果物に何も残さないもの（コンパイラー、CI の道具、照合に使う外部のソフト） | 問わない |

- cbindgen（MPL）は、作ったヘッダーが成果物に入るので使わない。C のヘッダーは、自前の生成器で作る。
- GPL の実装のソースを読まない規則（AGENTS.md）は、道具でも変わらない。
- 寛容なライセンスの一覧（cargo-deny の許可リスト）は、次のとおり。
  - `MIT`・`MIT-0`・`Apache-2.0`・`Apache-2.0 WITH LLVM-exception`
  - `BSD-2-Clause`・`BSD-3-Clause`・`ISC`・`Zlib`・`0BSD`・`BSL-1.0`・`Unicode-3.0`
- 一覧にないライセンスを足すときは、理由を記録する。
- 依存は crates.io のものに限る。git の依存は、crates.io に公開できない。

## 影響

- 言語ごとのパッケージは、手書きの小さな包みと、生成したコードになる。レビューの重みは、生成器のテンプレートに移る。
- 多くの言語が同じ C ABI のライブラリを使うので、ライブラリの互換性が大事になる。版の方針と、ファイル名に ABI の大版を入れるかは、[#17](https://github.com/zumen-jp/zumen/issues/17) で決める。
- 32 bit の Windows では、関数を stdcall で出す。ヘッダーには、呼び出し規約のマクロを置く。
- 旧版の Python の結合（PyO3）は移さない。PyPI の `zumen`（[0003](0003-names-and-domain.md)）は、新しい大版から ctypes の包みになる。
- Node.js も、当面は wasm で動かす。ネイティブの速さやスレッドが要るときは、napi-rs（MIT）のアドオンを、記録を書いてから足す。
- Java の下限（[#17](https://github.com/zumen-jp/zumen/issues/17)）が 22 以上になったら、JNA の代わりに標準の FFM を使える。
- [#15](https://github.com/zumen-jp/zumen/issues/15) の完了の条件のうち、cargo-deny の設定（`deny.toml`）は、Cargo のワークスペースを作る PR で入れる。
- AGENTS.md の「ビルド時の道具の扱いは #15 で決める」を、この記録への参照に変える。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「7. あらゆる言語から使う」、[#13](https://github.com/zumen-jp/zumen/issues/13)・[#14](https://github.com/zumen-jp/zumen/issues/14)・[#15](https://github.com/zumen-jp/zumen/issues/15)・[#17](https://github.com/zumen-jp/zumen/issues/17)
- [0002](0002-dual-license.md)、[0003](0003-names-and-domain.md)、[0006](0006-language-tiers.md)、[0007](0007-single-rust-implementation.md)
- [旧版の決定 0013](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0013-st-bridge-out-of-zumen.md)

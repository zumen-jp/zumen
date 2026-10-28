# 0007: 読み書きと解析を Rust の 1 つの実装にし、どの環境でも同じ出力を返す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版は、JWW・SXF・DXF を TypeScript で書いてから Rust へ移した。TypeScript の読み取り器は、試験の正解を書く参照実装として残した（[旧版の構成](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/architecture.en.md#keeping-the-two-implementations-equal)）。2 つを値の単位で一致させるために、次のものが要った。
  - `zumen_core::js`（601 行）: `toFixed`・`Number(string)`・`Intl.NumberFormat("ja-JP")` など、JavaScript の数値の扱いの再現
  - `zumen_core::sort`（594 行）: V8 の `Array.prototype.sort`（TimSort）の移植
  - JavaScript のオブジェクトのキーの順と、UTF-16 単位での文字の処理の再現
  - Node.js（ICU）と WHATWG で Shift_JIS の解読が違うバイトを、比べる前に置き換える処理
- それでも値は完全には一致しなかった。Node.js の `Math.sin` は x64 と arm64 で最後の桁が違うので、正解との比較に相対 1e-9 の許容差を置いた。
- DWG（[旧版の決定 0003](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0003-dwg-in-rust.md)）・PDF（[旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md)）・IFC（[旧版の決定 0028](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0028-ifc-in-rust.md)）・JWC・JWS は、すでに Rust だけで読んでいた。参照実装の代わりに、最小の合成ファイル、同じ図面の別の形式（DWG と、同じ図面の DXF）、書き出したものの読み直し、外部のソフトで確かめた。
- これから BIM・土木・点群・納品の形式と、多くの言語（[0006](0006-language-tiers.md)）に広げる。二重実装のままでは、その費用が形式ごとに増える。

## 選択肢

1. **TypeScript の参照実装と Rust の移植を続ける。** 2 つの実装が互いを確かめられる。ただし再現の費用が形式ごとに増え、許容差なしには一致しない。DWG・PDF・IFC には、すでに参照実装がない。
2. **ほかの言語（C++ など）の 1 つの実装にする。** C ABI で各言語に出せる。ただし、受け取ったファイルを読む部分のメモリ安全を、言語が保証しない。旧版の Rust の資産（DWG のデコーダー、PDF・IFC の読み取り）も書き直しになる。
3. **Rust の 1 つの実装にし、同じコードを各環境に結合する。** メモリ安全で、WebAssembly と C ABI に出しやすく、旧版の資産を移せる。同じ結果は、決定性の規則と、環境の間での出力の比較で保つ。

## 決定

3 を選ぶ。

### 範囲

- 読み書き・変換・解析（L0〜L3、[0008](0008-model-layers-and-codecs.md)）と境界の実装は、Rust だけで書く。
- 各言語の側（表示エンジンを含む）は、境界の包みと、表示・操作（描画、当たり判定、吸着など）だけを持つ。ファイルの解釈と、結果として返す値の計算を書き直さない。
- 形式の判定も書き直さない。判定の規則を Rust の側にデータで持ち、TypeScript の判定はそこから生成する。
- 外部の形式プラグイン（[#10](https://github.com/zumen-jp/zumen/issues/10)）も、同じ決定性の規則と適合試験に従う。

### 参照実装の代わり

正しさは、次のもので確かめる。

- **最小の合成ファイル。** 規則を 1 つずつ確かめるファイルを試験の中で作る。期待値は、実装の出力からではなく、仕様から書く。
- **往復の性質。** L1 は、読んで書くとバイト列が一致する。L2 は、lift → lower → lift で等価になる。
- **同じ図面の別の形式。** たとえば DWG と、同じ図面を保存した DXF。形式の違いで説明できない差を調べる。
- **外部のソフトとの照合。** ezdxf、ODA File Converter・DWG TrueView・Jw_cad・SXF ブラウザ（Windows の実機、[旧版の決定 0014](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0014-verify-on-native-windows.md)）など。結果は検証の記録に残す。
- **fuzz。** 壊した入力で、パニックせず、決まった作業量で止まることを確かめる。

出力の記録（スナップショット）は、退行を見つけるためだけに使い、正しさの根拠にしない。記録を更新する PR には、変わった理由を書く。

### どこでも同じ結果

- 同じ版のコアに、同じ入力と同じ設定（予算を含む）を渡すと、どの環境・どの言語の結合からも、ビット単位で同じ出力を返す。許容差は置かない。
- 出力は、書き出したバイト列、L2・L3 のデータ、診断（順を含む）、ID。予算で止まるときも、同じ所で止まる（[#8](https://github.com/zumen-jp/zumen/issues/8)）。
- 次のものは約束に含めない。
  - 環境の資源（メモリなど）が先に尽きて止まった場合
  - 表示エンジンが画面に描いたピクセル。コアが描く PNG・SVG・PDF は含む
  - 版をまたいだ一致。版をまたいで約束するのは ID だけで、ID は浮動小数の計算から作らない
- **確かめ方**
  - CI で、ネイティブの各対象（[#16](https://github.com/zumen-jp/zumen/issues/16) で決める）と wasm32 が同じ入力の組を処理し、出力ごとのハッシュの一覧を作る。最後の job で、すべての一覧が一致することを確かめる。
  - wasm32 では、ブラウザーに配るのと同じモジュールを使う。
  - 各言語の結合は、同じ入力の組を境界（[#13](https://github.com/zumen-jp/zumen/issues/13)）を通して処理し、同じ一覧になることを確かめる。階層 1 は PR ごと、階層 2 は毎晩（[0006](0006-language-tiers.md)）。

### 決定性の規則

- **浮動小数**
  - 四則演算・`sqrt`・`mul_add` など、結果が IEEE 754 で決まる演算は、どの環境でも同じ値になる。これらはそのまま使う。
  - std の文書で精度が決まっていないとされる関数（`sin`・`cos`・`tan`・`atan2`・`exp`・`ln`・`powf`・`powi`・`hypot`・`cbrt` など）は使わず、`libm` crate を使う。std のこれらの関数は、環境や Rust の版によって結果が変わる。
  - NaN の符号とペイロードは、環境によって変わる。出力に書くときは 1 つのビット列にそろえる。
  - 32 bit の x86 では SSE2 を前提にし、x87 の拡張精度を使わない。
  - SIMD などの速さのための経路は、ほかの経路とビット単位で同じ結果にする。wasm の relaxed SIMD は使わない。
- **整数**
  - ファイルの中の位置・長さ・個数は `u64` で持ち、`usize` の幅に頼らない（wasm32 の `usize` は 32 bit）。
  - あふれは checked な演算などで明示的に扱い、debug と release で結果を変えない。
- **順**
  - `HashMap`・`HashSet` の反復の順を、出力や処理の順に使わない。ハッシュ関数を固定しても、`usize` の幅や CPU の命令で順が変わるものがある。`BTreeMap` か、挿入の順を保つ表を使う。
  - 並べ替えの比較は全順序にする。浮動小数は `total_cmp` で比べ、同じ順位は ID か元の位置で決める。
  - メモリの番地を、順や ID に使わない。
- **並列**
  - 結果は入力の順に集め、スレッドの数や終わった順に依存させない。
  - 浮動小数の総和などの畳み込みは、スレッドの数と分け方によらない順で行う。
- **環境**
  - 時計・乱数・環境変数・ロケール・OS のフォントと設定・ファイルシステムを、結果に効く所で読まない。
  - 要るものは呼び出し側から受け取る。たとえば書き出しの日時（DXF の `$TDUPDATE`、ISO 10303-21 の `FILE_NAME` の `time_stamp`）、フォント、外部参照のファイル。
  - 再帰の深さは、スタックの大きさではなく、決まった数で制限する（[#8](https://github.com/zumen-jp/zumen/issues/8)）。
- **依存**も、これらの規則を満たすものに限る。依存を足すときに確かめる。
- 機械で検査できる規則は、clippy の `disallowed-methods`・`disallowed-types` で禁じる。

## 影響

- 旧版の TypeScript の読み取り器、`zumen_core::js`・`zumen_core::sort`、TypeScript で正解を書く仕組みは移さない。形式の知見と、合成の試験ファイルの作り方は移す（[0001](0001-rebuild-in-zumen-jp.md)）。
- ブラウザーでは、どの形式も WebAssembly で読む。モジュールの分け方と大きさは [#14](https://github.com/zumen-jp/zumen/issues/14) で決める。
- 参照実装がない分、外部のソフトとの照合が、正しさの大きな根拠になる。Windows の実機での検証は、[0004](0004-repositories.md) の案の `verify` で行う。
- 決定性の規則は、依存の選び方を狭める。
  - 幾何カーネル（[#11](https://github.com/zumen-jp/zumen/issues/11)、[zumen-jp/zumen-legacy#129](https://github.com/zumen-jp/zumen-legacy/issues/129)）は、wasm32 とネイティブで同じ結果になることを、選ぶ条件にする。
  - 旧版が PDF の読み取りに使った hayro の依存の kurbo は、std があると std の関数を使う。PDF を移すときに、結果に効くかを確かめる。
- `libm` の関数は、プラットフォームのものより遅いことがある。速さより、同じ結果を優先する。
- [#2](https://github.com/zumen-jp/zumen/issues/2) の完了の条件のうち、ネイティブと wasm32 の出力のハッシュの一致の検査は、最初のコーデックと一緒に入れる。ハッシュをかける符号化は [#6](https://github.com/zumen-jp/zumen/issues/6)・[#13](https://github.com/zumen-jp/zumen/issues/13) で決める。それまでは、書き出したバイト列と、試験用の決まった符号化にかける。
- AGENTS.md の「どこでも同じ結果」から、この記録を参照する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「骨格」の 1 と「8. 検証」、[#2](https://github.com/zumen-jp/zumen/issues/2)
- 旧版の構成の [Keeping the two implementations equal](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/architecture.en.md#keeping-the-two-implementations-equal)
- [旧版の決定 0003](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0003-dwg-in-rust.md)・[旧版の決定 0014](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0014-verify-on-native-windows.md)・[旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md)・[旧版の決定 0028](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0028-ifc-in-rust.md)
- Rust の [f64 の文書](https://doc.rust-lang.org/std/primitive.f64.html)（各関数の「Precision」「Unspecified precision」の節）

## 後の記録

- 2026-10-24: [0016](0016-boundary-message-details.md) で、ハッシュの一覧を最後の job で比べる代わりに、コミットした一覧（`bindings/conformance/hashes.txt`）と、どの環境・どの形も比べることにした。

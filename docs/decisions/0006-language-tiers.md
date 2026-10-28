# 0006: 各言語への対応を 3 つの階層に分ける

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 日本の建設で使われる言語は多い。
  - Web: TS・JS、Ruby、PHP、Java、Go、Python
  - CAD・BIM のアドイン: C#、C++、AutoLISP、Python、Ruby
  - Excel・社内の Windows アプリ: VBA、VB.NET、Delphi、PowerShell
  - Jw_cad の外部変形: BAT・VBScript
  - 現場の端末: Swift、Kotlin、Dart
  - 解析・研究: Python、MATLAB、R、Julia、Fortran
- 各言語へは、1 本の細い境界（[#13](https://github.com/zumen-jp/zumen/issues/13)）を通して届ける。新しい言語も、C の FFI、wasm のランタイム、プロセスの起動、HTTP のどれかがあれば使える（[#14](https://github.com/zumen-jp/zumen/issues/14)）。
- それでも、すべての言語に同じ手間をかけることはできない。

## 選択肢

1. **すべての言語を同じに扱う。** 約束が守れない。
2. **少しの言語だけに対応する。** ほかの言語の利用者を置き去りにする。
3. **階層に分け、階層ごとに約束を変える。** 上の階層へ上げる条件を決めておく。

## 決定

3 を選ぶ。

| 階層 | 言語 | 約束 |
| --- | --- | --- |
| 1 | Rust、TS・JS、Python、.NET、C・C++、CLI・MCP | その言語らしいパッケージを作り、PR ごとに適合試験を走らせ、コアと同じ日に公開する |
| 2 | Java・Kotlin、Swift、Go、Dart、VBA | 生成したパッケージを作り、適合試験を毎晩走らせる |
| 3 | Ruby、PHP、R、Julia、MATLAB、Fortran、Delphi、AutoLISP、PowerShell など | C ABI・wasm・CLI・HTTP を使う手順とサンプルを用意し、共通の試験装置で確かめる |

上の階層へ上げる条件は、次の 3 つ。これを、言語を足す PR のチェックリストにする。

- 生成のテンプレートがある
- 境界の 10 個ほどの関数の包みがある
- 適合試験に通る

## 影響

- 階層 1 の言語は、境界とメッセージを変えるたびに PR の中で確かめる。
- 階層 2・3 の言語の不具合は、毎晩の適合試験で見つける。
- 階層の割り当ては、利用者の数と需要に応じて、この記録を置き換えて変える。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「7. あらゆる言語から使う」
- [#13](https://github.com/zumen-jp/zumen/issues/13)〜[#17](https://github.com/zumen-jp/zumen/issues/17)

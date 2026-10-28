# 0001: 新しい org zumen-jp で、コンセプトだけを引き継いで作り直す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版（`zumenjs/zumen`）は、SXF・JWW・DXF・DWG・PDF の読み書き・編集・作図と、ブラウザー・Rust・Python の結合を持つ。
- BIM（IFC）・土木・点群・納品のデータと、多くの言語への対応を足すには、次のことが壁になっていた（詳しくは [#1](https://github.com/zumen-jp/zumen/issues/1) の「捨てるもの」）。
  - TypeScript の参照実装と Rust を、値の単位で一致させていること
  - 表示用のモデルと AutoCAD のモデルを、形式に依存しないモデルの代わりに使っていること
  - 2D を前提にしたコアと、言語ごとに手で書く結合
- org の名前 `zumenjs` は JavaScript だけのものに見え、npm のスコープや URL にも焼き付いていた。

## 選択肢

1. **旧版のリポジトリを少しずつ直す。** 動くものを保てる。ただし上の前提は全体に及んでおり、直すたびに二重実装の一致を保つ費用がかかる。
2. **org の名前を変える。** 手順は少ない。ただし旧名 `zumenjs` が空き、他人に取られると旧 URL の転送が切れ、なりすましにも使われる（repojacking）。
3. **新しい org を作り、コンセプトだけを引き継いで作り直す。** 旧リポジトリは新しい org へ移し、旧 org は空のまま持ち続ける。

## 決定

3 を選ぶ。

- 新しい org は `zumen-jp` にする。日本の建設に向けたものだと一目で分かる。GitHub の `zumen` と npm の `@zumen` は、すでに使われている。
- 中心のリポジトリは `zumen-jp/zumen` にする。
- 旧リポジトリは `zumen-jp/zumen-legacy` として移した（2026-10-24）。`zumenjs` は空の org として持ち続ける。
- 引き継ぐのは、コンセプトと知見と試験の作り方にする。コードは、新しい層の構成に合わせて作り直す。
- 次のものは旧版から移して使う。
  - 形式の知見（`docs/formats/`）
  - 合成の試験ファイルの作り方
  - `ids.json`
  - 検証の記録
  - DWG のデコーダー
- 設計は #1 とその子 issue で進める。

## 影響

- **旧版の保守。** `zumen-jp/zumen-legacy` で続け、新しいリポジトリが旧版の機能に追いついたら archive する。
- **旧版の Pages。** URL は `https://zumen-jp.github.io/zumen-legacy/` に変わった。GitHub は project の Pages の URL を転送しない。
- **npm・PyPI への公開の設定。**
  - 旧版から公開する場合は、trusted publisher の設定を `zumen-jp/zumen-legacy` に直す。
  - 新しいリポジトリから公開するときは、`zumen-jp/zumen` を登録する。
- **GitHub App。** 旧 org に入れていたもの（コードを書くエージェントなど）は、`zumen-jp` にも入れ直す。
- **旧版の issue。** 旧リポジトリに残し、新しい issue からは `zumen-jp/zumen-legacy#番号` で参照する。新しいリポジトリへ移すと、本文の `#番号` が新しいリポジトリの番号を指してしまうため。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1)
- 旧版: [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy)

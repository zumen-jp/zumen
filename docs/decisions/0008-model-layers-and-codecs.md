# 0008: モデルをファイル・領域・派生の 3 層に分け、ファイルモデルを形式の系統ごとに 1 つにする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版では、2 つのモデルが、形式に依存しないモデルの役を兼ねていた。
  - 表示用の `Scene`（用紙の mm、2D）
  - AutoCAD のモデル `DxfDocument`。変換は、このモデルを経て書いた（[旧版の決定 0006](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0006-write-through-the-autocad-model.md)、[旧版の決定 0007](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0007-mapping-crate-between-formats.md)）。SXF には、DXF のシーンが描いた形を書いた（[旧版の決定 0010](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0010-write-sxf-from-the-autocad-model.md)）。
- そのため、形式の間で意味が落ち、変換の側に無理が出た。
  - SXF・JWW に書くと、寸法と引出線が線と文字になる（[zumen-jp/zumen-legacy#112](https://github.com/zumen-jp/zumen-legacy/issues/112)）
  - SXF・JWW が持ち、AutoCAD のモデルが持たないものが落ちる（[zumen-jp/zumen-legacy#113](https://github.com/zumen-jp/zumen-legacy/issues/113)）
  - DXF・DWG の版を変えるにもモデルを経るので、モデルが持たないものが落ちる（[zumen-jp/zumen-legacy#114](https://github.com/zumen-jp/zumen-legacy/issues/114)）
  - 縦横で縮尺の違う部分図が、ビューポートにならない（[旧版の決定 0008](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0008-sheets-and-scales-in-the-autocad-model.md)）
  - 形式による文字高の意味の違いを、変換の側で吸収している（[旧版の決定 0019](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0019-text-heights-between-formats.md)）
- その場の編集は、形式ごとの編集器が、操作で名指ししないものをバイト単位で残して行った（[旧版の決定 0012](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0012-edit-in-place-and-write-every-format.md)）。操作で名指しできない値（拡張データ、寸法スタイルの変数、JWW のヘッダーの設定など）を読み書きする低水準の API はなかった（[zumen-jp/zumen-legacy#117](https://github.com/zumen-jp/zumen-legacy/issues/117)）。
- DWG は、DXF と同じ AutoCAD のデータベースを、別の方法で符号化したもの。`zumen-dwg` だけが、形式の crate どうしの依存の例外だった（[旧版の決定 0003](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0003-dwg-in-rust.md)）。
- 新しい対象には、建物・土木・地理・点群・納品が加わり、3D も最初から扱う（[#1](https://github.com/zumen-jp/zumen/issues/1) の「2. 対象のデータ」）。

## 選択肢

### 層の分け方

1. **1 つの共通モデルに、すべての形式を読む**（旧版の `Scene` を広げる）。表示と解析は 1 度書けばよい。ただし、共通部分だけの薄いモデルでは形式の情報が落ち、何でも入れると巨大なモデルになる。
2. **形式ごとのモデルだけを持ち、形式の組ごとに変換する。** 情報は落ちにくい。ただし変換器が形式の数の 2 乗に増え、表示と解析も形式ごとに書くことになる。
3. **層に分ける。** 形式に忠実なファイルモデルと、形式に依存しない領域モデルを分け、その間を形式ごとに 1 組の変換でつなぐ。表示と解析は、領域モデルの上に 1 度だけ書く。

### ファイルモデルの単位

1. **符号化ごとに 1 つのモデル**（DXF と DWG を別のモデルにする）。同じデータベースの編集・版の変更・変換を、符号化の数だけ書くことになる。
2. **形式の系統ごとに 1 つのモデルと、複数のコーデック。** 同じ意味のデータを 1 つのモデルで扱い、符号化の違いをコーデックに閉じ込める。

## 決定

層の分け方は 3、ファイルモデルの単位は 2 を選ぶ。

### 層

| 層 | 中身 | 約束 |
| --- | --- | --- |
| L0 字句・コンテナ | ZIP、ISO 10303-21、XML、DXF のタグ、MFC CArchive、DWG のビット列・圧縮・CRC、PDF、JPEG・XMP、LAS など | 形式の意味を知らない。ストリームと予算を受け取る |
| L1 ファイルモデル | 形式の系統ごとのモデルと、そのコーデック | 形式に忠実で、何も失わない |
| L2 領域モデル | 図面・建物・土木・地理・点群・納品・レビュー | 形式に依存しない |
| L3 派生 | 表示用シーン（2D・3D）、表、描画（PNG・SVG・PDF・glTF・3D Tiles）、解析 | L2 からいつでも作り直せるキャッシュ |

横断の部品（base: ID・出所・診断・予算、text: 文字コード・フォント、crs: 座標系、geom2d・geom3d: 幾何、units: 単位）は、どの層からも使う。

**依存の向き**

- 下の層は上の層を知らない。
- L2 は L1 を知らない。L1 と L2 をつなぐ lift・lower・apply は、系統と領域の組ごとに、両方に依存する所に置く。L1 は L2 なしでも使える。置き場所（領域ごとの feature か、組ごとの crate か）は試作で決める。
- L3 は L2 から作り、形式ごとに書き分けない。下敷きの画像や PDF のページの扱いは [#4](https://github.com/zumen-jp/zumen/issues/4) で決める。
- L1 の系統どうしは依存しない。旧版の `zumen-dwg` の例外は、AutoCAD の系統の中のことになる。
- L0 の部品は、系統の間で共有する（ISO 10303-21 は SXF と IFC、ZIP は P2Z・SFZ・ifcZIP）。

### L1: 系統ごとに 1 つのモデルと、複数のコーデック

| 系統 | コーデック |
| --- | --- |
| AutoCAD | DXF（ASCII・バイナリ）、DWG |
| SXF | SFC、P21、SFZ、P2Z |
| Jw_cad | JWW、JWC、JWS |
| IFC | IFC-SPF、ifcZIP |

- 同じ意味のデータを別の方法で符号化した形式は、同じ系統のコーデックにする（LAS と LAZ など）。ほかの形式は、1 つの系統に 1 つのコーデックから始める。
- 未知のものも含めて、全レコードを元のバイト範囲とともに辿れるようにする。巨大なデータ（点群など）では、すべてを読み込まず、索引と必要な範囲の読み出しにしてよい。
- 型付きの見方と、生の値の読み書きの両方を公開する。
- 変更はトランザクションで行い、触れたレコードだけを書き直す。
  - 変更に伴って直すレコード（図面の範囲、ハンドルの次の値など）も、触れたレコードに含める。
  - すべての変更を確かめてから書き、失敗したら何も書かない（[旧版の決定 0012](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0012-edit-in-place-and-write-every-format.md) を続ける）。
- 同じ符号化・同じ版・同じ文字コードで書くときは、変えていないレコードを元のバイトのまま書く。
- 版の変更（DXF 2018 → R12、SXF 3.1 → 2.0、JWW の版）と、同じ系統の別の符号化（DWG → DXF）は、L1 の中で書き分ける。出力先が持てないものは、忠実度として記録する。
- 形式の判定の規則（シグネチャ）は、コーデックがデータとして持つ（[0007](0007-single-rust-implementation.md) の形式の判定）。

### L1 と L2 をつなぐ lift・lower・apply

- 系統と領域の組ごとに 1 組を持つ。変換器は、形式の数の 2 乗ではなく、系統の数に比例して増える。
- `lift` は、L1 から L2 を作る。
- `lower` は、L2 から L1 を作る。別の形式への変換と、新規作成に使う。既定値と語彙は、出力先のプロファイル（[#9](https://github.com/zumen-jp/zumen/issues/9)）が決める。
- `apply` は、L2 の変更セットを、L1 の触れたレコードにだけ書き戻す。その場の編集に使う。
- どれも、要素ごとの出所と忠実度を必ず返す。
  - 忠実度は `native`・`equivalent`・`approximated`・`drawn`・`dropped` に分け、種類ごとに数える。記録の形は [#7](https://github.com/zumen-jp/zumen/issues/7) で決める。
  - 出所は、L3 の要素 → L2 の要素 → L1 のレコードとバイト範囲、と辿れるようにする。

### 書き出しの経路

| 場面 | 経路 |
| --- | --- |
| 低水準の編集 | L1 のトランザクションで、レコードと生の値を直接書き換える |
| 同じ形式の編集 | L2 の変更セット → `apply` → L1 の触れたレコードだけを書き換える |
| 版の変更、同じ系統の別の符号化 | L1 の系統の中で書き分ける |
| 別の形式への変換 | L1(A) → `lift` → L2 → `lower` → L1(B) → B のコーデック |
| 新規作成・作図 | L2 → `lower` → L1(B) → B のコーデック |

- どの経路でも、L1 への書き込みはトランザクションで行う。
- 書き出す前に、忠実度の見積もり（dry-run）を返せるようにする。

### 文脈 Cx

- 予算・診断・中断の合図を運ぶ文脈 `Cx` を、作業をするすべての関数に明示的に渡す。旧版の thread_local の上限はやめる。
- 並列に処理するときは、文脈を分けて渡し、診断を入力の順にまとめる（[0007](0007-single-rust-implementation.md) の並列の規則）。
- 予算の数え方は [#8](https://github.com/zumen-jp/zumen/issues/8)、診断の形は [#7](https://github.com/zumen-jp/zumen/issues/7) で決める。

### API の 3 つの水準

| 水準 | 中身 |
| --- | --- |
| 低水準 | L1 の型付きのレコード、生の値、トランザクション |
| 中水準 | L2 の問い合わせ、変更セット（元のハッシュを前提条件に持つ）、変更の取り消し、差分、忠実度の見積もり |
| 高水準 | AI エージェント向けの、スキーマのある JSON の操作 |

- 高水準の操作は、生の値を出さない（[旧版の決定 0017](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0017-add-annotations-and-definitions.md) の判断を続ける）。操作は、中水準の変更セットに直してから適用する。
- 3 つの水準とも、境界の `zm_call` のメッセージとして出す（[#13](https://github.com/zumen-jp/zumen/issues/13)）。どの言語からも、低水準まで使える。
- これを [zumen-jp/zumen-legacy#117](https://github.com/zumen-jp/zumen-legacy/issues/117) への答えにする。そこでの案 1（全形式のファイルモデルを公開し、変えていないものを保って書く）を、全言語に広げたものにあたる。

### 素案

```rust
/// 予算・診断・中断の合図を明示的に運ぶ文脈（thread_local の上限をやめる）
pub struct Cx<'a> {
    pub budget: &'a mut Budget,
    pub diag: &'a mut Diagnostics,
    pub cancel: &'a CancelToken,
}

/// L1: 形式の系統のモデルと、その符号化の 1 つ（DXF と DWG は、同じ AcadDb のコーデック）
pub trait Codec {
    type File: FileModel;
    const SIGNATURES: &'static [Signature]; // 判定の規則はデータ。TypeScript の判定もここから生成する
    fn read(src: &Source, cx: &mut Cx) -> Result<Self::File, ReadError>;
    fn write(file: &Self::File, opts: &WriteOptions, out: &mut dyn Sink, cx: &mut Cx) -> Result<(), WriteError>;
}

pub trait FileModel {
    fn records(&self) -> Records<'_>;     // 未知のものも含む全レコード。元のバイト範囲つき
    fn transaction(&mut self) -> Txn<'_>; // 触れたレコードだけを書き直す
}

/// L1 ⇄ L2: 系統と領域の組ごとに 1 組。出所と忠実度を必ず返す
pub trait Lift<M>: FileModel {
    fn lift(&self, cx: &mut Cx) -> Lifted<M>;
}
pub trait Lower<M>: FileModel + Sized {
    fn lower(model: &M, profile: &Profile, cx: &mut Cx) -> Lowered<Self>;    // 別の形式への変換、新規作成
    fn apply(&mut self, changes: &ChangeSet<M>, cx: &mut Cx) -> Lowered<()>; // その場の編集
}
```

素案の形は、AutoCAD の系統の試作で決める。この記録の約束を守る限り、形が変わっても記録は置き換えない。

## 影響

- 旧版の `Scene` と `DxfDocument` は、形式に依存しないモデルとしては移さない。旧版の `zumen-convert` の変換は、系統ごとの lift・lower に置き換わる。
- 形式を 1 つ足すには、L1 のモデルとコーデック、領域ごとの lift・lower・apply が要る。旧版の「読み取り器から `Scene` へ」より部品は多いが、表示と解析は L2・L3 に 1 度だけ書けばよい。
- L1 が全レコードを持つので、旧版よりメモリを使う。大きなファイルで測り、性能の予算を CI に置く（[#1](https://github.com/zumen-jp/zumen/issues/1) の「8. 検証」）。
- 決めることが、ほかの issue に分かれる。
  - L2 の中身: 図面は [#4](https://github.com/zumen-jp/zumen/issues/4)、建物は [#5](https://github.com/zumen-jp/zumen/issues/5)。形式に固有の情報を L2 にどう残すか（型付きの拡張など）も、そこで決める
  - L2・L3 を渡すバイナリの形式: [#6](https://github.com/zumen-jp/zumen/issues/6)
  - プロファイル: [#9](https://github.com/zumen-jp/zumen/issues/9)
  - 境界のメッセージ: [#13](https://github.com/zumen-jp/zumen/issues/13)
- [#3](https://github.com/zumen-jp/zumen/issues/3) の完了の条件のうち、AutoCAD の系統（DXF・DWG）の試作は、別の PR で行う。L1 での編集、版の変更、L2 への `lift` を試し、素案の形を決める。
- AGENTS.md の「新しい層の構成」から、この記録を参照する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「骨格」「3. 全体の構造」「5. 層をまたぐ仕組み」、[#3](https://github.com/zumen-jp/zumen/issues/3)
- [旧版の決定 0003](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0003-dwg-in-rust.md)・[旧版の決定 0006](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0006-write-through-the-autocad-model.md)・[旧版の決定 0007](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0007-mapping-crate-between-formats.md)・[旧版の決定 0008](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0008-sheets-and-scales-in-the-autocad-model.md)・[旧版の決定 0010](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0010-write-sxf-from-the-autocad-model.md)・[旧版の決定 0012](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0012-edit-in-place-and-write-every-format.md)・[旧版の決定 0017](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0017-add-annotations-and-definitions.md)・[旧版の決定 0019](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0019-text-heights-between-formats.md)
- [zumen-jp/zumen-legacy#112](https://github.com/zumen-jp/zumen-legacy/issues/112)・[zumen-jp/zumen-legacy#113](https://github.com/zumen-jp/zumen-legacy/issues/113)・[zumen-jp/zumen-legacy#114](https://github.com/zumen-jp/zumen-legacy/issues/114)・[zumen-jp/zumen-legacy#117](https://github.com/zumen-jp/zumen-legacy/issues/117)
- [0007](0007-single-rust-implementation.md)

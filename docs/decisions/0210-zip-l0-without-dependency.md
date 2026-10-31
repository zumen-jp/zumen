# 0210: ZIP の L0 は、inflate を含めて依存を足さずに zumen-zip で読み、ZIP64 は読み、暗号化は読まない

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

ifcZIP・BCF・PLATEAU と基盤地図情報の配布の書庫・P2Z と SFZ・xlsx は、どれも ZIP の書庫に入れる。中央ディレクトリを読み、項目を展開する L0（[決定 0008](0008-model-layers-and-codecs.md)）を作り、これらで共有する（#365）。受け取ったファイルなので、パニックせず、作業量に上限を持ち（[決定 0010](0010-budget-by-work.md)）、ネイティブと wasm32 で同じ結果を返す必要がある（[決定 0007](0007-single-rust-implementation.md)）。書庫には、小さな入力が大きく展開される書庫の爆弾と、`..` などで書庫の外を指す名前がある。旧版は `miniz_oxide` で展開し、ZIP64 と暗号化を扱わなかった。

## 選択肢

1. `zip` を足す。
2. 中央ディレクトリは自前で読み、展開に `miniz_oxide` を足す。
3. 依存を足さずに、inflate（RFC 1951）も書く。

選択肢 1 は、暗号化・bzip2・zstd などの依存を引き、書庫の爆弾の検査と予算を外で行うことになる。

選択肢 2 は、ライセンス（MIT・Zlib・Apache-2.0）と wasm32 を満たす。ただし、展開の途中で中断の合図を確かめるには、ストリームの API で少しずつ展開することになる。

選択肢 3 の inflate は、zlib に含まれる puff と同じく、符号を 1 ビットずつたどれば 300 行ほどで書ける。表を引かないので遅いが、配る書庫の大きさなら足りる。展開の途中で、出力の上限と中断の合図を、そのまま確かめられる。前例として、[決定 0180](0180-read-truetype-metrics-without-dependency.md) と [決定 0209](0209-xml-l0-without-dependency.md) も依存を足さずに書いた。

## 決定

選択肢 3 を採り、`crates/zumen-zip` に置く。速さが要るようになったら、表を引く方法に変えるか、選択肢 2 ともう一度比べる。

- **読むもの。** 中央ディレクトリの終わりの記録を後ろから探し、中央ディレクトリの見出しを順に読む。項目ごとに局所の見出しを読み、データの範囲を決める。大きさと CRC-32 は、中央ディレクトリの値を使う（データ記述子の値は読まない）。
- **ZIP64。** 読む。ZIP64 の終わりの記録とその位置の記録、見出しの ZIP64 の拡張の欄を読む。配布の書庫には 4 GiB を超えるものがあるためである。
- **読まないもの。** 複数の部分に分けた書庫は `zip.split.unsupported` で断る。暗号化した項目は `zip.encryption.unsupported`、stored（0）と deflate（8）でない方式の項目は `zip.method.unsupported` を報告し、その項目を除いて残りを読む。図面の交換で、暗号化した書庫とほかの方式は見つかっていない。
- **名前。** UTF-8 の印（ビット 11）があれば UTF-8、なければ Shift_JIS として読む。文字にできない名前と、`zumen_base::name::is_safe` で安全でない名前（[決定 0016](0016-boundary-message-details.md)）は、`zip.path.unsafe` を報告して除く。ディレクトリの項目は、終わりの `/` を除いて確かめる。同じ名前の項目は、初めのものを使い、残りを `zip.duplicate.dropped` で除く。
- **書庫の爆弾。** 局所の見出しからデータの終わりまでの範囲がほかの項目と重なる項目は、どれも `zip.range.unsafe` で除く。重ならなければ、展開の作業は入力の大きさで抑えられる。展開した大きさは、展開する前に記録の値を `decoded-bytes` で数え、展開がその値を超えたら止めて `zip.data.invalid` にする。
- **予算。** 入力を `input-bytes`、項目を `records`、展開した大きさを `decoded-bytes` で数える。書庫の中の書庫は `Archive::open` で開き、入れ子の段の数を `depth` で数える。
- **確かめること。** 展開した大きさが記録と合わなければ `zip.data.invalid`、CRC-32 が合わなければ `zip.crc.invalid` を返し、データは返さない。CRC-32 は `zumen_base::crc` に置き、DWG の R2004 の見出しと共有する。
- **書き出し。** 書き出しは、P2Z・SFZ の issue で足す。

## 影響

- `encoding_rs` は zumen-acad・zumen-jww・zumen-sxf がすでに使うので、新しい依存はない。
- 試験はファイルを同梱せず、試験の中で書庫を組み立てる。deflate のデータは、zlib で圧縮したバイト列を試験に書く。切り詰めたり 1 バイトずつ変えたりしても、パニックしないことを確かめる。
- ifcZIP（#407）・BCF（#429）と段階 3 からの書庫の形式は、この L0 の上で読む。

## 参照

- [決定 0007](0007-single-rust-implementation.md)、[決定 0008](0008-model-layers-and-codecs.md)、[決定 0010](0010-budget-by-work.md)、[決定 0016](0016-boundary-message-details.md)、[決定 0180](0180-read-truetype-metrics-without-dependency.md)、[決定 0209](0209-xml-l0-without-dependency.md)
- [APPNOTE.TXT - .ZIP File Format Specification 6.3.10（PKWARE）](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
- [RFC 1951: DEFLATE Compressed Data Format Specification version 1.3](https://www.rfc-editor.org/rfc/rfc1951)

# 旧版の赤入れをレビュー文書に移す

旧版の `zumen-markup` バージョン 1 の JSON、その JSON が指す図面、対象とビューの対応表を用意します。コマンドは手元で図面の SHA-256 を旧版の記録と比較し、一致した図面の `v2:` ハッシュを新しい文書の対象に記録します。[旧版の書式](https://github.com/zumen-jp/zumen-legacy/blob/7c1c400e9791bd60f4a5bba355ca90362ad607c2/docs/markup.md)・[現行の書式](decisions/0218-review-document-and-ops.md)

## 対応表を作る

元の図面を現行の `open`・`drawing` で開き、用紙の ID と表示を確認します。旧版の `view` と同じ位置・向きの用紙またはビューを選び、次のように `mapping.toml` に書きます。この例の `sheet/0` と `sheet/h12` は、実際の図面で確かめた ID に置き換えます。

```toml
target = "plan"
default_view = "sheet/0"

[views]
"二階平面図" = "sheet/h12"

[topic_ids]
"指摘-1" = "00000000-0000-4000-8000-000000000001"
```

- `target` は新しい文書内でその図面に付ける ID です。
- `views` は、元の `view` の値を現行の用紙・ビューの ID に結びます。元の項目が `view` を省略している場合だけ `default_view` を使います。
- `topic_ids` は、旧版の `finding` のうち UUID 以外の ID を持つものに用意します。指摘ごとに UUID を 1 つ決め、対応表を保管してください。元から UUID の指摘はその ID を使います。2D の場所には元の項目の ID が残ります。

## 移行して確認する

ソースを取得したリポジトリのルートで実行します。ファイルはリポジトリの外に置けます。出力先に新しいファイル名を指定します。

```sh
cargo xtask migrate-markup markup.json drawing.dxf mapping.toml review.json
```

図形の件数、指摘の件数、現行のフィールドに直接移せなかった値の件数と元の JSON Pointer が標準エラーに出ます。コマンドは変換結果を `Review::read` で読み直して ID と参照を検査し、成功した文書を保存します。ハッシュの不一致・対応の不足・不正な座標の場合は、示された位置を確認してもう一度実行します。

| 旧版の項目 | 新しい文書で確かめる値 |
| --- | --- |
| `redline`・`cloud` | `markups` の ID・対象・ビュー・点の順・文字 |
| `text` | `markups` の最初の点と文字列 |
| `photo` | `markups` の最初の点と `text` に入る写真の参照。参照先を開く操作は利用側で行います |
| `finding` | `topics` の UUID・状態・作者・日時・題と、2D の場所の ID・ピン。本文があれば題に使い、省略時は元の項目の ID を使います |

指摘の多角形は、座標の最小・最大で囲む `extent` と、元の頂点を持つ `legacy.points` になります。図形と指摘が混在した場合は `/items` を報告し、両方を含めた元の描画順を `legacy_item_order` に旧版の項目の ID で残します。元のビュー名は `legacy.view` でも確認できます。

コマンドが報告した各フィールドの元の値は、対応する文書・対象・図形・指摘の `legacy.` で始まるキーにあります。たとえば `/items/0/entities` は、その項目の `legacy.entities` です。旧版の要素 ID・引き継ぎの記録・線幅・矢印・撮影方向などの固有の値は、用途に応じてこの報告から照合できます。

図面の SHA-256 は対象の `legacy.sha256` でも確認できます。`sfc`・`p21`・`sfz`・`p2z` は現行の形式名 `sxf` に合わせ、元の形式名を `legacy.format` に保ちます。ツールの 1 回の入力上限は図面 256 MiB、旧版の JSON 8 MiB、対応表 1 MiB、項目 16,384 件です。対応表に指定する対象とビューの ID は UTF-8 の 1〜1024 バイトです。[決定 0282](decisions/0282-legacy-markup-review-identity.md)

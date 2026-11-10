# 0253: 地理の地物と点群を wasm の civil・pointcloud のモジュールに分け、J-LandXML と civil の op は要るモジュールのそれぞれに置く

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-03 |
| 状態 | 採用 |

## 背景

[決定 0221](0221-wasm-modules.md) は、wasm のモジュールを `zumen-abi` の feature で図面と BIM に分け、土木・地理と点群のモジュールは feature と表の行を足して作るとした（#380）。[決定 0232](0232-point-clouds.md) は点群のモジュール（LAS・LAZ・E57・PLY・XYZ と、TIN との差のための土木の領域モデルと J-LandXML の読み取り）を、[決定 0233](0233-geo-model-features.md) は地理の領域モデルと CityGML・基盤地図情報・拡張 DM を土木・地理のモジュールに入れるとした。いまは、どれも図面のモジュールにある。

決めることは 2 つ。

- J-LandXML と `civil` の op の置き場。[決定 0239](0239-civil-drawing-op.md) は、J-LandXML の線形から図を作る `civil_drawing` を図面のモジュールに置いた。作った図は DXF の文書として同じセッションで開くので、DXF の読み書き（図面のモジュールのコード）が要る。点群との差（`deviation`）は、J-LandXML の TIN と点群を同じセッションで使う。
- IFC4.3 の線形の置き場。IFC4.3 の線形は、IFC を開くと土木の領域モデルにもなり、`civil` の op で使える（[決定 0235](0235-ifc-alignments.md)）。IFC は BIM のモジュールだけが読む。

## 選択肢

### J-LandXML と土木の op

1. J-LandXML の読み取りと `civil` の op を、図面・土木地理・点群の 3 つのモジュールに置く。判定の表は図面のモジュールを指す。`civil_drawing` は図面のモジュールに残す。
2. J-LandXML と `civil`・`civil_drawing` を土木・地理のモジュールに移す。DXF の読み書きも土木・地理のモジュールに入る。
3. J-LandXML を土木・地理のモジュールだけに置き、`civil_drawing` と `deviation` には、モジュールをまたいで図面モデルのバイナリや土木の領域モデルのバイナリで渡す。

### IFC4.3 の線形

1. BIM のモジュールにも `civil` の op を置く。
2. 土木・地理のモジュールにも IFC の読み取りを置く。
3. すべてを入れた `zumen_abi1.wasm` だけで使えるようにする（いまの形）。

## 決定

J-LandXML は選択肢 1、IFC4.3 の線形も選択肢 1 を採る。土木・地理のモジュールは `civil`、点群のモジュールは `pointcloud` と名付ける。

- J-LandXML の選択肢 2 は、DXF の読み書きを土木・地理のモジュールにも入れるので、分ける意味が薄れる。選択肢 3 は、`civil_drawing` の答え（DXF の文書）と `deviation` の要求の形を変える大きな変更で、決定 0239 と決定 0232 を改めることになる。J-LandXML の読み取りと土木の領域モデルは小さいので、3 つのモジュールに置いても大きさへの影響は小さい。
- IFC4.3 の線形の選択肢 2 は、IFC の読み取り（BIM のモジュールの大半）を土木・地理のモジュールにも入れる。選択肢 3 は、分けたモジュールを使うラッパーで IFC4.3 の線形を使えない。選択肢 1 で BIM のモジュールに増えるのは、土木の領域モデルと線形の計算だけ（brotli で約 30 KB）。
- モジュールをまたいでバイナリで渡す仕組みは、まだ作らない。いまの op は、どれも 1 つのモジュールで閉じる。

### モジュール

| モジュール | 形式 | op（capabilities・open・close・describe のほか） |
| --- | --- | --- |
| `drawing` | DXF・DWG・SXF・JWW・図面モデルのバイナリ・J-LandXML | 図面の op、`review_carry`・`review_adopt`、`civil`・`civil_drawing`、`delivery_check`・`photo`・`sheet`・`boring`・`lift` |
| `bim` | IFC | 建物の op、`validate`・`review_carry`・`ids_check`・`review_adopt_ids`・`bcf_read`・`bcf_write`・`civil` |
| `civil` | J-LandXML・CityGML・基盤地図情報・拡張 DM | `civil`・`geo` |
| `pointcloud` | LAS・LAZ・PLY・XYZ・E57・J-LandXML | `civil`・`pointcloud`・`deviation` |

- 判定の表は、J-LandXML を図面のモジュールに、CityGML と基盤地図情報を `civil` に、LAS・PLY・E57 を `pointcloud` に向ける。名指して開く拡張 DM は `civil`、XYZ は `pointcloud` が読む。持たないモジュールで名指すと、`/format` の値の誤り（`call.value.invalid`）を返す。
- 分けたモジュールは `zumen_abi1_civil.wasm`（`--no-default-features --features civil`）と `zumen_abi1_pointcloud.wasm`（`--features pointcloud`）。すべてを入れた `zumen_abi1.wasm` と、ネイティブ・CLI・Go のラッパーは、今までどおりすべてを入れる（`zumen-abi` の既定の feature に `civil`・`pointcloud` を足す）。
- capabilities の形式の一覧は、入れたモジュールの形式の一覧にある形式だけを返す。`cargo xtask codegen` が、`has_op` と同じ形で `has_format` を作る。
- 適合テストの組の `module` は、地理の組（CityGML・基盤地図情報・拡張 DM）を `civil` にする。点群の組と `deviation` の組は、op から `pointcloud` が選ばれる。
- SIMA はまだ読まない。足すときは `civil` のモジュールに置く。座標の換算（`zumen-crs`）は、各形式の読み取りが使う小さな crate なので、要るモジュールのそれぞれに入る。

## 影響

- 図面のモジュールから、地理の領域モデルと 3 つの形式、点群の読み取り（LAZ の展開を含む）が抜ける。BIM のモジュールには、土木の領域モデルと線形の計算が入る。分ける前と後の大きさ（`wasm32-unknown-unknown` の release、名前のセクションを除いたもの。brotli は Node.js の `zlib.brotliCompressSync` の既定の水準）:

| モジュール | 前（バイト） | 前（brotli） | 後（バイト） | 後（brotli） |
| --- | --- | --- | --- | --- |
| すべて | 7,185,368 | 1,452,239 | 7,186,250 | 1,453,048 |
| 図面（`drawing`） | 6,542,525 | 1,319,517 | 5,889,456 | 1,170,885 |
| 建物（`bim`） | 1,483,986 | 407,217 | 1,603,024 | 437,262 |
| 土木・地理（`civil`） | — | — | 1,176,593 | 341,242 |
| 点群（`pointcloud`） | — | — | 1,426,201 | 403,514 |
| プラグインの土台（`plugin`） | 773,672 | 152,133 | 773,672 | 152,133 |

- ラッパーで J-LandXML の TIN と点群の差を求めるときは、`forOp("deviation")` で点群のモジュールを読み込み、同じセッションで両方を開く。
- CI とリリースは、分けたモジュールを 5 つ（リリースはプラグインの土台を除く 4 つ）作る。

## 参照

- [決定 0013](0013-boundary-in-four-forms.md)・[決定 0221](0221-wasm-modules.md)・[決定 0232](0232-point-clouds.md)・[決定 0233](0233-geo-model-features.md)・[決定 0235](0235-ifc-alignments.md)・[決定 0239](0239-civil-drawing-op.md)
- #380・#416
- 0.2.0 で LAZ の展開を止めたことは、[0289](0289-release-cut-and-shipped-code.md) で定めた。

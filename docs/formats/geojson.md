# GeoJSON

IETF RFC 7946（`rfc_7946`）の GeoJSON を読み書きし、地理の領域モデル（`zumen-geo`）と相互に変換する規則を記録します。実装は `crates/zumen-geojson` です。API では、`open` に `format: "geojson"` を書いて名指しで開き、`geo` の op で要約と地物の中身を返し、`convert` に `format: "geojson"` を書いて地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON など）から GeoJSON を書き出します。仕様の本文は写しません（`formats/ledger.toml`）。節の番号は RFC 7946 のものです。

## L1 と予算

- UTF-8 の JSON テキストを読む（1.4 節）。先頭に UTF-8 の BOM（`EF BB BF`）があれば読み飛ばし、診断やレコードのバイト範囲は元の入力の先頭からの位置で持つ。
- L1 のレコード（`FileModel`）は、非空の `FeatureCollection` では `features` 配列の各要素（重ならない昇順のバイト範囲）とし、空の `FeatureCollection`・単体の `Feature`・単体の `Geometry` では根オブジェクトの 1 件とする。
- JSON の字句・構文・領域モデルへの変換・書き出しは、`Cx` の予算の下で行う。全体のバイト数を `InputBytes`、文字列・数値・木構造・出力 JSON の確保を `DecodedBytes`、配列・オブジェクト・入れ子プロパティ・`GeometryCollection`・立体の入れ子を `Depth`、字句とメンバーと頂点の走査を `Work`、L1 のレコード数を `Records`、移した地物と図形の数を `Elements` で数える。予算超過や中断で止まるときは、扱っていたバイト範囲（または地物の出所）を付けた診断を報告して止まる。数値の `-0.0` は `0.0` に正規化する。JSON として読めないときと、根が 9 種の GeoJSON オブジェクト（`FeatureCollection`・`Feature`・7 種の Geometry）でないときは、`geojson.file.invalid` を返して失敗する。
- 同じオブジェクトの中に同じ名前のメンバーが複数あるときは、後に現れたメンバーを採用する。`bbox` と仕様にない追加メンバー（Foreign Members、6.1 節）は読み飛ばす。

## 座標参照系（4 節）

- 座標参照系は WGS 84 の経緯度（`Crs::Wgs84`、EPSG:4326）とし、座標の並びは `[経度, 緯度]` または `[経度, 緯度, 標高]`（3.1.1 節）として読む。高さの基準（楕円体高か標高か）はファイルから決まらないので `GeoModel::height` は `None` にする。
- 旧版の GeoJSON（GJ2008）の `crs` メンバーがあれば、`geojson.crs.dropped` を報告して無視し、RFC 7946 の WGS 84（経度・緯度）として読む。

## 地物と属性（3.2・3.3 節）

- `FeatureCollection`: `features` 配列の要素を順に地物（`Feature`）にする。`Feature` でない型が混ざっていれば、`geojson.type.unsupported` を報告して飛ばす。
- `Feature`:
  - `id`（文字列または数値）があればその文字列を ID にする。ないときや前の地物と重なるときは、`Feature/<1 からの連番>` で一意の ID を作る。
  - `kind` は `"Feature"` にする。根が単体の Geometry（`Point` など）のときは、1 つの地物にし、`kind` をその Geometry の型名、ID を `<型名>/1` にする。
  - `properties` がオブジェクトなら、メンバーを入力順に属性にする。文字列は `Value::Text`（`"name"` が文字列なら `Feature::name` にも入れる）、`i64` に収まる整数表記は `Value::Integer`、それ以外の有限の数値は `Value::Real`、真偽値は `Value::Bool` にする。入れ子のオブジェクトは外から `/` で名前をつないで平坦化する。`properties` 自体が `null` のときは空の属性とし（3.2 節）、オブジェクトの中のメンバーの値が `null` または配列のときは `geojson.property.dropped` を報告して移さない。

## 7 種の Geometry（3.1 節）

- 位置（Position、3.1.1 節）は 2 つ以上の有限の数値の配列とし、緯度が `[-90, 90]` の外にあるものや数値が足りないものは `geojson.geometry.invalid`（`reason = "position"`）を報告してその形を移さない。経度は日付変更線をまたぐ表現（3.1.9 節の `> 180` など）を許すため、有限であることだけを確かめる。4 つ目以降の要素があれば有限の数値であることだけ確かめて値は無視する。
- `Point`・`MultiPoint`: `Form::Point` にする。
- `LineString`・`MultiLineString`: 2 点以上の位置の配列を `Form::Line` にする（3.1.4 節）。2 点未満は `geojson.geometry.invalid`（`reason = "line_positions"`）にする。
- `Polygon`・`MultiPolygon`: 線形環（3.1.6 節）の配列を `Form::Area` にする。外周とすべての穴の環がそれぞれ 4 点以上であり、始点と終点が一致することを確かめ、終点を除いた開いた輪として `outer`（最初の環）と `holes`（2 番目以降の環）に入れる。外周または穴のいずれか 1 つでも 4 点未満（`reason = "ring_positions"`）や未閉合（`reason = "ring_unclosed"`）であれば、`geojson.geometry.invalid` を報告してその `Polygon` 全体を移さない（穴だけを消した中実の面にはしない）。読むときは環の回転方向が右手の法則と逆でも拒まない。
- `GeometryCollection`: `geometries` 配列の要素を再帰的にたどり（`Depth` で深さを制限）、得られた形を同じ地物の `geometries` に並べる。

## 書き出し（`lower_to_geojson`）

- `GeoModel` を RFC 7946 の `FeatureCollection` の UTF-8 JSON（BOM なし、最短の浮動小数点表記）に書く。同じモデルからは常に同じバイト列になる。
- 座標系の扱い:
  - `Crs::Wgs84`: そのまま `[経度, 緯度]`（標高があれば第 3 要素も）を書く。
  - `Crs::Geographic`（EPSG:6668）: JGD2011（JGD2024）の経緯度をそのまま WGS 84 として書き、`geojson.datum.approximated` を報告する。
  - `Crs::Plane(zone)`（EPSG:6669〜6687）: `zumen-crs` で JGD2011 の経緯度に逆投影してから書き、`geojson.datum.approximated` を報告する。逆投影できない点は `crs.projection.unsupported` と `geojson.geometry.dropped` を報告してその形を省く。
  - `None`（形を持つ地物があるとき）は `geojson.crs.missing`、`Crs::Jgd2000`・`Crs::Tokyo` は `geojson.crs.unsupported` を報告して失敗する。
- 形の書き方:
  - 地物の形が空なら `"geometry": null`、1 つなら単体の Geometry、複数なら 2D の同種図形（すべて点なら `MultiPoint`、すべて線なら `MultiLineString`、すべて `Form::Area` 由来の面なら `MultiPolygon`）は Multi 系にし、立体の複数面や異種が混ざるときは `GeometryCollection` にする。
  - `Form::Area` の外周と穴の輪は、水平の `(経度, 緯度)` 上で連続する同一点を除いて 3 点以上かつ非ゼロの面積を持つことを確かめ、外周が反時計回り、穴が時計回り（3.1.6 節の右手の法則）になるように向きを整えて始点を末尾に足す。外周または穴のいずれか 1 つでも退化・変換不能であれば、`geojson.geometry.dropped`（`form = "area"`）を報告してその面全体を省く。
  - `Form::Solid`（`FaceSet`・`Triangles`）は、水平投影で非ゼロの面積を持つ面だけを閉じた `Polygon` にし（鉛直な壁面や一直線に潰れる面は省く）、1 面なら `Polygon`、複数面（上面と底面のように水平投影が重なりうるものを含む）なら `GeometryCollection` に並べて `geojson.solid.approximated` を報告する。すべての面が省かれた立体や、頂点の座標自体が不正な面を含む立体は `geojson.geometry.dropped`（`form = "solid"`）を報告する。
- 属性の書き方:
  - `Feature::kind` が `"Feature"` でなく同名の属性がなければ `"kind"`、`Feature::code` があれば `"code"`、`Feature::name` があり同名の属性がなければ `"name"` を出し、続けて `attributes` を入力順（重複キーは後勝ち）に出す。`Value::Integer` はそのまま JSON の整数リテラルで書く。有限でない `Value::Real`・`Value::Measure` は JSON に書けないため、`geojson.attribute.dropped` を報告してその属性を省く。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `geojson` | 読み | `synthetic` |
| `geojson` | 書き | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-geojson)にあります。

- 外部実装との照合（書きの `external`）: `zumen-geojson` が WGS 84・JGD2011 経緯度・JGD2011 平面直角座標系から書き出した GeoJSON（点・線・穴あき面・鉛直壁と上面・底面を持つ閉じた 3D 立体を含む）を Shapely 2.2.0（GEOS）で読み、有効な図形（`is_valid`）として読めることと、図形種別・向き・座標・属性の一致を確かめた（[GeoJSON の外部照合](../../xtask/verify/shapely/README.md)）。
- 合成テストにとどまる範囲（読みの `synthetic` と書きの境界条件）: 再配布条件と個人情報の不在を確認した第三者の公開サンプルはまだ `samples/` に置いていないため（`#504`）、読みはテストの中で組み立てた GeoJSON（`crates/zumen-geojson/tests/geojson.rs`、`bindings/conformance/cases/geojson.json`）と Shapely 2.2.0 に書かせた合成 Geometry（`crates/zumen-geojson/tests/external.rs`）による `synthetic` とする。QGIS などの GIS ソフトとの実データ照合も `#504` で扱う。

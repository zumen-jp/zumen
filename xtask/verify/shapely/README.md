# GeoJSON の外部照合

`zumen-geojson` の書き出しと読み取りを [Shapely](https://github.com/shapely/shapely) 2.2.0（GEOS）と照らします（`#876`・`#504`）。`crates/zumen-geojson/tests/external.rs` が、コアの出力を別の Python プロセスで動く `verify.py` に渡して図形の妥当性（`is_valid`）・回転方向・座標・属性を検査し、あわせて Shapely が出力した 7 種の合成 Geometry をコアで読み比べます。外部ツールのソースとバイナリはリポジトリに置かず、Cargo の依存にも加えません。

## 実行

Python は、すでに入っているものを使います。

```powershell
uv venv --python 3.14 --no-python-downloads target/shapely-venv
uv pip install --python target/shapely-venv/Scripts/python.exe --only-binary :all: shapely==2.2.0 numpy==2.5.4
$env:ZUMEN_SHAPELY_PYTHON = (Resolve-Path target/shapely-venv/Scripts/python.exe).Path
cargo test --locked -p zumen-geojson --test external -- --nocapture
```

ほかの環境では、既存の Python に付く venv も使えます。

```bash
python3 -m venv target/shapely-venv
target/shapely-venv/bin/python -m pip install --only-binary :all: shapely==2.2.0 numpy==2.5.4
ZUMEN_SHAPELY_PYTHON="$PWD/target/shapely-venv/bin/python" cargo test --locked -p zumen-geojson --test external -- --nocapture
```

`ZUMEN_SHAPELY_PYTHON` がなければ `python` を使います。Python か Shapely がないときは外部照合だけを飛ばし、理由をテストの出力に書きます。`ZUMEN_SHAPELY_PYTHON` または `ZUMEN_SHAPELY_REQUIRED` を指定したときは、不在も失敗にします。バージョン違い・起動の失敗・図形の不正や値の食い違いは、指定の有無によらず失敗です。

## 入力と比較する値

- 書き出しの照合（書きの `external` の根拠）:
  - WGS 84・JGD2011 経緯度・JGD2011 平面直角座標系 IX 系の `GeoModel` から `lower_to_geojson` で書いた `FeatureCollection`（`Point`・`LineString`・穴あき `Polygon`・`MultiPolygon`・鉛直壁面 4 枚と上面・底面を持つ閉じた直方体 `Form::Solid`・属性）。
  - `verify.py` が JSON と各 `Feature` の `geometry` を `shapely.from_geojson` で読み、`shapely.is_valid` が真であること、外周が反時計回り・穴が時計回り（RFC 7946 3.1.6 の右手の法則）であること、図形種別・座標・属性が一致することを確かめる。
- 読み取りの補助照合（合成入力のため読みの段は `synthetic` のまま）:
  - `verify.py --emit` が Shapely 2.2.0 の `shapely.to_geojson` で出力した 7 種の Geometry（3D 座標・穴あき面・`GeometryCollection` を含む）を `GeoJson::read` と `Lift::<GeoModel>::lift` で読み、図形数と頂点座標が一致することを確かめる。

## ツールの権利

2026-11-13 に、[Shapely 2.2.0 のライセンス](https://github.com/shapely/shapely/blob/2.2.0/LICENSE.txt)を確認しました。BSD-3-Clause です。別に導入するテストのツールであり、リポジトリには同梱しません。

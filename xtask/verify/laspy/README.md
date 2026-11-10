# LAS の外部照合

非圧縮 LAS を [laspy](https://github.com/laspy/laspy) 2.7.0 と照らします（#804・#504）。`crates/zumen-las/tests/external.rs` が、コアと、別の Python プロセスで動く `read.py` の読みを比較します。外部ツールのソースとバイナリはリポジトリに置かず、Cargo の依存にも加えません。

## 実行

Python は、すでに入っているものを使います。laspy は Python 3.10 以降、ここで固定した NumPy は 3.12 以降が必要です。毎晩の CI は Python 3.14 を使います。LAZ の追加機能は入れません。

Windows では Python Install Manager の Python 3.14 と uv で、リポジトリの `target/` 内に環境を作れます。uv に Python 本体をダウンロードさせません。

```powershell
uv venv --python 3.14 --no-python-downloads target/laspy-venv
uv pip install --python target/laspy-venv/Scripts/python.exe --only-binary :all: laspy==2.7.0 numpy==2.5.4
$env:ZUMEN_LASPY_PYTHON = (Resolve-Path target/laspy-venv/Scripts/python.exe).Path
cargo test --locked -p zumen-las --test external -- --nocapture
```

ほかの環境では、既存の Python に付く venv も使えます。

```bash
python3 -m venv target/laspy-venv
target/laspy-venv/bin/python -m pip install --only-binary :all: laspy==2.7.0 numpy==2.5.4
ZUMEN_LASPY_PYTHON="$PWD/target/laspy-venv/bin/python" cargo test --locked -p zumen-las --test external -- --nocapture
```

`ZUMEN_LASPY_PYTHON` がなければ `python` を使います。Python か laspy・NumPy がないときは外部照合だけを飛ばし、理由をテストの出力に書きます。`ZUMEN_LASPY_PYTHON` または `ZUMEN_LASPY_REQUIRED` を指定したときは、不在も失敗にします。バージョン違い・起動の失敗・入力を読めない場合・値の食い違いは、指定の有無によらず失敗です。毎晩の CI は `ZUMEN_LASPY_REQUIRED=1` で流します。

## 入力と比較する値

| 入力 | バージョン・点形式 | 比較するもの |
| --- | --- | --- |
| テスト内で作る各 4 点の LAS | 1.2 の 0〜3、1.3 の 4・5、1.4 の 6〜10 | ヘッダーのバージョン・点形式・レコードの長さ・点数・尺度・原点・範囲、全点の座標・強度・分類・反射番号と反射数、持つ形式の GPS 時刻と RGB |
| `samples/las/pdal-synthetic.las` | PDAL が作った 1.2 の点形式 3、合成の 1 点 | 同上。反射番号と反射数は 1、強度・分類・時刻・RGB は 0。分類の synthetic フラグは領域モデルに移さず、分類を 0 として読む |

自前で作るファイルは、負の整数座標・軸ごとに異なる尺度と原点・分類のフラグ・3 ビットと 4 ビットの反射番号を持ちます。期待する座標と属性を定数でも確認します。読みの `external` の根拠は、これらの合成入力ではなく、公開サンプルでの照合です。

`read.py` は標準入力の LAS を laspy で読み、ヘッダーと点をファイルの順に TSV で返します。浮動小数は Python の `repr` で送り、Rust で f64 に戻したビット列を比較します。点を抽出したり、座標の許容差を設けたりしません。`-` は、その点形式に属性がないことを示します。コアの読み取りコードや座標変換を Python 側に写していません。

## 検証の範囲

- `external.rs` は、公開サンプルの全体読みと範囲読みで、L1・点群モデル・診断が一致することも確認します。テストキットのレコード範囲・決定性・予算と、入力バイト・要素・作業の予算による停止も確認します。点群の API は点ごとの ID を返さないので、新しい ID の規則は作りません。出所のバイト範囲と点の順を含むモデルを比較します。
- `bindings/conformance/cases/pointcloud_samples.json` は同じ公開サンプルを `open`・`pointcloud` で開き、全点を選んだ応答と診断のハッシュをネイティブ・wasm で比較します。これは実行環境間の一致で、外部の実装で形式を確かめるテストとは分けます。
- laspy との照合はネイティブの Rust テストです。座標系の解釈・VLR と EVLR の内容・落とした属性の値・波形の外部データは照合していません。GPS 時刻は記録値を比較し、時刻系の換算はしません。LAS 1.0・1.1、LAS 1.4 の旧点形式、LAZ・COPC、5 MB 超の実データ、基準になるアプリで開くことも未確認です（#371・#504・#510）。

## ツールと公開サンプルの権利

2026-11-11 に、[laspy 2.7.0 のメタデータ](https://github.com/laspy/laspy/blob/2.7.0/pyproject.toml)と[ライセンス](https://github.com/laspy/laspy/blob/2.7.0/LICENSE.txt)を確認しました。BSD-2-Clause で、必須の外部依存は NumPy だけです。[NumPy 2.5.4](https://pypi.org/project/numpy/2.5.4/) は BSD-3-Clause・0BSD・MIT・Zlib・CC0-1.0 のライセンスを表記しています。どちらも別に導入するテストのツールです。

公開サンプルは [PDAL #2925](https://github.com/PDAL/PDAL/pull/2925) で追加された `test/data/las/synthetic_test.las` を、変更せずに取得しました。追加したコミットは `afbe6f7794bfe40394f27f46b2a6da0da3f1afb2` です。[PDAL #2876](https://github.com/PDAL/PDAL/issues/2876) は分類の synthetic フラグの読み書きのテスト用と説明し、同時に追加した書き取りテストも定数の座標（1・2・3）から 1 点を作っています。取得したファイルのヘッダーにも、PDAL 2.0.0 が作ったことが書かれています。

同じコミットの [LICENSE.txt](https://github.com/PDAL/PDAL/blob/afbe6f7794bfe40394f27f46b2a6da0da3f1afb2/LICENSE.txt) は、別記のない全ファイルを BSD と明記しています。コードにだけ適用する条件ではなく、このデータへの別条件もありません。個人の氏名・住所・連絡先を含むヘッダーや VLR がないことも確認しました。取得元・ハッシュ・大きさは `samples/ledger.toml`、BSD-3-Clause の本文と元の著作権表示は `licenses/pdal.txt` にあります。

laz-rs の `tests/data` の LAS も調べましたが、実測の原データの来歴と再配布条件を確認できなかったため採用していません。コードのライセンスだけを、そのデータの再配布の根拠にはしません。

# ST-Bridge の公開サンプルの照合

MIT の公開サンプル `samples/stbridge/hoaryfox-sample-building.stb`（HoaryFox が書いた 2.0.2 の鉄骨の架構）を、zumen の公開 API と [STBDotNet](https://github.com/hrntsm/STBDotNet)（MIT）0.2.3 の v202 のクラスで読み、値を照らします（[#400](https://github.com/zumen-jp/zumen/issues/400)）。STBDotNet はテストのツールで、zumen の依存にも成果物にも入れません。バージョンは `DumpStb.csproj` で決め、`packages.lock.json` で中身のハッシュも固定します。

| ファイル | 中身 |
| --- | --- |
| `DumpStb.csproj`・`Program.cs` | STBDotNet で読んだ節点・階・部材・断面・鋼材の規格の属性を JSON に書き出す .NET のツール |
| `check.py` | CLI（`zumen serve --stdio`）の `building_elements` の値と、書き出した JSON を照らす |
| `test_check.py` | 合成の入力の値・参照・配置を変え、`check.py` が食い違いにすることを確かめる |

## 照らすもの

- 部材（柱・大梁・ブレース）と階の要素の数・ID・名前
- 部材・階・断面・鋼材の規格の属性の値。数は値で、文字は文字列で比べる。どちらかにしかない属性も食い違いにする
- 部材から断面（`id_section`）、断面の形を書く子の要素から鋼材の規格（`shape`）への参照
- 線の部材の配置。原点が始めの節点（柱は下の節点）に、局所の Z が終わりの節点へ向かう向きに合うこと。節点の座標は STBDotNet が読んだ値を使う

断面の形（押し出しの寸法と位置）、部材の局所の X の向き（`rotate`）、階への所属、診断は照らしません。STBDotNet は 2.1 系のクラスを持たないので、2.1.0 の公開サンプル（STB-DiffChecker の一組）も照らしません。

## 流し方

.NET 10 の SDK と Python 3.13 以降で流します。出力先には、リポジトリの外の新しいか空のフォルダーを指定してください。

```bash
cargo build -p zumen-cli
python -I -m unittest discover -s xtask/verify/stbridge -p test_check.py
python -I xtask/verify/stbridge/check.py --cli target/debug/zumen --out ../zumen-stbridge-check
```

Windows では CLI に `.exe` を付けます。終了コードは、全件一致なら 0、食い違いがあれば 1、流せなければ 2 です。STBDotNet の書き出し（`*.stbdotnet.json`）と、件数・食い違いの一覧（`results.json`）を出力先に残します。毎晩の CI（`.github/workflows/nightly.yml` の `stbridge`）も同じことを流し、成果物の `verify-stbridge` に置きます。

## 確かめた範囲

2026-11-18 に、部材 307・階 6・断面 307・鋼材の規格 275 の参照と、属性の値 4492 個、配置 307 件を照らし、食い違いは 0 でした。

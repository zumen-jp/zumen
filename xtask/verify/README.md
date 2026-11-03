# 外部の実装との照合と確認の束

zumen が書き戻した（apply）・変換した（convert）図面を、zumen でない実装で読んで照らし、AutoCAD・Jw_cad・SXF ブラウザで開いて確かめるファイルの束を作ります（[決定 0253](../../docs/decisions/0253-parallel-agents-and-cross-review.md)、#503）。自前の往復の試験は、書く側と同じ読みで確かめるので、仕様の読み違いを見つけません（#345）。

| ファイル | 中身 |
| --- | --- |
| `make.py` | zumen の CLI（`zumen serve --stdio`）で、公開サンプル（`samples/`）と適合試験の中で作った図面を開き、書き戻しと変換を当てて、束のフォルダーに書く。zumen が読んだ値・当てた変更・期待する形を `manifest.json` に、issue ごとの手順と期待する結果を `README.md` に書く |
| `check_ezdxf.py` | 束の DXF を ezdxf で読み、zumen の読みと照らす。ezdxf の監査の誤りと、書き戻しで決めた形も確かめる |
| `dump.lsp` | AutoCAD で開いた図面の図形を、ハンドルごとに 1 行で書き出す。束の README の手順で使う |

## 使い方

```bash
cargo build --release -p zumen-cli
python -m pip install ezdxf==1.4.5
python -I xtask/verify/make.py target/release/zumen verify-out
python -I xtask/verify/check_ezdxf.py verify-out
```

毎晩の CI（`.github/workflows/nightly.yml` の `external`）が同じことを流し、束を成果物の `verify-bundle` に置きます。確認の束の手順と期待する結果は、束の `README.md` にあります。

受け取った図面で確かめるときは、`make.py` に `--inputs <フォルダー>` を足します。その束は、Git にも CI の成果物にも入れません。

## 照らすこと

`check_ezdxf.py` は、DXF のファイルごとに次を確かめます。

- ezdxf が読める。ezdxf の監査で、書き戻したファイルに入力にない誤りが出ず、変換したファイルに誤りが出ない。
- zumen が読んだ画層の名前が、ファイルの画層の表と同じ。ezdxf が読むときに足す Defpoints は除く。
- zumen の要素の出所のハンドルの図形があり、種類と画層が同じ。モデル空間の要素は、形の値（端点・中心・半径・角度・位置・文字・尺度・格子・属性など）も同じ。数は相対の誤差 1e-9 まで同じとみなす。
- モデル空間の図形は、どれも zumen の要素になるか、zumen がそのレコードを指す診断を返している（黙って捨てない）。
- 書き戻したファイルで、変更で決めた形（`manifest.json` の `expect`）が、ezdxf で読んだ図形と同じ。期待する形は、zumen の読みでなく、変更そのものと変換の行列から決める。

既知の食い違いは、`check_ezdxf.py` の `KNOWN` に issue と一緒に置き、失敗にしません。食い違いが出なくなったら失敗にして、`KNOWN` から消すことを知らせます。

## 外部の実装

外部の実装は、試験で別のプログラムとして動かし、成果物にも依存にも入れません。版を決めて入れます。

| 形式 | 実装 | ライセンス | 扱い |
| --- | --- | --- | --- |
| DXF | [ezdxf](https://github.com/mozman/ezdxf) 1.4.5 | MIT | 照らす（`check_ezdxf.py`） |
| JWW | [ezjww](https://github.com/monozukuri-ai/ezjww) | MIT | 読み・監査・DXF への変換を持つ。照らす台本はまだない |
| SFC・P21 | [ezsxf](https://github.com/monozukuri-ai/ezsxf) | MIT | SFC・P21 の読みを持つ。照らす台本はまだない |
| DWG | [ACadSharp](https://github.com/DomCR/ACadSharp) | MIT | R2000〜R2018 の読み書きを持つ（.NET）。照らす台本はまだない |

次のものは使いません。

- LibreDWG などの GPL の実装。ソースも読みません（`AGENTS.md`）。
- ODA File Converter。使うには利用の条件への同意が要るため（決定 0253 の人を待つもの）。

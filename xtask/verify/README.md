# 外部の実装との照合と検証キット

zumen が書き戻した（apply）・変換した（convert）図面を、zumen でない実装で読んで照らし、AutoCAD・Jw_cad・SXF ブラウザで開いて確かめる検証キットを作ります（決定 0254（PR #500）、#503）。自前の往復のテストは、書く側と同じ読みで確かめるので、仕様の読み違いを見つけません（#345）。

| ファイル | 中身 |
| --- | --- |
| `make.py` | zumen の CLI（`zumen serve --stdio`）で、公開サンプル（`samples/`）と適合テストの中で作った図面を開き、書き戻しと変換を当てて、キットのフォルダーに書く。zumen が読んだ値・入力の出所とライセンス・当てた変更・期待する形を `manifest.json` に、issue ごとの手順と期待する結果を `README.md` に、ライセンスの本文を `licenses/` に書く |
| `check_ezdxf.py` | キットの DXF を ezdxf で読み、zumen の読みと照らす。ezdxf の監査の誤りと、書き戻しで決めた形も確かめる |
| `dump.lsp` | AutoCAD で開いた図面の図形を、ハンドルごとに 1 行で書き出す。キットの README の手順で使う |

## 使い方

```bash
cargo build --release -p zumen-cli
python -m pip install ezdxf==1.4.5
python -I xtask/verify/make.py target/release/zumen ../zumen-verify
python -I xtask/verify/check_ezdxf.py ../zumen-verify
```

キットのフォルダーは、リポジトリの外に作ります。`make.py` は、前のキット（`manifest.json` がある）か空のフォルダーだけを消して作り直し、ほかのフォルダーは消さずに止まります。

毎晩の CI（`.github/workflows/nightly.yml` の `external`）が同じことを流し、キットを成果物の `verify-bundle` に置きます。検証キットの手順と期待する結果は、キットの `README.md` にあります。

受け取った図面で確かめるときは、`make.py` に `--inputs <フォルダー>` を足します。キットをリポジトリの中に作ろうとすると止まります。そのキットは、Git にも CI の成果物にも入れず、issue には件数と種類だけを書き、ファイルの名前と中身を書きません（決定 0254）。

## 照らすこと

`check_ezdxf.py` は、DXF のファイルごとに次を確かめます。

- ezdxf が読める。読む中で流れる ezdxf の監査（`recover.readfile`）で、書き戻したファイルの誤りと直した誤りが、code ごとの件数で入力より増えない。変換したファイルには、誤りも直した誤りもない。
- zumen が読んだ画層の名前が、ファイルの画層の表と同じ。ezdxf が読むときに足す Defpoints は除く。
- zumen の要素の出所のハンドルの図形があり、種類と画層が同じ。モデル空間の要素は、形の種類と値（端点・中心・半径・角度・位置・文字・尺度・格子・属性など）も同じ。ezdxf で比べられる種類（LINE・CIRCLE・ARC・ELLIPSE・LWPOLYLINE・POINT・TEXT・MTEXT・INSERT）の要素は、形を持つ。数は、相対と絶対の誤差 1e-9 まで同じとみなす。角度は、360° の違いを同じとみなす。
- モデル空間の図形は、どれも zumen の要素になるか、zumen がそのレコードを指す診断を返している（黙って捨てない）。診断の場所は、数を限らずに求める。
- 書き戻したファイルで、変更で決めた形と画層（`manifest.json` の `expect`）、足した画層の色、色を変えた図形の色が、ezdxf で読んだ値と同じ。期待する値は、zumen の読みでなく、変更そのものと変換の行列から決める。当てる候補のない段は、`skipped` としてキットに残す。

既知の食い違いは、`check_ezdxf.py` の `KNOWN` に、issue と当たるファイルの数と一緒に置き、失敗にしません。当たるファイルの数が変わったら（直ったか、同じ形の食い違いが増えた）失敗にして、`KNOWN` を直すことを知らせます。

## 外部の実装

外部の実装は、テストで別のプログラムとして動かし、成果物にも依存にも入れません。バージョンを決めて入れます。

| 形式 | 実装 | ライセンス | 扱い |
| --- | --- | --- | --- |
| DXF | [ezdxf](https://github.com/mozman/ezdxf) 1.4.5 | MIT | 照らす（`check_ezdxf.py`） |
| JWW | [ezjww](https://github.com/monozukuri-ai/ezjww) | MIT | 読み・監査・DXF への変換を持つ。照らすスクリプトはまだない |
| SFC・P21 | [ezsxf](https://github.com/monozukuri-ai/ezsxf) | MIT | SFC・P21 の読みを持つ。照らすスクリプトはまだない |
| DWG | [ACadSharp](https://github.com/DomCR/ACadSharp) | MIT | R2000〜R2018 の読み書きを持つ（.NET）。照らすスクリプトはまだない |

次のものは使いません。

- LibreDWG などの GPL の実装。ソースも読みません（`AGENTS.md`）。
- ODA File Converter。使うには利用の条件への同意が要るため（決定 0254 の人を待つもの）。

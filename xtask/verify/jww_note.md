# 公開版で JWW の注記とレイヤを直す

Windows x64、Python 3.14.8 で、PyPI の `zumen==0.3.0` を空の仮想環境に入れて確認しました。リポジトリのルートで次を実行します。出力先の `jww-note-kit` は新規のフォルダーにしてください。[編集と照合を行うスクリプト](jww_note.py)

```powershell
uv venv --no-python-downloads --python 3.14 ../jww-note-env
uv pip install --python ../jww-note-env/Scripts/python.exe --index-url https://pypi.org/simple zumen==0.3.0
../jww-note-env/Scripts/python.exe -I xtask/verify/jww_note.py make ../jww-note-kit
uv run --no-python-downloads --python 3.14 --with ezjww==0.7.6 python -I xtask/verify/jww_note.py check ../jww-note-kit/input.jww ../jww-note-kit/edited.jww
```

`input.jww` は自作の線分と注記を持つ図面です。スクリプトはこの入力を新しいセッションに渡し、`open` と `entities` で注記の ID とハッシュを取得します。`apply` の `replace` で文字列を書き換え、ハッシュを取り直して `restyle` で同じグループの別レイヤへ移します。最後の `apply` の直後に `take` でファイルを取り出し、`edited.jww` に保存します。

| 項目 | 入力 | 編集後 |
| --- | --- | --- |
| 注記 | 施工前 | 施工後 |
| レイヤ | グループ 0・レイヤ 0 | グループ 0・レイヤ 1「注記」 |
| 文字の位置 | 実寸 (2000, 3000) mm、用紙上 (20, 30) mm | 同じ |
| 文字高・角度 | 用紙上 3 mm・30° | 同じ |
| 線分 | 用紙上 (0, 0) から (100, 50) mm、グループ 0・レイヤ 0 | 同じ |

0.3.0 の `entities` は文字の形を返さないため、置き換えには自作入力の既知の位置・高さ・角度・書体を指定します。用紙上の高さを示す `height_on_paper` を送ると `call.field.unsupported` になります。縮尺 1/100 のこの入力には実寸の高さ 300 mm を渡します。現行の開発版では同じ 2 ファイルを開くと、文字列と位置に加え `height: 3`、`height_on_paper: true` が返ります。

2026-11-21 に公開版の編集を診断なく終えました。Jw_cad 10.3.6.0 で入力と編集後を開き、「文字」で注記を選ぶと、文字の入力フィールドにそれぞれ「施工前」「施工後」と表示されました。編集後はレイヤ 1 を非表示にすると注記が消え、再表示すると戻りました。線分はどちらでも残りました。

Jw_cad の「名前を付けて保存」で編集後の図面を新しい JWW に保存しました。上の `check` の最後の引数をこの JWW に変え、ezjww 0.7.6 で表の全項目に加え、書体、字幅、文字の終点を照合しました。位置・文字高・角度の数値はこの独立した読み取りの結果です。再保存で増えた用紙外の印刷・表示設定の文字は比較から除きます。[実機照合の記録先](https://github.com/zumen-jp/zumen/issues/921)

0.3.0 では線だけの図面に新しい注記を加えると、始点と終点が一致します。Jw_cad ではその文字を選択できず、別名保存すると消えました。上の手順は終点を持つ既存の注記を編集します。開発版は追加時と、始点・終点が一致する既存文字の書き換え時に、字幅と角度から終点を求めます。開発版の `add` で同じ線分を持つ図面に「施工後」を新しく置くと、Jw_cad の文字コマンドで選択できました。別名保存した図面も上の `check` の編集後の期待値と一致しました。

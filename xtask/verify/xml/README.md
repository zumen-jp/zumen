# XML の文字コードの独立検査

XML の L0 と J-LandXML の読み取りについて、合成入力の原本・公開値・診断・予算を比較します（#468）。受け取った図面や外部のサンプルは使いません。

## 実行

リポジトリが指定する Rust、Node.js 22 以降、Python 3.14 を使います。Python は標準の [`xml.parsers.expat`](https://docs.python.org/3.14/library/pyexpat.html) だけを使い、npm・Python・Cargo の依存は追加しません。

```powershell
rustup target add wasm32-wasip1
$env:CARGO_TARGET_DIR = 'target/xml-utf16-portability-468'
cargo test --locked -p zumen-landxml --test utf16_contract
py -3.14 -m unittest discover -s xtask/verify/xml -p test_expat.py
node xtask/verify/xml/compare_utf16.mjs
```

Windows では Python Install Manager の `py -3.14`、ほかの環境では `python3.14` を使います。別の実行ファイルを使うときは、`ZUMEN_XML_EXPAT_PYTHON` にその Python 3.14 のパスを指定します。`CARGO_TARGET_DIR` の既定値は `target/xml-utf16-portability-468` です。

## ネイティブと wasm の比較

[utf16_contract.rs](../../../crates/zumen-landxml/tests/utf16_contract.rs) は、UTF-8・Shift_JIS・UTF-16LE・UTF-16BE・未対応の UTF-32 の計 39 入力を作ります。[utf16_probe.rs](../../../crates/zumen-landxml/examples/utf16_probe.rs) は同じ検査の記録を出力します。

- UTF-16 の BOM と汎用の宣言、BOM と宣言なし、BOM なしの正しい LE・BE 宣言を受理することを確かめます。宣言の不一致、BOM と LE・BE 宣言、孤立したサロゲート、逆順の組、奇数バイト、不正な XML 文字とマークアップは読まず、UTF-32 は未対応として返すことを確かめます。これらの受理条件は、コアの契約を検査する期待値です。
- `src`、名前の `raw`・`local`・`prefix`、名前空間の宣言、要素・属性・Text・CDATA・DTD・診断の位置が、元の文字コードのバイト範囲を指すことを確かめます。日本語と補助平面の文字、文字参照、CRLF、接頭辞つきの名前を含めます。`parsed_local_name` と `local_name_text` の値も記録します。
- J-LandXML では、平面線形と一定の片勾配の値、出所、原本の `RecordInfo.kind`、未変更の書き出しを確かめます。浮動小数の値は `f64::to_bits` で記録します。
- XML の読み取り、J-LandXML の読み取りから `lift` まで、`lift` 単体を分けます。成功する入力では全 `Kind` の使用量と上限ちょうどの成功を比較し、正の使用量の上限を 1 減らすと、該当する `Exceeded` で止まり、文書やモデルを返さないことを確かめます。使わない単位は上限 0 で成功させます。`Depth` は最終使用量とは別に、成功する最小の上限を探します。
- `cancel` と `expire` は各処理の前に通知し、部分結果も予算使用もない停止を確かめます。通知する実時間や、別スレッドで通知したときの停止位置は比較しません。

runner はネイティブと `wasm32-wasip1` をビルドして各 2 回実行し、原本、全公開範囲、値、診断の順・code・件数・値・場所、全 `Kind` の使用量・上限・停止理由をバイト単位で比較します。これはコアを直接呼ぶ 32 bit の wasm の検査です。配布する `wasm32-unknown-unknown` の API・バインディングの適合テストとは分けます。

## Expat との比較

`check_expat.py` は probe の `--external` が返す受理済みの 10 入力を、Python 標準の Expat で読みます。要素の順・親子・名前空間・日本語の名前・属性と正規化した値・文字と CDATA、および元のバイト列での要素の開始と終了を比較します。

原本位置は Expat の `CurrentByteIndex` と、引用符を区別するタグ終端の独立した走査から求めます。文字参照や注記・処理命令によるコールバックの分割は、同じ種類の隣接する文字を連結して比較します。外部の DTD と実体は取得しません。内部 DTD の実体の宣言は、照合する入力に含めません。`test_expat.py` は、原本位置の計算と、位置や文字を故意に変えたときの差分検出を検査します。

Expat は受理条件の比較先にしません。たとえば、コアが拒否する BOM と LE・BE 宣言の組を、Expat が読む場合があります。Shift_JIS は Expat の対応する文字コードに含まれないため、独立照合から除きます。属性・文字・CDATA・DTD・診断それぞれの原本範囲、未知の実体の保持、J-LandXML の領域モデルは、Rust の期待値で検査します。これらを Expat との一致の範囲に含めません。

## 既存の文字コードの固定値

`legacy_probe.rs` は、UTF-16 の新しい API を使わず、UTF-8・Shift_JIS の 2 XML と UTF-8 の 1 J-LandXML について、公開値・原本範囲・診断・全 `Kind` の使用量を記録します。`legacy-e8d389a3.txt` は、公開済みのコミット `e8d389a3b552b3a9fd2a047e2b7a25f77b1d2cec` で、同じ probe ソースから作った固定値です。通常の runner は現在の出力をこの固定値と比較します。

固定値の由来は、次のコマンドで再検査できます。

```powershell
$env:CARGO_TARGET_DIR = 'target/xml-utf16-portability-468'
node xtask/verify/xml/check_legacy.mjs
```

このコマンドは `git archive` で公開コミットの snapshot を `target/` に展開し、その中に同じ probe の example を置いてビルドします。作業中の checkout や未コミット変更は書き換えません。固定値を作り直す `--write` は、公開コミットでの記録だけを書きます。現在の実装の結果を固定値に採りません。

## Shift_JIS の文字境界とホストの比較

```powershell
$env:CARGO_TARGET_DIR = 'target/xml-shift-jis-824'
node xtask/verify/xml/compare_shift_jis.mjs
node xtask/verify/xml/compare_shift_jis.mjs chromium
node xtask/verify/xml/compare_shift_jis.mjs webkit
```

ブラウザーを指定するときは、[JS の手順](../../../bindings/js/README.md)に従って `bindings/js/test/browser` の固定した Playwright とブラウザーを用意します。`firefox` も指定できます。

`shift_jis_probe` は全 1 バイトと、0x81–0x9f・0xe0–0xfc を先頭に持つ全 2 バイト、計 15,616 入力を作ります。`n` の後ろへその符号を足した XML について、SDK の期待名は `encoding_rs` の置換しない復号から作り、XML の成否・診断・予算の全使用量も記録します。ネイティブと WASI の各 2 回の全出力を比べ、JS のプラグインの署名を同じ期待名で比べます。これはホスト間の一致の検査で、外部の仕様適合の証明にはしません。名前の正否・原本範囲・全予算の境界・中断は `zumen-xml/tests/shift_jis.rs`、署名の切断と長い入力は SDK と JS の `xml_root` の自作テストで検査します。

実行した Node.js 24.22.2・Chromium 156・WebKit 27.2 は一致しました。Firefox 1555 は Windows の起動時の `spawn UNKNOWN` で実行できず、対応範囲の確認を残しています。単独 0x80 を ICU が拒否する Node.js と WHATWG の差は、名前の比較の復号内で U+0080 にそろえます。

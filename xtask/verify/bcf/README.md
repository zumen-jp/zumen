# BCF の読み書きを外部実装と照らす

自作の BCF 2.1・3.0 を公開操作の `bcf_write` で書き、Smino.Bcf.Toolkit 3.2.0 の読み取りと照らします（#522）。外部実装で保存し直したものを `bcf_read` でも読み、入力のレシピで決めた値と比べます。コアの読み取り結果を、書いた値の正解にはしません。

## 入力と期待する値

公開 IFC と自作 BCF は [acc のキット](../acc/README.md)の `make.py` で用意します。IFC は MIT の Text2BIM の `ifc/text2bim/sample_model.ifc` です。[サンプル台帳](../../../samples/ledger.toml)に従ってヘッダーの端末の道筋を除いた、258,353 バイトのファイルを使います。SHA-256 は `f3e6b444dfc08bea7b1a8c74746ce4ad4752951fd2912df067f1bf65dff72797` です。IFC と MIT の本文をキットへ写します。

題・説明・状態・作者・日時・コメント・GUID・選択要素・カメラ・画像は自分たちで定めた値です。`compare.py` は `expected_cases` と固定定数から期待値を作ります。公開の BCF の例や XSD を写した入力ではありません。

| ケース | 版 | 指摘 | カメラ | 画像 |
| --- | --- | ---: | --- | ---: |
| `rewritten-2.1` | 2.1 | 4 | 透視 4 | 自作 PNG 4 |
| `new-2.1` | 2.1 | 4 | 透視 2・平行 2 | 0 |
| `new-3.0` | 3.0 | 4 | 透視 2・平行 2 | 0 |

各ケースに状態 `Open`・`Resolved`・`Closed`、日本語の題とコメント、選択要素のある視点とない視点を含めます。平行投影の幅は 12、透視投影の視野角は 60、3.0 の縦横比は 1 です。元の 3 ファイルと外部実装による再保存の 3 ファイル、延べ 24 指摘を照らします。

## 外部ツール

Python 3.13 以降の標準ライブラリと .NET 10 SDK を使います。`prepare.py` は次の公式 NuGet パッケージを固定 SHA-256 で確かめ、指定した DLL だけを Git の外へ展開します。実装ソースと XSD は展開しません。公開 API の説明とアセンブリの参照情報を根拠に、別プロセスで呼びます。

| パッケージ | 版 | ライセンス |
| --- | --- | --- |
| [Smino.Bcf.Toolkit](https://www.nuget.org/packages/Smino.Bcf.Toolkit/3.2.0) | 3.2.0 | Apache-2.0 |
| Newtonsoft.Json | 13.0.3 | MIT |
| RecursiveDataAnnotationsValidation | 2.0.0 | MIT |
| Serilog | 4.0.0 | Apache-2.0 |
| System.CommandLine | 2.0.0-beta4.22272.1 | MIT |

旧 `System.Text.RegularExpressions` 4.3.1 は入れず、既存の .NET 10 の Regex を使っていることを検査します。今回の公開 API が参照しない `Json.Net` も入れません。導入済みの安定版 SDK 10 系からもっとも新しいものを選び、ツールのフォルダーの `global.json` で固定します。NuGet の復元元を空にし、上の DLL を直接参照して自作の呼び出し部分をビルドします。この構成の確認は今回の入出力 API に限ります。

版を変える共通の `Worker` を避け、2.1 と 3.0 の `Converter` で読みます。書き出しは自作の `MemoryStream` を渡す経路を使います。Stream を返す経路は Windows で一時ファイルを再び開けなかったため使いません。ツールの作業場所も Git の外にします。外部 DLL・生成された型・ビルドログは、リポジトリと CI の成果物へ入れません。

## 実行

```sh
cargo build --locked -p zumen-cli --bin zumen
python -I xtask/verify/bcf/prepare.py --out ../zumen-bcf-tools
python -I xtask/verify/bcf/check.py --cli target/debug/zumen --tools ../zumen-bcf-tools --out ../zumen-bcf-check
python -I xtask/verify/bcf/test_check.py ../zumen-bcf-check
```

Windows は CLI を `target/debug/zumen.exe` に替えます。ツールとキットの出力先は Git の外の新しいフォルダーに限り、既存の内容を消しません。照合の終了値は、一致が 0、不一致が 1、入力・ツール・実行の失敗が 2 です。毎晩の CI の `bcf` ジョブは `verify-bcf` にキットだけを保存します。

## 比べ方と記録

外部実装の直接の読み取りと再保存後の読み取りを、固定期待値に照らします。指摘・コメント・視点の GUID と参照、題・説明・状態・作者・日時・本文、カメラの全座標と尺度、選択する IFC の GlobalId、可視性、画像の SHA-256 を比べます。GUID で順序をそろえますが、重複は消しません。座標では同じ値の整数と実数を許し、真偽値による数値の代用と非有限の値は拒否します。

再保存した 3 ファイルをコアでも読み、同じ値と診断 0 件を確かめます。Review の対象の ID・名前・形式・ハッシュは編集前の読み取りから変わらないことを別に検査します。これは参照の保持の検査であり、ハッシュの定義の独立した照合ではありません。Review に画像と縦横比はないため、それらは外部実装の観測で比べます。

2 つの新規出力は同じ要求を 2 回呼び、出力バイト列と診断の一致を検査します。合成入力の ZIP は時刻と作成 OS の識別値を固定します。外部実装の ZIP の時刻や列挙順はこの一致の対象にしません。`test_check.py` の 28 テストは、保存した実測値の改変 336 件、正常な列挙順の変更、既存の出力と Git 内への出力の拒否、SDK 10 系の選択と不在の拒否を含みます。

`results.json` は完了状態、比較結果、入力・CLI・検査器・成果物の SHA-256 とチェックアウトのコミットを記録します。CLI のビルド元はコミットの記録だけでは証明できないので、同じチェックアウトで上記のビルドを実行してください。`tools.json` はパッケージと DLL のハッシュ、SDK とランタイム、Regex の出所を記録します。`api.json`、各ケースの期待値・外部観測・コア応答・差、`test-results.json` も保存します。例外の本文・スタック・端末の道筋は写しません。

2026-11-16 に Windows x64・Python 3.13.17・.NET SDK 10.0.404／ランタイム 10.0.15 で、3 ケースと 28 テストが通りました。Linux の結果は CI の実行記録で確認します。

## 範囲の限界と公開サンプルの調査

自作の指摘を元にした照合なので、BCF の読みの段は `synthetic` のままにします。書きの段は、上記の値について外部実装で確かめた `external` です。未知の XML、文書参照、切断面、色、別の文字コード、仕様の全組み合わせ、製品の表示、ネイティブと wasm の一致は未確認です。CAD の実機確認率には加えません。

2026-11-16 に [buildingSMART/BCF-XML の release_3_0](https://github.com/buildingSMART/BCF-XML/tree/release_3_0) を調べ、根の LICENSE は CC BY-ND 4.0 でした（Git blob `f7f7427d769f961c0b9b83f0bd855261a05d06f0`）。個々の例の別の許諾は未確認のため、例・XSD・生成物を同梱していません。外部実装の使い方は [BcfToolkit の README](https://github.com/bimspot/bcf-toolkit/blob/develop/README.md) と NuGet の公開 API の説明を参照しました。

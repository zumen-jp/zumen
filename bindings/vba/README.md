# VBA のバインディング

Windows の VBA7（Office 2010 以降）、32 bit・64 bit 向けです。C ABI を `Declare PtrSafe` で呼びます。Office の実機でのコンパイルと実行は、まだ確認していません。[対応する環境](../../docs/support.md) の下限を変えるものではありません。

## 導入

Office と同じビット数の `zumen_abi1.dll` をビルドし、Office が DLL を検索するフォルダーに置きます。開発時は、そのフォルダーを `PATH` に含めた PowerShell から Office を起動できます。DLL のフォルダーは利用者が管理する場所にしてください。

```powershell
# 64 bit
cargo build -p zumen-abi --release --locked
# 32 bit（事前に rustup target add i686-pc-windows-msvc）
cargo build -p zumen-abi --release --target i686-pc-windows-msvc --locked
```

VBE の「ファイル」→「ファイルのインポート」で、次の 4 ファイルを取り込みます。コードは ASCII にしており、VBE のコードページによる文字化けを避けています。日本語の値は UTF-16 の文字列から UTF-8 に変換します。

- `ZumenNative.bas`: 生成した ABI の宣言、`ZmBuf`、状態コードとシーンの定数
- `ZumenRuntime.bas`: UTF-8・バイト列・TSV の変換
- `ZumenSession.cls`: セッションとバッファーの管理
- `ZumenScene.cls`: 2D のシーンの読み取り

```vb
Dim session As ZumenSession
Dim status As Long, response As String
Set session = New ZumenSession
status = session.OpenSession()
If status <> ZM_OK Then Err.Raise 5
status = session.PushText("a.dxf", "0" & vbLf & "SECTION" & vbLf & "2" & vbLf & "ENTITIES" & vbLf & "0" & vbLf & "ENDSEC" & vbLf & "0" & vbLf & "EOF" & vbLf)
status = session.CallText("{""v"":1,""op"":""open"",""input"":""a.dxf""}", response)
Debug.Print status, response
session.CloseSession
```

## 型と所有権

| ABI の型 | VBA7 の型と渡し方 |
| --- | --- |
| `u32`・状態コード | `Long`（32 bit）。符号なしの全域を渡す場合は `ZmSigned32` でビット列を保つ |
| ポインター・セッション・`size_t` | `ByVal LongPtr` |
| セッションの出力先 | `ByRef LongPtr` |
| `zm_buf *` | `ByRef ZmBuf`。ポインターと長さの 2 フィールド |
| Arrow のストリームの出力先 | `ByVal LongPtr`。生の宣言のみ。VBA からのコールバックのラッパーはない |

- `OpenSession`・`PushText`・`CallText` は文字列を UTF-8 にします。変換には Windows の `WideCharToMultiByte`・`MultiByteToWideChar` と不正な文字を拒否するフラグを使い、NUL も長さに含めます。空の配列は長さ 0 と NULL を渡します。
- `OpenBytes`・`PushBytes`・`CallBytes` は `Byte()` を渡します。入力は呼び出しの間だけ借り、応答と `Take` の結果は VBA の配列にコピーしてから `zm_free` で返します。コピー中にエラーになっても解放します。コピーした結果は次の呼び出しやセッションの終了後も読めます。
- 配列の長さと添字は `Long` の正の範囲に限ります。大きすぎるネイティブの結果は、解放して VBA のエラー 6 にします。64 bit のポインターは `Long` に縮めません。
- 各メソッドは状態コードを返します。`ZM_FAILED`・`ZM_PANICKED` の応答も読み取れます。文字コード、ABI の不一致、VBA の確保失敗は VBA のエラーです。`CloseSession` は繰り返し呼べ、クラスの終了時にも閉じます。業務のマクロでは明示して閉じてください。
- 呼び出しは同期的で、実行中に `DoEvents` を呼びません。同じスレッドのボタンから `Cancel` を割り込ませる機能はありません。コアの予算で作業量を制限できます。
- JSON は文字列またはバイト列のまま渡し、未知のフィールドを落としません。TSV の blob は `ZmUtf8Decode` と `ZmTsvRows` で、見出しを含む 1 始まりの 2 次元配列にできます。空の TSV は `Empty` です。数と ID は文字列のまま保つので、Excel への書き込み前に必要な列の書式を決めてください。

## シーン

`Take` で得たシーンの `Byte()` を `ZumenScene.Read` に渡します。`Rows("line_xy")`、`Value("line_xy", 行, 成分)`、`Text(開始バイト, 長さ)` で値を引けます。行と成分は 0 始まりです。`u32` は全域を正確に表す `Double`、`f32`・`f64` も `Double` で返します。ID の元のバイト列も `u8` の値として読めます。

リーダーは入力をコピーし、ヘッダー、メジャーバージョン、表と既知のセクションの範囲・行幅・整列を確かめます。ないセクションは空の列、未知のセクションは読み飛ばします。メジャーバージョンが違うものや範囲の外の読み取りは VBA のエラーになります。

## 確かめ方

生成器は `schema/boundary.toml` の `native` の関数だけを出し、未対応の型を拒否します。コアと同じパッケージのバージョンを生成するので、`cargo xtask codegen --check` がバージョンの違いも見つけます。

```powershell
cargo xtask codegen --check
python bindings/vba/test_contract.py target/debug/zumen_abi1.dll
python bindings/vba/prepare.py target/vba-conformance.tsv
```

`test_contract.py` は宣言の名前と幅をスキーマに照らし、共通の適合テストの全手順とハッシュが対応することを検査します。DLL を指定すると、その DLL で同じ手順を流し、状態と応答・blob の全ハッシュを比較します。CI は既存の Windows x86/x64 の DLL のビルドと stdcall・公開名の検査に加え、この検査を流します。これは VBA のコンパイル・実行の確認ではありません。

Office の実機では、`ZumenConformance.bas` も取り込み、VBE の「デバッグ」→「VBAProject のコンパイル」を実行します。イミディエイトウィンドウから、生成したファイルの絶対パスを指定してください。

```vb
ZumenConformance "C:\work\zumen\target\vba-conformance.tsv"
```

マクロは配列の空値・UTF-8 の日本語と補助文字・NUL・TSV のエスケープ・バッファーの配置・失敗の応答・終了後の呼び出しを確かめたあと、共通手順をラッパーで流します。SHA-256 は Windows の BCrypt で計算します。状態と全ハッシュが一致し、シーンの列を読めた場合だけ件数を表示します。入力・応答の中身やパスはログに出しません。

実機の確認では、Office の製品とビルド・32 bit/64 bit・DLL のコミット・マクロの表示件数を記録します。Microsoft 365 または Office LTSC 2024 の両ビット数で確認します。マクロの信頼設定を自動で変えたり、Office を自動起動したりしません。実機の確認待ちは [#800](https://github.com/zumen-jp/zumen/issues/800) で追跡します。Release の zip への同梱は別の公開作業です。

## 一次資料

- [Microsoft: VBA の 64 bit 対応と LongPtr](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/64-bit-visual-basic-for-applications-overview)
- [Microsoft: LenB とユーザー定義型の配置](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/len-function)
- [Microsoft: WideCharToMultiByte](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-widechartomultibyte)・[MultiByteToWideChar](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-multibytetowidechar)
- [Microsoft: BCryptHash](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcrypthash)・[BCryptOpenAlgorithmProvider](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptopenalgorithmprovider)

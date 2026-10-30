# .NET の包み

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、P/Invoke で呼ぶ薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0196](../../docs/decisions/0196-dotnet-wrapper.md)）。.NET Framework 4.8 には netstandard2.0、.NET 8 以降には net8.0 のアセンブリを出します。

- まだ公開しません。プロジェクトとアセンブリの名前（`Zumen.Provisional`）は仮で、公開の時期と名前は利用者が決めます（#317）。`IsPackable` を `false` にしています。
- NuGet の依存はありません。型つきの要求と応答（System.Text.Json）は、標準で持つ net8.0 の側にだけあります。netstandard2.0 の側では、JSON の文字列かバイト列で読み書きします。
- ネイティブのライブラリは包みに入れず、利用者が道筋を渡します。32 bit の Windows では、関数を stdcall で呼びます。
- `Zumen.Provisional/Messages.g.cs`・`Zumen.Provisional/Constants.g.cs` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```csharp
using System;
using System.IO;
using Zumen.Provisional;
using Zumen.Provisional.Messages;

var zumen = ZumenLibrary.Load("target/release/zumen_abi1.dll");
using var session = zumen.Open();
session.Push("a.dxf", File.ReadAllBytes("a.dxf"));
try
{
    var doc = session.Call<OpenResult>(new OpenRequest { V = Versions.SchemaMajor, Input = "a.dxf" }).Result!.Doc;
    var scene = session.Call<SceneResult>(new SceneRequest { V = Versions.SchemaMajor, Doc = doc, From = "space/0" });
    var columns = Scene.Read(session.Take(scene.Blobs[0].Id));
    ReadOnlySpan<double> origins = columns.Column<double>("chunk_origins");
}
catch (ZumenException error)
{
    // failed の応答は error.Response にあり、診断を持ちます
    Console.WriteLine($"{error.StatusName} {error.ResponseText}");
}
```

| 名前 | 中身 |
| --- | --- |
| `ZumenLibrary.Load` | ライブラリを読み込み、関数と ABI の大版を確かめる |
| `ZumenLibrary.Open` | セッションを作る（`zm_session_new`）。`using` で閉じる |
| `Session.Push`・`Call`・`CallBytes`・`Take`・`Cancel`・`Dispose` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenException` を投げる |
| `Scene.Read` | シーンの blob を読む。区画は `Column<T>`（net8.0、写さない `ReadOnlySpan<T>`）か `ToArray<T>` で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあり、`Zumen.Provisional.Messages` に record の型があります。`Operations.Types` が、op から要求と結果の型を引きます。列挙は `string` で持ち、値を定数で出すので、知らない値も受け取れます。形が値で決まる欄（`geometry`・`edit` など）は `JsonElement`（netstandard2.0 では `object`）で持ち、目印の値から `Geometry.Variants` などで型を引いて読み直します。`Cancel` は、ほかのスレッドから呼べます。

## 試験

```bash
cargo build -p zumen-abi
dotnet run --project bindings/dotnet/tests/Conformance --framework net10.0 -- target/debug/libzumen_abi1.so
```

`tests/Conformance` は、[適合試験](../conformance/README.md)の組をこの包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。CI は、試験の行列の Linux x64・macOS arm64・Windows x64 で .NET 10 を、Windows x86 で 32 bit の .NET Framework 4.8 を流します。

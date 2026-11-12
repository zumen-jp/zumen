# .NET のラッパー

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、P/Invoke で呼ぶ薄いラッパーです（[依存とツールの条件](../../AGENTS.md#範囲と権利)）。.NET Framework 4.8 には netstandard2.0、.NET 8 以降には net8.0 のアセンブリを出します。

- NuGet の `Zumen` で配ります（[コアの設計](../../docs/architecture.md)・[配布の手順](../../xtask/release/README.md)）。パッケージには、対象ごとのネイティブのライブラリを `runtimes/{rid}/native` に入れます。
- NuGet の依存はありません。型つきの要求と応答（System.Text.Json）は、標準で持つ net8.0 の側にだけあります。netstandard2.0 の側では、JSON の文字列かバイト列で読み書きします。
- `ZumenLibrary.Load()` は、パッケージに入れたライブラリを読みます。手で作ったライブラリは `Load(道筋)` で読みます。32 bit の Windows では、関数を stdcall で呼びます。
- `Zumen/Messages.g.cs`・`Zumen/Constants.g.cs` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。
- パッケージの直下に `LICENSE-MIT`・`LICENSE-APACHE`、各 `runtimes/{rid}/native/` に対象の ABI の依存の本文 `THIRD_PARTY_LICENSES` を入れます。作成と検査の手順は [xtask/release](../../xtask/release/README.md) にあります。
- 各 RID の CycloneDX SBOM は、同じディレクトリの `{ライブラリ名}.cdx.json` にあります。

## 使い方

```csharp
using System;
using System.IO;
using Zumen;
using Zumen.Messages;

var zumen = ZumenLibrary.Load(); // 手で作ったものは Load("target/release/zumen_abi1.dll")
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
| `ZumenLibrary.Load` | ライブラリを読み込み、関数と ABI のメジャーバージョンを確かめる。道筋を渡さなければ、パッケージに入れたものを読む |
| `ZumenLibrary.Open` | セッションを作る（`zm_session_new`）。`using` で閉じる |
| `Session.Push`・`Call`・`CallBytes`・`Take`・`Cancel`・`Dispose` | API の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenException` を投げる |
| `Scene.Read` | シーンの blob を読む。セクションは `Column<T>`（net8.0、写さない `ReadOnlySpan<T>`）か `ToArray<T>` で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあり、`Zumen.Messages` に record の型があります。`Operations.Types` が、op から要求と結果の型を引きます。列挙は `string` で持ち、値を定数で出すので、知らない値も受け取れます。形が値で決まるフィールド（`geometry`・`edit` など）は `JsonElement`（netstandard2.0 では `object`）で持ち、タグの値から `Geometry.Variants` などで型を引いて読み直します。`Cancel` は、ほかのスレッドから呼べます。

## テスト

```bash
cargo build -p zumen-abi
dotnet run --project bindings/dotnet/tests/Conformance --framework net10.0 -- target/debug/libzumen_abi1.so
```

`tests/Conformance` は、[適合テスト](../conformance/README.md)の組をこのラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。CI は、テストの行列の Linux x64・macOS arm64・Windows x64 で .NET 10 を、Windows x86 で 32 bit の .NET Framework 4.8 を流します。

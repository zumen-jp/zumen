# 0196: .NET のラッパーは関数ポインターの委譲で C ABI を呼び、生成した record と netstandard2.0・net8.0 の 2 つのアセンブリで書き、名前を仮にして公開しない

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- .NET は Tier 1 の言語で、C ABI を P/Invoke で呼ぶ（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。生成器は xtask に自前で書き、生成したコードはコミットして `cargo xtask codegen --check` で更新漏れを調べる（[0011](0011-bindings-through-ffi.md)）。API は足すだけで、ラッパーは知らない列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- 下限は .NET Framework 4.8 と .NET 8 で、netstandard2.0 の 1 つのアセンブリで両方に出す想定だった。PR ごとに .NET Framework 4.8 と .NET 10 で確かめる（[docs/support.md](../support.md)）。
- NuGet では `Zumen` の名前で、netstandard2.0 のラッパーとプラットフォームごとのネイティブのライブラリを配る（[0014](0014-platforms-signing-and-distribution.md)）。アセンブリには強い名前を付け、AssemblyVersion を ABI のメジャーバージョンに固定する（[0015](0015-add-only-boundary-and-version-floors.md)）。公開の時期とパッケージの名前は、利用者が決める（#317）。
- netstandard2.0 には、System.Text.Json・`Span<T>`・`NativeLibrary` がない。どれも NuGet のパッケージで足せるが、依存が増え、.NET Framework のホスト（Revit・AutoCAD）でアドインどうしのバージョンがぶつかる元になる。
- 32 bit の Windows では、API の関数を stdcall で出す。
- 手本は Python のラッパー（[0195](0195-python-wrapper.md)）。

## 選択肢

### ネイティブの呼び方

1. **`DllImport` で決まった名前（`zumen_abi1`）を宣言する。** 速いが、ライブラリを探す場所が実行環境に任される。.NET 8 では探し方を差し替えられるが、netstandard2.0 ではできない。
2. **利用者が渡した道筋から読み込み、関数の番地を、生成した委譲の型（`UnmanagedFunctionPointer(CallingConvention.Winapi)`）にする。** net8.0 は `NativeLibrary`、netstandard2.0 は OS の `LoadLibraryW`・`dlopen` を P/Invoke で呼ぶ。Python のラッパーと同じく道筋を明示でき、`Winapi` が 32 bit の Windows では stdcall、ほかでは cdecl になる。呼び出しは粗いので、委譲の手間は効かない。

### アセンブリの分け方

1. **netstandard2.0 の 1 つ。** JSON の型つきの読み書きと `ReadOnlySpan<T>` には、NuGet の依存が要る。
2. **netstandard2.0 と net8.0 の 2 つを 1 つのプロジェクトから作る。** .NET Framework 4.8 は netstandard2.0、.NET 8 以降は net8.0 を選ぶ。net8.0 の側だけ、標準の System.Text.Json で型つきの要求と応答を読み書きし、シーンの列を写さずに `ReadOnlySpan<T>` で見せる。

### 型の書き方

1. **共用体を抽象の record と派生の型にし、System.Text.Json の多態で読む。** タグのフィールドが先頭にないと読めず（.NET 8）、知らないタグの値で失敗する。
2. **オブジェクトは `init` の record、列挙は `string` と値の定数、共用体とスカラーの共用体は `JsonElement`（netstandard2.0 では `object`）にする。** 知らない値で失敗しない。共用体は、タグの値から型を引く表（`Geometry.Variants` など）で読み直す。

## 決定

ネイティブの呼び方は 2、アセンブリの分け方は 2、型の書き方は 2 を選ぶ。

- `bindings/dotnet` にラッパーを置く。名前は仮（`Zumen.Provisional`）とし、`IsPackable` を `false` にする。バージョンはコアと同じにし、AssemblyVersion を ABI のメジャーバージョン（`1.0.0.0`）に固定して、xtask が確かめる（[0015](0015-add-only-boundary-and-version-floors.md)）。強い名前と、配る形（[0014](0014-platforms-signing-and-distribution.md)）は、公開の手順を決めるときに足す。
- NuGet の依存を持たない。netstandard2.0 で C# の新しい書き方を使うための型（`IsExternalInit`、`JsonPropertyName` の代わり）は、ラッパーの中に internal で置く。
- **形**
  - `ZumenLibrary.Load` で、利用者が渡した道筋からライブラリを読み込み、関数がそろうことと ABI のメジャーバージョンを確かめる。読み込んだライブラリは外さない。
  - `ZumenLibrary.Open` でセッションを作る。`Session` は `Push`・`Call`・`CallBytes`・`Take`・`Cancel`・`Dispose` を持ち、API の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。net8.0 では `Call<TResult>(要求)` が型つきの `Response<TResult>` を返す。コアが返したバッファーは、写したあとにラッパーの中で `zm_free` で返す。長さ 0 の入力にも NULL でない番地を渡す。
  - 状態が `ok` でなければ `ZumenException` を投げる。`failed` の応答も、例外の `ResponseText`（net8.0 は `Response`）で読める。閉じたセッションへの呼び出しは NULL のまま API に渡し、API が `invalid` を返す。`Cancel` は、ほかのスレッドから呼べる。
  - シーンの blob は `Scene.Read` で読む。セクションは `ToArray<T>`（写す）と、net8.0 では `Column<T>`（環境がリトルエンディアンなら写さない `ReadOnlySpan<T>`）で引く。知らないセクションは読み飛ばし、ないセクションは空の列にし、メジャーバージョンが違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `Zumen.Provisional/Messages.g.cs`: `schema/messages.json` の定義ごとの型と、`Operations.Types`（op から要求と結果の型を引く表）。
  - `Zumen.Provisional/Constants.g.cs`: バージョン、状態コード（`Status` と名前）、ネイティブのライブラリが出す関数の委譲の型、シーンのセクションの定義と値の組。
- **適合テスト**: `bindings/conformance` の組をラッパーのメソッドで流し、状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob をリーダーで読んで応答の数と合うことを確かめる。テストは .NET Framework 4.8 でも動くように、小さな JSON の読み書きをテストの中に持つ。CI のテストの行列で、Linux x64・macOS arm64・Windows x64 は .NET 10（net8.0 のアセンブリ）、Windows x86 は 32 bit の .NET Framework 4.8（netstandard2.0 のアセンブリと stdcall）で流す。出力は、端末のコード・ページによらず UTF-8 で書く。

## 影響

- Tier 1 に上げる 3 つの条件（生成のテンプレート・API の関数のラッパー・適合テスト）を、この PR で満たす。
- 64 bit の .NET Framework 4.8 と、Linux・macOS の netstandard2.0 の道筋（`dlopen`）は、CI で流していない。
- `zm_take_arrow`（Arrow の C Stream Interface）は委譲の型だけを持ち、ラッパーのメソッドにしない。
- netstandard2.0 の側の型は、フィールドの名前をラッパーの中の属性で持つので、System.Text.Json のパッケージを足しても名前を引けない。.NET Framework で型つきに読むときは、名前の規則を合わせるか、net8.0 と同じ扱いを公開の前に決める。
- ラッパーの API と生成した型は、公開するまで変えてよい。公開してからは、API と同じく足すだけにする。
- 新しい op・フィールド・セクションを足すと、`bindings/dotnet/Zumen.Provisional` の生成したファイルの差分が PR に並ぶ。

## 参照

- [#351](https://github.com/zumen-jp/zumen/issues/351)、[#317](https://github.com/zumen-jp/zumen/issues/317)
- [0006](0006-language-tiers.md)、[0007](0007-single-rust-implementation.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0195](0195-python-wrapper.md)
- 名前と公開: [0205: バージョン 0.2.0 から、タグを push してワークフローで crates.io・npm・PyPI・NuGet と GitHub の Release に出す](0205-release-and-publishing.md)

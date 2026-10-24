# 0014: 対象のプラットフォームを決め、成果物に署名し、言語の段に合わせて配る

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 各言語へは、C ABI のライブラリと wasm のモジュール（[0011](0011-bindings-through-ffi.md)）、CLI（[0013](0013-boundary-in-four-forms.md)）を配る。言語は 3 つの段に分けた（[0006](0006-language-tiers.md)）。
- 利用者の環境は広い。
  - Windows の x64 が多い。32 bit の Office が残り、ARM64 の PC も増えた。
  - Mac は arm64 が主で、Intel の Mac も残る。
  - Linux のサーバーとコンテナには、glibc のものと musl のもの（Alpine）がある。
  - 現場では iPad と Android の端末を使う。Unity の IL2CPP は、iOS と Web では静的ライブラリをリンクする。
- 会社の PC では、署名のない DLL や実行ファイルが止められることがある。
- 2026-10-24 に確かめた事情
  - Rust 1.99.1 では、Windows の x64・x86・ARM64、arm64 の macOS、glibc の Linux（x64・arm64）が Tier 1 にあたる。Intel の macOS（1.90 から）と musl の Linux は Tier 2 で、iOS・Android・wasm32 も Tier 2。32 bit の Windows は、1.100 から、ホストの道具を持たない Tier 1 になる。
  - Rust 1.99.0 には、bool を返す `extern "C"` の関数を x64 で誤ってコンパイルする不具合があった（1.99.1 で直った）。
  - Windows の署名
    - コード署名の証明書の鍵は、2023 年から、ハードウェアの暗号モジュールに置くことが求められる。2026-03-01 からは、証明書の有効期間が 460 日までになった。
    - Microsoft の Azure Artifact Signing（旧 Trusted Signing）は、鍵を預かって署名する。最新の文書では、日本の法人も使える（個人は米国とカナダだけ）。月 9.99 ドルから。
    - SignPath Foundation は、OSS に無料で署名する。ただし、証明書の名前（利用者に見える発行者）は「SignPath Foundation」になる。
    - SmartScreen は、2024 年から、EV と OV の証明書を同じに扱う。
  - Apple は、Developer ID で署名したソフトに公証を求める。zip で出す CLI と dylib も対象になる。組織で入るには D-U-N-S 番号が要る。Homebrew の formula の bottle には、署名も公証も要らない。
  - 長く有効なトークンを CI に置かずに公開する仕組み（trusted publishing）は、PyPI・npm・crates.io・NuGet・RubyGems・pub.dev にある。Maven Central にはない。npm と PyPI は、来歴の証明も付けられる。
  - vcpkg は、ビルドした成果物を配るポートを強く勧めない。Conan Center と conda-forge は、ソースから作るレシピを求める。
  - SwiftPM の `binaryTarget` は、Apple のプラットフォームの XCFramework だけを受ける。
  - Go の module は、zip で 500 MiB まで。

## 選択肢

### 対象

1. **主要な 3 つ**（Windows x64・macOS arm64・Linux x64）だけにする。32 bit の Office、現場の端末、コンテナで使えない。
2. **利用者の環境を広く対象にし、確かめ方の重さを分ける。** PR ごとに試すもの、毎晩に試すもの、作るだけのものに分ける。

### Windows の署名

1. **署名しない。** 会社の PC で止められる。
2. **OV の証明書を買い、鍵をクラウドの保管に置く。** 年に数万円かかる。発行者の名前は、公開する主体になる。
3. **Azure Artifact Signing。** Microsoft が鍵を預かって署名する。費用が小さく、発行者の名前は公開する主体になる。日本の法人が使えることは最新の文書にあるが、古い頁と食い違い、創業からの年数の条件も確かめが要る。
4. **SignPath Foundation。** OSS には無料。ただし、発行者の名前が「SignPath Foundation」になる。

### 配る場所

1. **最初から、すべてのレジストリに出す。** 段 3 の言語まで、包みと公開の手間がかかる。
2. **言語の段に合わせて、順に広げる。**

## 決定

対象は 2、配る場所は 2 を選ぶ。署名は 3 を選び、使えなければ 2、それもできなければ 4 にする。

### 対象

対象のプラットフォームと、確かめ方の重さは、[docs/support.md](../support.md) の表で管理する。最初の版は、次のとおり。

| OS | アーキテクチャ | Rust の target | 確かめ方 |
| --- | --- | --- | --- |
| Windows | x64・x86 | `x86_64-pc-windows-msvc`・`i686-pc-windows-msvc` | PR ごとに試験 |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | 毎晩に試験 |
| macOS | arm64 | `aarch64-apple-darwin` | PR ごとに試験 |
| macOS | x64 | `x86_64-apple-darwin` | 毎晩に作る |
| Linux | x64 | `x86_64-unknown-linux-gnu` | PR ごとに試験 |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | 毎晩に試験 |
| Linux | x64・arm64（musl） | `x86_64-unknown-linux-musl`・`aarch64-unknown-linux-musl` | 毎晩に試験 |
| iOS・iPadOS | arm64（実機・シミュレーター） | `aarch64-apple-ios`・`aarch64-apple-ios-sim` | 毎晩に作る |
| Android | arm64-v8a・armeabi-v7a・x86_64 | `aarch64-linux-android`・`armv7-linux-androideabi`・`x86_64-linux-android` | 毎晩に作る |
| wasm | wasm32 | `wasm32-unknown-unknown` | PR ごとに試験 |

- 32 bit の Windows は、32 bit の Office の VBA のために、PR ごとに試す。関数を stdcall で出す所（[0011](0011-bindings-through-ffi.md)）と、ポインタの幅の違いで壊れやすい。
- [0007](0007-single-rust-implementation.md) のハッシュの一致は、PR ごとに試験する対象と wasm32 で確かめる。
- リリースでは、表のすべての対象を作る。

### 作り方

- **静的にリンクする。**
  - Windows では、C の実行時ライブラリも静的にリンクする。Visual C++ の再頒布可能パッケージを入れていない PC でも動く。
  - glibc の Linux では、glibc 2.28 の環境で作る（Python の `manylinux_2_28`）。musl の版は、すべてを静的にリンクする。glibc 2.28 より古い Linux では、musl の CLI を、プロセスの外か HTTP の形（[0013](0013-boundary-in-four-forms.md)）で使う。
  - iOS・Android・Unity のために、静的ライブラリも作る。Apple のプラットフォームには、XCFramework にまとめて配る。
- **外のものに頼らない。** 核とライブラリは、システムのフォント・OpenSSL・ネットワークを使わない。CLI の HTTP も、TLS を持たない（[0013](0013-boundary-in-four-forms.md)）。
- **ライセンスの文。** どの成果物にも、依存のライセンスの文（`THIRD_PARTY_LICENSES`）を入れる。cargo-about（MIT OR Apache-2.0、Cargo の外の道具）で作る。
- **SBOM と来歴**
  - リリースに、CycloneDX の SBOM を付ける（cargo-cyclonedx、Apache-2.0）。
  - 実行ファイルと動的ライブラリには、cargo-auditable で依存の一覧を埋め込む。
  - リリースのファイルに、GitHub の artifact attestations で来歴の証明を付ける。
- **作る所。** リリースは、GitHub のホストする runner だけで作る（公開リポジトリに self-hosted runner をつながない。[0004](0004-repositories.md)）。
- **ツールチェーン。** 境界の関数は `bool` を使わない（[0012](0012-boundary-functions-and-messages.md)）。ツールチェーンを上げるときは、Rust の既知の不具合を確かめる。

### 署名

- **Windows。** すべての .dll と .exe に、Authenticode で署名し、時刻印を付ける。
  - 署名の手段は、上の順に選ぶ。どれにしたかは、最初のリリースの前に記録する。
  - EV の証明書は使わない。SmartScreen で、OV と違いがない。
- **macOS。** GitHub Releases で配る CLI と dylib に、Developer ID で署名し（hardened runtime）、公証を受ける。zip で公証に出し、署名したファイルを配る。
- **鍵と権限。** 署名と公開の資格は、承認の要る GitHub の environment（`release`）に置き、リリースの workflow だけが使う。
- **署名の前のリリース。** 署名の準備が整うまでは、0.x のリリースに限り、署名していないことをリリースノートに書いて出す。

### 配る場所

| 段 | 言語 | 配る場所 | 中身 |
| --- | --- | --- | --- |
| 1 | Rust | crates.io | crate |
| 1 | TS・JS | npm（`@zumen-jp/*`） | wasm のモジュールと包み |
| 1 | Python | PyPI（`zumen`） | ctypes の包みと、ネイティブのライブラリ。言語の版によらない、プラットフォームごとの wheel |
| 1 | .NET | NuGet（`Zumen`） | netstandard2.0 の包みと、プラットフォームごとのネイティブのライブラリ |
| 1 | C・C++ | GitHub Releases | ヘッダー、ライブラリ、CMake の設定 |
| 1 | CLI・MCP | GitHub Releases、winget、Homebrew（`zumen-jp/tap/zumen`）、コンテナのイメージ（GHCR） | `zumen` |
| 2 | Java・Kotlin | Maven Central（`dev.zumen`） | JAR と、ネイティブのライブラリ。Android には AAR |
| 2 | Swift | SwiftPM（鏡の `zumen-swift`） | XCFramework |
| 2 | Go | `zumen.dev/go`（鏡の `zumen-go`） | wasm を埋め込んだ module |
| 2 | Dart | pub.dev | 包みと、ネイティブのライブラリ |
| 2 | VBA | GitHub Releases | zip: 32 bit と 64 bit の DLL、子プロセス用の CLI、.bas、サンプル |
| 3 | Ruby・PHP・R など | 文書とサンプル | 段を上げるときに、パッケージを作る（[0006](0006-language-tiers.md)） |

- **子プロセス用の CLI**（[0013](0013-boundary-in-four-forms.md) の `process`）は、MCP と HTTP を外して作り、PyPI・npm・NuGet では別のパッケージにする。主なパッケージを小さく保ち、ホストアプリで使う人だけが足す。VBA の zip には入れる。
- **鏡のリポジトリ**は、リリースの CI が GitHub App で書く（[0004](0004-repositories.md)）。
- **あとで足す所。** conda-forge（ソースから作るレシピ）、Pyodide、vcpkg と Conan（ソースから作るレシピか、自前のレジストリ）、Homebrew の本家。PyPI と C の配り方が固まってから行う。

### 公開の仕方

- trusted publishing のあるレジストリ（PyPI・npm・crates.io・NuGet・RubyGems・pub.dev）では、それを使い、長く有効なトークンを置かない。crates.io と pub.dev は、最初の版だけを手で出す。
- Maven Central には、承認の要る environment に置いたトークンと、署名の鍵で出す。
- npm の provenance と、PyPI の attestations を付ける。
- 公開は、承認の要る environment（`release`）の中だけで行う。

## 影響

- 費用がかかる。Apple Developer Program（年 99 ドル）と、Windows の署名（Azure Artifact Signing なら月 9.99 ドルから）。組織で入るには、D-U-N-S 番号などの手続きが要る。
- 公開する主体（法人）を、最初のリリースの前に決める。署名の名前と、各レジストリの持ち主になる。
- CI に、毎晩の workflow と、リリースの workflow を足す。今の CI（PR ごと）には、32 bit の Windows と、wasm32 の試験を、境界の crate を作る PR で足す。
- Intel の macOS は、Homebrew が 2026 年 9 月に bottle を作らなくなり、2027 年 9 月に対応を終える。Rust と Homebrew の対応がなくなったら、対象から外す。
- [#16](https://github.com/zumen-jp/zumen/issues/16) は、この記録で完了する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「6. 組織とリポジトリ」と「7. あらゆる言語から使う」、[#16](https://github.com/zumen-jp/zumen/issues/16)
- [0003](0003-names-and-domain.md)、[0004](0004-repositories.md)、[0006](0006-language-tiers.md)、[0007](0007-single-rust-implementation.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0013](0013-boundary-in-four-forms.md)、[0015](0015-add-only-boundary-and-version-floors.md)
- [Rust の対応するプラットフォーム](https://doc.rust-lang.org/rustc/platform-support.html)、[Rust 1.99.1](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/)
- [Azure Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)、[SignPath Foundation の条件](https://signpath.org/terms)、[CA/Browser Forum の code signing の要件](https://cabforum.org/working-groups/code-signing/requirements/)、[Windows のコード署名の選択肢](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
- [Apple の公証](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [npm の trusted publishing](https://docs.npmjs.com/trusted-publishers)、[NuGet の trusted publishing](https://learn.microsoft.com/en-us/nuget/nuget-org/trusted-publishing)、[PyPI の attestations](https://blog.pypi.org/posts/2024-11-14-pypi-now-supports-digital-attestations/)

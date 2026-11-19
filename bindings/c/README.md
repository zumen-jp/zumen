# C と C++ のバインディング

C ABI のライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib` と、静的ライブラリ）を、C と C++ から使うためのヘッダーです（[依存とツールの条件](../../AGENTS.md#範囲と権利)・[API のスキーマ](../../schema/README.md)）。

| ファイル | 中身 | 作り方 |
| --- | --- | --- |
| `include/zumen.h` | API の関数・状態コード・バージョン（C11） | `schema/boundary.toml` から生成 |
| `include/zumen_scene.h` | シーンの blob のセクションの番号・型・幅と、列に入れる値（C11） | `schema/scene.toml` から生成 |
| `include/zumen.hpp` | C++ のラッパー。RAII のセッションとバッファー、例外、シーンのリーダー（C++17、ヘッダーだけ） | 手で書く |

- 生成したヘッダーは `cargo xtask codegen` で作り直します。手で直さないでください。
- C11 と C++17 で使えます（[docs/support.md](../../docs/support.md)）。
- 要求と応答は、C でも C++ でも JSON の文字列のまま渡します。JSON の読み書きには、好きなライブラリを使ってください。
- 関数は 64 bit の標準 C 呼び出し規約で出します（`ZM_CALL` は互換用の空のマクロです）。
- 日本語のコメントを、MSVC が OS のコードページによらず UTF-8 として読むように、ヘッダーは BOM つきの UTF-8 にしています。

## 使い方

```c
#include "zumen.h"

zm_session *session = NULL;
zm_buf response = {NULL, 0};
const char request[] = "{\"v\":1,\"op\":\"capabilities\"}";

if (zm_session_new(NULL, 0, &session) == ZM_OK) {
    zm_status status = zm_call(session, (const uint8_t *)request, sizeof request - 1, &response);
    /* status が ZM_OK・ZM_FAILED・ZM_PANICKED なら、response に応答の JSON がある */
    zm_free(&response);
    zm_session_close(session);
}
```

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあります。

C++ では、`zumen.hpp` のラッパーを使えます。セッションとコアが返したバッファーは、壊すときに閉じ・返します。状態が `ok` でなければ `zumen::Error` を投げ、`failed` の応答は `Error::response` で読めます。投げない `try_` の関数もあります。

```cpp
#include "zumen.hpp"

zumen::Session session = zumen::Session::open();
session.push("a.dxf", dxf_text);
session.call(R"({"v":1,"op":"open","input":"a.dxf"})");
zumen::Bytes response = session.call(R"({"v":1,"op":"scene","doc":"a.dxf","from":"space/0"})");
zumen::Bytes blob = session.take(0);  // 応答の blobs の番号
zumen::Scene scene = zumen::Scene::read(blob);  // blob を写さずに見る。blob を生かしておく
zumen::Column<float> xy = scene.column<float>(ZM_SCENE_SECTION_LINE_XY);
```

シーンのリーダーは、知らないセクションを読み飛ばし、ないセクションを空の列にし、メジャーバージョンが違えば `zumen::SceneError` を投げます。列の値は、環境のバイト順によらず、リトルエンディアンから直して返します。

## テスト

この Cargo のパッケージ（`zumen-c`）はテストのためだけのもので、成果物には入りません。テストの C と C++ を、ヘッダーとラッパーで、警告を失敗にして組み立てます。

- `tests/smoke.c`・`tests/smoke.cpp`: 生成したヘッダーで、API の関数を呼びます。
- `tests/conformance.c`・`tests/conformance.cpp`: 適合テスト（[bindings/conformance](../conformance/README.md)）の組を、C はヘッダーだけで、C++ は `zumen.hpp` のラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。C++ では、シーンの blob を `zumen::Scene` で読み、応答の数と合うことも確かめます。組の手順を読んで応答を確かめるのは、Rust のテストのツール（`tests/conformance.rs`）です。

```bash
cargo test -p zumen-c
```

CI は Windows x64 でもこのテストを流し、DLL が飾りのない名前（`zm_call` など）で関数を出すことも確かめます。

## 3D のシーン

C では zumen_scene3d_reader.h の zm_scene3d_read、C++ では zumen::Scene3D::read で ZMS3 を読みます。C の zm_scene3d_instance_at と zm_scene3d_element_id、C++ の instance_element と element_id で建物の要素 ID を引けます。列と文字列は元の blob を借用します。

同じ blob の 2D 用の読み手との混用はできません。列の配置・座標・単位・色・選択と検査範囲は [3D のシーンの契約](../../schema/README.md#3d-のシーンの契約)に従います。元の blob を共有する読み手の利用中は、そのバイト列を変更しません。

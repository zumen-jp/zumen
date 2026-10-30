# C と C++ の結合

C ABI のライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib` と、静的ライブラリ）を、C と C++ から使うためのヘッダーです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0012](../../docs/decisions/0012-boundary-functions-and-messages.md)・[決定 0198](../../docs/decisions/0198-c-and-cpp-wrapper.md)）。

| ファイル | 中身 | 作り方 |
| --- | --- | --- |
| `include/zumen.h` | 境界の関数・状態コード・版（C99） | `schema/boundary.toml` から生成 |
| `include/zumen_scene.h` | シーンの blob の区画の番号・型・幅と、列に入れる値（C99） | `schema/scene.toml` から生成 |
| `include/zumen.hpp` | C++ の包み。RAII のセッションとバッファー、例外、シーンの読み手（C++17、ヘッダーだけ） | 手で書く |

- 生成したヘッダーは `cargo xtask codegen` で作り直します。手で直さないでください。
- C99 と C++17 で使えます（[docs/support.md](../../docs/support.md)）。
- 要求と応答は、C でも C++ でも JSON の文字列のまま渡します。JSON の読み書きには、好きなライブラリを使ってください。
- 32 bit の Windows では、関数を stdcall で出します（`ZM_CALL`）。VBA の `Declare` から呼ぶためです。
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

C++ では、`zumen.hpp` の包みを使えます。セッションとコアが返したバッファーは、壊すときに閉じ・返します。状態が `ok` でなければ `zumen::Error` を投げ、`failed` の応答は `Error::response` で読めます。投げない `try_` の関数もあります。

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

シーンの読み手は、知らない区画を読み飛ばし、ない区画を空の列にし、大版が違えば `zumen::SceneError` を投げます。列の値は、環境のバイト順によらず、リトルエンディアンから直して返します。

## 試験

この Cargo のパッケージ（`zumen-c`）は試験のためだけのもので、成果物には入りません。試験の C と C++ を、ヘッダーと包みで、警告を失敗にして組み立てます。

- `tests/smoke.c`・`tests/smoke.cpp`: 生成したヘッダーで、境界の関数を呼びます。
- `tests/conformance.c`・`tests/conformance.cpp`: 適合試験（[bindings/conformance](../conformance/README.md)）の組を、C はヘッダーだけで、C++ は `zumen.hpp` の包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。C++ では、シーンの blob を `zumen::Scene` で読み、応答の数と合うことも確かめます。組の手順を読んで応答を確かめるのは、Rust の試験の道具（`tests/conformance.rs`）です。

```bash
cargo test -p zumen-c
```

32 bit の Windows では、ヘッダーの呼び出し規約がコアと合わないと、リンクできません。CI は、`i686-pc-windows-msvc` でもこの試験を流し、DLL が飾りのない名前（`zm_call` など）で関数を出すことも確かめます。

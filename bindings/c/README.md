# C と C++ の結合

C ABI のライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib` と、静的ライブラリ）を、C と C++ から使うためのヘッダーです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0012](../../docs/decisions/0012-boundary-functions-and-messages.md)）。

- `include/zumen.h` は `schema/boundary.toml` から生成します。手で直さないでください。
- C99 と C++17 で使えます（[docs/support.md](../../docs/support.md)）。
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

## 試験

この Cargo のパッケージ（`zumen-c`）は試験のためだけのもので、成果物には入りません。`tests/smoke.c` と `tests/smoke.cpp` を、生成したヘッダーで、警告を失敗にして組み立て、境界の関数を呼びます。

```bash
cargo test -p zumen-c
```

32 bit の Windows では、ヘッダーの呼び出し規約が核と合わないと、リンクできません。CI は、`i686-pc-windows-msvc` でもこの試験を流し、DLL が飾りのない名前（`zm_call` など）で関数を出すことも確かめます。

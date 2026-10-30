# Python の包み

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、標準ライブラリの ctypes で呼ぶ薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0195](../../docs/decisions/0195-python-wrapper.md)）。Python 3.9 以降で動きます。

- まだ公開しません。`pyproject.toml` の名前（`zumen-provisional`）とモジュールの名前（`zumen_provisional`）は仮で、公開の時期と名前は利用者が決めます（#317）。
- 依存はなく、ネイティブの拡張も作りません。ネイティブのライブラリは包みに入れず、利用者が道筋を渡します。
- `zumen_provisional/messages.py`・`zumen_provisional/constants.py` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```python
from pathlib import Path
from zumen_provisional import ZumenError, load, read_scene

zumen = load("target/release/libzumen_abi1.so")
with zumen.open() as session:
    session.push("a.dxf", Path("a.dxf").read_bytes())
    try:
        result = session.call({"v": 1, "op": "open", "input": "a.dxf"})["result"]
        scene = session.call({"v": 1, "op": "scene", "doc": result["doc"], "from": "space/0"})
        columns = read_scene(session.take(scene["result"]["blob"])).columns
        print(columns["chunk_origins"].tolist(), columns["line_xy"].tolist())
    except ZumenError as error:
        # failed の応答は error.response にあり、診断を持ちます
        print(error.status, error.response and error.response["diagnostics"])
```

| 名前 | 中身 |
| --- | --- |
| `load` | ライブラリを読み込み、関数と ABI の大版を確かめる。32 bit の Windows では stdcall で呼ぶ |
| `Zumen.open` | セッションを作る（`zm_session_new`）。`with` の文で閉じられる |
| `Session.push`・`call`・`call_bytes`・`take`・`cancel`・`close` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenError` を投げる |
| `read_scene` | シーンの blob を、区画ごとの memoryview で読む（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあり、`messages.py` に `TypedDict` の型があります。応答の `result` は、`OPERATIONS` が op から引く結果の型（`OpenResult` など）で `typing.cast` して読みます。ctypes はネイティブの関数を呼ぶ間 GIL を手放すので、長い `call` を、ほかのスレッドから `cancel` で止められます。

## 試験

```bash
cargo build -p zumen-abi
python3 bindings/python/tests/conformance.py target/debug/libzumen_abi1.so
```

`tests/conformance.py` は、[適合試験](../conformance/README.md)の組をこの包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。CI は、試験の行列（Linux x64・macOS arm64・Windows x64・Windows x86）で、Python 3.9 と最新を分けて流します。

"""適合試験を、Python の包み（bindings/python）を通して流す（決定 0006・0195）。

使い方: python bindings/python/tests/conformance.py <ネイティブのライブラリ>

bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し、状態と応答の値が期待どおりで、ハッシュの
一覧が hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。
手順の読み方と応答の確かめ方は、Rust の側（bindings/conformance/src/lib.rs）と、JavaScript の側
（bindings/conformance/cases.mjs）と同じ規則で書く。
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
import threading
from pathlib import Path
from typing import Any, Callable, List, Optional, Tuple

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from zumen import (
    SCENE_MAJOR,
    SCENE_SECTIONS,
    SCHEMA_MAJOR,
    ZumenError,
    load,
    read_scene,
)

REPOSITORY = HERE.parent.parent.parent
CONFORMANCE = REPOSITORY / "bindings" / "conformance"

# 手順の種類。手順は、どれか 1 つの欄を持つ
ACTIONS = ["session", "push", "call", "call_text", "call_hex", "take"]

_MISSING = object()


def decode_hex(text: str) -> bytes:
    digits = re.sub(r"\s+", "", text)
    if not re.fullmatch(r"([0-9a-fA-F]{2})*", digits):
        raise ValueError(f"16 進として読めない: {text}")
    return bytes.fromhex(digits)


def pointer(value: Any, path: str) -> Any:
    """JSON Pointer で値を引く。なければ _MISSING"""
    if path == "":
        return value
    current = value
    for raw in path[1:].split("/"):
        token = raw.replace("~1", "/").replace("~0", "~")
        if isinstance(current, list):
            if not re.fullmatch(r"0|[1-9][0-9]*", token) or int(token) >= len(current):
                return _MISSING
            current = current[int(token)]
        elif isinstance(current, dict) and token in current:
            current = current[token]
        else:
            return _MISSING
    return current


def same(a: Any, b: Any) -> bool:
    """JSON の値として等しいか。真偽値と数は別にする"""
    if isinstance(a, bool) or isinstance(b, bool):
        return isinstance(a, bool) and isinstance(b, bool) and a == b
    if isinstance(a, list) or isinstance(b, list):
        return isinstance(a, list) and isinstance(b, list) and len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    if isinstance(a, dict) or isinstance(b, dict):
        return isinstance(a, dict) and isinstance(b, dict) and a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return a == b
    return type(a) is type(b) and a == b


def dumps(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def bytes_of(step_input: Any, value_key: str, text_key: str) -> bytes:
    """session の設定か、call の要求のバイト列。値は JSON に直し、文字列はそのまま渡す"""
    if step_input is None:
        return b""
    if value_key in step_input:
        return dumps(step_input[value_key])
    if text_key in step_input:
        return step_input[text_key].encode("utf-8")
    return b""


def read_sample(file: str) -> bytes:
    """公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る"""
    parts = file.split("/")
    if parts[0] != "samples" or len(parts) < 2 or any(p in ("", ".", "..") or "\\" in p for p in parts[1:]):
        raise ValueError(f"push の file は samples/ の中のファイルにする: {file}")
    return REPOSITORY.joinpath(*parts).read_bytes()


def push_of(step_input: Any) -> Tuple[bytes, bytes]:
    """push の手順の、入力の名前と中身"""
    name = decode_hex(step_input["name_hex"]) if "name_hex" in step_input else step_input["name"].encode("utf-8")
    if "hex" in step_input:
        data = decode_hex(step_input["hex"])
    elif "file" in step_input:
        data = read_sample(step_input["file"])
    else:
        data = step_input.get("text", "").encode("utf-8")
    return name, data


def request_of(action: str, step_input: Any) -> bytes:
    """call・call_text・call_hex の手順の、要求のバイト列"""
    if action == "call_text":
        return step_input.encode("utf-8")
    if action == "call_hex":
        return decode_hex(step_input)
    return dumps(step_input)


def load_cases() -> List[Tuple[str, list]]:
    """試験の組を、名前の順に返す"""
    files = sorted(path for path in (CONFORMANCE / "cases").iterdir() if path.suffix == ".json")
    return [(path.stem, json.loads(path.read_text(encoding="utf-8"))["steps"]) for path in files]


def check_response(at: str, step: dict, response: Any, failures: List[str]) -> None:
    """応答の値を、手順の expect と absent で確かめる"""
    for path, value in step.get("expect", {}).items():
        found = pointer(response, path)
        if found is _MISSING:
            failures.append(f"{at}: {path} がない")
        elif not same(found, value):
            failures.append(f"{at}: {path} が {json.dumps(found)}（期待は {json.dumps(value)}）")
    for path in step.get("absent", []):
        if pointer(response, path) is not _MISSING:
            failures.append(f"{at}: {path} があってはいけない")


def compare_hashes(hashes: List[str], failures: List[str]) -> None:
    """ハッシュの一覧が hashes.txt と一致するかを確かめる"""
    lines = (CONFORMANCE / "hashes.txt").read_text(encoding="utf-8").split("\n")
    expected = [line for line in lines if line != "" and not line.startswith("#")]
    if hashes != expected:
        missing = [f"- {line}" for line in expected if line not in hashes]
        extra = [f"+ {line}" for line in hashes if line not in expected]
        failures.append("ハッシュの一覧が hashes.txt と合わない:\n" + "\n".join(missing + extra))


def attempt(fn: Callable[[], Any]) -> Tuple[str, Any]:
    """境界の関数を呼び、状態の名前と、書かれたバイト列（か戻り値）を返す"""
    try:
        return "ok", fn()
    except ZumenError as error:
        return error.status, error.data


def check_scene(at: str, blob: bytes, result: dict, failures: List[str]) -> None:
    """シーンの blob を読み手で読み、応答の数と合うかを確かめる"""
    try:
        scene = read_scene(blob)
    except ValueError as error:
        failures.append(f"{at}: シーンを読めない: {error}")
        return
    counts = result["counts"]
    checks = [
        ("大版", scene.major, SCENE_MAJOR),
        ("区画の数", scene.sections, len(SCENE_SECTIONS)),
        ("塊の数", scene.chunks, counts["chunks"]),
        ("chunk_origins の行", scene.rows("chunk_origins"), counts["chunks"]),
        ("text_xy の行", scene.rows("text_xy"), counts["texts"]),
        ("point_xy の行", scene.rows("point_xy"), counts["points"]),
        ("glyph_xy の行", scene.rows("glyph_xy"), counts["glyphs"]),
        ("line_xy と face_xy の行", scene.rows("line_xy") + scene.rows("face_xy"), counts["vertices"]),
    ]
    # 文字の文字列は、strings の区画から引ける
    strings = scene.columns["text_strings"]
    for row in range(0, len(strings), 2):
        try:
            scene.text(strings[row], strings[row + 1])
        except (IndexError, UnicodeDecodeError) as error:
            failures.append(f"{at}: text_strings の行 {row // 2} を引けない: {error}")
    for label, actual, expected in checks:
        if actual != expected:
            failures.append(f"{at}: シーンの{label}が {actual}（期待は {expected}）")


def main() -> int:
    if len(sys.argv) != 2:
        print("使い方: python bindings/python/tests/conformance.py <ネイティブのライブラリ>", file=sys.stderr)
        return 2
    zumen = load(sys.argv[1])
    failures: List[str] = []
    hashes: List[str] = []
    scenes = 0
    cases = load_cases()
    for name, steps in cases:
        session = None
        # 直前の応答が返したシーンの blob の番号と結果
        last_scene: Optional[dict] = None
        for index, step in enumerate(steps):
            at = f"{name} の手順 {index}"
            action = next(key for key in ACTIONS if key in step)
            expected = step.get("status", "ok")
            step_input = step[action]
            if action != "session" and session is None:
                session = zumen.open()
            output = None
            if action == "session":
                if session is not None:
                    session.close()
                status, session = attempt(lambda: zumen.open(bytes_of(step_input, "options", "options_text")))
                if status != "ok":
                    # 作れなかったら、閉じたセッションで続ける。境界は、空のセッションへの呼び出しを invalid にする
                    session = zumen.open()
                    session.close()
            elif action == "push":
                status, _ = attempt(lambda: session.push(*push_of(step_input)))
            elif action == "take":
                status, data = attempt(lambda: session.take(step_input))
                output = ("blob", data) if data is not None else None
                if status == "ok" and last_scene is not None and last_scene["blob"] == step_input:
                    check_scene(at, data, last_scene, failures)
                    scenes += 1
            else:
                request = request_of(action, step_input)
                status, data = attempt(lambda: session.call_bytes(request))
                output = ("response", data) if data is not None else None
            if status != expected:
                failures.append(f"{at}: 状態が {status}（期待は {expected}）")
            if output is None:
                continue
            kind, data = output
            hashes.append(f"{name} {index} {kind} {hashlib.sha256(data).hexdigest()}")
            if kind != "response":
                continue
            response = json.loads(data)
            check_response(at, step, response, failures)
            blob = next((entry for entry in response.get("blobs", []) if entry.get("kind") == "scene"), None)
            last_scene = dict(response["result"], blob=blob["id"]) if blob and response.get("result") else None
        # 動いている呼び出しがないので、何もしない
        session.cancel()
        session.close()

    compare_hashes(hashes, failures)

    # 値を渡す call と、失敗した call と、閉じたあとの呼び出し
    with zumen.open({"v": SCHEMA_MAJOR}) as session:
        response = session.call({"v": SCHEMA_MAJOR, "op": "capabilities"})
        if response.get("v") != SCHEMA_MAJOR or not isinstance(response.get("result", {}).get("package"), str):
            failures.append(f"call の応答が capabilities の形でない: {json.dumps(response)[:200]}")
        try:
            session.call({"v": SCHEMA_MAJOR, "op": "open", "input": "none.dxf"})
            failures.append("ない入力を開く call が失敗しない")
        except ZumenError as error:
            if error.status != "failed" or not (error.response or {}).get("diagnostics"):
                failures.append(f"ない入力を開く call の失敗が、診断のある failed でない: {error}")
        # ほかのスレッドから cancel を呼べる（動いている呼び出しがなければ、何もしない）
        thread = threading.Thread(target=session.cancel)
        thread.start()
        thread.join()
    status, _ = attempt(lambda: session.call({"v": SCHEMA_MAJOR, "op": "capabilities"}))
    if not session.closed or status != "invalid":
        failures.append(f"閉じたあとの call が {status}（期待は invalid）")

    summary = f"包みを通した試験の組 {len(cases)} 件、ハッシュ {len(hashes)} 件、読んだシーン {scenes} 件"
    print(summary)
    step_summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if step_summary:
        with open(step_summary, "a", encoding="utf-8") as file:
            file.write(f"### Python の包みの適合試験（Python {sys.version.split()[0]}）\n\n{summary}\n")
    if scenes == 0:
        failures.append("シーンの blob を 1 つも読んでいない")
    if failures:
        print("期待と違った:\n" + "\n".join(failures), file=sys.stderr)
        return 1
    print("ネイティブと同じ一覧になった")
    return 0


if __name__ == "__main__":
    # Windows の端末の既定（cp1252 など）では日本語を書けないので、UTF-8 で書く
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8")
    sys.exit(main())

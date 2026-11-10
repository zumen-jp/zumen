"""共通の適合テストを、JSON パーサーの要らない VBA 用の手順に直す。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
CONFORMANCE = ROOT / "bindings/conformance"
STATUS = {"ok": 0, "failed": 1, "invalid": 2, "busy": 3, "panicked": 4}


def encoded(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def input_bytes(value: dict, *, name: bool = False) -> bytes:
    if name:
        return bytes.fromhex(value["name_hex"]) if "name_hex" in value else value["name"].encode("utf-8")
    if "hex" in value:
        return bytes.fromhex(value["hex"])
    if "text" in value:
        return value["text"].encode("utf-8")
    if "file" in value:
        path = (ROOT / value["file"]).resolve()
        if not path.is_relative_to(ROOT / "samples"):
            raise ValueError("公開サンプル以外は読み込まない")
        return path.read_bytes()
    return b""


def rows() -> list[list[str]]:
    hashes = {}
    for line in (CONFORMANCE / "hashes.txt").read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            case, step, kind, digest = line.split()
            hashes[(case, int(step), kind)] = digest
    result = []
    consumed = set()
    for path in sorted((CONFORMANCE / "cases").glob("*.json")):
        steps = json.loads(path.read_text(encoding="utf-8"))["steps"]
        if "session" not in steps[0]:
            result.append([path.stem, "-1", "session", "0", "", "", ""])
        for index, step in enumerate(steps):
            kind = ""
            name = ""
            data = b""
            if "session" in step:
                command = "session"
                session = step["session"]
                data = session["options_text"].encode("utf-8") if "options_text" in session else encoded(session["options"]) if "options" in session else b""
            elif "push" in step:
                command = "push"
                name = input_bytes(step["push"], name=True).hex()
                data = input_bytes(step["push"])
            elif "take" in step:
                command = "take"
                name = str(step["take"])
                kind = "blob"
            else:
                command = "call"
                kind = "response"
                if "call" in step:
                    data = encoded(step["call"])
                elif "call_text" in step:
                    data = step["call_text"].encode("utf-8")
                else:
                    data = bytes.fromhex(step["call_hex"])
            key = (path.stem, index, kind)
            digest = hashes.get(key, "")
            if digest:
                consumed.add(key)
            result.append([path.stem, str(index), command, str(STATUS[step.get("status", "ok")]), name, data.hex(), digest])
    if consumed != set(hashes):
        raise ValueError("ハッシュの一覧と手順の対応が足りない")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    fixture = rows()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # VBA の Line Input # は CR または CRLF で区切る。OS と Python の版によらず同じバイト列にする。
    args.output.write_bytes(("\r\n".join("\t".join(row) for row in fixture) + "\r\n").encode("ascii"))
    print(f"{len({row[0] for row in fixture})} 組、{sum(bool(row[6]) for row in fixture)} ハッシュ")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    main()

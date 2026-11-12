"""BCF の未知・欠落の状態を、実際の CLI を通した製品の比較で検出する（#792 のレビュー）。"""

import argparse
import contextlib
import importlib.util
import io
import json
import shutil
import sys
import tempfile
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("acc_make", Path(__file__).with_name("make.py"))
kit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kit)


def changed_bcf(entries, topic_id, attribute):
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w") as archive:
        for name, source in entries.items():
            data = source.replace(b'TopicStatus="Resolved"', b'TopicStatus="Open"') if name.endswith("/markup.bcf") else source
            if attribute is not None and name == topic_id + "/markup.bcf":
                original = data
                data = data.replace(b'TopicStatus="Open"', attribute, 1)
                if data == original:
                    raise AssertionError("変更する TopicStatus がありません")
            archive.writestr(name, data)
    return output.getvalue()


def check(exe, folder):
    expected = json.loads((folder / "expected.json").read_bytes())
    with zipfile.ZipFile(folder / "for-acc.bcf") as archive:
        entries = {item.filename: archive.read(item) for item in archive.infolist()}
    with tempfile.TemporaryDirectory(prefix="zumen-acc-status-") as temporary:
        attributes = [None, b'TopicStatus="Unknown"', b'', b'TopicStatus=""', b'xmlns:x="urn:status" x:TopicStatus="Open"']
        for number, attribute in enumerate(attributes):
            case = Path(temporary) / str(number)
            case.mkdir()
            for name in [kit.MODEL, "for-acc.bcf", "expected.json"]:
                shutil.copyfile(folder / name, case / name)
            data = changed_bcf(entries, expected[0]["id"], attribute)
            exported = case / "exported.bcf"
            exported.write_bytes(data)
            with contextlib.redirect_stdout(io.StringIO()):
                kit.observe(exe, case, exported)
            review = json.loads((case / "observed/review.json").read_bytes())
            # どの入力でもコアの正規化された状態は open。元 XML を見なければ欠落を見逃す。
            topic = next(item for item in review["topics"] if item["id"] == expected[0]["id"])
            if topic["status"] != "open":
                raise AssertionError("状態の回帰テストの前提が違います")
            checks = json.loads((case / "observed/comparison.json").read_bytes())["checks"]
            failed = [item for item in checks if not item["equal"]]
            if attribute is None:
                if failed:
                    raise AssertionError("既知の状態の比較が一致しません")
            elif not any(item["field"] == "topic_status" and item.get("case") == expected[0]["key"] for item in failed):
                raise AssertionError("未知か欠落の状態を検出できません")
        print("CLI 経由の observe: 既知・未知・欠落・空・名前空間付き属性の 5 件を確認しました。")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path)
    parser.add_argument("folder", type=Path)
    args = parser.parse_args()
    check(args.exe, args.folder)

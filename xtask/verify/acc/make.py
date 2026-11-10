"""公開 IFC と BCF 2.1 のオフライン検証キットを作る（#686・#787）。外部には送信しない。"""

import argparse
import hashlib
import importlib.util
import io
import json
import shutil
import struct
import sys
import tomllib
import zipfile
import zlib
from collections import Counter
from pathlib import Path
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
SOURCE = "ifc/text2bim/sample_model.ifc"
MODEL = "model.ifc"
TARGET = "model"
ELEMENT = "13v5x2IG0HxwZGA6irHSbP"
PROJECT = "1HvDjhIG0HxwZGA6irHSbP"
AUTHOR = "zumen-verification@example.invalid"
DATE = "2026-11-10T00:00:00Z"
MAX_EXPANDED_BYTES = 64 * 1024 * 1024
MAX_ZIP_ENTRIES = 4096
MAX_SNAPSHOT_REFERENCES = 4096
ASCII_LOWER = str.maketrans("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz")
CAMERA = {"projection": "perspective", "position": [-5, 1.5, 1.5], "direction": [1, 0, 0], "up": [0, 0, 1], "field_of_view": 60}
CASES = [
    ("open-selected", "open", True, "Open", "選択要素あり・未解決"),
    ("resolved-selected", "resolved", True, "Open", "選択要素あり・解決済み"),
    ("closed-selected", "closed", True, "Closed", "選択要素あり・終了"),
    ("open-unselected", "open", False, "Open", "選択要素なし・未解決"),
]


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def output_folder(path):
    """既存の内容を消さず、Git の作業ツリーの外の空のフォルダーだけに書く。"""
    path = path.resolve()
    if any((parent / ".git").exists() for parent in [path, *path.parents]):
        raise ValueError("出力先は Git の作業ツリーの外にしてください")
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("出力先は新しいフォルダーか空のフォルダーにしてください")
    path.mkdir(parents=True, exist_ok=True)
    return path


def client(exe):
    # 既存の JSON-RPC の運搬を共用する。-I で起動しても、作業フォルダーの同名モジュールは探さない。
    spec = importlib.util.spec_from_file_location("verify_make", ROOT / "xtask/verify/make.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.Zumen(exe)


def call(z, request, records):
    status, result = z.call(request)
    records.append({"request": request, "status": status, "response": result})
    if status != "ok":
        codes = [item["code"] for item in (result or {}).get("diagnostics", [])]
        raise ValueError("API の失敗: " + request["op"] + " " + ",".join(codes))
    return result


def blob(z, result):
    return z.take(next(item["id"] for item in result["blobs"] if item["kind"] == "file"))


def snapshot():
    """標準ライブラリで有効な PNG を作る。モデルの画面ではなく、画像保持を調べる青枠と赤い十字。"""
    width, height = 320, 200
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            rgb = (255, 255, 255)
            if x < 8 or x >= width - 8 or y < 8 or y >= height - 8:
                rgb = (0, 23, 193)
            if (abs(x - width // 2) < 3 and abs(y - height // 2) < 35) or (abs(y - height // 2) < 3 and abs(x - width // 2) < 35):
                rgb = (220, 0, 0)
            rows.extend(rgb)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")


def guid(number, kind):
    return f"{number:08x}-{'1111' if kind == 'topic' else '2222' if kind == 'view' else '3333'}-4000-8000-000000000001"


def vector(name, values):
    return "<" + name + ">" + "".join(f"<{axis}>{value}</{axis}>" for axis, value in zip("XYZ", values)) + "</" + name + ">"


def seed_bcf(image):
    # 決定 0248。BCF 2.1 Documentation の markup.bcf と Visualization information の形で、検証用の入力を作る。
    # https://github.com/buildingSMART/BCF-XML/blob/release_2_1/Documentation/README.md
    entries = {"bcf.version": b'<?xml version="1.0" encoding="UTF-8"?><Version VersionId="2.1"><DetailedVersion>2.1</DetailedVersion></Version>'}
    for number, (_, status, selected, _, _) in enumerate(CASES, 1):
        topic, view, comment = (guid(number, kind) for kind in ["topic", "view", "comment"])
        markup = f'''<?xml version="1.0" encoding="UTF-8"?>
<Markup><Header><File IfcProject="{PROJECT}" isExternal="true"><Filename>{MODEL}</Filename></File></Header>
<Topic Guid="{topic}" TopicType="Issue" TopicStatus="{status.title()}"><Title>変更前 {number}</Title><CreationDate>{DATE}</CreationDate><CreationAuthor>{AUTHOR}</CreationAuthor><Description>変更前の説明</Description></Topic>
<Comment Guid="{comment}"><Date>{DATE}</Date><Author>{AUTHOR}</Author><Comment>変更前のコメント</Comment><Viewpoint Guid="{view}"/></Comment>
<Viewpoints Guid="{view}"><Viewpoint>viewpoint.bcfv</Viewpoint><Snapshot>snapshot.png</Snapshot></Viewpoints></Markup>'''
        selection = f'<Selection><Component IfcGuid="{ELEMENT}"/></Selection>' if selected else ""
        camera = "".join(vector(name, CAMERA[key]) for name, key in [("CameraViewPoint", "position"), ("CameraDirection", "direction"), ("CameraUpVector", "up")])
        viewpoint = f'''<?xml version="1.0" encoding="UTF-8"?>
<VisualizationInfo Guid="{view}"><Components>{selection}<Visibility DefaultVisibility="true"/></Components><PerspectiveCamera>{camera}<FieldOfView>60</FieldOfView></PerspectiveCamera></VisualizationInfo>'''
        entries.update({topic + "/markup.bcf": markup.encode(), topic + "/viewpoint.bcfv": viewpoint.encode(), topic + "/snapshot.png": image})
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, data in sorted(entries.items()):
            item = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            item.external_attr = 0o100644 << 16
            archive.writestr(item, data)
    return buffer.getvalue()


def expected_cases(element_hash):
    return [{"id": guid(number, "topic"), "key": key, "title": f"検証 {number:02d}: {label}", "description": f"公開 IFC の BCF 検証 {number:02d}",
             "status": status, "acc_status": acc_status, "comment": f"書き戻したコメント {number:02d}", "selected": [ELEMENT] if selected else [], "element_hash": element_hash, "camera": CAMERA}
            for number, (key, status, selected, acc_status, label) in enumerate(CASES, 1)]


def differences(review, expected, raw_topics, *, product=False):
    """期待値は変更のレシピ。製品の export では GUID の再採番を許し、題で指摘を結ぶ。"""
    topics = review.get("topics", [])
    result = []
    if len(topics) != len(expected):
        result.append({"field": "topics", "equal": False})
    for want in expected:
        matches = [topic for topic in topics if topic.get("title") == want["title"]] if product else [topic for topic in topics if topic.get("id") == want["id"]]
        if len(matches) != 1:
            result.append({"case": want["key"], "field": "topic", "equal": False})
            continue
        topic = matches[0]
        values = {"title": topic.get("title"), "description": topic.get("description"), "status": topic.get("status"),
                  "comment": any(comment.get("text") == want["comment"] for comment in topic.get("comments", []))}
        for field in ["title", "description", "status", "comment"]:
            expected_value = True if field == "comment" else want["acc_status"].lower() if product and field == "status" else want[field]
            result.append({"case": want["key"], "field": field, "equal": values[field] == expected_value})
        raw = [item for item in raw_topics if item["guid"] == topic["id"].strip().translate(ASCII_LOWER)]
        status = want["acc_status"] if product else want["status"].title()
        result.append({"case": want["key"], "field": "topic_status", "equal": len(raw) == 1 and raw[0]["status_present"] and raw[0]["topic_status"] == status})
        # 未知の状態は open と status_label に写る。正規化された open だけでは一致としない。
        result.append({"case": want["key"], "field": "status_label", "equal": topic.get("status_label") is None})
        locations = topic.get("locations", [])
        result.append({"case": want["key"], "field": "location", "equal": len(locations) == 1})
        if len(locations) == 1:
            location = locations[0]
            selected = location.get("selected", [])
            result.append({"case": want["key"], "field": "selected", "equal": [item["id"] for item in selected] == want["selected"]})
            result.append({"case": want["key"], "field": "camera", "equal": location.get("camera") == want["camera"]})
            if not product:
                result.append({"case": want["key"], "field": "element_hash", "equal": all(item.get("hash") == want["element_hash"] for item in selected)})
    return result


def xml_topics(data):
    """元 XML の状態と画像を読む。各 ZIP 項目は一度だけ展開し、画像も一度だけハッシュする。"""
    result, references = [], 0
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        entries = archive.infolist()
        if len(entries) > MAX_ZIP_ENTRIES or sum(item.file_size for item in entries) > MAX_EXPANDED_BYTES:
            raise ValueError("BCF の比較用の展開量の上限を超えました")
        by_name = {item.filename: item for item in entries}
        if len(by_name) != len(entries):
            raise ValueError("BCF の ZIP に同じ名前の項目があります")
        expanded, hashes = {}, {}

        def read_entry(name):
            if name not in expanded:
                expanded[name] = archive.read(by_name[name])
            return expanded[name]

        for item in entries:
            if not item.filename.endswith("/markup.bcf"):
                continue
            markup = ET.fromstring(read_entry(item.filename))
            # XML の名前空間の有無は製品による。比較ではローカル名だけを使う。
            for element in markup.iter():
                element.tag = element.tag.rsplit("}", 1)[-1]
            topics = markup.findall("Topic")
            if len(topics) != 1:
                raise ValueError("BCF の Markup の Topic が 1 件ではありません")
            topic = topics[0]
            title = topic.findtext("Title")
            folder = item.filename.rsplit("/", 1)[0]
            images = []
            for node in markup.iter("Snapshot"):
                references += 1
                if references > MAX_SNAPSHOT_REFERENCES:
                    raise ValueError("BCF の画像の参照数の上限を超えました")
                if not node.text:
                    continue
                name = folder + "/" + node.text
                # 参照だけが残り、画像が欠けた出力も、比較できる欠落として残す。
                if name in by_name and name not in hashes:
                    hashes[name] = sha(read_entry(name))
                images.append(hashes.get(name))
            # 属性の名前空間は取り除かない。無名前空間の TopicStatus の存在と値をそのまま残す。
            result.append({"guid": topic.get("Guid", "").strip().translate(ASCII_LOWER), "title": title,
                           "status_present": "TopicStatus" in topic.attrib, "topic_status": topic.get("TopicStatus"), "snapshots": images})
    return result


def images_by_title(raw_topics):
    result = {}
    for topic in raw_topics:
        if topic["title"] in result:
            raise ValueError("BCF の比較対象に同じ題の指摘が複数あります")
        result[topic["title"]] = topic["snapshots"]
    return result


def xml_snapshots(data):
    return images_by_title(xml_topics(data))


def open_model(z, data, records):
    z.push(MODEL, data)
    call(z, {"op": "open", "input": MODEL}, records)


def read_review(z, name, data, records):
    z.push(name, data)
    response = call(z, {"op": "bcf_read", "doc": MODEL, "input": name, "target": TARGET}, records)
    if response["result"] != {"version": "2.1", "topics": 4}:
        raise ValueError("BCF 2.1 の指摘 4 件として読めませんでした")
    return json.loads(blob(z, response))


def make(exe, output):
    out = output_folder(output)
    source = next(item for item in tomllib.loads((ROOT / "samples/ledger.toml").read_text(encoding="utf-8"))["samples"] if item["path"] == SOURCE)
    model = (ROOT / "samples" / SOURCE).read_bytes()
    if sha(model) != source["sha256"]:
        raise ValueError("公開 IFC のハッシュが台帳と違います")
    image, records = snapshot(), []
    seed = seed_bcf(image)
    z = client(exe)
    try:
        capabilities = call(z, {"op": "capabilities"}, records)
        open_model(z, model, records)
        elements = call(z, {"op": "building_elements", "doc": MODEL, "ids": [ELEMENT], "components": ["class", "identity", "placement"]}, records)
        items = elements["result"]["elements"]
        if len(items) != 1 or items[0]["id"] != ELEMENT or items[0].get("class", {}).get("original") != "IfcWallStandardCase":
            raise ValueError("公開 IFC に選択する壁がありません")
        expected = expected_cases(items[0]["hash"])
        review = read_review(z, "seed.bcf", seed, records)
        for topic, want in zip(review["topics"], expected):
            if topic["id"] != want["id"]:
                raise ValueError("元の BCF の指摘の順が期待と違います")
            topic.update({key: want[key] for key in ["title", "description", "status"]})
            topic["comments"][0]["text"] = want["comment"]
        z.push("edited.json", json_bytes(review))
        written = call(z, {"op": "bcf_write", "review": "edited.json", "target": TARGET, "original": "seed.bcf", "version": "2.1"}, records)
        bcf = blob(z, written)
        reread = read_review(z, "for-acc.bcf", bcf, records)
        raw_topics = xml_topics(bcf)
        checks = differences(reread, expected, raw_topics)
        expected_images = {item["title"]: [sha(image)] for item in expected}
        if any(not check["equal"] for check in checks) or images_by_title(raw_topics) != expected_images:
            raise ValueError("書き戻した値か画像が期待と違います")
        if any(record["response"].get("diagnostics") for record in records if record["request"]["op"] != "open"):
            raise ValueError("BCF の往復か要素の確認で診断が出ました")
        artifacts = {MODEL: model, "seed.bcf": seed, "for-acc.bcf": bcf, "snapshot.png": image,
                     "edited-review.json": json_bytes(review), "reread-review.json": json_bytes(reread), "api.json": json_bytes(records)}
        for name, data in artifacts.items():
            (out / name).write_bytes(data)
        (out / "expected.json").write_bytes(json_bytes(expected))
        input_diagnostics = Counter()
        for record in records:
            if record["request"]["op"] == "open":
                for diagnostic in record["response"].get("diagnostics", []):
                    input_diagnostics[diagnostic["code"]] += diagnostic["count"]
        manifest = {"issue": 686, "kit_issue": 787, "verification": "synthetic", "source": source, "element": items[0], "input_diagnostics": dict(input_diagnostics),
                    "capabilities": capabilities["result"], "artifacts": {name: sha(data) for name, data in artifacts.items()}, "checks": checks,
                    "snapshot": "モデルの画面ではなく、画像保持を調べる青枠と赤い中央十字の合成 PNG"}
        (out / "manifest.json").write_bytes(json_bytes(manifest))
        shutil.copyfile(Path(__file__).with_name("README.md"), out / "README.md")
        shutil.copyfile(Path(__file__).with_name("report.md"), out / "report.md")
        (out / "licenses").mkdir()
        for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
            shutil.copyfile(ROOT / name, out / "licenses" / name)
        shutil.copyfile(ROOT / "licenses/text2bim-MIT.txt", out / "licenses/text2bim-MIT.txt")
        print("BCF 2.1 の指摘 4 件・値と画像の往復・BCF の診断 0 件を確認しました。IFC 入力の診断は manifest に記録し、製品での確認は未実施です。")
    finally:
        z.close()


def observe(exe, kit, exported):
    kit = kit.resolve()
    # 製品の出力にはアカウント等の情報が入り得る。Git 内には結果を書かない。
    out = output_folder(kit / "observed")
    expected = json.loads((kit / "expected.json").read_bytes())
    records, z = [], client(exe)
    try:
        open_model(z, (kit / MODEL).read_bytes(), records)
        with exported.open("rb") as stream:
            data = stream.read(MAX_EXPANDED_BYTES + 1)
        if len(data) > MAX_EXPANDED_BYTES:
            raise ValueError("BCF の比較用の入力の上限を超えました")
        raw_topics = xml_topics(data)
        images = images_by_title(raw_topics)
        z.push("exported.bcf", data)
        response = call(z, {"op": "bcf_read", "doc": MODEL, "input": "exported.bcf", "target": TARGET}, records)
        review = json.loads(blob(z, response))
        checks = differences(review, expected, raw_topics, product=True)
        original_images = xml_snapshots((kit / "for-acc.bcf").read_bytes())
        checks += [{"case": item["key"], "field": "snapshot", "equal": images.get(item["title"]) == original_images[item["title"]]} for item in expected]
        (out / "review.json").write_bytes(json_bytes(review))
        (out / "api.json").write_bytes(json_bytes(records))
        (out / "comparison.json").write_bytes(json_bytes({"version": response["result"]["version"], "topics": response["result"]["topics"], "xml_topics": raw_topics,
                                                        "checks": checks, "failed": sum(not check["equal"] for check in checks)}))
        print("製品の出力の比較: " + str(len(checks)) + " 項目、違い " + str(sum(not check["equal"] for check in checks)) + " 件。詳細はリポジトリの外の observed に記録しました。")
    finally:
        if records:
            (out / "api.json").write_bytes(json_bytes(records))
        z.close()


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    make_args = commands.add_parser("make")
    make_args.add_argument("exe", type=Path)
    make_args.add_argument("output", type=Path)
    observe_args = commands.add_parser("observe")
    observe_args.add_argument("exe", type=Path)
    observe_args.add_argument("kit", type=Path)
    observe_args.add_argument("exported", type=Path)
    args = parser.parse_args()
    if args.command == "make":
        make(args.exe, args.output)
    else:
        observe(args.exe, args.kit, args.exported)


if __name__ == "__main__":
    main()

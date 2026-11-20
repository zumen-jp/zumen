"""工事・業務の公式作成例を配布プロファイルと公開 API の delivery_check に渡す（#305）。"""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

import lxml
from lxml import etree


def require(condition, message):
    if not condition:
        raise ValueError(message)


def example(bundle, name):
    matches = [path for path in bundle.rglob("*") if path.name.upper() == name]
    require(len(matches) == 1, f"{name} の作成例が 1 件必要です")
    path = matches[0]
    parser = etree.XMLParser(dtd_validation=True, no_network=True, resolve_entities=False)
    return path, etree.parse(str(path), parser)


def check(executable, inputs, profile):
    args = [executable, "call"]
    for name, path in inputs.items():
        args.extend(["--input", f"{name}={path}"])
    request = {"v": 1, "op": "delivery_check", "root": "out/", "profiles": ["profile.json"],
               "profile": {"id": profile["id"], "edition": profile["edition"]}}
    response = subprocess.run([*args, "-"], input=json.dumps(request).encode(),
                              capture_output=True, check=False)
    require(response.returncode == 0, "delivery_check を実行できませんでした")
    value = json.loads(response.stdout)
    require(not value["diagnostics"], "公開操作に予期しない診断があります")
    result = value["result"]
    identity = {key: profile[key] for key in ("id", "edition", "revision")}
    require(result["chain"] == [identity], "適用した要領の版・revision が一致しません")
    return result


def verify(executable, bundle, profile_path, index, manager, element, folder, count, workspace):
    profile = json.loads(profile_path.read_bytes())
    source = profile["sources"]["main"]
    require(profile["edition"] == "2026-02" and source["version"] == "令和 8 年 2 月"
            and source["title"] == profile["title"],
            "要領の版が一致しません")
    require(any(r["id"] == "link" and r["source"] == {"document": "main", "section": "4"}
                for r in profile["rules"]), "参照規則が要領の第 4 章を指していません")
    index_path, index_tree = example(bundle, index)
    path, tree = example(bundle, manager + ".XML")
    managed = f"{manager}/{manager}.XML"
    original = {"profile.json": profile_path, "out/" + index: index_path, "out/" + managed: path}
    leaves = [node.text.strip() for node in tree.iter(element)]
    aliases = [node.text.strip() for node in tree.iter(element.replace("ファイル名", "ファイル日本語名"))]
    require(len(leaves) == len(set(leaves)) == count and set(leaves).isdisjoint(aliases),
            "実ファイルの名前を説明用の名前と区別できません")
    require(len(aliases) == 1, "作成例の説明用の名前の数が一致しません")
    require(all(name and Path(name).name == name and "/" not in name and "\\" not in name
                for name in [*leaves, *aliases]), "作成例に想定外の参照が含まれます")
    inputs = {**original, **{f"out/{folder}/{name}": workspace / "attachment" for name in leaves}}
    correct = check(executable, inputs, profile)
    require(correct["total"] == 0, "参照先をそろえた組に指摘があります")
    summaries = {item["name"]: item for item in correct["managed"]}
    require(summaries[index]["doctype"] == index_tree.docinfo.system_url
            and summaries[managed]["doctype"] == tree.docinfo.system_url, "作成例の DTD が一致しません")
    # プロファイルによらない要約はファイル名の形を数えるので、説明用の名前も含む。
    alias_path = f"{manager}/{aliases[0]}"
    require(summaries[managed]["missing"] == [alias_path]
            and summaries[managed]["links"] == len(leaves) + len(aliases), "管理ファイルの要約が一致しません")
    for name in leaves:
        omitted = {k: v for k, v in inputs.items() if k != f"out/{folder}/{name}"}
        result = check(executable, omitted, profile)
        require(result["total"] == 1, "参照先を除いた組の指摘数が一致しません")
        finding = result["findings"][0]
        diag = finding["diagnostic"]
        require(finding["rule"] == "link" and diag["code"] == "profile.delivery-link.invalid"
                and diag["severity"] == "error", "欠落した参照先の規則が一致しません")
        require(diag["params"] == {"managed": managed, "name": f"{manager}/{name}", "rule": "link"}
                and diag["at"][0]["id"] == managed, "指摘が管理ファイルと欠落した名前を指しません")
        # 指摘の元のバイト範囲が、該当するファイル名要素そのものを指すことも別に読む。
        span = diag["at"][1]["bytes"]
        record = path.read_bytes()[span["start"]:span["end"]]
        item = etree.fromstring(record.decode(tree.docinfo.encoding))
        require(item.tag == element and item.text.strip() == name, "元の XML の指摘位置が一致しません")
    # 作成例を改変せず、要領名の項目がない自作の管理 XML を同じ名前で渡す。
    synthetic = workspace / "synthetic.xml"
    synthetic.write_text(f'<!DOCTYPE example SYSTEM "{index_tree.docinfo.system_url}"><example/>',
                         encoding="ascii")
    result = check(executable, {"profile.json": profile_path, "out/" + index: synthetic}, profile)
    require(result["total"] == 1, "項目のない管理 XML の指摘数が一致しません")
    finding = result["findings"][0]
    diag = finding["diagnostic"]
    require(finding["rule"] == "standard" and diag["code"] == "profile.delivery-standard.unsupported"
            and diag["severity"] == "info" and diag["params"]["managed"] == index
            and diag["params"]["element"] == "適用要領基準" and diag["at"][0]["id"] == index,
            "確認できない管理項目の診断が一致しません")
    rule = next(r for r in result["rules"] if r["id"] == "standard")
    require(rule["unsupported"] == 1 and rule["findings"] == 0, "確認できない管理項目の集計が一致しません")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("zumen", type=Path)
    parser.add_argument("dtd", type=Path)
    args = parser.parse_args()
    require(lxml.__version__ == "6.1.3", "lxml 6.1.3 が必要です")
    profiles = Path(__file__).resolve().parents[3] / "profiles"
    with tempfile.TemporaryDirectory() as folder:
        workspace = Path(folder)
        (workspace / "attachment").write_bytes(b"reference check")
        for kind, index, manager, element, target, count in [
            ("const", "INDEX_C.XML", "PLAN", "施工計画書オリジナルファイル名", "PLAN/ORG", 3),
            ("design", "INDEX_D.XML", "OPENREP", "公開用成果品ファイル名", "OPENREP", 1),
        ]:
            profile = profiles / f"mlit-delivery-{kind}" / "2026-02" / "profile.json"
            verify(args.zumen.resolve(), args.dtd / f"{kind}_R8.2", profile, index,
                   manager, element, target, count, workspace)
            print(f"{kind}: 作成例 2 件、参照先 {count} 件、正しい組と各欠落、確認できない管理項目を確認")


if __name__ == "__main__":
    main()

"""新しい IFC のひな形と、変更だけで作った 2 階建てと、それを直した IFC と、変換した IFC を buildingSMART の
ifc-gherkin-rules にかける（#898・#395）。

使い方: python gherkin.py <ifc-gherkin-rules のフォルダー> <フォルダー>

フォルダーには、`author.rs` の `writes_the_authored_files_for_other_readers` が書いた
`<スキーマ>.built.ifc` と `<スキーマ>.changed.ifc` と、`create.rs` の `writes_the_templates_for_other_readers` が
書いた `<スキーマ>.template.ifc` と、`convert.rs` の `writes_the_conversions_for_other_readers` が書いた
`<スキーマ>.converted.ifc` と、`author.rs` の `writes_face_sets`・`writes_booleans_and_mappings`・
`writes_revolutions_and_sweeps` が書いた `<スキーマ>.faces.ifc`・`<スキーマ>.csg.ifc`・`<スキーマ>.sweep.ifc` がある。ifc-gherkin-rules（MIT）は、検証サービスと同じ
規則（実装者の合意と非公式の命題）を behave で流す。データベースを使わない試験の動かし方で、規則ごとの
結果を JSON に書かせて読む。

- 誤り（ERROR）の結果がない。
- 警告（WARNING）は、試験の入力のプロジェクトに座標参照系がないことの GRF003 だけ。
- 規則の手順が例外で止まらない。規則の側に IFC2X3 の資料がない GEM004 と QTY001 の IFC2X3 は除く。

試験の動かし方では、規則の試験ファイルの名前の決まりを確かめる `protocol_errors` が全ての規則に付くので
数えない。
"""

import base64
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCHEMAS = ["IFC2X3", "IFC4", "IFC4X3_ADD2"]
STATES = ["template", "structural", "built", "changed", "converted", "faces", "csg", "sweep"]

# 試験の入力は地理参照を持たない。作る変更と直す変更の範囲の外なので、警告とその手順の止まりを許す
ALLOWED_WARNINGS = {"GRF003"}

# 規則の側にない IFC2X3 の資料。GEM004 は表現の識別子の表を、QTY001 は数量のセットの表
# （qto_definitions.csv）を features/resources/IFC2X3 に持たない
TOOL_GAPS = {("GEM004", "FileNotFoundError", "IFC2X3"), ("QTY001", "TypeError", "IFC2X3")}


def decoded(element, key):
    embedded = element.get(key)
    if not embedded:
        return []
    return json.loads(base64.b64decode(embedded[0].get("data", "")).decode("utf-8"))


def run(rules, path):
    fd, out = tempfile.mkstemp(suffix=".json")
    os.close(fd)
    try:
        subprocess.run(
            [
                sys.executable,
                "-m",
                "behave",
                "--tags=-disabled",
                "--define",
                f"input={path.resolve()}",
                "--define",
                "execution_mode=ExecutionMode.TESTING",
                "--define",
                "max_outcomes_per_rule=0",
                "--define",
                "purepythonparser=False",
                "-f",
                "outcome_embedding_json",
                "-o",
                out,
            ],
            cwd=rules,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        with open(out, encoding="utf-8") as f:
            return json.load(f)
    finally:
        os.unlink(out)


def check(rules, path, schema):
    failures = []
    executed = 0
    for feature in run(rules, path):
        code = feature["name"].split(" ")[0]
        for element in feature.get("elements", []):
            for exception in decoded(element, "caught_exceptions"):
                if code in ALLOWED_WARNINGS or (code, exception.get("error_type"), schema) in TOOL_GAPS:
                    continue
                failures.append(f"{code}: 手順が例外で止まった: {exception.get('error_type')} {exception.get('step')}")
            for outcome in decoded(element, "validation_outcomes"):
                severity = str(outcome.get("severity"))
                if severity == "EXECUTED":
                    executed += 1
                elif severity == "ERROR" or (severity == "WARNING" and code not in ALLOWED_WARNINGS):
                    failures.append(
                        f"{code}: {severity} #{outcome.get('instance_id')} "
                        f"期待 {outcome.get('expected')} 観測 {outcome.get('observed')}"
                    )
    if executed == 0:
        failures.append("規則が 1 つも当たらなかった")
    return failures, executed


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    rules = Path(sys.argv[1]).resolve()
    folder = Path(sys.argv[2])
    failures = []
    for schema in SCHEMAS:
        for state in STATES:
            name = f"{schema}.{state}.ifc"
            found, executed = check(rules, folder / name, schema)
            print(f"{name}: 当たった規則の結果 {executed} 件")
            failures += [f"{name}: {f}" for f in found]
    for failure in failures:
        print(failure)
    if failures:
        sys.exit(1)
    print("ifc-gherkin-rules: ひな形と、作った IFC・直した IFC・変換した IFC・面集合・集合演算と写像・回転と掃引を 3 つのスキーマで照らした")


if __name__ == "__main__":
    main()

"""公開 IFC の validate を照合し、逆属性・ヘッダー・規則の対象範囲の差を分ける。"""

import argparse
import base64
from collections import Counter
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import tomllib
import urllib.request

spec = importlib.util.spec_from_file_location("express_declarations", Path(__file__).with_name("check.py"))
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
ROOT = common.ROOT
RULE_TYPES = {"entity_rule", "simpletype_rule", "global_rule"}


def normalize_external(statements, schema):
    """既知の検査だけを分類する。未知の指摘を対象外として隠さない。"""
    import ifcopenshell
    result = {key: [] for key in ("explicit", "inverse", "header", "rules", "unknown")}
    for row in statements:
        instance, kind = row.get("instance"), row.get("type")
        attribute = row.get("attribute")
        message = row.get("message", "")
        entity = instance.is_a() if isinstance(instance, ifcopenshell.entity_instance) else None
        identifier = instance.id() if entity else None
        field = attribute.rsplit(".", 1)[-1].upper() if isinstance(attribute, str) else None
        observed = {"id": identifier, "entity": entity.upper() if entity else None,
                    "attribute": field, "type": kind, "level": row.get("level")}
        if row.get("level") != "error":
            result["unknown"].append(observed)
        elif kind in RULE_TYPES:
            # 規則の式・元の値・メッセージ全文は保存しない。
            result["rules"].append({"type": kind, "attribute": attribute})
        elif kind == "schema" and entity and field:
            declaration = schema.declaration_by_name(entity).as_entity()
            inverse = {a.name().upper() for a in declaration.all_inverse_attributes()}
            attrs = {a.name().upper(): (index, a) for index, a in enumerate(declaration.all_attributes())}
            if field in inverse:
                result["inverse"].append(observed)
            elif (field in attrs and message == "Attribute not optional"
                  and not attrs[field][1].optional() and instance[attrs[field][0]] is None):
                result["explicit"].append({"id": identifier, "entity": entity.upper(),
                                           "attribute": field, "reason": "express.value.missing"})
            else:
                result["unknown"].append(observed)
        elif (kind == "schema" and instance is None and "FILE_NAME(" in message
              and "Attribute 'name' has invalid type:" in message
              and "Current value type: NoneType" in message):
            result["header"].append({"entity": "FILE_NAME", "attribute": "name", "reason": "missing"})
        else:
            result["unknown"].append(observed)
    return result


def external_worker(source, output, expected_schema):
    import ifcopenshell
    import ifcopenshell.validate
    if ifcopenshell.version != "0.9.0.post1":
        raise ValueError("外部実装の版が違います")
    data = source.read_bytes()
    alias = b"FILE_SCHEMA(('IFC4X3'))" in data
    if alias:
        # 既存の公開試料照合と同じ別名処理。DATA は変えず、変更した写しのハッシュを記録する。
        data = data.replace(b"FILE_SCHEMA(('IFC4X3'))", b"FILE_SCHEMA(('IFC4X3_ADD2'))", 1)
        source = output.with_suffix(".ifc")
        source.write_bytes(data)
    model = ifcopenshell.open(str(source))
    if model.schema_identifier != expected_schema:
        raise ValueError("外部実装が選んだスキーマが違います")
    schema = ifcopenshell.schema_by_name(expected_schema)
    result = {"schema": model.schema_identifier, "headerAlias": alias,
              "inputSha256": common.sha(data), "modes": []}
    for rules in (False, True):
        logger = ifcopenshell.validate.json_logger()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            ifcopenshell.validate.validate(str(source), logger, express_rules=rules)
        result["modes"].append({"expressRules": rules, "count": len(logger.statements),
                                **normalize_external(logger.statements, schema)})
    common.write_json(output, result)


def core_validate(cli, data, schema):
    requests = [("session_new", {}),
                ("push", {"name": "input.ifc", "base64": base64.b64encode(data).decode()}),
                ("push", {"name": "schema.exp", "base64": base64.b64encode(schema).decode()}),
                ("call", {"request": {"v": 1, "op": "validate", "input": "input.ifc", "express": "schema.exp", "limit": 10000}})]
    payload = "".join(json.dumps({"jsonrpc": "2.0", "id": index + 1, "method": method, "params": params}) + "\n"
                      for index, (method, params) in enumerate(requests)).encode()
    # EOF でサーバーを終了させ、応答待ちを含む全プロセスに制限時間を設ける。
    run = subprocess.run([str(cli), "serve", "--stdio"], input=payload, capture_output=True, timeout=120)
    if run.returncode != 0:
        raise ValueError("CLI の終了コードが0ではありません")
    replies = [json.loads(line) for line in run.stdout.splitlines()]
    if len(replies) != len(requests) or [row.get("id") for row in replies] != [1, 2, 3, 4]:
        raise ValueError("CLI の応答がそろいません")
    if any("error" in row for row in replies):
        raise ValueError("JSON-RPC が失敗しました")
    if any(row["result"]["status"] != "ok" for row in replies[1:3]):
        raise ValueError("入力を渡せませんでした")
    result = replies[-1]["result"]
    if result["status"] != "ok":
        raise ValueError("validate が成功しませんでした")
    return json.loads(result["response"])


def counted(values):
    return Counter(json.dumps(value, sort_keys=True, ensure_ascii=False) for value in values)


def compare(core, external):
    failures, explicit = [], []
    result = core["result"]
    if result["total"] != len(result["findings"]) or result.get("next_offset") is not None:
        failures.append("core-incomplete")
    for row in result["findings"]:
        places, params = row.get("at", []), row.get("params", {})
        if len(places) != 1 or row.get("count") != 1 or not str(places[0].get("id", "")).startswith("#"):
            failures.append("core-location-or-count")
            continue
        explicit.append({"id": int(places[0]["id"][1:]), "entity": params.get("kind"),
                         "attribute": params.get("attribute"), "reason": row["code"]})
    if external["schema"] != result["schema"]:
        failures.append("schema")
    alias = external["headerAlias"]
    if result["file_schema"] != ("IFC4X3" if alias else external["schema"]):
        failures.append("file-schema")
    expected_diagnostics = Counter({"express.rule.unsupported": 1})
    if alias:
        expected_diagnostics["express.schema.mixed"] = 1
    if Counter(row["code"] for row in core["diagnostics"]) != expected_diagnostics:
        failures.append("core-diagnostics")
    base, full = external["modes"]
    if base["expressRules"] is not False or full["expressRules"] is not True:
        failures.append("external-modes")
    for mode in (base, full):
        if mode["unknown"]:
            failures.append("external-unclassified")
        if mode["count"] != sum(len(mode[key]) for key in ("explicit", "inverse", "header", "rules", "unknown")):
            failures.append("external-count")
        if counted(explicit) != counted(mode["explicit"]):
            failures.append("explicit-findings")
    for key in ("explicit", "inverse", "header", "unknown"):
        if counted(base[key]) != counted(full[key]):
            failures.append("rule-mode-changed-" + key)
    if base["rules"]:
        failures.append("unexpected-rules")
    return sorted(set(failures))


def sample_schema(data):
    import re
    found = re.search(rb"FILE_SCHEMA\s*\(\s*\(\s*'([^']+)'\s*\)\s*\)", data[:16384])
    if found is None:
        raise ValueError("公開試料のスキーマ名を読めません")
    name = found[1].decode("ascii").upper()
    return "IFC4X3_ADD2" if name == "IFC4X3" else name


def input_differences(data, external):
    alias = b"FILE_SCHEMA(('IFC4X3'))" in data
    expected = data.replace(b"FILE_SCHEMA(('IFC4X3'))", b"FILE_SCHEMA(('IFC4X3_ADD2'))", 1) if alias else data
    return ([] if external["headerAlias"] == alias else ["header-alias"]) + (
        [] if external["inputSha256"] == common.sha(expected) else ["external-input"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--worker", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--schema", help=argparse.SUPPRESS)
    args = parser.parse_args()
    output = args.out.resolve()
    if output.exists() or common.in_repository(output):
        parser.error("リポジトリ外の新規の --out が必要です")
    if args.worker:
        external_worker(args.worker.resolve(strict=True), output, args.schema)
        return 0
    if args.cli is None:
        parser.error("--cli が必要です")
    cli = args.cli.resolve(strict=True)
    cli_hash = common.sha(cli.read_bytes())
    sources = ["xtask/verify/express/check.py", "xtask/verify/express/validate.py", "xtask/verify/express/test_validate.py",
               "samples/ledger.toml", "crates/zumen-ifc/src/express.rs", "crates/zumen-abi/src/validate.rs"]
    fingerprints = [{"path": path, "sha256": common.sha((ROOT / path).read_bytes())} for path in sources]
    samples = sorted((row for row in tomllib.loads((ROOT / "samples/ledger.toml").read_text(encoding="utf-8"))["samples"]
                      if row["path"].startswith("ifc/") and row["path"].endswith(".ifc")), key=lambda row: row["path"])
    output.mkdir(parents=True)
    report = {"schemaVersion": 1, "issue": 663, "complete": False,
              "sourceCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
              "sources": fingerprints, "cliSha256": cli_hash, "platform": platform.system(), "machine": platform.machine(),
              "python": sys.version.split()[0], "ifcopenshell": "0.9.0.post1", "cases": [],
              "limits": ["明示属性の欠落の指摘をインスタンス・型・属性・理由・重複数で比較。その他の指摘は未分類として失敗させる。",
                         "外部実装は同じ版名の組み込みスキーマを使用。宣言の同一入力比較とは別の経路。",
                         "逆属性・FILE_NAME・外部の規則評価は対象範囲の差として別計上。規則の全実装や全規則の網羅を示さない。",
                         "IFC4X3 の外部入力のみヘッダーの版名を IFC4X3_ADD2 に置き換える。DATA は不変。",
                         "ネイティブ CLI の公開 validate を実行。別 OS/wasm の一致や表示・形の有効性は未確認。"]}
    with tempfile.TemporaryDirectory(prefix="zumen-express-validate-") as folder:
        temporary = Path(folder).resolve()
        assert temporary.parent == Path(tempfile.gettempdir()).resolve() and temporary.name.startswith("zumen-express-validate-")
        schemas = {}
        for name, path, size, digest in common.SOURCES:
            with urllib.request.urlopen(common.BASE + path, timeout=30) as response:
                data = response.read(size + 1)
            if len(data) != size or common.sha(data) != digest:
                raise ValueError("スキーマのハッシュが違います")
            schemas[name] = data
        for index, sample in enumerate(samples):
            path = (ROOT / "samples" / sample["path"]).resolve(strict=True)
            if not path.is_relative_to((ROOT / "samples").resolve()) or path.stat().st_size > 64 * 1024 * 1024:
                raise ValueError("台帳の試料の場所またはサイズが範囲外です")
            data = path.read_bytes()
            if common.sha(data) != sample["sha256"]:
                raise ValueError("公開試料のハッシュが違います")
            schema = sample_schema(data)
            core = core_validate(cli, data, schemas[schema])
            external_path = temporary / f"{index}.json"
            child = subprocess.run([sys.executable, "-I", str(Path(__file__).resolve()), "--worker", str(path),
                                    "--out", str(external_path), "--schema", schema],
                                   capture_output=True, timeout=180, cwd=temporary)
            if child.returncode != 0 or not external_path.exists():
                raise ValueError("外部実装の検査が完了しませんでした")
            external = json.loads(external_path.read_bytes())
            failures = compare(core, external) + input_differences(data, external)
            if common.sha(path.read_bytes()) != sample["sha256"]:
                raise ValueError("検査中に公開試料が変わりました")
            base, full = external["modes"]
            record = {"sample": sample["path"], "inputSha256": sample["sha256"], "license": sample["license"],
                      "schema": schema, "schemaSha256": common.sha(schemas[schema]), "headerAlias": external["headerAlias"],
                      "externalInputSha256": external["inputSha256"], "coreFindings": core["result"]["total"],
                      "coreRulesNotEvaluated": core["result"]["rules_not_evaluated"],
                      "externalWithoutRules": base["count"], "externalWithRules": full["count"],
                      "externalExplicit": len(base["explicit"]), "externalInverse": len(base["inverse"]),
                      "externalHeader": len(base["header"]), "externalRuleFindings": len(full["rules"]),
                      "ruleKinds": dict(Counter(row["type"] for row in full["rules"])),
                      "failures": failures, "verified": not failures}
            report["cases"].append(record)
            common.write_json(output / "results.json", report)
            print(json.dumps({key: record[key] for key in ("sample", "verified", "coreFindings", "externalInverse", "externalHeader", "externalRuleFindings", "failures")}), flush=True)
    if common.sha(cli.read_bytes()) != cli_hash or any(common.sha((ROOT / row["path"]).read_bytes()) != row["sha256"] for row in fingerprints):
        raise ValueError("実行中にソースまたは CLI が変わりました")
    report["complete"] = True
    common.write_json(output / "results.json", report)
    return 0 if samples and all(row["verified"] for row in report["cases"]) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(json.dumps({"status": "runner_error", "kind": type(error).__name__}))
        sys.exit(2)

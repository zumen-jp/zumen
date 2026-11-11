// Rust の独立した固定表の復号結果と署名の判定を、JS のホストで比較する（#824）。
import { Modules } from "../src/modules.js";

export function compareShiftJisSignatures(snapshot) {
  const rule = { kind: "xml_root", name: "", namespace: null };
  const modules = new Modules({}, undefined, [{
    name: "sjis-probe", ops: [], formats: [{ format: "probe", rules: [rule] }],
  }]);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const fromHex = (text) => new Uint8Array(text.match(/../g).map((value) => Number.parseInt(value, 16)));
  const head = encoder.encode("<?xml version='1.0' encoding='Shift_JIS'?><n");
  let count = 0;
  for (const line of snapshot.trimEnd().split("\n")) {
    const [raw, expected, matches] = line.split(" ");
    const name = fromHex(raw);
    const input = new Uint8Array(head.length + name.length + 2);
    input.set(head);
    input.set(name, head.length);
    input.set([0x2f, 0x3e], head.length + name.length);
    rule.name = decoder.decode(fromHex(expected));
    const actual = modules.detect(input)?.format === "probe";
    if (actual !== (matches === "1")) {
      throw new Error(`Shift_JIS の署名が Rust と違う: ${raw}`);
    }
    count++;
  }
  if (count !== 15_616) { throw new Error(`Shift_JIS の検査数が違う: ${count}`); }
  return count;
}

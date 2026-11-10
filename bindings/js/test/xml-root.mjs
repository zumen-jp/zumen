// XML の署名を、SDK と同じ合成入力で確かめる。単独でも modules.mjs からも実行する。
import assert from "node:assert/strict";

import { Modules, detect } from "../src/modules.js";

const namespace = "http://www.landxml.org/schema/LandXML-1.2";
const encoder = new TextEncoder();

function utf16(text, little, bom) {
  const bytes = new Uint8Array(text.length * 2 + (bom ? 2 : 0));
  const view = new DataView(bytes.buffer);
  if (bom) {
    view.setUint16(0, 0xfeff, little);
  }
  for (let i = 0; i < text.length; i++) {
    view.setUint16((bom ? 2 : 0) + i * 2, text.charCodeAt(i), little);
  }
  return bytes;
}

const matches = (bytes) => detect(bytes)?.format === "landxml";
for (const little of [false, true]) {
  for (const bom of [false, true]) {
    const label = bom ? "UTF-16" : little ? "UTF-16LE" : "UTF-16BE";
    const text = `<?xml version='1.0' encoding='${label}'?>\n<!-- 表示😀 -->\n<!DOCTYPE x [<!ENTITY e "]>">]>\n<?pi data?>\n<ĺ😀:LandXML a='ļľĢ' xmlns:ĺ😀='${namespace}' xmlns:x='other'>`;
    assert.ok(matches(utf16(text, little, bom)));
    assert.ok(matches(utf16(`<LandXML xmlns='${namespace}'/>`, little, true)));
    assert.ok(matches(utf16(`<?xml version='1.0' encoding='UTF-8'?><LandXML xmlns='${namespace}'>`, little, bom)));
  }
  const start = utf16(`<LandXML xmlns='${namespace}'>`, little, true);
  for (let end = 0; end < start.length; end++) {
    assert.equal(matches(start.subarray(0, end)), false, `${little} ${end}`);
  }
  assert.ok(matches(start));
  const brokenBody = new Uint8Array(start.length + 3);
  brokenBody.set(start);
  brokenBody.set(little ? [0, 0xd8, 0xff] : [0xd8, 0, 0xff], start.length);
  assert.ok(matches(brokenBody));
  for (const text of [
    `<LandXMLx xmlns='${namespace}'>`, "<LandXML xmlns='urn:other'>", "<LandXML>",
    `<p:LandXML xmlns='${namespace}'>`, "<!-- <LandXML> ", "text<LandXML>",
  ]) {
    assert.equal(matches(utf16(text, little, true)), false);
  }
}

// プラグインも同じ署名を使う。規則側の非 BMP 文字と、名前空間を問わない場合も比べる。
const plugins = new Modules({}, undefined, [{
  name: "xml-signature-test", ops: [],
  formats: [
    { format: "unicode", rules: [{ kind: "xml_root", name: "図😀", namespace: "urn:測😀" }] },
    { format: "any", rules: [{ kind: "xml_root", name: "LandXML", namespace: null }] },
  ],
}]);
for (const little of [false, true]) {
  assert.equal(plugins.detect(utf16("<測😀:図😀 xmlns:測😀='urn:測😀'/>", little, true))?.format, "unicode");
  assert.equal(plugins.detect(utf16("<LandXML/>", little, true))?.format, "any");
  assert.equal(plugins.detect(utf16("<\ud800/>", little, true)), undefined);
}

for (const prefix of [
  [0, 0, 0xfe, 0xff], [0xff, 0xfe, 0, 0], [0, 0, 0xff, 0xfe], [0xfe, 0xff, 0, 0],
  [0, 0, 0, 0x3c], [0x3c, 0, 0, 0], [0, 0, 0x3c, 0], [0, 0x3c, 0, 0],
]) {
  const tail = utf16(`<LandXML xmlns='${namespace}'>`, true, false);
  const bytes = new Uint8Array(prefix.length + tail.length);
  bytes.set(prefix);
  bytes.set(tail, prefix.length);
  assert.equal(matches(bytes), false);
}
for (const bytes of [[0xff], [0xff, 0xfe], [0xff, 0xfe, 0x3c], [0xfe, 0xff, 0]]) {
  assert.equal(matches(new Uint8Array(bytes)), false);
}

const shiftJisStart = encoder.encode("<?xml version='1.0' encoding='Shift_JIS'?><LandXML a='");
const shiftJisEnd = encoder.encode(`' xmlns='${namespace}'>`);
const shiftJis = new Uint8Array(shiftJisStart.length + 4 + shiftJisEnd.length);
shiftJis.set(shiftJisStart);
shiftJis.set([0x95, 0x5c, 0x8e, 0xa6], shiftJisStart.length);
shiftJis.set(shiftJisEnd, shiftJisStart.length + 4);
assert.ok(matches(shiftJis));
const longHeader = `<!-- ${"x".repeat(65_536)} --><LandXML a='${"y".repeat(65_536)}' xmlns='${namespace}'>`;
assert.ok(matches(encoder.encode(longHeader)));
for (const little of [false, true]) {
  assert.ok(matches(utf16(longHeader, little, true)));
}
console.log("XML の UTF-8/Shift_JIS/UTF-16 の署名・切り詰め・後続本文の判定が一致");

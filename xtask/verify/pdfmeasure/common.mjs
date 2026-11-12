// #520: Cargo と配布物へ入れない、計測辞書の外部照合。
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

export const pin = "0cfea2f280c40d5f5e0d065496704654468390b5";
export const sourcePath = "Pdf4QtLibCore/sources/pdfmeasure.cpp";
export const sourceUrl = `https://raw.githubusercontent.com/JakubMelka/PDF4QT/${pin}/${sourcePath}`;
export const sourceBlob = "f2319cb468706d11da5d2eccc681b97bebe2b138";
export const sourceSha256 = "89f9c96b9cee4b56697ae9d37549edfe440ed6ea0362b4ab8c55c7b6d1c58fa7";
export const artifactDirectory = fileURLToPath(new URL("../../../target/pdfmeasure/", import.meta.url));
export const executableName = process.platform === "win32" ? "pdf4qt-distance.exe" : "pdf4qt-distance";
export const signatures = [
  "QString PDFNumberFormat::format(const std::vector<PDFNumberFormat>& formats, PDFReal value)",
  "PDFReal PDFMeasure::getUnitsPerUserSpaceUnit() const",
  "PDFReal PDFMeasure::getUnitsPerUserSpaceUnitY() const",
  "QString PDFMeasure::formatDistance(PDFReal userSpaceLength) const",
  "QString PDFMeasure::formatDistance(const QPointF& userSpaceVector) const",
];

export const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
export const gitBlob = bytes => createHash("sha1").update(`blob ${bytes.length}\0`).update(bytes).digest("hex");

export async function readInput(limit) {
  const chunks = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > limit) throw new Error("入力が照合ツールの上限を超えました");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

export async function validateArtifacts() {
  const manifest = JSON.parse(await readFile(`${artifactDirectory}/manifest.json`, "utf8"));
  if (manifest.protocol !== 1 || manifest.pin !== pin || manifest.source.sha256 !== sourceSha256
      || manifest.platform !== process.platform || manifest.arch !== process.arch) {
    throw new Error("照合プログラムの版または実行環境が違います。setup.mjs を実行してください");
  }
  for (const [name, hash] of Object.entries(manifest.files)) {
    if (sha256(await readFile(`${artifactDirectory}/${name}`)) !== hash) {
      throw new Error(`照合プログラムの hash が違います: ${name}`);
    }
  }
  if (sha256(await readFile(new URL("setup.mjs", import.meta.url))) !== manifest.setup_sha256
      || sha256(await readFile(new URL("common.mjs", import.meta.url))) !== manifest.common_sha256
      || sha256(await readFile(new URL("harness.mjs", import.meta.url))) !== manifest.harness_sha256) {
    throw new Error("照合ツールが更新されています。setup.mjs を実行してください");
  }
  return manifest;
}

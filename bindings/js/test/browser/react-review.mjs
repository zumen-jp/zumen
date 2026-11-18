// プレイグラウンドのレビュー（#589）。変更雲・注記・指摘を作り、直し、絞り、保存し、読み直す。
// 描画はコアの review_scene を Worker で呼び、図面に重ねた Canvas に描く。
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

export async function checkReview(page, ready, dxf) {
  await page.getByLabel("図面ファイル").setInputFiles({ name: "review.dxf", mimeType: "application/octet-stream", buffer: dxf });
  await ready("DXF");
  const panel = page.getByRole("region", { name: "レビュー", exact: true });
  const status = panel.getByRole("status");
  const shown = async (count) => {
    try { await status.getByText("レビューの項目を " + count + " 件描きました。", { exact: true }).waitFor({ timeout: 10000 }); }
    catch (error) { throw new Error("レビューを " + count + " 件描いた表示にならない: " + await status.textContent(), { cause: error }); }
  };
  const items = panel.getByRole("list").getByRole("button");
  const overlay = page.locator("canvas.review-overlay");
  const ink = () => overlay.evaluate((canvas) => {
    const data = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
    let count = 0;
    for (let i = 3; i < data.length; i += 4) if (data[i] > 0) count++;
    return count;
  });
  await shown(0);
  assert.equal(await ink(), 0, "空のレビューは図面を隠さない");
  assert.equal(await overlay.evaluate((canvas) => getComputedStyle(canvas).pointerEvents), "none", "入力は図面の Canvas が受ける");

  await panel.getByRole("button", { name: "変更雲を追加", exact: true }).click();
  await shown(1);
  assert.ok(await ink() > 0, "変更雲を重ねて描く");
  assert.equal(await items.first().getAttribute("aria-pressed"), "true", "追加した項目を選ぶ");
  await panel.getByRole("textbox", { name: /^文言/ }).fill("寸法の確認");
  await panel.getByRole("button", { name: "指摘を追加", exact: true }).click();
  await shown(2);
  assert.match(await items.nth(1).innerText(), /指摘「寸法の確認」・未対応/);

  await panel.getByRole("button", { name: "元に戻す", exact: true }).click();
  await shown(1);
  await panel.getByRole("button", { name: "やり直す", exact: true }).click();
  await shown(2);

  // 図面の中央をクリックすると、指摘の場所の印を選ぶ
  await items.first().click();
  const canvas = page.getByLabel("図面の表示", { exact: true });
  const box = await canvas.boundingBox();
  await canvas.click({ position: { x: box.width / 2, y: box.height / 2 } });
  await page.waitForFunction(() => document.querySelectorAll(".review [aria-pressed=true]").length === 1 &&
    document.querySelector(".review [aria-pressed=true]")?.textContent.includes("指摘の場所"));

  // 指摘の状態を変えて、状態で絞る
  await panel.getByRole("combobox", { name: /^指摘の状態/ }).selectOption("resolved");
  await panel.getByRole("combobox", { name: /^状態/ }).selectOption("resolved");
  await shown(1);
  await panel.getByRole("combobox", { name: /^状態/ }).selectOption("open");
  await shown(0);
  await panel.getByRole("combobox", { name: /^状態/ }).selectOption("");
  await shown(2);

  // 保存した文書は、開いた図面の版を対象に持つ
  const [download] = await Promise.all([page.waitForEvent("download"), panel.getByRole("button", { name: "レビューを保存", exact: true }).click()]);
  assert.equal(download.suggestedFilename(), "review.json");
  const saved = JSON.parse(await readFile(await download.path(), "utf8"));
  assert.equal(saved.format, "zumen-review");
  assert.equal(saved.markups.length, 1);
  assert.equal(saved.topics[0].status, "resolved");
  assert.equal(saved.topics[0].locations[0].view, "space/0");
  assert.match(saved.targets[0].hash, /^v2:/);

  // 削除してから、保存した文書を読み直す
  await items.first().click();
  await panel.getByRole("button", { name: "選んだ項目を削除", exact: true }).click();
  await shown(1);
  const reviewFile = panel.getByLabel("レビューを読む", { exact: true });
  const load = (name, value) => reviewFile.setInputFiles({ name, mimeType: "application/json", buffer: Buffer.from(typeof value === "string" ? value : JSON.stringify(value)) });
  await load("review.json", saved);
  await shown(2);
  assert.equal(await panel.getByRole("button", { name: "元に戻す", exact: true }).isDisabled(), true, "読み直しで履歴を消す");

  // 移した文書の状態を一覧に出す
  const carried = structuredClone(saved);
  carried.markups[0].carried = { from: "v2:00000000000000000000000000000000", state: "changed" };
  await load("carried.json", carried);
  await shown(2);
  assert.match(await items.first().innerText(), /移した位置: 変更あり/);

  // 版の違う文書はコアが描かず、診断を出す
  const stale = structuredClone(saved);
  stale.targets[0].hash = "v2:00000000000000000000000000000000";
  await load("stale.json", stale);
  await status.getByText("レビューの対象の版が、開いた図面と違います。", { exact: true }).waitFor();
  assert.match(await panel.getByRole("region", { name: /^レビューの診断/ }).innerText(), /review\.target\.stale|版/);
  assert.equal(await ink(), 0, "描けない文書の前の描画を消す");

  // 読めない文書は、前の文書を保つ
  await load("broken.json", "{");
  await status.getByText("レビューの文書を読めませんでした。", { exact: true }).waitFor();
  assert.equal(await items.count(), 2);
}

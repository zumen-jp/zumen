import { View2D } from "../../../../../packages/view2d/src/index.js";
import { bundle, fixture } from "../../../../../packages/view2d/test/fixtures.mjs";
import { View3D } from "../../../../../packages/view3d/src/index.js";
import { document as model, encode, result, synthetic, topView } from "../../../../../packages/view3d/test/fixture.mjs";

function inspect(shot, x, y, id, expectedId, expectedColor) {
  const offset = (y * shot.width + x) * 4;
  const rgb = Array.from(shot.pixels.subarray(offset, offset + 3));
  const matches = rgb.length === 3 && rgb.every((value, i) => Math.abs(value - expectedColor[i]) <= 3);
  return { x, y, rgb, id, expectedColor, expectedId, passed: matches && id === expectedId };
}

async function drawing() {
  const viewer = await View2D.create(document.querySelector("#drawing"), { width: 200, height: 200, pixelRatio: 1, pixelsPerMillimeter: 2 });
  viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 2, rotation: 0 });
  viewer.setScene(bundle(fixture()));
  const point = { x: 100, y: 70 };
  const shot = await viewer.snapshot();
  return inspect(shot, point.x, point.y, viewer.hitTest(point, 0)[0]?.id, "space/0/h1", [0, 0, 255]);
}

async function building() {
  const viewer = await View3D.create(document.querySelector("#building"), { width: 200, height: 200, pixelRatio: 1, shading: "unlit", views: [topView()] });
  const { columns } = synthetic();
  viewer.setScene({ document: model, blob: encode(columns).bytes, result: result(columns) });
  viewer.render();
  const point = { x: 30, y: 70 };
  const shot = await viewer.snapshot("top");
  return inspect(shot, point.x, point.y, viewer.hitTest("top", point).hits[0]?.id, "red", [255, 0, 0]);
}

const observations = {};
for (const [view, run] of Object.entries({ drawing, building })) {
  try {
    observations[view] = await run();
  } catch (error) {
    observations[view] = { passed: false, error: error.reason ?? error.message };
  }
}
const output = document.querySelector("#observed");
output.textContent = JSON.stringify(observations, null, 2);
output.dataset.complete = "true";

// 適合テストの組を、TypeScript・JavaScript のラッパー（bindings/js）を通して流す本体（決定 0194・0199）。
//
// Node.js（conformance.mjs）とブラウザー（browser/run.mjs）の両方で読み込むので、ファイルもハッシュのツールも
// 引数で受け取る。組の手順の状態と応答の値を確かめ、ハッシュの一覧を返す。一覧を hashes.txt と比べるのは呼んだ側。

import { ACTIONS, bytesOf, checkResponse, pushOf, requestOf } from "../../conformance/steps.mjs";
import { SCENE_MAJOR, SCENE_SECTIONS, SCHEMA_MAJOR, ZumenError, readScene, readScene3D } from "../src/index.js";

/** API の関数を呼び、状態の名前と、書かれたバイト列を返す */
async function attempt(fn) {
  try {
    return ["ok", await fn()];
  } catch (error) {
    if (!(error instanceof ZumenError)) {
      throw error;
    }
    return [error.status, error.bytes];
  }
}

/** シーンの blob をリーダーで読み、応答の数と合うかを確かめる */
function checkScene3D(at, blob, result, failures) {
  try {
    const scene = readScene3D(blob);
    // deviation などはシーンの応答を scene3d の下に入れる
    const counts = (result.scene3d ?? result).counts;
    for (const [name, column] of [["chunks", "chunk_origins"], ["meshes", "mesh_vertices"], ["instances", "instance_meshes"], ["elements", "element_ids"], ["colors", "color_rgba"], ["vertices", "vertex_xyz"], ["triangles", "triangle_vertices"], ["edges", "edge_vertices"]]) {
      if (scene.rows(column) !== counts[name]) throw new Error(name + " count mismatch");
    }
    for (let row = 0; row < scene.rows("instance_meshes"); row += 1) {
      const instance = scene.instance(row);
      const element = scene.element(instance.element);
      if (scene.elementById(element.id) !== instance.element) throw new Error("element ID mismatch");
      scene.mesh(instance.mesh);
      scene.color(instance.color);
    }
  } catch (error) {
    failures.push(at + ": 3D のシーンを読めない: " + error.message);
  }
}

function checkScene(at, blob, result, failures) {
  if (blob[3] === 0x33) return checkScene3D(at, blob, result, failures);
  let scene;
  try {
    scene = readScene(blob);
  } catch (error) {
    failures.push(`${at}: シーンを読めない: ${error.message}`);
    return;
  }
  const counts = result.counts;
  const checks = [
    ["メジャーバージョン", scene.major, SCENE_MAJOR],
    ["セクションの数", scene.sections, SCENE_SECTIONS.length],
    ["塊の数", scene.chunks, counts.chunks],
    ["chunk_origins の行", scene.rows("chunk_origins"), counts.chunks],
    ["text_xy の行", scene.rows("text_xy"), counts.texts],
    ["point_xy の行", scene.rows("point_xy"), counts.points],
    ["glyph_xy の行", scene.rows("glyph_xy"), counts.glyphs],
    ["line_xy と face_xy の行", scene.rows("line_xy") + scene.rows("face_xy"), counts.vertices],
  ];
  // 文字の文字列は、strings のセクションから引ける
  const strings = scene.columns.text_strings;
  for (let row = 0; row < strings.length; row += 2) {
    try {
      scene.text(strings[row], strings[row + 1]);
    } catch (error) {
      failures.push(`${at}: text_strings の行 ${row / 2} を引けない: ${error.message}`);
    }
  }
  for (const [label, actual, expected] of checks) {
    if (actual !== expected) {
      failures.push(`${at}: シーンの${label}が ${actual}（期待は ${expected}）`);
    }
  }
}

/**
 * 組を流し、ハッシュの一覧・読んだシーンの数・期待と違ったことを返す。
 *
 * - zumen: open でセッションを返すもの。各メソッドは Promise でもよい
 * - cases: cases.mjs の loadCases が返す組
 * - readSample: samples/ の中のファイルの道筋から、中身の Uint8Array を返す
 * - sha256: バイト列から、SHA-256 の 16 進の文字列を返す（非同期でよい）
 * - pick: 分けた wasm のモジュールで流すとき、組から、流すモジュール（load で読み込んだもの）を返す。組のあとのテストは
 *   zumen で流す
 */
export async function runConformance({ zumen: base, cases, readSample, sha256, pick }) {
  const failures = [];
  const hashes = [];
  let scenes = 0;
  for (const testCase of cases) {
    const { name, steps } = testCase;
    // 分けたモジュールで流すときは、組ごとにモジュールを選ぶ（決定 0221）
    const zumen = pick ? pick(testCase) : base;
    let session = null;
    // 直前の応答が返したシーンの blob の番号と結果
    let lastScene = null;
    try {
      for (const [index, step] of steps.entries()) {
        const at = `${name} の手順 ${index}`;
        const action = ACTIONS.find((key) => Object.hasOwn(step, key));
        const expected = step.status ?? "ok";
        const input = step[action];
        if (action !== "session" && session === null) {
          session = await zumen.open();
        }
        let status;
        let output = null;
        if (action === "session") {
          await session?.close();
          [status, session] = await attempt(() => zumen.open(bytesOf(input, "options", "options_text")));
          if (status !== "ok") {
            // 作れなかったら、閉じたセッションで続ける。API は、空のセッションへの呼び出しを invalid にする
            session = await zumen.open();
            await session.close();
          }
        } else if (action === "push") {
          [status] = await attempt(() => session.push(...pushOf(input, readSample)));
        } else if (action === "take") {
          const [taken, data] = await attempt(() => session.take(input));
          status = taken;
          output = data && ["blob", data];
          if (taken === "ok" && lastScene !== null && lastScene.blob === input) {
            checkScene(at, data, lastScene, failures);
            scenes += 1;
          }
        } else {
          const request = action === "call" ? JSON.stringify(input) : requestOf(action, input);
          const [called, response] = await attempt(() => session.callBytes(request));
          status = called;
          output = response && ["response", response];
        }
        if (status !== expected) {
          failures.push(`${at}: 状態が ${status}（期待は ${expected}）`);
        }
        if (!output) {
          continue;
        }
        const [kind, data] = output;
        hashes.push(`${name} ${index} ${kind} ${await sha256(data)}`);
        if (kind !== "response") {
          continue;
        }
        const response = JSON.parse(new TextDecoder().decode(data));
        checkResponse(at, step, response, failures);
        const blob = response.blobs?.find((entry) => ["scene", "scene3d"].includes(entry.kind));
        lastScene = blob && response.result ? { ...response.result, blob: blob.id } : null;
      }
      // 動いている呼び出しがないので、何もしない
      await session?.cancel();
    } finally {
      await session?.close();
    }
  }

  // 値を渡す call と、閉じたあとの呼び出し
  {
    const session = await base.open({ v: SCHEMA_MAJOR });
    try {
      const response = await session.call({ v: SCHEMA_MAJOR, op: "capabilities" });
      if (response.v !== SCHEMA_MAJOR || typeof response.result?.package !== "string") {
        failures.push(`call の応答が capabilities の形でない: ${JSON.stringify(response).slice(0, 200)}`);
      }
      try {
        await session.call({ v: SCHEMA_MAJOR, op: "open", input: "none.dxf" });
        failures.push("ない入力を開く call が失敗しない");
      } catch (error) {
        if (!(error instanceof ZumenError) || error.status !== "failed" || !error.response?.diagnostics?.length) {
          failures.push(`ない入力を開く call の失敗が、診断のある failed でない: ${error}`);
        }
      }
      await session.close();
      const [status] = await attempt(() => session.call({ v: SCHEMA_MAJOR, op: "capabilities" }));
      if (!session.closed || status !== "invalid") {
        failures.push(`閉じたあとの call が ${status}（期待は invalid）`);
      }
    } finally {
      await session.close();
    }
  }

  // 値を渡す call と、閉じたあとの呼び出し
  {
    const session = await base.open({ v: SCHEMA_MAJOR });
    try {
      const response = await session.call({ v: SCHEMA_MAJOR, op: "capabilities" });
      if (response.v !== SCHEMA_MAJOR || typeof response.result?.package !== "string") {
        failures.push(`call の応答が capabilities の形でない: ${JSON.stringify(response).slice(0, 200)}`);
      }
      try {
        await session.call({ v: SCHEMA_MAJOR, op: "open", input: "none.dxf" });
        failures.push("ない入力を開く call が失敗しない");
      } catch (error) {
        if (!(error instanceof ZumenError) || error.status !== "failed" || !error.response?.diagnostics?.length) {
          failures.push(`ない入力を開く call の失敗が、診断のある failed でない: ${error}`);
        }
      }
      await session.close();
      const [status] = await attempt(() => session.call({ v: SCHEMA_MAJOR, op: "capabilities" }));
      if (!session.closed || status !== "invalid") {
        failures.push(`閉じたあとの call が ${status}（期待は invalid）`);
      }
    } finally {
      await session.close();
    }
  }

  if (scenes === 0) {
    failures.push("シーンの blob を 1 つも読んでいない");
  }
  return { hashes, scenes, failures };
}

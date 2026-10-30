// 適合試験の組を、TypeScript・JavaScript の包み（bindings/js）を通して流す本体（決定 0194・0199）。
//
// Node.js（conformance.mjs）とブラウザー（browser/run.mjs）の両方で読み込むので、ファイルもハッシュの道具も
// 引数で受け取る。組の手順の状態と応答の値を確かめ、ハッシュの一覧を返す。一覧を hashes.txt と比べるのは呼んだ側。

import { ACTIONS, bytesOf, checkResponse, pushOf, requestOf } from "../../conformance/steps.mjs";
import { SCENE_MAJOR, SCENE_SECTIONS, SCHEMA_MAJOR, ZumenError, readScene } from "../src/index.js";

/** 境界の関数を呼び、状態の名前と、書かれたバイト列を返す */
function attempt(fn) {
  try {
    return ["ok", fn()];
  } catch (error) {
    if (!(error instanceof ZumenError)) {
      throw error;
    }
    return [error.status, error.bytes];
  }
}

/** シーンの blob を読み手で読み、応答の数と合うかを確かめる */
function checkScene(at, blob, result, failures) {
  let scene;
  try {
    scene = readScene(blob);
  } catch (error) {
    failures.push(`${at}: シーンを読めない: ${error.message}`);
    return;
  }
  const counts = result.counts;
  const checks = [
    ["大版", scene.major, SCENE_MAJOR],
    ["区画の数", scene.sections, SCENE_SECTIONS.length],
    ["塊の数", scene.chunks, counts.chunks],
    ["chunk_origins の行", scene.rows("chunk_origins"), counts.chunks],
    ["text_xy の行", scene.rows("text_xy"), counts.texts],
    ["point_xy の行", scene.rows("point_xy"), counts.points],
    ["glyph_xy の行", scene.rows("glyph_xy"), counts.glyphs],
    ["line_xy と face_xy の行", scene.rows("line_xy") + scene.rows("face_xy"), counts.vertices],
  ];
  // 文字の文字列は、strings の区画から引ける
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
 * - zumen: load か loadSync で読み込んだもの
 * - cases: cases.mjs の loadCases が返す組
 * - readSample: samples/ の中のファイルの道筋から、中身の Uint8Array を返す
 * - sha256: バイト列から、SHA-256 の 16 進の文字列を返す（非同期でよい）
 */
export async function runConformance({ zumen, cases, readSample, sha256 }) {
  const failures = [];
  const hashes = [];
  let scenes = 0;
  for (const { name, steps } of cases) {
    let session = null;
    // 直前の応答が返したシーンの blob の番号と結果
    let lastScene = null;
    for (const [index, step] of steps.entries()) {
      const at = `${name} の手順 ${index}`;
      const action = ACTIONS.find((key) => Object.hasOwn(step, key));
      const expected = step.status ?? "ok";
      const input = step[action];
      if (action !== "session" && session === null) {
        session = zumen.open();
      }
      let status;
      let output = null;
      if (action === "session") {
        session?.close();
        [status, session] = attempt(() => zumen.open(bytesOf(input, "options", "options_text")));
        if (status !== "ok") {
          // 作れなかったら、閉じたセッションで続ける。境界は、空のセッションへの呼び出しを invalid にする
          session = zumen.open();
          session.close();
        }
      } else if (action === "push") {
        [status] = attempt(() => session.push(...pushOf(input, readSample)));
      } else if (action === "take") {
        const [taken, data] = attempt(() => session.take(input));
        status = taken;
        output = data && ["blob", data];
        if (taken === "ok" && lastScene !== null && lastScene.blob === input) {
          checkScene(at, data, lastScene, failures);
          scenes += 1;
        }
      } else {
        const request = action === "call" ? JSON.stringify(input) : requestOf(action, input);
        const [called, response] = attempt(() => session.callBytes(request));
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
      const blob = response.blobs?.find((entry) => entry.kind === "scene");
      lastScene = blob && response.result ? { ...response.result, blob: blob.id } : null;
    }
    // 動いている呼び出しがないので、何もしない
    session.cancel();
    session.close();
  }

  // 値を渡す call と、閉じたあとの呼び出し
  {
    const session = zumen.open({ v: SCHEMA_MAJOR });
    const response = session.call({ v: SCHEMA_MAJOR, op: "capabilities" });
    if (response.v !== SCHEMA_MAJOR || typeof response.result?.package !== "string") {
      failures.push(`call の応答が capabilities の形でない: ${JSON.stringify(response).slice(0, 200)}`);
    }
    try {
      session.call({ v: SCHEMA_MAJOR, op: "open", input: "none.dxf" });
      failures.push("ない入力を開く call が失敗しない");
    } catch (error) {
      if (!(error instanceof ZumenError) || error.status !== "failed" || !error.response?.diagnostics?.length) {
        failures.push(`ない入力を開く call の失敗が、診断のある failed でない: ${error}`);
      }
    }
    session.close();
    const [status] = attempt(() => session.call({ v: SCHEMA_MAJOR, op: "capabilities" }));
    if (!session.closed || status !== "invalid") {
      failures.push(`閉じたあとの call が ${status}（期待は invalid）`);
    }
  }

  // 値を渡す call と、閉じたあとの呼び出し
  {
    const session = zumen.open({ v: SCHEMA_MAJOR });
    const response = session.call({ v: SCHEMA_MAJOR, op: "capabilities" });
    if (response.v !== SCHEMA_MAJOR || typeof response.result?.package !== "string") {
      failures.push(`call の応答が capabilities の形でない: ${JSON.stringify(response).slice(0, 200)}`);
    }
    try {
      session.call({ v: SCHEMA_MAJOR, op: "open", input: "none.dxf" });
      failures.push("ない入力を開く call が失敗しない");
    } catch (error) {
      if (!(error instanceof ZumenError) || error.status !== "failed" || !error.response?.diagnostics?.length) {
        failures.push(`ない入力を開く call の失敗が、診断のある failed でない: ${error}`);
      }
    }
    session.close();
    const [status] = attempt(() => session.call({ v: SCHEMA_MAJOR, op: "capabilities" }));
    if (!session.closed || status !== "invalid") {
      failures.push(`閉じたあとの call が ${status}（期待は invalid）`);
    }
  }

  if (scenes === 0) {
    failures.push("シーンの blob を 1 つも読んでいない");
  }
  return { hashes, scenes, failures };
}

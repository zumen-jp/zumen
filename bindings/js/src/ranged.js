// open の needs に答え、入力の範囲を読んで渡す（決定 0232・#474）。
//
// 初めの部分を request.input の名前で渡し、size を書いて open を呼ぶ。足りない範囲を needs の名前で push して同じ要求を
// 呼び直し、読み終えた done の入力を close で捨てる。範囲で読めない形式なら、コアは初めの部分を全体として扱うので、
// 全体を読んで渡し直す。

/** 1 回目に渡す初めの部分の既定の大きさ。LAS の頭と VLR が入る */
const HEAD = 64 * 1024;
/** needs に答える回数の上限。範囲の名前と同じく、コアの要求が進まなければ止める */
const ROUNDS = 64;

const isBlob = (value) => typeof Blob === "function" && value instanceof Blob;
const isUrl = (value) => typeof value === "string" || (typeof URL === "function" && value instanceof URL);

/** 読み口を (start, end) => Promise<Uint8Array> と全体の大きさにする */
async function reader(source, head, signal) {
  if (typeof source === "function") {
    return { read: async (start, end) => toBytes(await source(start, end)), size: null };
  }
  if (isBlob(source)) {
    return { read: async (start, end) => new Uint8Array(await source.slice(start, end).arrayBuffer()), size: source.size };
  }
  if (isUrl(source)) {
    const url = String(source);
    const fetchRange = async (start, end) => {
      const response = await fetch(url, { headers: { Range: `bytes=${start}-${end - 1}` }, signal });
      if (!response.ok) throw new Error(`範囲を読めない: ${response.status}`);
      return response;
    };
    // 最初の範囲の応答の Content-Range から全体の大きさを知る。範囲に答えないサーバーなら全体が届く
    const first = await fetchRange(0, head);
    const firstBytes = new Uint8Array(await first.arrayBuffer());
    const total = first.status === 206 ? /\/(\d+)\s*$/.exec(first.headers.get("Content-Range") ?? "")?.[1] : null;
    return {
      first: firstBytes,
      size: first.status === 206 ? (total === undefined || total === null ? null : Number(total)) : firstBytes.length,
      read: async (start, end) => {
        const response = await fetchRange(start, end);
        if (response.status !== 206) throw new Error("サーバーが範囲の読み出しに答えない");
        return new Uint8Array(await response.arrayBuffer());
      },
    };
  }
  throw new TypeError("source には Blob・URL・(start, end) => バイト列 の関数を渡す");
}

function toBytes(value) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  throw new TypeError("読み口はバイト列を返す");
}

const limitOf = (request) => {
  const limit = request.budget?.input_bytes;
  return typeof limit === "number" ? limit : Infinity;
};

/**
 * 入力の範囲を読みながら open する。request.input の名前で初めの部分を渡し、size は読み口から決める（関数なら
 * options.size で渡す）。範囲の入力は読み終えたら close で捨てる。失敗したら初めの部分も捨てる。応答は最後の open の応答
 */
export async function openRanged(session, request, source, options = {}) {
  if (request === null || typeof request !== "object" || typeof request.input !== "string") {
    throw new TypeError("request には input を書いた open の要求を渡す");
  }
  const head = options.head ?? HEAD;
  if (!Number.isSafeInteger(head) || head <= 0) throw new RangeError("head は正の整数");
  const signal = options.signal;
  const { read, first, size: known } = await reader(source, head, signal);
  const size = options.size ?? known;
  if (size !== null && (!Number.isSafeInteger(size) || size < 0)) throw new RangeError("size は 0 以上の整数");
  const limit = limitOf(request);
  const pushed = new Set();
  const call = { ...request, op: "open" };
  if (size !== null) call.size = size;
  try {
    const bytes = first ?? await read(0, size === null ? head : Math.min(head, size));
    signal?.throwIfAborted();
    session.push(request.input, bytes);
    pushed.add(request.input);
    let previous = "";
    for (let round = 0; ; round++) {
      const response = session.call(call);
      const { needs, done } = response.result ?? {};
      if (done?.length) {
        const names = done.map((range) => range.input);
        session.call({ v: 1, op: "close", inputs: names });
        for (const name of names) pushed.delete(name);
      }
      if (!needs?.length) {
        // 文書ができたら、初めの部分は普段の open の入力と同じく残す
        pushed.delete(request.input);
        return response;
      }
      const key = JSON.stringify(needs);
      if (key === previous || round >= ROUNDS) throw new Error("open が同じ範囲を求め続ける");
      previous = key;
      let total = 0;
      for (const range of needs) {
        const { start, end } = range;
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start || (size !== null && end > size)) {
          throw new RangeError("open の needs の範囲が入力の外");
        }
        total += end - start;
      }
      if (total > limit) throw new RangeError("needs の範囲の合計が予算の input_bytes を超える");
      for (const range of needs) {
        const part = await read(range.start, range.end);
        signal?.throwIfAborted();
        if (part.length !== range.end - range.start) throw new Error("読み口が求めた長さを返さない");
        session.push(range.input, part);
        pushed.add(range.input);
      }
    }
  } catch (error) {
    // 読み終える前に止まったら、渡した初めの部分と範囲を捨てる。閉じたセッションなどで捨てられなくても、先の例外を隠さない
    if (pushed.size && !session.closed) {
      try { session.call({ v: 1, op: "close", inputs: [...pushed] }); } catch { /* 先の例外を返す */ }
    }
    throw error;
  }
}

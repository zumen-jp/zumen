// ブラウザーの Worker の側。worker.mjs と同じく、取ってきたバイト列を wasm のメモリの代わりにする。
let memory;
let format;
self.onmessage = async (event) => {
  if (event.data.load) {
    format = event.data.load;
    memory = new Uint8Array(await (await fetch(`/data/${format}.bin`)).arrayBuffer());
    self.postMessage("ready");
    return;
  }
  const sent = performance.timeOrigin + performance.now();
  if (format === "json") {
    self.postMessage({ sent, text: new TextDecoder().decode(memory) });
  } else {
    const buffer = memory.slice().buffer;
    self.postMessage({ sent, buffer }, [buffer]);
  }
};

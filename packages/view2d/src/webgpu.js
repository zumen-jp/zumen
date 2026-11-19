// View2D 用の WebGPU 2D レンダラー。
// 4x MSAA + ステンシル（偶奇・非零・線の和集合）+ 深度切り抜きで図面を 1 パスで描き、
// 永続の rgba8unorm テクスチャへ解決したうえで表示先 Canvas へ転送する。

const SAMPLE_COUNT = 4;
const FLOATS_PER_VERTEX = 8;
const BYTES_PER_VERTEX = FLOATS_PER_VERTEX * 4;

const SHADER_WGSL = /* wgsl */ `
struct ViewportUniforms {
  canvasSize: vec2<f32>,
  pixelRatio: f32,
  _pad: f32,
};

@group(0) @binding(0) var<uniform> vp: ViewportUniforms;

struct VsIn {
  @location(0) pos: vec2<f32>,
  @location(1) aux: vec2<f32>,
  @location(2) color: vec4<f32>,
};

struct VsOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
};

@vertex
fn vsMain(in: VsIn) -> VsOut {
  var out: VsOut;
  // aux.y >= 0.5 のときは pos をそのまま NDC 座標、aux.x を深度 z として使う。
  if (in.aux.y >= 0.5) {
    out.position = vec4<f32>(in.pos, in.aux.x, 1.0);
  } else {
    let dev = in.pos * vp.pixelRatio;
    let ndc = vec2<f32>(
      (dev.x / vp.canvasSize.x) * 2.0 - 1.0,
      1.0 - (dev.y / vp.canvasSize.y) * 2.0,
    );
    out.position = vec4<f32>(ndc, in.aux.x, 1.0);
  }
  out.uv = in.aux;
  out.color = in.color;
  return out;
}

@vertex
fn vsTextured(in: VsIn) -> VsOut {
  var out: VsOut;
  let dev = in.pos * vp.pixelRatio;
  let ndc = vec2<f32>(
    (dev.x / vp.canvasSize.x) * 2.0 - 1.0,
    1.0 - (dev.y / vp.canvasSize.y) * 2.0,
  );
  out.position = vec4<f32>(ndc, 0.5, 1.0);
  out.uv = in.aux;
  out.color = in.color;
  return out;
}

@fragment
fn fsSolid(in: VsOut) -> @location(0) vec4<f32> {
  return in.color;
}

@fragment
fn fsMask(_in: VsOut) {}

@group(1) @binding(0) var texSampler: sampler;
@group(1) @binding(1) var texSource: texture_2d<f32>;

@fragment
fn fsTextured(in: VsOut) -> @location(0) vec4<f32> {
  let c = textureSample(texSource, texSampler, in.uv);
  // copyExternalImageToTexture（premultipliedAlpha: false）はストレート RGBA を格納するため事前乗算する。
  let a = c.a * in.color.a;
  return vec4<f32>(c.rgb * a, a);
}

struct BlitOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vsBlit(@builtin(vertex_index) idx: u32) -> BlitOut {
  var positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(3.0, -1.0),
    vec2<f32>(-1.0, 3.0),
  );
  let p = positions[idx];
  var out: BlitOut;
  out.position = vec4<f32>(p, 0.0, 1.0);
  out.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
  return out;
}

@group(0) @binding(0) var blitSampler: sampler;
@group(0) @binding(1) var blitTexture: texture_2d<f32>;

@fragment
fn fsBlit(in: BlitOut) -> @location(0) vec4<f32> {
  return textureSample(blitTexture, blitSampler, in.uv);
}
`;

export function createWebGPUUnavailableError(message, cause) {
  const error = new Error(message, cause !== undefined ? { cause } : undefined);
  error.reason = "webgpu-unavailable";
  return error;
}

export function createContextLostError() {
  const error = new Error("WebGPU のデバイスを失っている");
  error.reason = "context-lost";
  return error;
}

class WebGPU2DBackend {
  constructor(device, preferredFormat) {
    this.device = device;
    this.preferredFormat = preferredFormat;

    const module = device.createShaderModule({ code: SHADER_WGSL });
    this.vpBindGroupLayout = device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform" } }],
    });
    this.texBindGroupLayout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "filtering" } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "float" } },
      ],
    });
    this.blitBindGroupLayout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "non-filtering" } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "float" } },
      ],
    });

    const solidLayout = device.createPipelineLayout({ bindGroupLayouts: [this.vpBindGroupLayout] });
    const texLayout = device.createPipelineLayout({ bindGroupLayouts: [this.vpBindGroupLayout, this.texBindGroupLayout] });
    const blitLayout = device.createPipelineLayout({ bindGroupLayouts: [this.blitBindGroupLayout] });

    const vertexBuffers = [{
      arrayStride: BYTES_PER_VERTEX,
      attributes: [
        { shaderLocation: 0, offset: 0, format: "float32x2" },
        { shaderLocation: 1, offset: 8, format: "float32x2" },
        { shaderLocation: 2, offset: 16, format: "float32x4" },
      ],
    }];

    const premulBlend = {
      color: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
      alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
    };

    const makeMaskPipeline = (label, depthStencil) => device.createRenderPipeline({
      label,
      layout: solidLayout,
      vertex: { module, entryPoint: "vsMain", buffers: vertexBuffers },
      fragment: { module, entryPoint: "fsMask", targets: [{ format: "rgba8unorm", writeMask: 0 }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      multisample: { count: SAMPLE_COUNT },
      depthStencil,
    });

    // 1. 切り抜き輪郭（偶奇規則で stencil 反転、深度書き込みなし）
    this.pipelineClipStencil = makeMaskPipeline("view2d-clip-stencil", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: false,
      depthCompare: "always",
      stencilFront: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "invert" },
      stencilBack: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "invert" },
    });

    // 2. 切り抜き適用（stencil == 0 の範囲に depth = 0.0 を書いて除外する）
    this.pipelineClipMaskOut = makeMaskPipeline("view2d-clip-mask-out", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: true,
      depthCompare: "always",
      stencilFront: { compare: "equal", failOp: "keep", depthFailOp: "keep", passOp: "keep" },
      stencilBack: { compare: "equal", failOp: "keep", depthFailOp: "keep", passOp: "keep" },
    });

    // 3. ステンシル消去（stencil != 0 を 0 に戻す）
    this.pipelineStencilReset = makeMaskPipeline("view2d-stencil-reset", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: false,
      depthCompare: "always",
      stencilFront: { compare: "not-equal", failOp: "keep", depthFailOp: "keep", passOp: "zero" },
      stencilBack: { compare: "not-equal", failOp: "keep", depthFailOp: "keep", passOp: "zero" },
    });

    // 4. 切り抜き解除（全画面に depth = 1.0 を書く）
    this.pipelineClipReset = makeMaskPipeline("view2d-clip-reset", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: true,
      depthCompare: "always",
    });

    // 5. 塗りステンシル（evenodd）
    this.pipelineStencilEvenOdd = makeMaskPipeline("view2d-stencil-evenodd", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: false,
      depthCompare: "less",
      stencilFront: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "invert" },
      stencilBack: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "invert" },
    });

    // 6. 塗りステンシル（nonzero）
    this.pipelineStencilNonZero = makeMaskPipeline("view2d-stencil-nonzero", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: false,
      depthCompare: "less",
      stencilFront: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "increment-wrap" },
      stencilBack: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "decrement-wrap" },
    });

    // 7. 線ステンシル（同一 stroke 内の重なりを stencil = 1 に統一）
    this.pipelineStencilStroke = makeMaskPipeline("view2d-stencil-stroke", {
      format: "depth24plus-stencil8",
      depthWriteEnabled: false,
      depthCompare: "less",
      stencilFront: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "replace" },
      stencilBack: { compare: "always", failOp: "keep", depthFailOp: "keep", passOp: "replace" },
    });

    // 8. カバー描画（stencil != 0 のサンプルを 1 回だけ混色し、同時に stencil を 0 へ戻す）
    this.pipelineCover = device.createRenderPipeline({
      label: "view2d-cover",
      layout: solidLayout,
      vertex: { module, entryPoint: "vsMain", buffers: vertexBuffers },
      fragment: { module, entryPoint: "fsSolid", targets: [{ format: "rgba8unorm", blend: premulBlend }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      multisample: { count: SAMPLE_COUNT },
      depthStencil: {
        format: "depth24plus-stencil8",
        depthWriteEnabled: false,
        depthCompare: "less",
        stencilFront: { compare: "not-equal", failOp: "zero", depthFailOp: "zero", passOp: "zero" },
        stencilBack: { compare: "not-equal", failOp: "zero", depthFailOp: "zero", passOp: "zero" },
      },
    });

    // 9. 画像・代替文字テクスチャ描画（深度切り抜きテストあり）
    this.pipelineTextured = device.createRenderPipeline({
      label: "view2d-textured",
      layout: texLayout,
      vertex: { module, entryPoint: "vsTextured", buffers: vertexBuffers },
      fragment: { module, entryPoint: "fsTextured", targets: [{ format: "rgba8unorm", blend: premulBlend }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      multisample: { count: SAMPLE_COUNT },
      depthStencil: {
        format: "depth24plus-stencil8",
        depthWriteEnabled: false,
        depthCompare: "less",
      },
    });

    // 10. 解決済み rgba8unorm テクスチャから表示先フォーマット（bgra8unorm 等）への転送
    this.pipelineBlit = device.createRenderPipeline({
      label: "view2d-blit",
      layout: blitLayout,
      vertex: { module, entryPoint: "vsBlit" },
      fragment: { module, entryPoint: "fsBlit", targets: [{ format: preferredFormat }] },
      primitive: { topology: "triangle-list" },
    });

    this.linearSampler = device.createSampler({ minFilter: "linear", magFilter: "linear" });
    this.nearestSampler = device.createSampler({ minFilter: "nearest", magFilter: "nearest" });
  }
}

let sharedBackend = null;
let sharedBackendPromise = null;
const customBackends = new WeakMap();

export async function initWebGPU() {
  if (typeof navigator === "undefined" || !navigator.gpu) {
    throw createWebGPUUnavailableError("WebGPU を使えない");
  }
  if (sharedBackend !== null) return sharedBackend.device;
  if (sharedBackendPromise !== null) return (await sharedBackendPromise).device;
  sharedBackendPromise = (async () => {
    const adapter = (await navigator.gpu.requestAdapter()) ?? (await navigator.gpu.requestAdapter({ fallbackAdapter: true }));
    if (!adapter) throw createWebGPUUnavailableError("WebGPU のアダプターを取得できない");
    const device = await adapter.requestDevice();
    const format = navigator.gpu.getPreferredCanvasFormat();
    const backend = new WebGPU2DBackend(device, format);
    sharedBackend = backend;
    device.lost.then(() => {
      if (sharedBackend === backend) {
        sharedBackend = null;
        sharedBackendPromise = null;
      }
    });
    return backend;
  })().catch((error) => {
    sharedBackendPromise = null;
    if (error?.reason === "webgpu-unavailable") throw error;
    throw createWebGPUUnavailableError("WebGPU のデバイスを取得できない", error);
  });
  return (await sharedBackendPromise).device;
}

export function getBackend(customDevice) {
  if (customDevice !== undefined) {
    let cached = customBackends.get(customDevice);
    if (!cached) {
      const format = typeof navigator !== "undefined" && navigator.gpu ? navigator.gpu.getPreferredCanvasFormat() : "rgba8unorm";
      cached = new WebGPU2DBackend(customDevice, format);
      customBackends.set(customDevice, cached);
    }
    return cached;
  }
  if (sharedBackend === null) {
    throw createWebGPUUnavailableError("WebGPU が初期化されていない（await View2D.create を先に呼ぶ）");
  }
  return sharedBackend;
}

export class WebGPU2DSurface {
  #canvas;
  #context;
  #backend;
  #width = 0;
  #height = 0;
  #pixelRatio = 1;
  #cssWidth = 0;
  #cssHeight = 0;
  #msaaColor = null;
  #msaaColorView = null;
  #msaaDepthStencil = null;
  #msaaDepthStencilView = null;
  #resolveColor = null;
  #resolveColorView = null;
  #blitBindGroup = null;
  #uniformBuffer;
  #vpBindGroup;
  #vertexBuffer = null;
  #vertexBufferCapacity = 0;
  #verts = new Float32Array(16384 * FLOATS_PER_VERTEX);
  #vertCount = 0;
  #commands = [];
  #tempTextures = [];
  #underlayTextures = new Map();
  #activeClip = false;
  #lost = false;
  #destroyed = false;

  constructor(canvas, customDevice, onLost) {
    const backend = getBackend(customDevice);
    const context = canvas.getContext("webgpu");
    if (!context) throw createWebGPUUnavailableError("Canvas から WebGPU コンテキストを取得できない");
    this.#canvas = canvas;
    this.#context = context;
    this.#backend = backend;
    this.#uniformBuffer = backend.device.createBuffer({
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.#vpBindGroup = backend.device.createBindGroup({
      layout: backend.vpBindGroupLayout,
      entries: [{ binding: 0, resource: { buffer: this.#uniformBuffer } }],
    });
    backend.device.lost.then(() => {
      if (this.#destroyed || this.#lost) return;
      this.#lost = true;
      onLost?.();
    });
  }

  get device() { return this.#backend.device; }
  get lost() { return this.#lost; }

  resize(pixelsX, pixelsY, cssWidth, cssHeight, pixelRatio) {
    if (this.#destroyed) return;
    this.#cssWidth = cssWidth;
    this.#cssHeight = cssHeight;
    this.#pixelRatio = pixelRatio;
    if (this.#lost) {
      this.#width = pixelsX;
      this.#height = pixelsY;
      return;
    }
    const { device, preferredFormat } = this.#backend;
    device.queue.writeBuffer(this.#uniformBuffer, 0, new Float32Array([pixelsX, pixelsY, pixelRatio, 0]));
    if (this.#width === pixelsX && this.#height === pixelsY && this.#resolveColor !== null) return;
    this.#destroyAttachments();
    this.#width = pixelsX;
    this.#height = pixelsY;
    this.#context.configure({
      device,
      format: preferredFormat,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
      alphaMode: "premultiplied",
    });
    this.#msaaColor = device.createTexture({
      size: [pixelsX, pixelsY],
      sampleCount: SAMPLE_COUNT,
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    this.#msaaColorView = this.#msaaColor.createView();
    this.#msaaDepthStencil = device.createTexture({
      size: [pixelsX, pixelsY],
      sampleCount: SAMPLE_COUNT,
      format: "depth24plus-stencil8",
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    });
    this.#msaaDepthStencilView = this.#msaaDepthStencil.createView();
    this.#resolveColor = device.createTexture({
      size: [pixelsX, pixelsY],
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.#resolveColorView = this.#resolveColor.createView();
    this.#blitBindGroup = device.createBindGroup({
      layout: this.#backend.blitBindGroupLayout,
      entries: [
        { binding: 0, resource: this.#backend.nearestSampler },
        { binding: 1, resource: this.#resolveColorView },
      ],
    });
  }

  #ensureVerts(extraVerts) {
    const needed = (this.#vertCount + extraVerts) * FLOATS_PER_VERTEX;
    if (needed <= this.#verts.length) return;
    let nextCap = this.#verts.length * 2;
    while (nextCap < needed) nextCap *= 2;
    const next = new Float32Array(nextCap);
    next.set(this.#verts.subarray(0, this.#vertCount * FLOATS_PER_VERTEX));
    this.#verts = next;
  }

  #pushVertex(x, y, a0, a1, r, g, b, a) {
    const o = this.#vertCount * FLOATS_PER_VERTEX;
    const v = this.#verts;
    v[o] = x; v[o + 1] = y;
    v[o + 2] = a0; v[o + 3] = a1;
    v[o + 4] = r; v[o + 5] = g; v[o + 6] = b; v[o + 7] = a;
    this.#vertCount++;
  }

  #pushFullscreenTriangle(z) {
    this.#ensureVerts(3);
    const first = this.#vertCount;
    this.#pushVertex(-1, -1, z, 1, 0, 0, 0, 0);
    this.#pushVertex(3, -1, z, 1, 0, 0, 0, 0);
    this.#pushVertex(-1, 3, z, 1, 0, 0, 0, 0);
    return first;
  }

  #pushCoverQuad(minX, minY, maxX, maxY, color) {
    const pad = 2 / this.#pixelRatio;
    const x0 = Math.max(-pad, minX - pad);
    const y0 = Math.max(-pad, minY - pad);
    const x1 = Math.min(this.#cssWidth + pad, maxX + pad);
    const y1 = Math.min(this.#cssHeight + pad, maxY + pad);
    if (!(x1 > x0 && y1 > y0)) return -1;
    this.#ensureVerts(6);
    const first = this.#vertCount;
    const [r, g, b, a] = color;
    this.#pushVertex(x0, y0, 0.5, 0, r, g, b, a);
    this.#pushVertex(x1, y0, 0.5, 0, r, g, b, a);
    this.#pushVertex(x1, y1, 0.5, 0, r, g, b, a);
    this.#pushVertex(x0, y0, 0.5, 0, r, g, b, a);
    this.#pushVertex(x1, y1, 0.5, 0, r, g, b, a);
    this.#pushVertex(x0, y1, 0.5, 0, r, g, b, a);
    return first;
  }

  beginFrame() {
    this.#vertCount = 0;
    this.#commands.length = 0;
    this.#tempTextures.length = 0;
    this.#activeClip = false;
  }

  resetClip() {
    if (!this.#activeClip) return;
    const first = this.#pushFullscreenTriangle(1.0);
    this.#commands.push({ type: "clipReset", first, count: 3 });
    this.#activeClip = false;
  }

  applyClipPart(subpaths) {
    const firstFan = this.#vertCount;
    for (const sub of subpaths) {
      const coords = sub.coords;
      const n = coords.length >> 1;
      if (n < 3) continue;
      this.#ensureVerts((n - 2) * 3);
      const x0 = coords[0], y0 = coords[1];
      for (let i = 1; i < n - 1; i++) {
        this.#pushVertex(x0, y0, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(coords[i * 2], coords[i * 2 + 1], 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(coords[(i + 1) * 2], coords[(i + 1) * 2 + 1], 0.5, 0, 0, 0, 0, 0);
      }
    }
    const fanCount = this.#vertCount - firstFan;
    if (fanCount > 0) {
      this.#commands.push({ type: "clipStencil", first: firstFan, count: fanCount });
    }
    const maskFirst = this.#pushFullscreenTriangle(0.0);
    this.#commands.push({ type: "clipMaskOut", first: maskFirst, count: 3 });
    if (fanCount > 0) {
      this.#commands.push({ type: "stencilReset", first: maskFirst, count: 3 });
    }
    this.#activeClip = true;
  }

  drawFill(subpaths, arcs, rule, color) {
    if (color[3] <= 0) return;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const track = (x, y) => {
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    };
    const firstFan = this.#vertCount;
    for (const sub of subpaths) {
      const coords = sub.coords;
      const n = coords.length >> 1;
      if (n < 3) continue;
      this.#ensureVerts((n - 2) * 3);
      const x0 = coords[0], y0 = coords[1];
      track(x0, y0);
      for (let i = 1; i < n - 1; i++) {
        const x1 = coords[i * 2], y1 = coords[i * 2 + 1];
        const x2 = coords[(i + 1) * 2], y2 = coords[(i + 1) * 2 + 1];
        track(x1, y1); track(x2, y2);
        this.#pushVertex(x0, y0, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(x1, y1, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(x2, y2, 0.5, 0, 0, 0, 0, 0);
      }
    }
    for (const arc of arcs) {
      const r = arc.radius;
      if (!(r > 0)) continue;
      const segs = Math.max(16, Math.min(1024, Math.ceil(2 * Math.PI * Math.sqrt(r * this.#pixelRatio * 4))));
      this.#ensureVerts(segs * 3);
      track(arc.x - r, arc.y - r);
      track(arc.x + r, arc.y + r);
      for (let s = 0; s < segs; s++) {
        const a0 = (2 * Math.PI * s) / segs;
        const a1 = (2 * Math.PI * (s + 1)) / segs;
        this.#pushVertex(arc.x, arc.y, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(arc.x + r * Math.cos(a0), arc.y + r * Math.sin(a0), 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(arc.x + r * Math.cos(a1), arc.y + r * Math.sin(a1), 0.5, 0, 0, 0, 0, 0);
      }
    }
    const fanCount = this.#vertCount - firstFan;
    if (fanCount === 0) return;
    const coverFirst = this.#pushCoverQuad(minX, minY, maxX, maxY, color);
    if (coverFirst < 0) {
      this.#vertCount = firstFan;
      return;
    }
    this.#commands.push({
      type: rule === "evenodd" ? "fillEvenOdd" : "fillNonZero",
      first: firstFan,
      count: fanCount,
      coverFirst,
    });
  }

  drawStroke(subpaths, arcs, width, color) {
    if (color[3] <= 0 || !(width > 0)) return;
    const hw = width * 0.5;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const trackBox = (x0, y0, x1, y1) => {
      if (x0 < minX) minX = x0;
      if (y0 < minY) minY = y0;
      if (x1 > maxX) maxX = x1;
      if (y1 > maxY) maxY = y1;
    };
    const firstVert = this.#vertCount;
    const pr = this.#pixelRatio;

    for (const sub of subpaths) {
      const coords = sub.coords;
      const n = coords.length >> 1;
      if (n < 2) continue;
      const segCount = sub.closed ? n : n - 1;
      for (let i = 0; i < segCount; i++) {
        const j = (i + 1) % n;
        const ax = coords[i * 2], ay = coords[i * 2 + 1];
        const bx = coords[j * 2], by = coords[j * 2 + 1];
        const dx = bx - ax, dy = by - ay;
        const len = Math.hypot(dx, dy);
        if (!(len > 0)) continue;
        const nx = (-dy / len) * hw, ny = (dx / len) * hw;
        trackBox(Math.min(ax, bx) - hw, Math.min(ay, by) - hw, Math.max(ax, bx) + hw, Math.max(ay, by) + hw);
        // 中心線 (ax, ay)-(bx, by) で 2 分割し、端点の join 扇形と頂点を一致させて T-junction を防ぐ。
        this.#ensureVerts(12);
        this.#pushVertex(ax, ay, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(bx, by, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(bx + nx, by + ny, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(ax, ay, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(bx + nx, by + ny, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(ax + nx, ay + ny, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(ax, ay, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(ax - nx, ay - ny, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(bx - nx, by - ny, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(ax, ay, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(bx - nx, by - ny, 0.5, 0, 0, 0, 0, 0);
        this.#pushVertex(bx, by, 0.5, 0, 0, 0, 0, 0);
      }

      // round join: 外側の扇形だけを描き、短い線分の butt 端からはみ出させない（HTML Canvas lineJoin=round 準拠）。
      const joinStart = sub.closed ? 0 : 1;
      const joinEnd = sub.closed ? n : n - 1;
      for (let i = joinStart; i < joinEnd; i++) {
        const prev = (i + n - 1) % n;
        const next = (i + 1) % n;
        const vx = coords[i * 2], vy = coords[i * 2 + 1];
        const dx1 = vx - coords[prev * 2], dy1 = vy - coords[prev * 2 + 1];
        const dx2 = coords[next * 2] - vx, dy2 = coords[next * 2 + 1] - vy;
        const l1 = Math.hypot(dx1, dy1), l2 = Math.hypot(dx2, dy2);
        if (!(l1 > 0 && l2 > 0)) continue;
        const nx1 = (-dy1 / l1) * hw, ny1 = (dx1 / l1) * hw;
        const nx2 = (-dy2 / l2) * hw, ny2 = (dx2 / l2) * hw;
        const u1x = dx1 / l1, u1y = dy1 / l1;
        const u2x = dx2 / l2, u2y = dy2 / l2;
        const cross = u1x * u2y - u1y * u2x;
        const dot = u1x * u2x + u1y * u2y;
        if (Math.abs(cross) <= 1e-12 && dot >= 0) continue;
        let a0, sweep, startX, startY, endX, endY;
        if (Math.abs(cross) <= 1e-12) {
          // 180° 折返し：進行方向側の半円。端点は前後の矩形と完全に一致させる。
          a0 = Math.atan2(u1y, u1x) - Math.PI * 0.5;
          sweep = Math.PI;
          startX = vx - nx1; startY = vy - ny1;
          endX = vx + nx1; endY = vy + ny1;
        } else if (cross > 0) {
          a0 = Math.atan2(-u1x, u1y);
          const a1 = Math.atan2(-u2x, u2y);
          sweep = a1 - a0;
          if (sweep < 0) sweep += 2 * Math.PI;
          if (sweep > Math.PI + 1e-6) sweep = Math.PI;
          startX = vx - nx1; startY = vy - ny1;
          endX = vx - nx2; endY = vy - ny2;
        } else {
          a0 = Math.atan2(u1x, -u1y);
          const a1 = Math.atan2(u2x, -u2y);
          sweep = a1 - a0;
          if (sweep > 0) sweep -= 2 * Math.PI;
          if (sweep < -Math.PI - 1e-6) sweep = -Math.PI;
          startX = vx + nx1; startY = vy + ny1;
          endX = vx + nx2; endY = vy + ny2;
        }
        const tol = 1 / (32 * pr);
        const stepAngle = hw > tol ? 2 * Math.acos(Math.max(-1, 1 - tol / hw)) : Math.PI / 4;
        const steps = Math.max(2, Math.min(256, Math.ceil(Math.abs(sweep) / Math.max(stepAngle, 1e-3))));
        trackBox(vx - hw, vy - hw, vx + hw, vy + hw);
        this.#ensureVerts(steps * 3);
        let prevX = startX, prevY = startY;
        for (let s = 1; s <= steps; s++) {
          let nextX, nextY;
          if (s === steps) {
            nextX = endX; nextY = endY;
          } else {
            const t = a0 + (sweep * s) / steps;
            nextX = vx + hw * Math.cos(t);
            nextY = vy + hw * Math.sin(t);
          }
          this.#pushVertex(vx, vy, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(prevX, prevY, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(nextX, nextY, 0.5, 0, 0, 0, 0, 0);
          prevX = nextX; prevY = nextY;
        }
      }
    }

    for (const arc of arcs) {
      const rOut = arc.radius + hw;
      if (!(rOut > 0)) continue;
      const rIn = Math.max(0, arc.radius - hw);
      const tol = 1 / (32 * pr);
      const stepAngle = rOut > tol ? 2 * Math.acos(Math.max(-1, 1 - tol / rOut)) : Math.PI / 8;
      const segs = Math.max(16, Math.min(4096, Math.ceil((2 * Math.PI) / Math.max(stepAngle, 1e-4))));
      trackBox(arc.x - rOut, arc.y - rOut, arc.x + rOut, arc.y + rOut);
      if (rIn === 0) {
        this.#ensureVerts(segs * 3);
        for (let s = 0; s < segs; s++) {
          const a0 = (2 * Math.PI * s) / segs;
          const a1 = (2 * Math.PI * (s + 1)) / segs;
          this.#pushVertex(arc.x, arc.y, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(arc.x + rOut * Math.cos(a0), arc.y + rOut * Math.sin(a0), 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(arc.x + rOut * Math.cos(a1), arc.y + rOut * Math.sin(a1), 0.5, 0, 0, 0, 0, 0);
        }
      } else {
        this.#ensureVerts(segs * 6);
        for (let s = 0; s < segs; s++) {
          const a0 = (2 * Math.PI * s) / segs;
          const a1 = (2 * Math.PI * (s + 1)) / segs;
          const c0 = Math.cos(a0), s0 = Math.sin(a0);
          const c1 = Math.cos(a1), s1 = Math.sin(a1);
          const ox0 = arc.x + rOut * c0, oy0 = arc.y + rOut * s0;
          const ix0 = arc.x + rIn * c0, iy0 = arc.y + rIn * s0;
          const ox1 = arc.x + rOut * c1, oy1 = arc.y + rOut * s1;
          const ix1 = arc.x + rIn * c1, iy1 = arc.y + rIn * s1;
          this.#pushVertex(ix0, iy0, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(ox0, oy0, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(ox1, oy1, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(ix0, iy0, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(ox1, oy1, 0.5, 0, 0, 0, 0, 0);
          this.#pushVertex(ix1, iy1, 0.5, 0, 0, 0, 0, 0);
        }
      }
    }

    const strokeCount = this.#vertCount - firstVert;
    if (strokeCount === 0) return;
    const coverFirst = this.#pushCoverQuad(minX, minY, maxX, maxY, color);
    if (coverFirst < 0) {
      this.#vertCount = firstVert;
      return;
    }
    this.#commands.push({ type: "stroke", first: firstVert, count: strokeCount, coverFirst });
  }

  /**
   * [a, b, c, d, e, f] は (u in 0..1, v in 0..1) を CSS px へ写すアフィン変換:
   * x = a * u + c * v + e, y = b * u + d * v + f。
   */
  drawImageQuad(source, width, height, [a, b, c, d, e, f], alpha = 1, nearest = false, cacheKey = null) {
    if (this.#destroyed || this.#lost) return;
    const { device } = this.#backend;
    let entry = cacheKey !== null ? this.#underlayTextures.get(cacheKey) : null;
    if (!entry || entry.width !== width || entry.height !== height) {
      if (entry) entry.texture.destroy();
      const texture = device.createTexture({
        size: [width, height],
        format: "rgba8unorm",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
      });
      device.queue.copyExternalImageToTexture(
        { source },
        { texture },
        [width, height],
      );
      const bindGroup = device.createBindGroup({
        layout: this.#backend.texBindGroupLayout,
        entries: [
          { binding: 0, resource: nearest ? this.#backend.nearestSampler : this.#backend.linearSampler },
          { binding: 1, resource: texture.createView() },
        ],
      });
      entry = { texture, bindGroup, width, height };
      if (cacheKey !== null) {
        this.#underlayTextures.set(cacheKey, entry);
      } else {
        this.#tempTextures.push(texture);
      }
    }
    const p00x = e, p00y = f;
    const p10x = a + e, p10y = b + f;
    const p11x = a + c + e, p11y = b + d + f;
    const p01x = c + e, p01y = d + f;
    this.#ensureVerts(6);
    const first = this.#vertCount;
    this.#pushVertex(p00x, p00y, 0, 0, 1, 1, 1, alpha);
    this.#pushVertex(p10x, p10y, 1, 0, 1, 1, 1, alpha);
    this.#pushVertex(p11x, p11y, 1, 1, 1, 1, 1, alpha);
    this.#pushVertex(p00x, p00y, 0, 0, 1, 1, 1, alpha);
    this.#pushVertex(p11x, p11y, 1, 1, 1, 1, 1, alpha);
    this.#pushVertex(p01x, p01y, 0, 1, 1, 1, 1, alpha);
    this.#commands.push({ type: "textured", first, count: 6, bindGroup: entry.bindGroup });
  }

  endFrame(bgPremul) {
    if (this.#destroyed || this.#lost || !this.#resolveColor) {
      for (const tex of this.#tempTextures.splice(0)) tex.destroy();
      return;
    }
    const b = this.#backend;
    const { device } = b;
    const byteLength = this.#vertCount * BYTES_PER_VERTEX;
    if (byteLength > 0) {
      if (!this.#vertexBuffer || this.#vertexBufferCapacity < byteLength) {
        this.#vertexBuffer?.destroy();
        this.#vertexBufferCapacity = Math.max(65536, 1 << Math.ceil(Math.log2(byteLength)));
        this.#vertexBuffer = device.createBuffer({
          size: this.#vertexBufferCapacity,
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
      }
      device.queue.writeBuffer(this.#vertexBuffer, 0, this.#verts.buffer, 0, byteLength);
    }

    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: this.#msaaColorView,
        resolveTarget: this.#resolveColorView,
        clearValue: { r: bgPremul[0], g: bgPremul[1], b: bgPremul[2], a: bgPremul[3] },
        loadOp: "clear",
        storeOp: "discard",
      }],
      depthStencilAttachment: {
        view: this.#msaaDepthStencilView,
        depthClearValue: 1.0,
        depthLoadOp: "clear",
        depthStoreOp: "discard",
        stencilClearValue: 0,
        stencilLoadOp: "clear",
        stencilStoreOp: "discard",
      },
    });

    if (byteLength > 0 && this.#commands.length > 0) {
      pass.setBindGroup(0, this.#vpBindGroup);
      pass.setVertexBuffer(0, this.#vertexBuffer);
      for (const cmd of this.#commands) {
        switch (cmd.type) {
          case "clipStencil":
            pass.setPipeline(b.pipelineClipStencil);
            pass.draw(cmd.count, 1, cmd.first, 0);
            break;
          case "clipMaskOut":
            pass.setPipeline(b.pipelineClipMaskOut);
            pass.setStencilReference(0);
            pass.draw(3, 1, cmd.first, 0);
            break;
          case "stencilReset":
            pass.setPipeline(b.pipelineStencilReset);
            pass.setStencilReference(0);
            pass.draw(3, 1, cmd.first, 0);
            break;
          case "clipReset":
            pass.setPipeline(b.pipelineClipReset);
            pass.draw(3, 1, cmd.first, 0);
            break;
          case "fillEvenOdd":
            pass.setPipeline(b.pipelineStencilEvenOdd);
            pass.draw(cmd.count, 1, cmd.first, 0);
            pass.setPipeline(b.pipelineCover);
            pass.setStencilReference(0);
            pass.draw(6, 1, cmd.coverFirst, 0);
            break;
          case "fillNonZero":
            pass.setPipeline(b.pipelineStencilNonZero);
            pass.draw(cmd.count, 1, cmd.first, 0);
            pass.setPipeline(b.pipelineCover);
            pass.setStencilReference(0);
            pass.draw(6, 1, cmd.coverFirst, 0);
            break;
          case "stroke":
            pass.setPipeline(b.pipelineStencilStroke);
            pass.setStencilReference(1);
            pass.draw(cmd.count, 1, cmd.first, 0);
            pass.setPipeline(b.pipelineCover);
            pass.setStencilReference(0);
            pass.draw(6, 1, cmd.coverFirst, 0);
            break;
          case "textured":
            pass.setPipeline(b.pipelineTextured);
            pass.setBindGroup(1, cmd.bindGroup);
            pass.draw(cmd.count, 1, cmd.first, 0);
            break;
        }
      }
    }
    pass.end();

    const swapTex = this.#context.getCurrentTexture();
    const blitPass = encoder.beginRenderPass({
      colorAttachments: [{
        view: swapTex.createView(),
        loadOp: "clear",
        clearValue: { r: 0, g: 0, b: 0, a: 0 },
        storeOp: "store",
      }],
    });
    blitPass.setPipeline(b.pipelineBlit);
    blitPass.setBindGroup(0, this.#blitBindGroup);
    blitPass.draw(3, 1, 0, 0);
    blitPass.end();

    const temps = this.#tempTextures.splice(0);
    device.queue.submit([encoder.finish()]);
    if (temps.length > 0) {
      device.queue.onSubmittedWorkDone().then(() => {
        for (const tex of temps) tex.destroy();
      });
    }
  }

  async readPixelsAsync() {
    if (this.#destroyed) throw new Error("View2D は破棄済み");
    if (this.#lost || !this.#resolveColor) throw createContextLostError();
    const { device } = this.#backend;
    const width = this.#width, height = this.#height;
    const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
    const buffer = device.createBuffer({
      size: bytesPerRow * height,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    try {
      const encoder = device.createCommandEncoder();
      encoder.copyTextureToBuffer(
        { texture: this.#resolveColor },
        { buffer, bytesPerRow, rowsPerImage: height },
        [width, height, 1],
      );
      device.queue.submit([encoder.finish()]);
      try {
        await buffer.mapAsync(GPUMapMode.READ);
      } catch (error) {
        if (this.#destroyed) throw new Error("View2D は破棄済み");
        if (this.#lost) throw createContextLostError();
        throw error;
      }
      if (this.#destroyed) throw new Error("View2D は破棄済み");
      if (this.#lost) throw createContextLostError();
      const mapped = new Uint8Array(buffer.getMappedRange());
      const pixels = new Uint8Array(width * height * 4);
      const stride = width * 4;
      for (let row = 0; row < height; row++) {
        const src = row * bytesPerRow;
        const dst = row * stride;
        for (let i = 0; i < stride; i += 4) {
          const r = mapped[src + i];
          const g = mapped[src + i + 1];
          const b = mapped[src + i + 2];
          const a = mapped[src + i + 3];
          if (a === 255 || a === 0) {
            pixels[dst + i] = r;
            pixels[dst + i + 1] = g;
            pixels[dst + i + 2] = b;
            pixels[dst + i + 3] = a;
          } else {
            pixels[dst + i] = Math.min(255, Math.round((r * 255) / a));
            pixels[dst + i + 1] = Math.min(255, Math.round((g * 255) / a));
            pixels[dst + i + 2] = Math.min(255, Math.round((b * 255) / a));
            pixels[dst + i + 3] = a;
          }
        }
      }
      buffer.unmap();
      return Object.freeze({ width, height, pixelRatio: this.#pixelRatio, pixels });
    } finally {
      buffer.destroy();
    }
  }

  releaseUnderlayTexture(cacheKey) {
    const entry = this.#underlayTextures.get(cacheKey);
    if (entry) {
      this.#underlayTextures.delete(cacheKey);
      entry.texture.destroy();
    }
  }

  #destroyAttachments() {
    this.#msaaColor?.destroy();
    this.#msaaColor = null;
    this.#msaaColorView = null;
    this.#msaaDepthStencil?.destroy();
    this.#msaaDepthStencil = null;
    this.#msaaDepthStencilView = null;
    this.#resolveColor?.destroy();
    this.#resolveColor = null;
    this.#resolveColorView = null;
    this.#blitBindGroup = null;
  }

  destroy() {
    if (this.#destroyed) return;
    this.#destroyed = true;
    try {
      if (!this.#lost && this.#width > 0 && this.#height > 0) {
        const { device } = this.#backend;
        const encoder = device.createCommandEncoder();
        const pass = encoder.beginRenderPass({
          colorAttachments: [{
            view: this.#context.getCurrentTexture().createView(),
            clearValue: { r: 0, g: 0, b: 0, a: 0 },
            loadOp: "clear",
            storeOp: "store",
          }],
        });
        pass.end();
        device.queue.submit([encoder.finish()]);
      }
    } catch { /* Canvas が無効化済みでも後始末を続ける */ }
    try { this.#context.unconfigure(); } catch { /* unconfigure 失敗は無視 */ }
    for (const tex of this.#tempTextures.splice(0)) tex.destroy();
    for (const entry of this.#underlayTextures.values()) entry.texture.destroy();
    this.#underlayTextures.clear();
    this.#destroyAttachments();
    this.#vertexBuffer?.destroy();
    this.#vertexBuffer = null;
    this.#uniformBuffer?.destroy();
    this.#uniformBuffer = null;
  }
}

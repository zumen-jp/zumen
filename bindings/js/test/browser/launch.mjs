// Playwright の Chromium で WebGPU を動かす起動設定。
// Linux のヘッドレス実行では同梱の vk_swiftshader_icd.json を VK_ICD_FILENAMES に指定し、
// Chromium の Vulkan（SharedImage）と Dawn のアダプターを同じ SwiftShader にそろえる。
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";

function linuxSwiftShaderIcd(launcher) {
  const exe = launcher.executablePath();
  const direct = join(dirname(exe), "vk_swiftshader_icd.json");
  if (existsSync(direct)) return direct;
  const shell = exe.replace(
    /\/chromium-(\d+)\/chrome-linux([^/]*)\/[^/]+$/,
    "/chromium_headless_shell-$1/chrome-headless-shell-linux$2/vk_swiftshader_icd.json",
  );
  return shell !== exe && existsSync(shell) ? shell : undefined;
}

export function chromiumLaunchOptions(launcher, channel) {
  const icd = process.platform === "linux" && !channel ? linuxSwiftShaderIcd(launcher) : undefined;
  return {
    ...(channel ? { channel } : {}),
    ...(icd ? { env: { ...process.env, VK_ICD_FILENAMES: icd } } : {}),
    ignoreDefaultArgs: ["--disable-gpu"],
    args: [
      "--enable-gpu",
      "--enable-unsafe-webgpu",
      "--ignore-gpu-blocklist",
      ...(process.platform === "win32" ? ["--use-webgpu-adapter=d3d11"] : []),
      ...(process.platform === "linux"
        ? [
            "--enable-features=Vulkan",
            "--use-angle=vulkan",
            "--use-vulkan=swiftshader",
            "--use-webgpu-adapter=swiftshader",
            "--enable-unsafe-swiftshader",
            "--disable-vulkan-surface",
          ]
        : []),
    ],
  };
}

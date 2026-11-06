// open は、上限を確保前に確認して、所有権を渡せる復号済みの画像を返す利用側の関数。
export function imageUnderlayRenderer(open, createCanvas) {
  return async (request) => {
    if (request.kind !== "image") throw new Error("画像ではありません");
    const image = await open(request), source = image.source;
    try {
      if (request.signal.aborted) throw new Error("中断しました");
      if (!Number.isSafeInteger(source.width) || !Number.isSafeInteger(source.height) || source.width <= 0 || source.height <= 0 || source.width * source.height > request.limits.maxBytes / 4) throw new RangeError("maxBytes");
      return {
        width: source.width, height: source.height,
        render(value) {
          if (value.signal.aborted) throw new Error("中断しました");
          const target = createCanvas(value.width, value.height), region = value.region;
          target.width = value.width; target.height = value.height;
          try {
            target.getContext("2d").drawImage(source, region.min_x, source.height - region.max_y, region.max_x - region.min_x, region.max_y - region.min_y, 0, 0, value.width, value.height);
            return { source: target, dispose() { target.width = 0; target.height = 0; } };
          } catch (error) { target.width = 0; target.height = 0; throw error; }
        },
        dispose() { return image.dispose(); },
      };
    } catch (error) { await image.dispose(); throw error; }
  };
}

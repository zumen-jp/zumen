import { useLayoutEffect, useState } from "react";
import { View2D } from "@zumen-jp/view2d";
export { useDrawing } from "./drawing.js";
export { useViewerEvent, useViewerState } from "./store.js";

const emptyOptions = Object.freeze({});

/** Canvas と設定を所有する間だけ有効な表示部品。再描画では同じ設定の参照を渡す。 */
export function useDrawingViewer(canvas, options = emptyOptions) {
  const [owner, setOwner] = useState(null);
  useLayoutEffect(() => {
    if (!canvas) { setOwner(null); return; }
    const viewer = new View2D(canvas, options);
    setOwner({ canvas, options, viewer });
    return () => {
      // Activity と Suspense は state を保ったまま layout effect を切り離す。
      setOwner(null);
      viewer.destroy();
    };
  }, [canvas, options]);
  return owner && owner.canvas === canvas && owner.options === options ? owner.viewer : null;
}

import { useLayoutEffect, useState } from "react";
import { ReviewLayer, View2D } from "@zumen-jp/view2d";
export { useDrawing } from "./drawing.js";
export { useViewerEvent, useViewerState } from "./store.js";
export { useViewerEvent as useReviewEvent, useViewerState as useReviewState } from "./store.js";

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

/** 図面と重ねる表示部品が揃う間だけ有効な ReviewLayer。store は同じ参照を渡す。 */
export function useReviewLayer(base, overlay, store = null) {
  const [owner, setOwner] = useState(null);
  useLayoutEffect(() => {
    if (!base || !overlay) { setOwner(null); return; }
    const layer = new ReviewLayer(base, overlay, store);
    setOwner({ base, overlay, store, layer });
    return () => { setOwner(null); layer.destroy(); };
  }, [base, overlay, store]);
  return owner && owner.base === base && owner.overlay === overlay && owner.store === store ? owner.layer : null;
}

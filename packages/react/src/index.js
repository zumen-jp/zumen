import { useLayoutEffect, useRef, useState } from "react";
import { ReviewLayer, View2D } from "@zumen-jp/view2d";
export { useDrawing } from "./drawing.js";
export { useViewerEvent, useViewerState } from "./store.js";
export { useViewerEvent as useReviewEvent, useViewerState as useReviewState } from "./store.js";

const emptyOptions = Object.freeze({});

/** Canvas と設定を所有する間だけ有効な表示部品。再描画では同じ設定の参照を渡す。 */
export function useDrawingViewer(canvas, options = emptyOptions, onError) {
  const [owner, setOwner] = useState(null);
  const errorRef = useRef(onError);
  errorRef.current = onError;
  useLayoutEffect(() => {
    if (!canvas) { setOwner(null); return; }
    let viewer = null, cancelled = false;
    Promise.resolve().then(() => {
      if (cancelled) return null;
      return View2D.create(canvas, options);
    }).then(
      (created) => {
        if (!created) return;
        if (cancelled) { created.destroy(); return; }
        viewer = created;
        setOwner({ canvas, options, viewer });
      },
      (error) => {
        if (cancelled) return;
        setOwner(null);
        errorRef.current?.(error instanceof Error ? error : new Error(String(error)));
      },
    );
    return () => {
      // Activity と Suspense は state を保ったまま layout effect を切り離す。GPU の資源もここで放す。
      cancelled = true;
      setOwner(null);
      viewer?.destroy();
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

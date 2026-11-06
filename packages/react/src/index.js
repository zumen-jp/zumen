import { useCallback, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { View2D } from "@zumen-jp/view2d";
export { useDrawing } from "./drawing.js";

const emptyOptions = Object.freeze({});
const empty = () => null;
const noop = () => {};

/** Canvas と設定を所有する間だけ有効な表示部品。再描画では同じ設定の参照を渡す。 */
export function useDrawingViewer(canvas, options = emptyOptions) {
  const [owner, setOwner] = useState(null);
  useLayoutEffect(() => {
    if (!canvas) { setOwner(null); return; }
    const viewer = new View2D(canvas, options);
    setOwner({ canvas, options, viewer });
    return () => viewer.destroy();
  }, [canvas, options]);
  return owner && owner.canvas === canvas && owner.options === options ? owner.viewer : null;
}

export function useViewerState(viewer) {
  const subscribe = useCallback((notify) => viewer ? viewer.subscribe(notify) : noop, [viewer]);
  const read = useCallback(() => viewer?.getState() ?? null, [viewer]);
  return useSyncExternalStore(subscribe, read, empty);
}

export function useViewerEvent(viewer, type, listener) {
  const latest = useRef(listener);
  useLayoutEffect(() => { latest.current = listener; }, [listener]);
  useLayoutEffect(() => viewer?.on(type, (event) => latest.current(event)), [viewer, type]);
}

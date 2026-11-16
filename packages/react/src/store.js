import { useCallback, useLayoutEffect, useRef, useSyncExternalStore } from "react";

const empty = () => null;
const noop = () => {};

/** 表示部品（2D・3D）の getState と subscribe を購読する。 */
export function useViewerState(viewer) {
  const subscribe = useCallback((notify) => viewer ? viewer.subscribe(notify) : noop, [viewer]);
  const read = useCallback(() => viewer?.getState() ?? null, [viewer]);
  return useSyncExternalStore(subscribe, read, empty);
}

/** 表示部品の on で種類を絞って受け取る。再描画後のコールバックを使う。 */
export function useViewerEvent(viewer, type, listener) {
  const latest = useRef(listener);
  useLayoutEffect(() => { latest.current = listener; }, [listener]);
  useLayoutEffect(() => viewer?.on(type, (event) => latest.current(event)), [viewer, type]);
}

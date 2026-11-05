import { valid } from "./validate.js";

/** Pointer Events §8/9。touch-action とフォーカスの指定は利用側の CSS/HTML に置く。 */
export function bindInput(viewer, element) {
  valid(element && typeof element.addEventListener === "function" && typeof element.removeEventListener === "function" && typeof element.getBoundingClientRect === "function", "input.element");
  const listeners = [], captures = new Set(), document = element.ownerDocument, window = document?.defaultView;
  let connected = true;
  const on = (target, type, callback, options) => {
    if (!target) return;
    target.addEventListener(type, callback, options); listeners.push(() => target.removeEventListener(type, callback, options));
  };
  const modifiers = (event) => ({ shiftKey: event.shiftKey, ctrlKey: event.ctrlKey, altKey: event.altKey, metaKey: event.metaKey });
  const editable = (event) => {
    for (let target = event.target; target && target !== element; target = target.parentElement) {
      if (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(target.tagName)) return true;
    }
    return false;
  };
  function point(event) {
    const rect = element.getBoundingClientRect(), size = viewer.getSize(), style = window?.getComputedStyle(element);
    const number = (name) => Number.parseFloat(style?.[name] ?? "0") || 0;
    const left = number("borderLeftWidth") + number("paddingLeft"), right = number("borderRightWidth") + number("paddingRight");
    const top = number("borderTopWidth") + number("paddingTop"), bottom = number("borderBottomWidth") + number("paddingBottom");
    const cssWidth = number("width"), cssHeight = number("height");
    const outerWidth = cssWidth > 0 ? cssWidth + (style.boxSizing === "border-box" ? 0 : left + right) : rect.width;
    const outerHeight = cssHeight > 0 ? cssHeight + (style.boxSizing === "border-box" ? 0 : top + bottom) : rect.height;
    const contentWidth = outerWidth - left - right, contentHeight = outerHeight - top - bottom;
    valid(rect.width > 0 && rect.height > 0 && contentWidth > 0 && contentHeight > 0, "input.size");
    return { x: ((event.clientX - rect.left) * outerWidth / rect.width - left) * size.width / contentWidth,
      y: ((event.clientY - rect.top) * outerHeight / rect.height - top) * size.height / contentHeight };
  }
  function release(id) {
    if (!captures.delete(id)) return;
    try { if (element.hasPointerCapture?.(id)) element.releasePointerCapture(id); }
    catch (error) { if (!["NotFoundError", "InvalidStateError"].includes(error.name)) throw error; }
  }
  function cancel(reason) { if (connected) viewer.cancelInput(reason); }
  function send(event, input) {
    try {
      const handled = viewer.handleInput(input);
      if (handled && event.cancelable) event.preventDefault();
      return handled;
    } catch (error) {
      try { viewer.cancelInput("error"); } catch { /* 元の例外を保ち、後始末は cancelInput が最後まで行う。 */ }
      throw error;
    }
  }
  for (const type of ["pointerdown", "pointermove", "pointerup"]) on(element, type, (event) => {
    if (editable(event)) return;
    const input = { type, pointerId: event.pointerId, pointerType: event.pointerType || "mouse", button: event.button, point: point(event), ...modifiers(event) };
    if (type === "pointerdown" && (input.point.x < 0 || input.point.y < 0 || input.point.x > viewer.getSize().width || input.point.y > viewer.getSize().height)) return;
    const handled = send(event, input);
    if (type === "pointerdown" && handled && element.setPointerCapture) {
      try { element.setPointerCapture(event.pointerId); captures.add(event.pointerId); }
      catch (error) { if (!["NotFoundError", "InvalidStateError"].includes(error.name)) throw error; }
    }
    if (type === "pointerup") release(event.pointerId);
  }, { passive: false });
  on(element, "pointercancel", () => cancel("pointercancel"));
  on(element, "lostpointercapture", (event) => { if (captures.has(event.pointerId)) cancel("lostpointercapture"); });
  on(element, "wheel", (event) => {
    if (editable(event)) return;
    // UI Events 2026-02-10 §3.5: deltaMode は px・行・ページ。行は表示用 16 CSS px。
    const multiplier = event.deltaMode === 0 ? 1 : event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? viewer.getSize().height : null;
    if (multiplier !== null) send(event, { type: "wheel", point: point(event), deltaY: event.deltaY * multiplier, ...modifiers(event) });
  }, { passive: false });
  on(element, "keydown", (event) => {
    if (!event.isComposing && !editable(event)) send(event, { type: "keydown", key: event.key, ...modifiers(event) });
  });
  on(element, "dblclick", (event) => { if (!editable(event)) send(event, { type: "dblclick", point: point(event), ...modifiers(event) }); });
  on(element, "blur", () => cancel("blur"));
  on(window, "blur", () => cancel("blur"));
  on(document, "visibilitychange", () => { if (document.hidden) cancel("hidden"); });
  return {
    release() {
      let failed = false, first;
      for (const id of [...captures]) {
        try { release(id); } catch (error) { if (!failed) { failed = true; first = error; } }
      }
      if (failed) throw first;
    },
    disconnect() { if (!connected) return; connected = false; for (const off of listeners) off(); listeners.length = 0; this.release(); },
  };
}

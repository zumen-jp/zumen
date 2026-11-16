// 表示部品の各部で使う例外と、検査・ベクトルの小さな関数。
import * as THREE from "three";

/** 表示部品の失敗。reason で種類を区別する。 */
export class View3DError extends Error {
  constructor(reason, message, options) {
    super(message, options);
    this.name = "View3DError";
    this.reason = reason;
  }
}
export const fail = (reason, message, cause) => { throw new View3DError(reason, message, cause === undefined ? undefined : { cause }); };
export const invalid = (message) => fail("invalid-input", message);

export function isObject(value) { return typeof value === "object" && value !== null && !Array.isArray(value); }
export function finite(value, message) { if (typeof value !== "number" || !Number.isFinite(value)) invalid(message); return value; }
export function positive(value, message) { if (finite(value, message) <= 0) invalid(message); return value; }
export function vector(value, message) {
  if (!Array.isArray(value) && !ArrayBuffer.isView(value)) invalid(message);
  if (value.length !== 3) invalid(message);
  return [finite(value[0], message), finite(value[1], message), finite(value[2], message)];
}
export function color(value, message) {
  if (typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 0xffffff) return new THREE.Color(value);
  if (typeof value !== "string" || !/^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(value)) invalid(message);
  const hex = value.length === 4 ? value.slice(1).split("").map((c) => c + c).join("") : value.slice(1);
  return new THREE.Color(Number.parseInt(hex, 16));
}
export const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
export const length = (a) => Math.hypot(a[0], a[1], a[2]);
export const scale = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
export const normalize = (a) => scale(a, 1 / length(a));

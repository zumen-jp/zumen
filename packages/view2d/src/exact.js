const floatBits = new DataView(new ArrayBuffer(8));

/** 有限な binary64 を整数と 2 の指数へ分ける。差と積は数千 bit の範囲に収まる。 */
export function dyadic(value) {
  floatBits.setFloat64(0, value);
  const high = floatBits.getUint32(0), low = floatBits.getUint32(4), exponent = high >>> 20 & 2047;
  let integer = BigInt(high & 0xfffff) << 32n | BigInt(low);
  if (exponent) integer |= 1n << 52n;
  return { integer: high >>> 31 ? -integer : integer, exponent: exponent ? exponent - 1075 : -1074 };
}

export function addExact(a, b, subtract = false) {
  const exponent = Math.min(a.exponent, b.exponent);
  const left = a.integer << BigInt(a.exponent - exponent), right = b.integer << BigInt(b.exponent - exponent);
  return { integer: subtract ? left - right : left + right, exponent };
}

export const multiplyExact = (a, b) => ({ integer: a.integer * b.integer, exponent: a.exponent + b.exponent });

/** 有理数を最後に一度だけ binary64 へ丸める（最も近い値、同距離なら偶数）。 */
export function divideExact(a, b) {
  if (b.integer === 0n) return NaN;
  const negative = (a.integer < 0n) !== (b.integer < 0n), n = a.integer < 0n ? -a.integer : a.integer, d = b.integer < 0n ? -b.integer : b.integer;
  if (n === 0n) return 0;
  let magnitude = n.toString(2).length - d.toString(2).length;
  if (magnitude >= 0 ? n < (d << BigInt(magnitude)) : (n << BigInt(-magnitude)) < d) magnitude--;
  const exponent = magnitude + a.exponent - b.exponent;
  if (exponent > 1023) return negative ? -Infinity : Infinity;
  const unit = Math.max(-1074, exponent - 52), shift = a.exponent - b.exponent - unit;
  const numerator = shift >= 0 ? n << BigInt(shift) : n, denominator = shift < 0 ? d << BigInt(-shift) : d;
  let quotient = numerator / denominator;
  const twiceRemainder = (numerator % denominator) * 2n;
  if (twiceRemainder > denominator || (twiceRemainder === denominator && (quotient & 1n))) quotient++;
  const result = scaleBinary(Number(quotient), unit);
  return negative ? -result : result;
}

export function scaleBinary(value, exponent) {
  if (value === 0) return 0;
  // 中間の 2 の累乗だけが桁あふれ・underflow するのを避ける。
  const first = Math.max(-1022, Math.min(1023, exponent));
  return value * 2 ** first * 2 ** (exponent - first);
}

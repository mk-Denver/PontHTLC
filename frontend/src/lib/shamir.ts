const GF_POLY = 0x1b;

export const SHARE_X_BUYER = 1;
export const SHARE_X_SELLER = 2;
export const SHARE_X_ARBITRATOR = 3;

export type Share = {
  x: number;
  bytes: Uint8Array;
};

export function gfMul(aIn: number, bIn: number): number {
  let a = aIn & 0xff;
  let b = bIn & 0xff;
  let product = 0;
  while (b !== 0) {
    if ((b & 1) !== 0) {
      product ^= a;
    }
    const highBit = a & 0x80;
    a = (a << 1) & 0xff;
    if (highBit !== 0) {
      a ^= GF_POLY;
    }
    b >>= 1;
  }
  return product;
}

export function gfPow(baseIn: number, exponentIn: number): number {
  let base = baseIn & 0xff;
  let exponent = exponentIn & 0xff;
  let accumulator = 1;
  while (exponent > 0) {
    if ((exponent & 1) !== 0) {
      accumulator = gfMul(accumulator, base);
    }
    base = gfMul(base, base);
    exponent >>= 1;
  }
  return accumulator;
}

export function gfInv(value: number): number {
  if ((value & 0xff) === 0) {
    throw new Error("gf_inv: zero has no inverse in GF(256)");
  }
  return gfPow(value, 254);
}

export function evalPoly(coeffs: [number, number], x: number): number {
  let value = coeffs[0] & 0xff;
  let xPower = 1;
  for (let i = 1; i < coeffs.length; i++) {
    xPower = gfMul(xPower, x);
    value ^= gfMul(coeffs[i], xPower);
  }
  return value;
}

export function combineShares(left: Share, right: Share): Uint8Array {
  const denom = (left.x ^ right.x) & 0xff;
  const denomInv = gfInv(denom);
  const secret = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    const termA = gfMul(left.bytes[i], gfMul(right.x, denomInv));
    const termB = gfMul(right.bytes[i], gfMul(left.x, denomInv));
    secret[i] = termA ^ termB;
  }
  return secret;
}

export function splitPreimageWithSlopes(preimage: Uint8Array, slopes: Uint8Array): [Share, Share, Share] {
  if (preimage.length !== 32 || slopes.length !== 32) {
    throw new Error("preimage and slopes must be 32 bytes");
  }
  const xs = [SHARE_X_BUYER, SHARE_X_SELLER, SHARE_X_ARBITRATOR];
  const shares: Share[] = xs.map((x) => ({ x, bytes: new Uint8Array(32) }));
  for (let i = 0; i < 32; i++) {
    const coeffs: [number, number] = [preimage[i], slopes[i]];
    for (let j = 0; j < shares.length; j++) {
      shares[j].bytes[i] = evalPoly(coeffs, xs[j]);
    }
  }
  return [shares[0], shares[1], shares[2]];
}

export function splitPreimage(preimage: Uint8Array): [Share, Share, Share] {
  return splitPreimageWithSlopes(preimage, crypto.getRandomValues(new Uint8Array(32)));
}

export function reconstructAndVerify(
  left: Share,
  right: Share,
  expectedHash: Uint8Array,
  digest: (bytes: Uint8Array) => Uint8Array,
): Uint8Array | null {
  const preimage = combineShares(left, right);
  const hash = digest(preimage);
  if (hash.length !== expectedHash.length) {
    return null;
  }
  for (let i = 0; i < hash.length; i++) {
    if (hash[i] !== expectedHash[i]) {
      return null;
    }
  }
  return preimage;
}

export function shareToJson(share: Share): { x: number; bytes: string } {
  return {
    x: share.x,
    bytes: Array.from(share.bytes, (byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

export function shareFromJson(value: { x: number; bytes: string }): Share {
  const clean = value.bytes.trim().toLowerCase();
  if (value.x < 1 || value.x > 255 || !/^[0-9a-f]{64}$/.test(clean)) {
    throw new Error("invalid share");
  }
  const bytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    bytes[i] = Number.parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return { x: value.x, bytes };
}

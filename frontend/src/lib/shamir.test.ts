import { sha256 } from "@noble/hashes/sha2.js";
import { describe, expect, it } from "vitest";
import {
  combineShares,
  evalPoly,
  gfInv,
  gfMul,
  gfPow,
  reconstructAndVerify,
  SHARE_X_ARBITRATOR,
  SHARE_X_BUYER,
  SHARE_X_SELLER,
  splitPreimage,
  splitPreimageWithSlopes,
} from "./shamir";

function vectorPreimage(seed: number): Uint8Array {
  const preimage = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    preimage[i] = (seed + i) & 0xff;
  }
  return preimage;
}

function vectorSlopes(seed: number): Uint8Array {
  const slopes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    slopes[i] = Math.max(1, ((seed * 3) + i) & 0xff);
  }
  return slopes;
}

describe("GF(2^8) field", () => {
  it("gf_mul identity", () => {
    for (let a = 1; a <= 255; a++) {
      expect(gfMul(a, 1)).toBe(a);
    }
  });

  it("gf_mul zero", () => {
    for (let a = 0; a <= 255; a++) {
      expect(gfMul(a, 0)).toBe(0);
    }
  });

  it("gf_mul commutative", () => {
    for (let a = 0; a <= 255; a++) {
      for (let b = 0; b < 255; b++) {
        expect(gfMul(a, b)).toBe(gfMul(b, a));
      }
    }
  });

  it("gf_mul associative", () => {
    for (let a = 1; a <= 255; a += 7) {
      for (let b = 1; b <= 255; b += 9) {
        for (let c = 1; c <= 255; c += 11) {
          expect(gfMul(gfMul(a, b), c)).toBe(gfMul(a, gfMul(b, c)));
        }
      }
    }
  });

  it("gf_inv identity", () => {
    for (let a = 1; a <= 255; a++) {
      expect(gfMul(a, gfInv(a))).toBe(1);
    }
  });

  it("gf_inv zero throws", () => {
    expect(() => gfInv(0)).toThrow(/zero has no inverse/);
  });

  it("eval_poly at zero", () => {
    expect(evalPoly([42, 99], 0)).toBe(42);
  });

  it("eval_poly at one", () => {
    expect(evalPoly([10, 20], 1)).toBe(10 ^ 20);
  });

  it("gf_pow identities", () => {
    expect(gfPow(5, 0)).toBe(1);
    expect(gfPow(5, 1)).toBe(5);
    expect(gfPow(5, 2)).toBe(gfMul(5, 5));
  });
});

describe("2-of-3 Shamir", () => {
  it("split then combine all pairs", () => {
    const preimage = new Uint8Array(32).fill(0x42);
    const [s1, s2, s3] = splitPreimage(preimage);
    expect(s1.x).toBe(SHARE_X_BUYER);
    expect(s2.x).toBe(SHARE_X_SELLER);
    expect(s3.x).toBe(SHARE_X_ARBITRATOR);
    expect(combineShares(s1, s2)).toEqual(preimage);
    expect(combineShares(s1, s3)).toEqual(preimage);
    expect(combineShares(s2, s3)).toEqual(preimage);
  });

  it("reconstruct and verify hash", () => {
    const preimage = new Uint8Array(32).fill(0xab);
    const hash = sha256(preimage);
    const [s1, s2] = splitPreimage(preimage);
    expect(reconstructAndVerify(s1, s2, hash, sha256)).toEqual(preimage);
  });

  it("mismatched shares fail the hash check", () => {
    const preimageA = new Uint8Array(32).fill(0xaa);
    const preimageB = new Uint8Array(32).fill(0xbb);
    const hashA = sha256(preimageA);
    const [s1] = splitPreimage(preimageA);
    const [, s2] = splitPreimage(preimageB);
    expect(reconstructAndVerify(s1, s2, hashA, sha256)).toBeNull();
  });

  it("matches rust split_preimage_with_slopes for 5 seeds", () => {
    for (const seed of [0x11, 0x42, 0xab, 0x5a, 0xfe]) {
      const preimage = vectorPreimage(seed);
      const slopes = vectorSlopes(seed);
      const [s1, s2, s3] = splitPreimageWithSlopes(preimage, slopes);
      expect(s1.bytes[0]).toBe(preimage[0] ^ slopes[0]);
      expect(s2.bytes[0]).toBe(preimage[0] ^ gfMul(slopes[0], 2));
      expect(combineShares(s1, s2)).toEqual(preimage);
      expect(combineShares(s1, s3)).toEqual(preimage);
      expect(combineShares(s2, s3)).toEqual(preimage);
    }
  });

  it("random preimage all pairs reconstruct", () => {
    const preimage = crypto.getRandomValues(new Uint8Array(32));
    const [s1, s2, s3] = splitPreimage(preimage);
    expect(combineShares(s1, s2)).toEqual(preimage);
    expect(combineShares(s1, s3)).toEqual(preimage);
    expect(combineShares(s2, s3)).toEqual(preimage);
  });
});

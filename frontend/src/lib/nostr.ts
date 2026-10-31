import { schnorr } from "@noble/curves/secp256k1.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, hexToBytes, randomBytes } from "./hex";
import { pubkeyFromSecret } from "./nip04";

export type NostrEvent = {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
};

export const KIND_GIFT_WRAP = 1059;
export const KIND_SEAL = 13;
export const KIND_SWAP_PAYLOAD = 7310;
export const SWAP_PROFILE = "pontmore/swap@1";

export function generateKeypair(): { secret: Uint8Array; pubkey: string } {
  const secret = randomBytes(32);
  return { secret, pubkey: pubkeyFromSecret(secret) };
}

function canonicalJson(
  pubkey: string,
  createdAt: number,
  kind: number,
  tags: string[][],
  content: string,
): string {
  const tagsJson = tags
    .map((tag) => `[${tag.map((item) => JSON.stringify(item)).join(",")}]`)
    .join(",");
  return `[0,"${pubkey}",${createdAt},${kind},[${tagsJson}],${JSON.stringify(content)}]`;
}

export function eventId(event: Omit<NostrEvent, "id" | "sig">): Uint8Array {
  return sha256(new TextEncoder().encode(
    canonicalJson(event.pubkey, event.created_at, event.kind, event.tags, event.content),
  ));
}

export function signEvent(
  secret: Uint8Array,
  unsigned: { created_at: number; kind: number; tags: string[][]; content: string },
): NostrEvent {
  const pubkey = pubkeyFromSecret(secret);
  const body = { pubkey, ...unsigned };
  const id = eventId(body);
  const sig = schnorr.sign(id, secret);
  return {
    id: bytesToHex(id),
    pubkey,
    created_at: unsigned.created_at,
    kind: unsigned.kind,
    tags: unsigned.tags,
    content: unsigned.content,
    sig: bytesToHex(sig),
  };
}

export function verifyEvent(event: NostrEvent): boolean {
  try {
    const id = eventId(event);
    if (bytesToHex(id) !== event.id) {
      return false;
    }
    return schnorr.verify(hexToBytes(event.sig), id, hexToBytes(event.pubkey));
  } catch {
    return false;
  }
}

export function now(): number {
  return Math.floor(Date.now() / 1000);
}

import { schnorr, secp256k1 } from "@noble/curves/secp256k1.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, hexToBytes } from "./hex";

function xonlyToCompressed(xonlyHex: string): Uint8Array {
  const xonly = hexToBytes(xonlyHex);
  if (xonly.length !== 32) {
    throw new Error("expected 32-byte x-only pubkey");
  }
  const compressed = new Uint8Array(33);
  compressed[0] = 0x02;
  compressed.set(xonly, 1);
  return compressed;
}

function deriveKey(peerPkHex: string, ourSk: Uint8Array): Uint8Array {
  const compressed = xonlyToCompressed(peerPkHex);
  const point = secp256k1.getSharedSecret(ourSk, compressed, false);
  return sha256(point.slice(1, 33));
}

function bytesToB64(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function b64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

function asBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

async function importAes(key: Uint8Array): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", asBuffer(key), { name: "AES-CBC" }, false, ["encrypt", "decrypt"]);
}

export async function encrypt(
  senderSk: Uint8Array,
  recipientPkHex: string,
  plaintext: Uint8Array,
): Promise<string> {
  const key = deriveKey(recipientPkHex, senderSk);
  const iv = crypto.getRandomValues(new Uint8Array(16));
  const cryptoKey = await importAes(key);
  const encrypted = new Uint8Array(
    await crypto.subtle.encrypt({ name: "AES-CBC", iv: asBuffer(iv) }, cryptoKey, asBuffer(plaintext)),
  );
  return `${bytesToB64(encrypted)}?iv=${bytesToB64(iv)}`;
}

export async function decrypt(
  recipientSk: Uint8Array,
  senderPkHex: string,
  payload: string,
): Promise<Uint8Array> {
  const split = payload.split("?iv=");
  if (split.length !== 2) {
    throw new Error("missing ?iv= separator");
  }
  const key = deriveKey(senderPkHex, recipientSk);
  const ciphertext = b64ToBytes(split[0]);
  const iv = b64ToBytes(split[1]);
  if (iv.length !== 16) {
    throw new Error("iv must be 16 bytes");
  }
  const cryptoKey = await importAes(key);
  return new Uint8Array(
    await crypto.subtle.decrypt({ name: "AES-CBC", iv: asBuffer(iv) }, cryptoKey, asBuffer(ciphertext)),
  );
}

export function pubkeyFromSecret(secret: Uint8Array): string {
  return bytesToHex(schnorr.getPublicKey(secret));
}

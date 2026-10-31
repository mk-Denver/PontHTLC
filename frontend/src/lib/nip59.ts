import { hexToBytes, isHex64 } from "./hex";
import { decrypt, encrypt, pubkeyFromSecret } from "./nip04";
import {
  generateKeypair,
  KIND_GIFT_WRAP,
  KIND_SEAL,
  KIND_SWAP_PAYLOAD,
  now,
  signEvent,
  SWAP_PROFILE,
  verifyEvent,
  type NostrEvent,
} from "./nostr";

export type SharePayload = {
  type: "share";
  x: number;
  bytes: string;
};

export type SwapPayload = {
  type: string;
  x?: number;
  bytes?: string;
  v: number;
  profile: string;
  coordination: string;
  participants: string[];
  commitment_scheme: string;
};

export type OpenedShare = {
  from: string;
  to: string;
  coordination: string;
  x: number;
  bytes: string;
};

async function buildSeal(
  senderSk: Uint8Array,
  senderPk: string,
  recipientPk: string,
  rumorJson: string,
): Promise<NostrEvent> {
  const encrypted = await encrypt(senderSk, recipientPk, new TextEncoder().encode(rumorJson));
  return signEvent(senderSk, {
    created_at: now(),
    kind: KIND_SEAL,
    tags: [["p", recipientPk]],
    content: encrypted,
  });
}

async function wrapSeal(seal: NostrEvent, recipientPk: string): Promise<NostrEvent> {
  const ephemeral = generateKeypair();
  const encrypted = await encrypt(
    ephemeral.secret,
    recipientPk,
    new TextEncoder().encode(JSON.stringify(seal)),
  );
  return signEvent(ephemeral.secret, {
    created_at: now(),
    kind: KIND_GIFT_WRAP,
    tags: [["p", recipientPk]],
    content: encrypted,
  });
}

export async function wrapShare(args: {
  senderSk: Uint8Array;
  senderPk: string;
  recipientPk: string;
  coordination: string;
  x: number;
  bytes: string;
}): Promise<NostrEvent[]> {
  const payload: SwapPayload = {
    type: "share",
    x: args.x,
    bytes: args.bytes,
    v: 1,
    profile: SWAP_PROFILE,
    coordination: args.coordination,
    participants: [args.senderPk, args.recipientPk],
    commitment_scheme: "sha256-bytes@1",
  };
  const rumor = signEvent(args.senderSk, {
    created_at: now(),
    kind: KIND_SWAP_PAYLOAD,
    tags: [
      ["p", args.recipientPk],
      ["e", args.coordination, "", "root"],
    ],
    content: JSON.stringify(payload),
  });
  const rumorJson = JSON.stringify(rumor);
  const sealToRecipient = await buildSeal(args.senderSk, args.senderPk, args.recipientPk, rumorJson);
  const sealToSender = await buildSeal(args.senderSk, args.senderPk, args.senderPk, rumorJson);
  return [
    await wrapSeal(sealToRecipient, args.recipientPk),
    await wrapSeal(sealToSender, args.senderPk),
  ];
}

export async function openWrap(wrap: NostrEvent, recipientSk: Uint8Array): Promise<OpenedShare | null> {
  if (wrap.kind !== KIND_GIFT_WRAP || !verifyEvent(wrap)) {
    return null;
  }
  const pTags = wrap.tags.filter((tag) => tag[0] === "p");
  if (pTags.length !== 1 || !pTags[0][1] || !isHex64(pTags[0][1])) {
    return null;
  }
  const to = pTags[0][1];
  const ourPk = pubkeyFromSecret(recipientSk);
  if (to !== ourPk) {
    return null;
  }
  let seal: NostrEvent;
  try {
    const sealJson = new TextDecoder().decode(await decrypt(recipientSk, wrap.pubkey, wrap.content));
    seal = JSON.parse(sealJson) as NostrEvent;
  } catch {
    return null;
  }
  if (seal.kind !== KIND_SEAL || !verifyEvent(seal)) {
    return null;
  }
  let rumor: NostrEvent;
  try {
    const rumorJson = new TextDecoder().decode(await decrypt(recipientSk, seal.pubkey, seal.content));
    rumor = JSON.parse(rumorJson) as NostrEvent;
  } catch {
    return null;
  }
  if (!verifyEvent(rumor) || rumor.kind !== KIND_SWAP_PAYLOAD) {
    return null;
  }
  const eTags = rumor.tags.filter((tag) => tag[0] === "e");
  if (eTags.length !== 1 || !eTags[0][1]) {
    return null;
  }
  let payload: SwapPayload;
  try {
    payload = JSON.parse(rumor.content) as SwapPayload;
  } catch {
    return null;
  }
  if (
    payload.profile !== SWAP_PROFILE
    || payload.coordination !== eTags[0][1]
    || payload.type !== "share"
    || payload.x === undefined
    || !payload.bytes
    || payload.commitment_scheme !== "sha256-bytes@1"
  ) {
    return null;
  }
  return {
    from: rumor.pubkey,
    to,
    coordination: payload.coordination,
    x: payload.x,
    bytes: payload.bytes,
  };
}

export function secretFromHex(secretHex: string): Uint8Array {
  return hexToBytes(secretHex);
}

import { sha256 } from "@noble/hashes/sha2.js";
import * as api from "./api";
import type { EscrowRecord, WireEvent } from "./api";
import { openBus, openRelay, type BusMessage, type DisputeMessage, type InvoiceMessage, type OfferMessage, type Role } from "./bus";
import { bytesToHex, hexToBytes, isHex64, randomBytes } from "./hex";
import { pubkeyFromSecret } from "./nip04";
import { openWrap, secretFromHex, wrapShare } from "./nip59";
import type { NostrEvent } from "./nostr";
import { generateKeypair } from "./nostr";
import {
  combineShares,
  SHARE_X_ARBITRATOR,
  SHARE_X_BUYER,
  SHARE_X_SELLER,
  shareFromJson,
  shareToJson,
  splitPreimage,
  type Share,
} from "./shamir";

const SESSION_KEY = "ponthtlc.session";

export type ShareMap = Partial<Record<1 | 2 | 3, { x: number; bytes: string }>>;

export type Trade = {
  paymentHash: string;
  amountMsat: number;
  peer: string;
  arb?: string;
  escrowId?: string;
  bolt11?: string;
  createdAt: number;
  funded?: boolean;
  released?: boolean;
  outcome?: "settled" | "refunded" | "disputed";
};

export type Peer = {
  role: Role;
  pubkey: string;
  name: string;
};

type Persisted = {
  role: Role;
  name: string;
  secret: string;
  pubkey: string;
  peer: string;
  arb: string;
  relay: string;
  trades: Trade[];
  shares: Record<string, ShareMap>;
  peers: Peer[];
};

export class Session {
  role = $state<Role | null>(null);
  name = $state("");
  secret = $state("");
  pubkey = $state("");
  peer = $state("");
  arb = $state("");
  relayUrl = $state("");
  health = $state<"unknown" | "up" | "down">("unknown");
  nodePubkey = $state("");
  blockHeight = $state<number | null>(null);
  channels = $state<string[]>([]);
  escrows = $state<EscrowRecord[]>([]);
  selectedId = $state<string | null>(null);
  events = $state<WireEvent[]>([]);
  offers = $state<OfferMessage[]>([]);
  invoices = $state<InvoiceMessage[]>([]);
  trades = $state<Trade[]>([]);
  peers = $state<Peer[]>([]);
  disputes = $state<DisputeMessage[]>([]);
  shares = $state<Record<string, ShareMap>>({});
  reconstructed = $state<Record<string, string>>({});
  giftWraps = $state<NostrEvent[]>([]);
  error = $state("");
  notice = $state("");
  busy = $state("");

  #bus: ReturnType<typeof openBus> | null = null;
  #relay: ReturnType<typeof openRelay> | null = null;
  #stopWs: (() => void) | null = null;
  #greeted = new Set<string>();

  constructor() {
    this.restore();
  }

  get selected(): EscrowRecord | undefined {
    return this.escrows.find((escrow) => escrow.id === this.selectedId)
      ?? this.escrows[0];
  }

  boot() {
    if (!this.role || !this.pubkey) {
      return;
    }
    this.#bus?.close();
    this.#bus = openBus((message) => {
      void this.onBus(message);
    });
    this.#bus.send({ type: "hello", role: this.role, pubkey: this.pubkey, name: this.name });
    this.connectRelay();
    void this.refresh();
    this.#stopWs?.();
    this.#stopWs = api.openEventStream(this.pubkey, (event) => {
      this.events = [event, ...this.events].slice(0, 80);
      if (!event.payment_hash) {
        return;
      }
      if (event.kind === "PaymentClaimable") {
        this.patchTrade(event.payment_hash, { funded: true });
      }
      if (event.kind === "PaymentClaimed" || event.kind === "PaymentSent") {
        this.patchTrade(event.payment_hash, { outcome: "settled" });
      }
      if (event.kind === "PaymentFailed") {
        this.patchTrade(event.payment_hash, { outcome: "refunded" });
      }
    });
  }

  chooseRole(role: Role, name = "", secretHex = "") {
    const keys = secretHex && isHex64(secretHex)
      ? { secret: hexToBytes(secretHex.toLowerCase()), pubkey: pubkeyFromSecret(hexToBytes(secretHex.toLowerCase())) }
      : generateKeypair();
    this.role = role;
    this.name = name.trim() || defaultName(role);
    this.secret = bytesToHex(keys.secret);
    this.pubkey = keys.pubkey;
    this.persist();
    this.boot();
  }

  leave() {
    this.#bus?.close();
    this.#relay?.close();
    this.#stopWs?.();
    this.#bus = null;
    this.#relay = null;
    this.#stopWs = null;
    this.#greeted.clear();
    sessionStorage.removeItem(SESSION_KEY);
    this.role = null;
    this.name = "";
    this.secret = "";
    this.pubkey = "";
    this.peer = "";
    this.arb = "";
    this.relayUrl = "";
    this.escrows = [];
    this.events = [];
    this.offers = [];
    this.invoices = [];
    this.trades = [];
    this.peers = [];
    this.disputes = [];
    this.shares = {};
    this.reconstructed = {};
    this.giftWraps = [];
    this.error = "";
    this.notice = "";
    this.selectedId = null;
    this.health = "unknown";
  }

  setPeer(peer: string) {
    this.peer = peer.trim().toLowerCase();
    this.persist();
  }

  setArb(arb: string) {
    this.arb = arb.trim().toLowerCase();
    this.persist();
  }

  setRelay(url: string) {
    this.relayUrl = url.trim();
    this.persist();
    this.connectRelay();
  }

  async refresh() {
    if (!this.pubkey) {
      return;
    }
    this.health = (await api.health()) ? "up" : "down";
    if (this.health !== "up") {
      return;
    }
    try {
      const [info, channels, escrows] = await Promise.all([
        api.nodeInfo(this.pubkey),
        api.nodeChannels(this.pubkey),
        api.listEscrows(this.pubkey),
      ]);
      this.nodePubkey = info.node_pubkey;
      this.blockHeight = info.block_height;
      this.channels = channels;
      this.escrows = escrows;
      if (!this.selectedId && escrows[0]) {
        this.selectedId = escrows[0].id;
      }
      this.error = "";
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    }
  }

  async originate(amountSats: number, buyerOverride = "", arbOverride = "") {
    if (this.role !== "seller") {
      throw new Error("only the seller originates the preimage");
    }
    const buyer = (buyerOverride || this.peer).toLowerCase();
    if (!isHex64(buyer)) {
      throw new Error("buyer pubkey required");
    }
    const arb = (arbOverride || this.arb).toLowerCase();
    this.busy = "originating";
    try {
      const preimage = randomBytes(32);
      const paymentHash = bytesToHex(sha256(preimage));
      const [s1, s2, s3] = splitPreimage(preimage);
      const amountMsat = amountSats * 1000;
      this.rememberShares(paymentHash, [s1, s2, s3]);
      this.upsertTrade({
        paymentHash,
        amountMsat,
        peer: buyer,
        arb: isHex64(arb) ? arb : undefined,
        createdAt: Date.now(),
      });
      this.peer = buyer;
      if (isHex64(arb)) {
        this.arb = arb;
      }
      this.#bus?.send({
        type: "offer",
        paymentHash,
        amountMsat,
        seller: this.pubkey,
      });
      await this.sendShare(paymentHash, SHARE_X_BUYER, buyer);
      if (isHex64(arb)) {
        await this.sendShare(paymentHash, SHARE_X_ARBITRATOR, arb);
      }
      this.notice = isHex64(arb)
        ? "s1 sent to the buyer. s3 sent to the arbitrator. Keep s2."
        : "Offer and buyer share sent. Keep s2 until you release.";
      this.persist();
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.busy = "";
    }
  }

  async createFromOffer(offer: OfferMessage) {
    this.busy = "creating";
    try {
      const created = await api.createEscrow(this.pubkey, offer.paymentHash, offer.amountMsat);
      this.upsertTrade({
        paymentHash: offer.paymentHash,
        amountMsat: offer.amountMsat,
        peer: offer.seller,
        escrowId: created.id,
        bolt11: created.bolt11_invoice,
        createdAt: Date.now(),
      });
      this.peer = offer.seller;
      this.selectedId = created.id;
      this.#bus?.send({
        type: "invoice",
        escrowId: created.id,
        bolt11: created.bolt11_invoice,
        paymentHash: offer.paymentHash,
        buyer: this.pubkey,
      });
      await this.refresh();
      this.notice = "Hold invoice created. Share it with the seller.";
      this.persist();
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.busy = "";
    }
  }

  async createManual(paymentHash: string, amountSats: number) {
    const created = await api.createEscrow(this.pubkey, paymentHash.toLowerCase(), amountSats * 1000);
    this.upsertTrade({
      paymentHash: created.payment_hash,
      amountMsat: amountSats * 1000,
      peer: this.peer,
      escrowId: created.id,
      bolt11: created.bolt11_invoice,
      createdAt: Date.now(),
    });
    this.selectedId = created.id;
    this.#bus?.send({
      type: "invoice",
      escrowId: created.id,
      bolt11: created.bolt11_invoice,
      paymentHash: created.payment_hash,
      buyer: this.pubkey,
    });
    await this.refresh();
    this.persist();
    return created;
  }

  async fund(invoice: InvoiceMessage | { escrowId: string; bolt11: string; paymentHash?: string }) {
    this.busy = "funding";
    try {
      const paid = invoice.escrowId
        ? await api.fundEscrow(this.pubkey, invoice.escrowId, invoice.bolt11)
        : await api.payInvoice(this.pubkey, invoice.bolt11);
      const hash = invoice.paymentHash || paid.payment_hash;
      this.patchTrade(hash, { funded: true, bolt11: invoice.bolt11, escrowId: invoice.escrowId || this.tradeFor(hash)?.escrowId });
      this.notice = "Invoice funded. HTLC is held until claim or fail.";
      await this.refresh();
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.busy = "";
    }
  }

  async release(paymentHash: string, to = "") {
    const peer = (to || this.tradeFor(paymentHash)?.peer || this.peer).toLowerCase();
    if (!isHex64(peer)) {
      throw new Error("counterparty pubkey required");
    }
    const x = this.role === "arbitrator" ? SHARE_X_ARBITRATOR : SHARE_X_SELLER;
    await this.sendShare(paymentHash, x, peer);
    this.patchTrade(paymentHash, { released: true });
    this.notice = x === SHARE_X_ARBITRATOR
      ? "Arbitrator share sent."
      : "Seller share sent. Buyer can reconstruct P.";
  }

  reconstruct(paymentHash: string): string {
    const map = this.shares[paymentHash] ?? {};
    const present = Object.values(map).filter(Boolean) as { x: number; bytes: string }[];
    if (present.length < 2) {
      throw new Error("need two shares");
    }
    const [a, b] = present;
    const preimage = combineShares(shareFromJson(a), shareFromJson(b));
    const hash = bytesToHex(sha256(preimage));
    if (hash !== paymentHash) {
      throw new Error("shares do not reconstruct the payment hash");
    }
    const hex = bytesToHex(preimage);
    this.reconstructed = { ...this.reconstructed, [paymentHash]: hex };
    return hex;
  }

  async claim(id: string, paymentHash: string) {
    const preimage = this.reconstructed[paymentHash] ?? this.reconstruct(paymentHash);
    this.busy = "claiming";
    try {
      await api.claimEscrow(this.pubkey, id, preimage);
      const next = { ...this.reconstructed };
      delete next[paymentHash];
      this.reconstructed = next;
      this.notice = "Claim submitted. Preimage left this tab.";
      await this.refresh();
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.busy = "";
    }
  }

  async fail(id: string) {
    this.busy = "failing";
    try {
      await api.failEscrow(this.pubkey, id);
      this.notice = "HTLC failed backwards.";
      await this.refresh();
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.busy = "";
    }
  }

  async dispute(id: string, resolver: string) {
    this.busy = "disputing";
    try {
      const result = await api.disputeEscrow(this.pubkey, id, resolver);
      this.giftWraps = result.gift_wraps;
      const record = this.escrows.find((item) => item.id === id);
      if (record) {
        this.patchTrade(record.payment_hash, { outcome: "disputed", arb: resolver });
        this.#bus?.send({
          type: "dispute",
          paymentHash: record.payment_hash,
          escrowId: id,
          from: this.pubkey,
          resolver,
        });
      }
      this.notice = "Dispute opened. The arbitrator can now send s3.";
      await this.refresh();
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.busy = "";
    }
  }

  ingestPastedShare(paymentHash: string, raw: string) {
    const parsed = JSON.parse(raw) as { x: number; bytes: string };
    this.rememberShares(paymentHash, [shareFromJson(parsed)]);
    this.persist();
  }

  shareStatus(paymentHash: string): { held: number[]; ready: boolean } {
    const map = this.shares[paymentHash] ?? {};
    const held = ([SHARE_X_BUYER, SHARE_X_SELLER, SHARE_X_ARBITRATOR] as const).filter((x) => map[x]);
    return { held, ready: held.length >= 2 };
  }

  tradeFor(paymentHash: string): Trade | undefined {
    return this.trades.find((trade) => trade.paymentHash === paymentHash);
  }

  async sendShare(paymentHash: string, x: 1 | 2 | 3, recipient: string) {
    const stored = this.shares[paymentHash]?.[x];
    if (!stored) {
      throw new Error(`share s${x} is not in this tab`);
    }
    const wraps = await wrapShare({
      senderSk: secretFromHex(this.secret),
      senderPk: this.pubkey,
      recipientPk: recipient,
      coordination: paymentHash,
      x: stored.x,
      bytes: stored.bytes,
    });
    for (const event of wraps) {
      this.#bus?.send({ type: "wrap", event });
      this.#relay?.publish(event);
    }
  }

  private async onBus(message: BusMessage) {
    if (message.type === "hello") {
      if (message.pubkey !== this.pubkey && isHex64(message.pubkey)) {
        this.rememberPeer({ role: message.role, pubkey: message.pubkey, name: message.name || message.role });
        if (!this.peer && (message.role === "buyer" || message.role === "seller")) {
          this.peer = message.pubkey;
        }
        if (!this.arb && message.role === "arbitrator") {
          this.arb = message.pubkey;
        }
        this.persist();
        if (this.role && this.pubkey && !this.#greeted.has(message.pubkey)) {
          this.#greeted.add(message.pubkey);
          this.#bus?.send({ type: "hello", role: this.role, pubkey: this.pubkey, name: this.name });
        }
      }
      return;
    }
    if (message.type === "offer") {
      if (message.seller !== this.pubkey && !this.offers.some((item) => item.paymentHash === message.paymentHash)) {
        this.offers = [message, ...this.offers];
      }
      return;
    }
    if (message.type === "invoice") {
      if (message.buyer !== this.pubkey && !this.invoices.some((item) => item.escrowId === message.escrowId)) {
        this.invoices = [message, ...this.invoices];
        this.upsertTrade({
          paymentHash: message.paymentHash,
          amountMsat: this.tradeFor(message.paymentHash)?.amountMsat ?? 0,
          peer: message.buyer,
          escrowId: message.escrowId,
          bolt11: message.bolt11,
          createdAt: Date.now(),
        });
        this.persist();
      }
      return;
    }
    if (message.type === "dispute") {
      if (!this.disputes.some((item) => item.escrowId === message.escrowId)) {
        this.disputes = [message, ...this.disputes];
      }
      this.patchTrade(message.paymentHash, {
        outcome: "disputed",
        escrowId: message.escrowId,
        arb: message.resolver,
      });
      if (this.role === "arbitrator") {
        this.notice = "A dispute landed. You can send s3.";
      }
      return;
    }
    if (message.type === "wrap") {
      await this.acceptWrap(message.event);
    }
  }

  private async acceptWrap(event: NostrEvent) {
    const opened = await openWrap(event, secretFromHex(this.secret));
    if (!opened) {
      return;
    }
    this.rememberShares(opened.coordination, [shareFromJson({ x: opened.x, bytes: opened.bytes })]);
    if (!this.tradeFor(opened.coordination)) {
      this.upsertTrade({
        paymentHash: opened.coordination,
        amountMsat: 0,
        peer: opened.from,
        createdAt: Date.now(),
      });
    }
    this.notice = `Received share s${opened.x} for ${opened.coordination.slice(0, 8)}…`;
    this.persist();
  }

  private rememberShares(paymentHash: string, incoming: Share[]) {
    const current = { ...(this.shares[paymentHash] ?? {}) };
    for (const share of incoming) {
      const x = share.x as 1 | 2 | 3;
      current[x] = shareToJson(share);
    }
    this.shares = { ...this.shares, [paymentHash]: current };
  }

  private upsertTrade(trade: Trade) {
    const rest = this.trades.filter((item) => item.paymentHash !== trade.paymentHash);
    const previous = this.trades.find((item) => item.paymentHash === trade.paymentHash);
    this.trades = [{ ...previous, ...trade }, ...rest];
  }

  private patchTrade(paymentHash: string, patch: Partial<Trade>) {
    const previous = this.tradeFor(paymentHash);
    this.upsertTrade({
      paymentHash,
      amountMsat: previous?.amountMsat ?? 0,
      peer: previous?.peer ?? this.peer,
      createdAt: previous?.createdAt ?? Date.now(),
      ...previous,
      ...patch,
    });
    this.persist();
  }

  private rememberPeer(peer: Peer) {
    this.peers = [peer, ...this.peers.filter((item) => item.pubkey !== peer.pubkey)];
  }

  peersOf(role: Role): Peer[] {
    return this.peers.filter((peer) => peer.role === role);
  }

  private connectRelay() {
    this.#relay?.close();
    this.#relay = null;
    if (!this.relayUrl || !this.pubkey) {
      return;
    }
    this.#relay = openRelay(this.relayUrl, this.pubkey, (event) => {
      void this.acceptWrap(event);
    });
  }

  private restore() {
    const raw = sessionStorage.getItem(SESSION_KEY);
    if (!raw) {
      return;
    }
    try {
      const parsed = JSON.parse(raw) as Persisted;
      this.role = parsed.role;
      this.name = parsed.name ?? defaultName(parsed.role);
      this.secret = parsed.secret;
      this.pubkey = parsed.pubkey || pubkeyFromSecret(hexToBytes(parsed.secret));
      this.peer = parsed.peer ?? "";
      this.arb = parsed.arb ?? "";
      this.relayUrl = parsed.relay ?? "";
      this.trades = parsed.trades ?? [];
      this.shares = parsed.shares ?? {};
      this.peers = parsed.peers ?? [];
    } catch {
      sessionStorage.removeItem(SESSION_KEY);
    }
  }

  persist() {
    if (!this.role || !this.secret) {
      return;
    }
    const payload: Persisted = {
      role: this.role,
      name: this.name,
      secret: this.secret,
      pubkey: this.pubkey,
      peer: this.peer,
      arb: this.arb,
      relay: this.relayUrl,
      trades: this.trades,
      shares: this.shares,
      peers: this.peers,
    };
    sessionStorage.setItem(SESSION_KEY, JSON.stringify(payload));
  }
}

export const session = new Session();

function defaultName(role: Role): string {
  if (role === "buyer") {
    return "Amina";
  }
  if (role === "seller") {
    return "Kwame";
  }
  return "Fatou";
}

<script lang="ts">
  import { sha256 } from "@noble/hashes/sha2.js";
  import * as api from "../../lib/api";
  import type { EscrowRecord } from "../../lib/api";
  import { bytesToHex, hexToBytes } from "../../lib/hex";
  import { loc } from "../../lib/router.svelte";
  import { combineShares, type Share } from "../../lib/shamir";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  type Piece = { x: number; hex: string };

  const id = $derived(loc.parts[1] ?? "");
  let row = $state<EscrowRecord | null>(null);
  let error = $state("");
  let fiatPaid = $state(false);
  let fundInvoice = $state("");
  let sellerAddress = $state("");
  let pasted = $state("");
  let handoff = $state<{ mine: Piece; seller: Piece; arbitrator: Piece } | null>(null);

  const steps = $derived.by(() => {
    const status = row?.protocol_status || "proposed";
    const released = status === "settled" || status === "refunded";
    return [
      { name: "Lock", done: status !== "proposed" && status !== "" },
      { name: "Pay fiat", done: fiatPaid || released },
      { name: "Confirm", done: released },
      { name: "Release", done: status === "settled" },
    ];
  });

  async function load() {
    if (!id) {
      return;
    }
    fiatPaid = localStorage.getItem(`ponthtlc.fiat.${id}`) === "1";
    const raw = sessionStorage.getItem(`ponthtlc.share.${id}`);
    try {
      handoff = raw ? JSON.parse(raw) : null;
    } catch {
      handoff = null;
    }
    try {
      row = await api.getEscrow(wallet.pubkey, id);
      error = "";
    } catch (err) {
      error = err instanceof Error ? err.message : "Swap not found.";
    }
  }

  $effect(() => {
    void load();
  });

  function markFiat() {
    localStorage.setItem(`ponthtlc.fiat.${id}`, "1");
    fiatPaid = true;
  }

  function piece(raw: Piece): Share {
    return { x: raw.x, bytes: hexToBytes(raw.hex) };
  }

  async function fund() {
    error = "";
    try {
      await api.fundEscrow(wallet.pubkey, id, fundInvoice, sellerAddress);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Funding failed.";
    }
  }

  async function release() {
    error = "";
    if (!row || !handoff) {
      error = "This browser does not hold a share for this swap.";
      return;
    }
    let other: Piece;
    try {
      other = JSON.parse(pasted) as Piece;
      if (!other.hex || !other.x) {
        throw new Error("missing share");
      }
    } catch {
      error = "Paste the other share as JSON: {\"x\":2,\"hex\":\"...\"}";
      return;
    }
    try {
      const preimage = combineShares(piece(handoff.mine), piece(other));
      const hash = bytesToHex(sha256(preimage));
      if (hash !== row.payment_hash) {
        error = "Those two shares do not match this swap.";
        return;
      }
      await api.claimEscrow(wallet.pubkey, id, bytesToHex(preimage));
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Release failed.";
    }
  }

  async function refund() {
    error = "";
    try {
      await api.failEscrow(wallet.pubkey, id);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Refund failed.";
    }
  }
</script>

<main class="flow">
  <p class="kicker">Swap</p>
  <h1>{id.slice(0, 8)}</h1>
  <p class="badge">2 of 3 Shamir</p>
  <ol class="steps">
    {#each steps as step, index}
      <li class:done={step.done}>{index + 1}. {step.name}</li>
    {/each}
  </ol>
  {#if row}
    <p class="muted">{row.protocol_status || row.state} · {(row.amount_msat ?? 0).toLocaleString()} msat</p>
    {#if row.bolt11}
      <label>
        Hold invoice
        <textarea readonly rows="4">{row.bolt11}</textarea>
      </label>
    {/if}
    {#if handoff}
      <label>
        Seller share — send this, do not keep it as yours
        <textarea readonly rows="3">{JSON.stringify(handoff.seller)}</textarea>
      </label>
      <label>
        Arbitrator share
        <textarea readonly rows="3">{JSON.stringify(handoff.arbitrator)}</textarea>
      </label>
    {/if}
    <button class="btn ghost" type="button" onclick={markFiat}>I've paid the fiat</button>
    <label>
      Fund this invoice from the other side
      <textarea bind:value={fundInvoice} rows="3" placeholder="bolt11"></textarea>
    </label>
    <label>
      Seller Lightning address
      <input bind:value={sellerAddress} placeholder="seller@domain" />
    </label>
    <button class="btn ghost" type="button" onclick={fund}>Fund</button>
    <label>
      Paste a second share to release
      <textarea bind:value={pasted} rows="3" placeholder={'{"x":2,"hex":"..."}'}></textarea>
    </label>
    <button class="btn" type="button" onclick={release}>Release</button>
    <button class="btn danger" type="button" onclick={refund}>Refund</button>
    <a class="btn ghost" href="#/swap/{id}/chat">Chat</a>
  {/if}
  {#if error}
    <p class="warn">{error}</p>
  {/if}
  <Nav />
</main>

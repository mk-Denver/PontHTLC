<script lang="ts">
  import { sha256 } from "@noble/hashes/sha2.js";
  import * as api from "../../lib/api";
  import type { EscrowRecord, WireEvent } from "../../lib/api";
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
  let pasted = $state("");
  let handoff = $state<{ mine: Piece; seller: Piece; arbitrator: Piece } | null>(null);
  let events = $state<WireEvent[]>([]);
  let closeWs: (() => void) | null = null;
  let busy = $state(false);

  const isAgent = $derived(wallet.isAgent || (row !== null && row.owner !== wallet.pubkey));
  const status = $derived(row?.protocol_status || "proposed");

  const steps = $derived.by(() => {
    const released = status === "settled" || status === "refunded";
    return [
      { name: "Lock", done: status !== "proposed" && status !== "" },
      { name: "Pay fiat", done: fiatPaid || released },
      { name: "Confirm", done: status === "settlement_authorized" || released },
      { name: "Release", done: status === "settled" },
    ];
  });

  async function load() {
    if (!id) return;
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
    if (wallet.pubkey) {
      closeWs?.();
      closeWs = api.openEventStream(wallet.pubkey, (ev) => {
        events = [...events, ev];
        void load();
      });
    }
    const interval = setInterval(() => {
      if (row && row.protocol_status !== "settled" && row.protocol_status !== "refunded") {
        void load();
      }
    }, 3000);
    return () => {
      closeWs?.();
      clearInterval(interval);
    };
  });

  function piece(raw: Piece): Share {
    return { x: raw.x, bytes: hexToBytes(raw.hex) };
  }

  async function accept() {
    busy = true; error = "";
    try {
      await api.acceptEscrow(wallet.pubkey, id);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Accept failed.";
    } finally { busy = false; }
  }

  async function lock() {
    busy = true; error = "";
    try {
      await api.lockEscrow(wallet.pubkey, id);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Lock failed.";
    } finally { busy = false; }
  }

  async function markFiatSent() {
    busy = true; error = "";
    try {
      await api.fiatSent(wallet.pubkey, id, row?.payment_hash ?? "");
      localStorage.setItem(`ponthtlc.fiat.${id}`, "1");
      fiatPaid = true;
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Failed to mark fiat sent.";
    } finally { busy = false; }
  }

  async function confirmFiat() {
    busy = true; error = "";
    try {
      await api.fiatConfirmed(wallet.pubkey, id);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Confirm failed.";
    } finally { busy = false; }
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
      if (!other.hex || !other.x) throw new Error("missing share");
    } catch {
      error = 'Paste the other share as JSON: {"x":2,"hex":"..."}';
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
    busy = true; error = "";
    try {
      await api.failEscrow(wallet.pubkey, id);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : "Refund failed.";
    } finally { busy = false; }
  }
</script>

<main class="flow">
  <p class="kicker">Swap</p>
  <h1>{id.slice(0, 8)}</h1>
  <p class="badge" class:ok={status === "settled"} class:warn={status === "refunded" || status === "disputed"}>
    {status} · {isAgent ? "Agent" : "Buyer"}
  </p>

  <ol class="steps">
    {#each steps as step, index}
      <li class:done={step.done}>
        <span class="dot">{step.done ? "✓" : index + 1}</span>
        {step.name}
      </li>
    {/each}
  </ol>

  {#if row}
    <p class="muted">
      <span class={`pill ${status}`}>{status}</span>
      &nbsp;{(row.amount_msat ?? 0).toLocaleString()} msat
    </p>

    {#if status === "secured" || status === "accepted"}
      <p class="badge ok" style="justify-self:start">Locked in wallet</p>
    {/if}

    {#if handoff}
      <label>
        Seller share — send this
        <div class="share-box">{JSON.stringify(handoff.seller)}</div>
      </label>
      <label>
        Arbitrator share
        <div class="share-box">{JSON.stringify(handoff.arbitrator)}</div>
      </label>
    {/if}

    {#if row.bolt11}
      <label>
        Hold invoice
        <textarea readonly rows="2">{row.bolt11}</textarea>
      </label>
    {/if}

    <!-- Agent actions -->
    {#if isAgent}
      {#if status === "proposed"}
        <button class="btn" disabled={busy} onclick={accept}>Accept swap</button>
      {/if}
      {#if status === "accepted"}
        <button class="btn" disabled={busy} onclick={lock}>Lock sats</button>
      {/if}
      {#if status === "fiat_sent"}
        <button class="btn" disabled={busy} onclick={confirmFiat}>Confirm fiat received</button>
      {/if}
    {/if}

    <!-- Buyer actions -->
    {#if !isAgent}
      {#if status === "secured"}
        <button class="btn ghost" disabled={busy || fiatPaid} onclick={markFiatSent}>
          {fiatPaid ? "Fiat marked paid" : "I've paid the fiat"}
        </button>
      {/if}
      {#if status === "settlement_authorized"}
        <label>
          Paste agent's share to release
          <textarea bind:value={pasted} rows="2" placeholder="paste second share JSON"></textarea>
        </label>
        <button class="btn" disabled={busy} onclick={release}>Claim sats</button>
      {/if}
    {/if}

    <!-- Both -->
    {#if status === "secured"}
      <button class="btn danger" disabled={busy} onclick={refund}>Refund</button>
    {/if}

    <a class="btn ghost" href="#/swap/{id}/chat">Chat</a>

    <details class="event-log">
      <summary>Event log ({events.length})</summary>
      <ul>
        {#each events as ev}
          <li>{ev.kind} {ev.payment_hash ? ev.payment_hash.slice(0, 12) : ""}</li>
        {/each}
      </ul>
    </details>
  {/if}

  {#if error}
    <p class="warn">{error}</p>
  {/if}
  <Nav />
</main>
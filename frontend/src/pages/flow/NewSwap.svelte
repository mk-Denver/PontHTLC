<script lang="ts">
  import { sha256 } from "@noble/hashes/sha2.js";
  import * as api from "../../lib/api";
  import type { OfferSummary } from "../../lib/api";
  import { bytesToHex } from "../../lib/hex";
  import { go } from "../../lib/router.svelte";
  import { splitPreimage } from "../../lib/shamir";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let offers = $state<OfferSummary[]>([]);
  let selected = $state<OfferSummary | null>(null);
  let fiatAmount = $state("");
  let address = $state(wallet.lightningAddress);
  let busy = $state(false);
  let error = $state("");
  let loading = $state(true);

  async function loadOffers() {
    loading = true;
    try {
      offers = await api.listOffers(wallet.pubkey);
      error = "";
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not load offers.";
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (wallet.pubkey) {
      void loadOffers();
    }
  });

  async function create() {
    error = "";
    if (!address.includes("@")) {
      error = "Add a Lightning address like name@domain.";
      return;
    }
    busy = true;
    try {
      const preimage = crypto.getRandomValues(new Uint8Array(32));
      const paymentHash = bytesToHex(sha256(preimage));
      const [buyer, seller, arbitrator] = splitPreimage(preimage);
      const amountMsat = selected
        ? Math.round(parseFloat(fiatAmount) / parseFloat(selected.price) * 100_000_000) * 1000
        : 50_000_000;
      const created = await api.createEscrow(
        wallet.pubkey,
        paymentHash,
        amountMsat,
        address,
      );
      const handoff = {
        mine: { x: buyer.x, hex: bytesToHex(buyer.bytes) },
        seller: { x: seller.x, hex: bytesToHex(seller.bytes) },
        arbitrator: { x: arbitrator.x, hex: bytesToHex(arbitrator.bytes) },
      };
      sessionStorage.setItem(`ponthtlc.share.${created.id}`, JSON.stringify(handoff));
      wallet.lightningAddress = address;
      wallet.save();
      go(`/swap/${created.id}`);
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not create the swap.";
    } finally {
      busy = false;
    }
  }
</script>

<main class="flow">
  <p class="kicker">New swap</p>
  <h1>Find an agent</h1>

  {#if loading}
    <p class="muted">Loading offers…</p>
  {:else if offers.length === 0}
    <p class="muted">No agents online. Ask an agent to publish an offer from the Desk tab.</p>
    <button class="btn ghost" onclick={loadOffers}>Refresh</button>
  {:else}
    <ul class="swap-list">
      {#each offers as offer}
        <li>
          <button
            class="ticket"
            style={selected?.event_id === offer.event_id ? "border-color: var(--accent)" : ""}
            onclick={() => (selected = offer)}
          >
            <strong>{offer.currency} {offer.direction === "fiat_to_btc" ? "→ BTC" : "← BTC"}</strong>
            <span class="muted">{offer.price} {offer.currency}/sat</span>
            <span class="muted">{offer.min_amount}–{offer.max_amount} {offer.currency}</span>
            <span class="muted">{offer.channels.join(", ")}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if selected}
    <p class="kicker" style="margin-top:10px">Selected: {selected.currency} {selected.direction === "fiat_to_btc" ? "→ BTC" : "← BTC"}</p>
    <label>
      Amount in {selected.currency}
      <input type="number" bind:value={fiatAmount} placeholder={selected.min_amount} />
    </label>
  {/if}

  <label>
    Your Lightning address
    <input bind:value={address} placeholder="name@domain" autocomplete="off" />
  </label>
  <p class="muted">The preimage is split into 3 shares in this browser. The node never sees it.</p>
  {#if error}
    <p class="warn">{error}</p>
  {/if}
  <button class="btn" type="button" disabled={busy || !address.includes("@")} onclick={create}>
    {busy ? "Creating…" : "Create swap proposal"}
  </button>
  <Nav />
</main>
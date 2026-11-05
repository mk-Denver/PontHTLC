<script lang="ts">
  import { sha256 } from "@noble/hashes/sha2.js";
  import * as api from "../../lib/api";
  import { bytesToHex } from "../../lib/hex";
  import { go } from "../../lib/router.svelte";
  import { splitPreimage } from "../../lib/shamir";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let sats = $state(50000);
  let address = $state(wallet.lightningAddress);
  let busy = $state(false);
  let error = $state("");

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
      const created = await api.createEscrow(wallet.pubkey, paymentHash, sats * 1000, address);
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
  <h1>Lock sats</h1>
  <label>
    Amount (sats)
    <input type="number" min="1" bind:value={sats} />
  </label>
  <label>
    Your Lightning address
    <input bind:value={address} placeholder="name@domain" autocomplete="off" />
  </label>
  <p class="muted">The preimage is split into 3 shares in this browser. The node never sees it.</p>
  {#if error}
    <p class="warn">{error}</p>
  {/if}
  <button class="btn" type="button" disabled={busy} onclick={create}>
    {busy ? "Creating…" : "Create hold invoice"}
  </button>
  <Nav />
</main>

<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "../../lib/api";
  import type { ChannelStatus, OfferSummary } from "../../lib/api";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let status = $state<ChannelStatus | null>(null);
  let height = $state<number | null>(null);
  let offers = $state<OfferSummary[]>([]);
  let error = $state("");
  let busy = $state(false);

  let currency = $state("KES");
  let direction = $state("fiat_to_btc");
  let price = $state("0.01");
  let minAmount = $state("100");
  let maxAmount = $state("10000");
  let channels = $state("mpesa_phone_ke_kes@2");

  async function refresh() {
    try {
      const [channels, info, listed] = await Promise.all([
        api.channelStatus(wallet.pubkey),
        api.nodeInfo(wallet.pubkey),
        api.listOffers(wallet.pubkey),
      ]);
      status = channels;
      height = info.block_height;
      offers = listed;
      error = "";
    } catch (err) {
      error = err instanceof Error ? err.message : "Desk is offline.";
    }
  }

  async function openChannel() {
    busy = true;
    error = "";
    try {
      await api.openChannel(wallet.pubkey);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not open a channel.";
    } finally {
      busy = false;
    }
  }

  async function publishOffer() {
    busy = true;
    error = "";
    try {
      await api.publishOffer(
        wallet.pubkey,
        currency,
        direction,
        price,
        minAmount,
        maxAmount,
        channels.split(",").map((c) => c.trim()).filter((c) => c),
      );
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not publish offer.";
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<main class="flow">
  <p class="kicker">Desk</p>
  <h1>Agent</h1>
  <p class="muted">Block {height ?? "—"} · peers {status?.peers_connected ? "connected" : "quiet"}</p>

  <p class="kicker" style="margin-top:10px">Channels</p>
  <ul class="swap-list">
    {#each status?.channels ?? [] as channel}
      <li>
        <strong>{channel.channel_id.slice(0, 12)}</strong>
        <span class={`pill ${channel.usable ? "settled" : channel.ready ? "secured" : "proposed"}`}>
          {channel.usable ? "usable" : channel.ready ? "ready" : "opening"}
        </span>
      </li>
    {:else}
      <li class="muted">No channel yet.</li>
    {/each}
  </ul>
  <button class="btn ghost" type="button" disabled={busy} onclick={openChannel}>
    {busy ? "Opening…" : "Open channel"}
  </button>

  <p class="kicker" style="margin-top:10px">Publish offer</p>
  <label>
    Currency
    <input bind:value={currency} placeholder="KES" />
  </label>
  <label>
    Direction
    <select bind:value={direction}>
      <option value="fiat_to_btc">Sell BTC (fiat to BTC)</option>
      <option value="btc_to_fiat">Buy BTC (BTC to fiat)</option>
    </select>
  </label>
  <label>
    Price ({currency}/sat)
    <input bind:value={price} placeholder="0.01" />
  </label>
  <label>
    Min amount ({currency})
    <input bind:value={minAmount} placeholder="100" />
  </label>
  <label>
    Max amount ({currency})
    <input bind:value={maxAmount} placeholder="10000" />
  </label>
  <label>
    Payment channels (comma-separated)
    <input bind:value={channels} placeholder="mpesa_phone_ke_kes@2" />
  </label>
  <button class="btn" type="button" disabled={busy} onclick={publishOffer}>
    {busy ? "Publishing…" : "Publish offer"}
  </button>

  {#if offers.length > 0}
    <p class="kicker" style="margin-top:10px">Your offers ({offers.length})</p>
    <ul class="swap-list">
      {#each offers as offer}
        <li>
          <strong>{offer.currency} {offer.direction === "fiat_to_btc" ? "→ BTC" : "← BTC"}</strong>
          <span class="muted">{offer.price} {offer.currency}/sat</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if error}
    <p class="warn">{error}</p>
  {/if}
  <Nav />
</main>
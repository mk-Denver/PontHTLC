<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "../../lib/api";
  import type { EscrowRecord, OfferSummary, ChannelStatus } from "../../lib/api";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let rows = $state<EscrowRecord[]>([]);
  let msat = $state(0);
  let offers = $state<OfferSummary[]>([]);
  let status = $state<ChannelStatus | null>(null);
  let note = $state("");

  async function refresh() {
    if (!wallet.pubkey) {
      return;
    }
    try {
      const [listed, balance] = await Promise.all([
        api.listEscrows(wallet.pubkey),
        api.balance(wallet.pubkey),
      ]);
      rows = listed;
      msat = balance.msat;

      if (wallet.isAgent) {
        const [listedOffers, channelStatus] = await Promise.all([
          api.listOffers(wallet.pubkey),
          api.channelStatus(wallet.pubkey),
        ]);
        offers = listedOffers;
        status = channelStatus;
      }
      note = "";
    } catch (err) {
      note = err instanceof Error ? err.message : "The node is not answering.";
    }
  }

  onMount(() => {
    void refresh();
  });

  function title(row: EscrowRecord) {
    const sats = Math.round((row.amount_msat ?? 0) / 1000);
    return sats > 0 ? `${sats} sats` : row.id.slice(0, 8);
  }

  function pillClass(status: string) {
    if (!status) return "pill proposed";
    return `pill ${status}`;
  }

  const incomingSwaps = $derived(
    rows.filter((r) => r.owner !== wallet.pubkey || r.protocol_status === "proposed" || r.protocol_status === "secured"),
  );
</script>

<main class="flow">
  {#if wallet.isAgent}
    <p class="kicker">Agent desk</p>
    <h1>{wallet.name || "Desk"}</h1>
    <p class="muted">
      Block {status ? "live" : "—"} ·
      {status?.peers_connected ? "peers connected" : "peers quiet"} ·
      {status?.channels.filter((c) => c.usable).length ?? 0} usable channels
    </p>
    {#if note}
      <p class="warn">{note}</p>
    {/if}

    {#if offers.length > 0}
      <p class="kicker" style="margin-top:12px">Your offers ({offers.length})</p>
      <ul class="swap-list">
        {#each offers as offer}
          <li>
            <strong>{offer.currency} {offer.direction === "fiat_to_btc" ? "→ BTC" : "← BTC"}</strong>
            <span class="muted">{offer.price} {offer.currency}/sat</span>
          </li>
        {/each}
      </ul>
    {/if}

    <p class="kicker" style="margin-top:12px">Incoming swaps ({incomingSwaps.length})</p>
    <ul class="swap-list">
      {#each incomingSwaps as row}
        <li>
          <a href="#/swap/{row.id}">
            <strong>{title(row)}</strong>
            <span class={pillClass(row.protocol_status || row.state)}>{row.protocol_status || row.state}</span>
          </a>
        </li>
      {:else}
        <li class="muted">No incoming swaps.</li>
      {/each}
    </ul>
    <a class="btn ghost" href="#/desk">Publish new offer</a>
  {:else}
    <p class="kicker">PontHTLC</p>
    <h1>{wallet.name || "Wallet"}</h1>
    <p class="balance">{msat.toLocaleString()} msat</p>
    <p class="muted">Settled sats go to your Lightning address automatically.</p>
    {#if note}
      <p class="warn">{note}</p>
    {/if}
    <a class="btn" href="#/swap/new">New swap</a>

    <p class="kicker" style="margin-top:12px">Your swaps ({rows.length})</p>
    <ul class="swap-list">
      {#each rows as row}
        <li>
          <a href="#/swap/{row.id}">
            <strong>{title(row)}</strong>
            <span class={pillClass(row.protocol_status || row.state)}>{row.protocol_status || row.state}</span>
          </a>
        </li>
      {:else}
        <li class="muted">No swaps yet.</li>
      {/each}
    </ul>
  {/if}
  <Nav />
</main>
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "../../lib/api";
  import type { EscrowRecord } from "../../lib/api";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let rows = $state<EscrowRecord[]>([]);
  let msat = $state(0);
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
</script>

<main class="flow">
  <p class="kicker">PontHTLC</p>
  <h1>{wallet.name || "Wallet"}</h1>
  <p class="balance">{msat.toLocaleString()} msat</p>
  <p class="muted">Settled sats go to your Lightning address automatically.</p>
  {#if note}
    <p class="warn">{note}</p>
  {/if}
  <a class="btn" href="#/swap/new">New swap</a>

  <p class="kicker" style="margin-top:10px">Active swaps</p>
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
  <Nav />
</main>
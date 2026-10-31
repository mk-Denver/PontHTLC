<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";
  import NextMove from "../components/NextMove.svelte";
</script>

<NextMove />

<div class="grid two">
  <section class="panel">
    <h2>Your splits</h2>
    {#if session.trades.length === 0}
      <p class="empty">No preimages yet. A new trade writes s1/s2/s3 into this tab only.</p>
    {:else}
      <div class="grid">
        {#each session.trades.slice(0, 4) as trade (trade.paymentHash)}
          <button class="ticket" type="button" onclick={() => go(`/app/trade/${trade.paymentHash}`)}>
            <strong>{trade.amountMsat / 1000} sats</strong>
            <p class="tiny mono break">{shortHex(trade.paymentHash, 10)}</p>
          </button>
        {/each}
      </div>
    {/if}
  </section>
  <section class="panel">
    <h2>Invoices waiting on you</h2>
    {#if session.invoices.length === 0}
      <p class="empty">After the buyer creates the hold, their BOLT11 shows up here.</p>
    {:else}
      <button class="btn" type="button" onclick={() => go("/app/fund")}>Pay the hold</button>
    {/if}
  </section>
</div>

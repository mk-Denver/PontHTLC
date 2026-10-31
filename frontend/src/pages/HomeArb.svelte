<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import NextMove from "../components/NextMove.svelte";
</script>

<NextMove />

<div class="grid two">
  <section class="panel">
    <h2>Shares you hold</h2>
    {#if session.trades.length === 0}
      <p class="empty">When a seller names your key, s3 arrives here. You do nothing until someone disputes.</p>
    {:else}
      <div class="grid">
        {#each session.trades as trade (trade.paymentHash)}
          <button class="ticket" type="button" onclick={() => go("/app/cases")}>
            <span class="pill {trade.outcome === "disputed" ? "disputed" : "open"}">{trade.outcome ?? "holding s3"}</span>
            <p class="tiny mono break">{trade.paymentHash}</p>
          </button>
        {/each}
      </div>
    {/if}
  </section>
  <section class="panel">
    <h2>Open disputes</h2>
    {#if session.disputes.length === 0}
      <p class="empty">Quiet. A dispute from buyer or seller will light this up.</p>
    {:else}
      <button class="btn" type="button" onclick={() => go("/app/cases")}>{session.disputes.length} to review</button>
    {/if}
  </section>
</div>

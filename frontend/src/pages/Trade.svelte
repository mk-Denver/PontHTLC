<script lang="ts">
  import { loc, go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import NextMove from "../components/NextMove.svelte";
  import ShareBoard from "../components/ShareBoard.svelte";
  import Timeline from "../components/Timeline.svelte";

  const hash = $derived(loc.parts[2] ?? "");
  const trade = $derived(session.tradeFor(hash));
</script>

<button class="btn ghost" type="button" onclick={() => go("/app")}>Back</button>

{#if !trade}
  <p class="empty">No trade for that hash in this tab.</p>
{:else}
  <section class="hero-card" style="margin-top:16px">
    <div class="kicker">Your split</div>
    <h1>{trade.amountMsat / 1000} sats</h1>
    <p class="tiny mono break">{trade.paymentHash}</p>
    <Timeline paymentHash={trade.paymentHash} />
  </section>
  <NextMove paymentHash={trade.paymentHash} />
  <ShareBoard paymentHash={trade.paymentHash} />
{/if}

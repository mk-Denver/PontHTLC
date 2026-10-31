<script lang="ts">
  import { nextMove } from "../lib/flow";
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";

  let { paymentHash }: { paymentHash?: string } = $props();
  const move = $derived(nextMove(session, paymentHash));
</script>

{#if move}
  <section class="hero-card">
    <div class="kicker">Your next move</div>
    <h1>{move.title}</h1>
    <p class="tiny">{move.body}</p>
    <button class="btn" type="button" onclick={() => go(move.href)}>Go</button>
  </section>
{/if}

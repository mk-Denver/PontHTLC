<script lang="ts">
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";
</script>

<h1>The tape</h1>
<p class="tiny">Live from `/api/ws`. Filtered to this key — PaymentClaimable, PaymentSent, PaymentFailed, ChannelReady.</p>

{#if session.events.length === 0}
  <p class="empty">The wire is quiet. Fund or claim to see movement.</p>
{:else}
  <div class="panel" style="margin-top:16px">
    {#each session.events as event, index (`${event.kind}-${event.payment_hash}-${index}`)}
      <div class="share-dot">
        <strong>{event.kind}</strong>
        <span class="tiny mono">{event.payment_hash ? shortHex(event.payment_hash) : "—"}</span>
      </div>
    {/each}
  </div>
{/if}

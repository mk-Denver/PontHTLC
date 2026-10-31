<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";
</script>

<h1>Escrows</h1>
<p class="tiny">Only tickets owned by this key. The seller funds them; they cannot see this list.</p>

{#if session.escrows.length === 0}
  <p class="empty">Empty book. Create a hold from Offers.</p>
{:else}
  <div class="grid" style="margin-top:16px">
    {#each session.escrows as escrow (escrow.id)}
      <button class="ticket" type="button" onclick={() => go(`/app/escrow/${escrow.id}`)}>
        <div class="row">
          <span class="pill {escrow.protocol_status || escrow.state}">{escrow.protocol_status || escrow.state}</span>
          <span class="tiny mono">{shortHex(escrow.id)}</span>
        </div>
        <p class="tiny mono break">{escrow.payment_hash}</p>
      </button>
    {/each}
  </div>
{/if}

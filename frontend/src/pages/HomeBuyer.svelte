<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";
  import NextMove from "../components/NextMove.svelte";
</script>

<NextMove />

<div class="grid two">
  <section class="panel">
    <h2>Open tickets</h2>
    {#if session.escrows.length === 0}
      <p class="empty">Nothing held yet. When an offer arrives, this fills with invoices only you can claim.</p>
    {:else}
      <div class="grid">
        {#each session.escrows.slice(0, 4) as escrow (escrow.id)}
          <button class="ticket" type="button" onclick={() => go(`/app/escrow/${escrow.id}`)}>
            <div class="row">
              <span class="pill {escrow.protocol_status || escrow.state}">{escrow.protocol_status || escrow.state}</span>
              <span class="tiny mono">{shortHex(escrow.id)}</span>
            </div>
            <p class="tiny mono break">{shortHex(escrow.payment_hash, 10)}</p>
          </button>
        {/each}
      </div>
    {/if}
  </section>
  <section class="panel">
    <h2>Why this desk feels quiet</h2>
    <p class="tiny">You never originate P. If the other tab is open as seller, their “New trade” appears here as an offer. Manual hashes live under Offers.</p>
  </section>
</div>

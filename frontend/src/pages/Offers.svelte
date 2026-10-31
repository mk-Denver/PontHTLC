<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";

  let paymentHash = $state("");
  let amountSats = $state(10000);

  async function createManual() {
    session.error = "";
    session.busy = "creating";
    try {
      const created = await session.createManual(paymentHash, amountSats);
      go(`/app/escrow/${created.id}`);
    } catch (err) {
      session.error = err instanceof Error ? err.message : String(err);
    } finally {
      session.busy = "";
    }
  }
</script>

<h1>Offers</h1>
<p class="tiny">Live from the seller tab, or paste a hash if you are trading across devices.</p>

{#if session.offers.length}
  <div class="grid" style="margin:18px 0">
    {#each session.offers as offer (offer.paymentHash)}
      <article class="panel">
        <div class="kicker">Incoming</div>
        <h2>{offer.amountMsat / 1000} sats</h2>
        <p class="tiny mono break">{shortHex(offer.paymentHash, 12)}</p>
        <button
          class="btn"
          type="button"
          disabled={session.busy !== ""}
          onclick={async () => {
            await session.createFromOffer(offer);
            if (session.selectedId) {
              go(`/app/escrow/${session.selectedId}`);
            }
          }}
        >
          Hold this invoice
        </button>
      </article>
    {/each}
  </div>
{:else}
  <p class="empty">No gift-wrapped offers yet. Keep this tab open while the seller starts a trade.</p>
{/if}

<section class="panel">
  <h2>Paste a hash</h2>
  <label class="field">
    <span>Payment hash</span>
    <input class="mono" bind:value={paymentHash} placeholder="64 hex" />
  </label>
  <label class="field">
    <span>Amount (sats)</span>
    <input type="number" min="1" bind:value={amountSats} />
  </label>
  <button class="btn ghost" type="button" disabled={session.busy !== ""} onclick={createManual}>Create hold invoice</button>
</section>

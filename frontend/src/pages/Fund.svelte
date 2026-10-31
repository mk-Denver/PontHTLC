<script lang="ts">
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";
  import InvoiceQr from "../components/InvoiceQr.svelte";

  let pastedId = $state("");
  let pastedBolt11 = $state("");

  async function fundPasted() {
    session.error = "";
    await session.fund({ escrowId: pastedId, bolt11: pastedBolt11 });
  }
</script>

<h1>Fund a hold</h1>
<p class="tiny">Paying is the seller’s job. The creator of an invoice cannot fund it.</p>

{#if session.invoices.length === 0}
  <p class="empty">No invoices from the buyer tab yet. Paste one below if they sent it another way.</p>
{:else}
  <div class="grid" style="margin-top:16px">
    {#each session.invoices as invoice (invoice.escrowId)}
      <article class="panel">
        <p class="tiny mono">{shortHex(invoice.escrowId)}</p>
        <InvoiceQr bolt11={invoice.bolt11} />
        <button class="btn" type="button" disabled={session.busy !== ""} onclick={() => session.fund(invoice)}>
          Pay this invoice
        </button>
      </article>
    {/each}
  </div>
{/if}

<section class="panel" style="margin-top:16px">
  <h2>Paste an invoice</h2>
  <label class="field">
    <span>Escrow id (if you have it)</span>
    <input class="mono" bind:value={pastedId} />
  </label>
  <label class="field">
    <span>BOLT11</span>
    <textarea class="mono" bind:value={pastedBolt11}></textarea>
  </label>
  <button class="btn ghost" type="button" disabled={session.busy !== "" || !pastedBolt11} onclick={fundPasted}>
    Pay pasted invoice
  </button>
</section>

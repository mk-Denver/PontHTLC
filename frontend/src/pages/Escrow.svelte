<script lang="ts">
  import { loc, go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import InvoiceQr from "../components/InvoiceQr.svelte";
  import NextMove from "../components/NextMove.svelte";
  import ShareBoard from "../components/ShareBoard.svelte";
  import Timeline from "../components/Timeline.svelte";

  const id = $derived(loc.parts[2] ?? "");
  const escrow = $derived(session.escrows.find((item) => item.id === id));
  let resolver = $state("");

  $effect(() => {
    if (!resolver && session.arb) {
      resolver = session.arb;
    }
  });
</script>

<button class="btn ghost" type="button" onclick={() => go("/app/escrows")}>All escrows</button>

{#if !escrow}
  <p class="empty">This ticket is not on this key.</p>
{:else}
  <section class="hero-card" style="margin-top:16px">
    <div class="row">
      <span class="pill {escrow.protocol_status || escrow.state}">{escrow.protocol_status || escrow.state}</span>
      <span class="tiny">1-hour hold window from registration</span>
    </div>
    <h1>Hold {escrow.payment_hash.slice(0, 6)}</h1>
    <p class="tiny mono break">{escrow.payment_hash}</p>
    <Timeline paymentHash={escrow.payment_hash} />
  </section>
  <NextMove paymentHash={escrow.payment_hash} />

  <div class="grid two">
    <section class="panel">
      {#if escrow.bolt11}
        <InvoiceQr bolt11={escrow.bolt11} />
      {/if}
      {#if session.role === "buyer"}
        <div class="row" style="margin-top:16px">
          <button
            class="btn ok"
            type="button"
            disabled={session.busy !== "" || !session.shareStatus(escrow.payment_hash).ready}
            onclick={() => session.claim(escrow.id, escrow.payment_hash)}
          >
            Claim
          </button>
          <button class="btn danger" type="button" disabled={session.busy !== ""} onclick={() => session.fail(escrow.id)}>
            Fail
          </button>
        </div>
      {/if}
      <h3>Dispute</h3>
      <label class="field">
        <span>Arbitrator pubkey</span>
        <input class="mono" bind:value={resolver} />
      </label>
      {#if session.peersOf("arbitrator").length}
        <div class="row" style="margin-bottom:10px">
          {#each session.peersOf("arbitrator") as peer (peer.pubkey)}
            <button class="btn ghost" type="button" onclick={() => (resolver = peer.pubkey)}>{peer.name}</button>
          {/each}
        </div>
      {/if}
      <button class="btn ghost" type="button" onclick={() => session.dispute(escrow.id, resolver)}>Ask for s3</button>
      {#if session.giftWraps.length}
        <textarea readonly class="mono">{JSON.stringify(session.giftWraps, null, 2)}</textarea>
      {/if}
    </section>
    <ShareBoard paymentHash={escrow.payment_hash} />
  </div>
{/if}

<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";

  let amountSats = $state(10000);
  let buyerPk = $state(session.peer);
  let arbPk = $state(session.arb);

  $effect(() => {
    if (!buyerPk && session.peer) {
      buyerPk = session.peer;
    }
    if (!arbPk && session.arb) {
      arbPk = session.arb;
    }
  });

  async function start() {
    await session.originate(amountSats, buyerPk, arbPk);
    const latest = session.trades[0];
    if (latest) {
      go(`/app/trade/${latest.paymentHash}`);
    }
  }
</script>

<section class="hero-card">
  <div class="kicker">Seller only</div>
  <h1>Cut the secret.</h1>
  <p class="tiny">s1 to the buyer. s3 to the arb if you name one. s2 stays here until cash is in your hand.</p>
</section>

<form class="panel" onsubmit={(event) => { event.preventDefault(); void start(); }}>
  <label class="field">
    <span>Amount in sats</span>
    <input type="number" min="1" bind:value={amountSats} />
  </label>
  <label class="field">
    <span>Buyer pubkey</span>
    <input class="mono" bind:value={buyerPk} placeholder="Other tab fills this when it says hello" />
  </label>
  {#if session.peersOf("buyer").length}
    <div class="row" style="margin-bottom:12px">
      {#each session.peersOf("buyer") as peer (peer.pubkey)}
        <button class="btn ghost" type="button" onclick={() => (buyerPk = peer.pubkey)}>{peer.name}</button>
      {/each}
    </div>
  {/if}
  <label class="field">
    <span>Arbitrator pubkey (optional)</span>
    <input class="mono" bind:value={arbPk} placeholder="Sends s3 now if set" />
  </label>
  {#if session.peersOf("arbitrator").length}
    <div class="row" style="margin-bottom:12px">
      {#each session.peersOf("arbitrator") as peer (peer.pubkey)}
        <button class="btn ghost" type="button" onclick={() => (arbPk = peer.pubkey)}>{peer.name}</button>
      {/each}
    </div>
  {/if}
  <button class="btn" type="submit" disabled={session.busy !== ""}>Split and send shares</button>
</form>

<script lang="ts">
  import { go } from "../lib/router.svelte";
</script>

<div class="wrap">
  <nav class="site-nav">
    <a class="mark" href="#/"><i>P</i> PontHTLC</a>
    <a class="tiny" href="#/">Back to the street</a>
  </nav>
  <section class="auth">
    <div>
      <div class="kicker">Sign up</div>
      <h1 style="font-size:clamp(2.6rem,6vw,4.4rem);margin:8px 0 12px">Which side of the table?</h1>
      <p class="lede">No email. No custodial wallet. This tab mints a Nostr key and keeps it in session storage. Open a second tab for the other role.</p>
    </div>
    <div class="doors">
      <button class="door buy" type="button" onclick={() => go("/start/buyer")}>
        <div class="kicker">Buyer</div>
        <b>I have cash</b>
        <span class="tiny">You hold the invoice and the claim button.</span>
        <span class="go" aria-hidden="true">→</span>
      </button>
      <button class="door sell" type="button" onclick={() => go("/start/seller")}>
        <div class="kicker">Seller</div>
        <b>I have bitcoin</b>
        <span class="tiny">You create the secret and fund the hold.</span>
        <span class="go" aria-hidden="true">→</span>
      </button>
      <button class="door arb" type="button" onclick={() => go("/start/arbitrator")}>
        <div class="kicker">Arbitrator</div>
        <b>I hold the tie-break</b>
        <span class="tiny">You keep s3 and send it to one side if they fight.</span>
        <span class="go" aria-hidden="true">→</span>
      </button>
    </div>
  </section>
</div>

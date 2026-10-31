<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
</script>

<div class="wrap">
  <nav class="site-nav">
    <a class="mark" href="#/"><i>P</i> PontHTLC</a>
    <div class="nav-links">
      <a href="#/" onclick={() => document.getElementById("how")?.scrollIntoView({ behavior: "smooth" })}>How it holds</a>
      {#if session.role}
        <a href="#/app">Open desk</a>
      {:else}
        <a href="#/start">Sign in</a>
      {/if}
    </div>
  </nav>

  <section class="hero">
    <div>
      <div class="kicker">Non-custodial · 2-of-3 Shamir · Lightning hold</div>
      <h1>Cash in the street.<br />Sats that wait.</h1>
      <p class="lede">
        A hosted Lightning node for African P2P trades. The invoice is held.
        The preimage is split. This browser — not the host — puts the pieces together.
      </p>
      <div class="doors pair">
        <button class="door buy" type="button" onclick={() => go("/start/buyer")}>
          <div class="kicker">Buyer desk</div>
          <b>I am paying cash</b>
          <span class="tiny">Create the hold invoice, then claim when the street deal is done.</span>
          <span class="go" aria-hidden="true">→</span>
        </button>
        <button class="door sell" type="button" onclick={() => go("/start/seller")}>
          <div class="kicker">Seller desk</div>
          <b>I am sending bitcoin</b>
          <span class="tiny">Originate the secret, fund the invoice, release your share after cash.</span>
          <span class="go" aria-hidden="true">→</span>
        </button>
      </div>
      <p class="tiny" style="margin-top:14px">
        Sitting in the middle? <a href="#/start/arbitrator">Open an arbitrator desk</a>.
      </p>
    </div>

    <aside class="stub" aria-hidden="true">
      <div class="kicker">Trade ticket</div>
      <h2 class="serif" style="font-size:2.4rem;margin:10px 0 0">10,000 sats</h2>
      <p class="tiny">held · not claimed · hash unknown to host</p>
      <div class="cut"></div>
      <p class="tiny">Shares</p>
      <div class="dots" style="margin:10px 0 18px">
        <span></span><span></span><span class="off"></span>
      </div>
      <p class="mono tiny break">H · 7f3a…c91e</p>
      <p class="tiny">s1 buyer · s2 seller · s3 arb</p>
    </aside>
  </section>

  <section class="steps" id="how">
    <article class="step">
      <em>01</em>
      <h3>Seller splits P</h3>
      <p class="tiny">A random preimage is cut 2-of-3. You never type a seed into our server.</p>
    </article>
    <article class="step">
      <em>02</em>
      <h3>Buyer holds the invoice</h3>
      <p class="tiny">LDK registers the hash without the secret. The HTLC sits until someone claims or fails.</p>
    </article>
    <article class="step">
      <em>03</em>
      <h3>This tab reconstructs</h3>
      <p class="tiny">Two shares become P in the browser. Claim is a pass-through. The host does not keep P.</p>
    </article>
  </section>
</div>

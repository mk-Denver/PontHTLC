<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";

  $effect(() => {
    if (!session.role) {
      go("/start");
    }
  });
</script>

<div class="wrap">
  <nav class="site-nav">
    <a class="mark" href="#/"><i>P</i> PontHTLC</a>
  </nav>
  <section class="auth">
    <div>
      <div class="kicker">Desk ready</div>
      <h1 style="font-size:clamp(2.6rem,6vw,4.2rem);margin:8px 0 12px">Welcome, {session.name}.</h1>
      <p class="lede">This tab is the {session.role}. Open another tab and sign up as the other side to run a full trade on one machine.</p>
    </div>
    <div class="panel">
      <p class="tiny">Your public key</p>
      <p class="mono break">{session.pubkey}</p>
      <p class="tiny" style="margin-top:12px">Short: {shortHex(session.pubkey)}</p>
      <div class="row" style="margin-top:20px">
        <button class="btn" type="button" onclick={() => go("/app")}>Enter the {session.role} desk</button>
        <button class="btn ghost" type="button" onclick={() => navigator.clipboard.writeText(session.pubkey)}>Copy key</button>
      </div>
    </div>
  </section>
</div>

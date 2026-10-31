<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import type { Role } from "../lib/bus";

  let { role }: { role: Role } = $props();
  let name = $state("");

  const copy = $derived({
    buyer: {
      kicker: "Buyer signup",
      title: "You sit with the cash.",
      body: "This desk creates the hold invoice from a hash you do not invent. When two shares arrive, you reconstruct P here and claim.",
      steps: ["Receive the seller’s offer", "Publish a hold invoice", "Claim or fail the HTLC"],
    },
    seller: {
      kicker: "Seller signup",
      title: "You sit with the bitcoin.",
      body: "This desk originates P, keeps s2, and sends s1 as a gift wrap. After you have the cash, you release the second share.",
      steps: ["Split the preimage", "Fund the buyer’s invoice", "Release s2 — or wait for the arb"],
    },
    arbitrator: {
      kicker: "Arbitrator signup",
      title: "You sit between them.",
      body: "You never touch the hosted node. You hold s3 and, if they dispute, you send it to one side.",
      steps: ["Give the seller your pubkey", "Receive s3 at split time", "On dispute, release s3 once"],
    },
  }[role]);

  let secret = $state("");

  function create() {
    const fallback = role === "buyer" ? "Amina" : role === "seller" ? "Kwame" : "Fatou";
    session.chooseRole(role, name || fallback, secret);
    go("/ready");
  }
</script>

<div class="wrap">
  <nav class="site-nav">
    <a class="mark" href="#/"><i>P</i> PontHTLC</a>
    <a class="tiny" href="#/start">Choose the other side</a>
  </nav>

  <section class="auth split">
    <div>
      <div class="kicker">{copy.kicker}</div>
      <h1 style="font-size:clamp(2.4rem,5vw,3.8rem);margin:8px 0 12px">{copy.title}</h1>
      <p class="lede">{copy.body}</p>
      <ol class="tiny" style="padding-left:18px;line-height:1.9">
        {#each copy.steps as step}
          <li>{step}</li>
        {/each}
      </ol>
    </div>
    <form class="panel" onsubmit={(event) => { event.preventDefault(); create(); }}>
      <label class="field">
        <span>What should this tab call you?</span>
        <input bind:value={name} maxlength="24" placeholder={role === "buyer" ? "Amina" : role === "seller" ? "Kwame" : "Fatou"} />
      </label>
      <label class="field">
        <span>Existing secret (optional)</span>
        <input class="mono" bind:value={secret} placeholder="Leave blank to mint a new key" />
      </label>
      <p class="tiny">The key stays in this tab’s session storage. The API only sees the pubkey header.</p>
      <div class="row" style="margin-top:18px">
        <button class="btn" type="submit">Create {role} desk</button>
        <button class="btn ghost" type="button" onclick={() => go("/start")}>Not this role</button>
      </div>
    </form>
  </section>
</div>

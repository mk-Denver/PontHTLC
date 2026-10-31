<script lang="ts">
  import { go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";

  let peer = $state(session.peer);
  let arb = $state(session.arb);
  let relay = $state(session.relayUrl);

  function save() {
    session.setPeer(peer);
    session.setArb(arb);
    session.setRelay(relay);
    void session.refresh();
  }

  function leave() {
    session.leave();
    go("/");
  }
</script>

<h1>Desk</h1>
<p class="tiny">Hosted node, counterparty, and the key that owns this tab.</p>

<div class="grid two" style="margin-top:16px">
  <section class="panel">
    <h2>Node</h2>
    <p class="tiny">{session.health === "up" ? "Reachable through the Vite proxy" : "Start `ponthtlc` on :9800"}</p>
    <p class="tiny mono break">{session.nodePubkey || "—"}</p>
    <p class="tiny">{session.channels.length} channel{session.channels.length === 1 ? "" : "s"}</p>
    <button class="btn ghost" type="button" onclick={() => session.refresh()}>Ping again</button>
  </section>
  <section class="panel">
    <h2>This key</h2>
    <p class="tiny">{session.name} · {session.role}</p>
    <p class="mono break">{session.pubkey}</p>
    <button class="btn danger" type="button" onclick={leave}>Leave desk</button>
  </section>
</div>

{#if session.peers.length}
  <section class="panel" style="margin-top:14px">
    <h2>Tabs on this browser</h2>
    {#each session.peers as person (person.pubkey)}
      <p class="tiny">{person.name} · {person.role} · <span class="mono">{person.pubkey.slice(0, 16)}…</span></p>
    {/each}
  </section>
{/if}

<section class="panel" style="margin-top:14px">
  <label class="field">
    <span>Counterparty pubkey</span>
    <input class="mono" bind:value={peer} />
  </label>
  <label class="field">
    <span>Arbitrator pubkey</span>
    <input class="mono" bind:value={arb} />
  </label>
  <label class="field">
    <span>Nostr relay</span>
    <input bind:value={relay} placeholder="ws://127.0.0.1:7777" />
  </label>
  <button class="btn" type="button" onclick={save}>Save</button>
</section>

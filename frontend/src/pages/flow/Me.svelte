<script lang="ts">
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let name = $state(wallet.name);
  let address = $state(wallet.lightningAddress);
  let saved = $state(false);

  function save() {
    wallet.name = name.trim();
    wallet.lightningAddress = address.trim();
    wallet.save();
    saved = true;
    setTimeout(() => (saved = false), 2000);
  }

  function toggleMode() {
    wallet.toggleAgent();
  }
</script>

<main class="flow">
  <p class="kicker">Me</p>
  <h1>Your keys</h1>

  <div style="display:flex;gap:8px;margin-bottom:8px">
    <button
      class="btn ghost"
      style={!wallet.isAgent ? "border-color:var(--buyer);color:var(--buyer)" : ""}
      onclick={() => { if (wallet.isAgent) toggleMode(); }}
    >
      Personal
    </button>
    <button
      class="btn ghost"
      style={wallet.isAgent ? "border-color:var(--seller);color:var(--seller)" : ""}
      onclick={() => { if (!wallet.isAgent) toggleMode(); }}
    >
      Agent
    </button>
  </div>

  <label>
    Name
    <input bind:value={name} placeholder="Your display name" />
  </label>
  <label>
    Lightning address
    <input bind:value={address} placeholder="name@domain" />
  </label>
  <label>
    Nostr pubkey
    <textarea readonly rows="2">{wallet.pubkey}</textarea>
  </label>
  <button class="btn" type="button" onclick={save}>Save</button>
  {#if saved}
    <p class="muted">Saved on this device.</p>
  {/if}

  {#if wallet.isAgent}
    <p class="kicker" style="margin-top:12px">Agent mode</p>
    <p class="muted">You are in agent mode. Publish offers from the Desk tab. Incoming swap proposals appear on the home screen.</p>
  {:else}
    <p class="kicker" style="margin-top:12px">Personal mode</p>
    <p class="muted">You are in personal mode. Discover agent offers and swap bitcoin for local currency.</p>
  {/if}
  <Nav />
</main>
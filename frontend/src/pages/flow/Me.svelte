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
  }
</script>

<main class="flow">
  <p class="kicker">Me</p>
  <h1>Your keys</h1>
  <label>
    Name
    <input bind:value={name} />
  </label>
  <label>
    Lightning address
    <input bind:value={address} placeholder="name@domain" />
  </label>
  <label>
    Nostr pubkey
    <textarea readonly rows="3">{wallet.pubkey}</textarea>
  </label>
  <button class="btn" type="button" onclick={save}>Save</button>
  {#if saved}
    <p class="muted">Saved on this device.</p>
  {/if}
  <Nav />
</main>

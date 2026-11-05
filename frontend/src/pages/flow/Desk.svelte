<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "../../lib/api";
  import type { ChannelStatus } from "../../lib/api";
  import { wallet } from "../../lib/wallet.svelte";
  import Nav from "./Nav.svelte";

  let status = $state<ChannelStatus | null>(null);
  let height = $state<number | null>(null);
  let error = $state("");
  let busy = $state(false);

  async function refresh() {
    try {
      const [channels, info] = await Promise.all([
        api.channelStatus(wallet.pubkey),
        api.nodeInfo(wallet.pubkey),
      ]);
      status = channels;
      height = info.block_height;
      error = "";
    } catch (err) {
      error = err instanceof Error ? err.message : "Desk is offline.";
    }
  }

  async function openChannel() {
    busy = true;
    error = "";
    try {
      await api.openChannel(wallet.pubkey);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not open a channel.";
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<main class="flow">
  <p class="kicker">Desk</p>
  <h1>Operator</h1>
  <p class="muted">Block {height ?? "—"} · peers {status?.peers_connected ? "connected" : "quiet"}</p>
  <ul class="swap-list">
    {#each status?.channels ?? [] as channel}
      <li>
        <strong>{channel.channel_id.slice(0, 12)}</strong>
        <span>{channel.usable ? "usable" : channel.ready ? "ready" : "opening"}</span>
      </li>
    {:else}
      <li class="muted">No channel yet. Open one, then mine confirmations on regtest.</li>
    {/each}
  </ul>
  <button class="btn" type="button" disabled={busy} onclick={openChannel}>
    {busy ? "Opening…" : "Open channel"}
  </button>
  {#if error}
    <p class="warn">{error}</p>
  {/if}
  <Nav />
</main>

<script lang="ts">
  import { session } from "../lib/session.svelte";

  let { paymentHash }: { paymentHash: string } = $props();
  let pasted = $state("");

  const labels: Record<1 | 2 | 3, string> = { 1: "s1 · buyer", 2: "s2 · seller", 3: "s3 · arb" };
  const status = $derived(session.shareStatus(paymentHash));
  const reconstructed = $derived(session.reconstructed[paymentHash]);

  function reconstruct() {
    session.error = "";
    try {
      session.reconstruct(paymentHash);
      session.notice = "P is in memory only. Claim sends it once.";
    } catch (err) {
      session.error = err instanceof Error ? err.message : String(err);
    }
  }

  async function copyShare(x: 1 | 2 | 3) {
    const share = session.shares[paymentHash]?.[x];
    if (share) {
      await navigator.clipboard.writeText(JSON.stringify(share));
    }
  }

  function paste() {
    session.ingestPastedShare(paymentHash, pasted);
    session.notice = "Share stored in this tab.";
    pasted = "";
  }
</script>

<section class="panel">
  <h2>Shares</h2>
  <p class="tiny">Two lights on — you can reconstruct. The host never sees these bytes.</p>
  {#each [1, 2, 3] as x (x)}
    {@const idx = x as 1 | 2 | 3}
    <div class="share-dot">
      <div>
        <strong>{labels[idx]}</strong>
        <div class="tiny">{status.held.includes(idx) ? "in this browser" : "not here"}</div>
      </div>
      {#if session.shares[paymentHash]?.[idx]}
        <button class="btn ghost" type="button" onclick={() => copyShare(idx)}>Copy</button>
      {/if}
    </div>
  {/each}
  <div class="row" style="margin-top:16px">
    {#if session.role === "seller"}
      <button class="btn ok" type="button" disabled={!session.shares[paymentHash]?.[2]} onclick={() => session.release(paymentHash)}>
        Release s2
      </button>
    {/if}
    <button class="btn" type="button" disabled={!status.ready} onclick={reconstruct}>Reconstruct P</button>
  </div>
  {#if reconstructed}
    <p class="tiny">Ready to claim. Not written to disk.</p>
    <code class="mono tiny break">{reconstructed}</code>
  {/if}
  <label class="field" style="margin-top:16px">
    <span>Paste a share JSON</span>
    <textarea class="mono" bind:value={pasted} placeholder={'{"x":1,"bytes":"..."}'}></textarea>
  </label>
  <button class="btn ghost" type="button" onclick={paste}>Store share</button>
</section>

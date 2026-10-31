<script lang="ts">
  import { session } from "../lib/session.svelte";
  import { shortHex } from "../lib/hex";
  import ShareBoard from "../components/ShareBoard.svelte";

  let winner = $state("");
  let selected = $derived(session.trades[0]?.paymentHash ?? "");

  const buyers = $derived(session.peersOf("buyer"));
  const sellers = $derived(session.peersOf("seller"));

  async function send() {
    session.error = "";
    try {
      await session.release(selected, winner);
    } catch (err) {
      session.error = err instanceof Error ? err.message : String(err);
    }
  }
</script>

<h1>Cases</h1>
<p class="tiny">You hold s3. Send it to exactly one side when you have decided.</p>

{#if session.trades.length === 0 && session.disputes.length === 0}
  <p class="empty">No shares and no disputes yet.</p>
{:else}
  <div class="grid two" style="margin-top:16px">
    <section class="panel">
      <h2>Tickets</h2>
      {#each session.trades as trade (trade.paymentHash)}
        <button class="ticket" class:active={selected === trade.paymentHash} type="button" onclick={() => (selected = trade.paymentHash)}>
          <span class="pill {trade.outcome ?? "open"}">{trade.outcome ?? "holding"}</span>
          <p class="tiny mono break">{shortHex(trade.paymentHash, 12)}</p>
        </button>
      {/each}
      {#each session.disputes as dispute (dispute.escrowId)}
        <p class="tiny">Dispute on {shortHex(dispute.paymentHash)} from {shortHex(dispute.from)}</p>
      {/each}
    </section>
    {#if selected}
      <div>
        <ShareBoard paymentHash={selected} />
        <form class="panel" style="margin-top:14px" onsubmit={(event) => { event.preventDefault(); void send(); }}>
          <label class="field">
            <span>Send s3 to</span>
            <input class="mono" bind:value={winner} placeholder="64 hex pubkey" />
          </label>
          {#if buyers[0] || sellers[0]}
            <div class="row">
              {#each [...buyers, ...sellers] as peer (peer.pubkey)}
                <button class="btn ghost" type="button" onclick={() => (winner = peer.pubkey)}>
                  {peer.name} · {peer.role}
                </button>
              {/each}
            </div>
          {/if}
          <button class="btn" type="submit" style="margin-top:12px">Release s3</button>
        </form>
      </div>
    {/if}
  </div>
{/if}

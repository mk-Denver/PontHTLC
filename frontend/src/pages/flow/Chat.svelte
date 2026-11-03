<script lang="ts">
  import { loc } from "../../lib/router.svelte";
  import Nav from "./Nav.svelte";

  const id = $derived(loc.parts[1] ?? "");
  const storageKey = $derived(`ponthtlc.chat.${id}`);
  let draft = $state("");
  let lines = $state<string[]>([]);

  function read(): string[] {
    try {
      return JSON.parse(localStorage.getItem(storageKey) || "[]") as string[];
    } catch {
      return [];
    }
  }

  $effect(() => {
    lines = read();
    const channel = new BroadcastChannel(`ponthtlc.chat.${id}`);
    channel.onmessage = () => {
      lines = read();
    };
    return () => channel.close();
  });

  function send() {
    const text = draft.trim();
    if (!text) {
      return;
    }
    const next = [...read(), text];
    localStorage.setItem(storageKey, JSON.stringify(next));
    lines = next;
    draft = "";
    const channel = new BroadcastChannel(`ponthtlc.chat.${id}`);
    channel.postMessage(text);
    channel.close();
  }
</script>

<main class="flow">
  <p class="kicker">Chat</p>
  <h1>Swap {id.slice(0, 8)}</h1>
  <ul class="swap-list">
    {#each lines as line}
      <li>{line}</li>
    {:else}
      <li class="muted">Say when the cash is sent, or paste a share the other person can copy.</li>
    {/each}
  </ul>
  <label>
    Message
    <textarea bind:value={draft} rows="3"></textarea>
  </label>
  <button class="btn" type="button" onclick={send}>Send</button>
  <a class="btn ghost" href="#/swap/{id}">Back to swap</a>
  <Nav />
</main>

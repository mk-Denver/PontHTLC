<script lang="ts">
  import { onMount } from "svelte";
  import { loc, startRouter } from "./lib/router.svelte";
  import { wallet } from "./lib/wallet.svelte";
  import Chat from "./pages/flow/Chat.svelte";
  import Desk from "./pages/flow/Desk.svelte";
  import Me from "./pages/flow/Me.svelte";
  import NewSwap from "./pages/flow/NewSwap.svelte";
  import Swap from "./pages/flow/Swap.svelte";
  import Wallet from "./pages/flow/Wallet.svelte";

  wallet.boot();

  onMount(() => startRouter());

  const path = $derived(loc.pathname);
  const swapId = $derived(loc.parts[0] === "swap" ? loc.parts[1] : "");
</script>

<svelte:head>
  <title>PontHTLC</title>
</svelte:head>

{#if path === "/swap/new"}
  <NewSwap />
{:else if swapId && loc.parts[2] === "chat"}
  <Chat />
{:else if swapId}
  <Swap />
{:else if path === "/me"}
  <Me />
{:else if path === "/desk"}
  <Desk />
{:else}
  <Wallet />
{/if}

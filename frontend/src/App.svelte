<script lang="ts">
  import { onMount } from "svelte";
  import { loc, startRouter } from "./lib/router.svelte";
  import { session } from "./lib/session.svelte";
  import Landing from "./pages/Landing.svelte";
  import Ready from "./pages/Ready.svelte";
  import Shell from "./pages/Shell.svelte";
  import Start from "./pages/Start.svelte";
  import StartRole from "./pages/StartRole.svelte";

  onMount(() => {
    const stop = startRouter();
    if (session.role) {
      session.boot();
    }
    return stop;
  });

  $effect(() => {
    const fromRoute = loc.pathname.includes("arbitrator")
      ? "arbitrator"
      : loc.pathname.includes("seller")
        ? "seller"
        : loc.pathname.includes("buyer")
          ? "buyer"
          : "";
    document.documentElement.dataset.role = session.role ?? fromRoute;
  });
</script>

<svelte:head>
  <title>PontHTLC</title>
</svelte:head>

{#if loc.pathname === "/"}
  <Landing />
{:else if loc.pathname === "/start"}
  <Start />
{:else if loc.pathname === "/start/buyer"}
  <StartRole role="buyer" />
{:else if loc.pathname === "/start/seller"}
  <StartRole role="seller" />
{:else if loc.pathname === "/start/arbitrator"}
  <StartRole role="arbitrator" />
{:else if loc.pathname === "/ready"}
  <Ready />
{:else if loc.pathname.startsWith("/app")}
  <Shell />
{:else}
  <Landing />
{/if}

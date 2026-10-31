<script lang="ts">
  import { loc, go } from "../lib/router.svelte";
  import { session } from "../lib/session.svelte";
  import Activity from "./Activity.svelte";
  import Cases from "./Cases.svelte";
  import Desk from "./Desk.svelte";
  import Escrow from "./Escrow.svelte";
  import Escrows from "./Escrows.svelte";
  import Fund from "./Fund.svelte";
  import Guide from "./Guide.svelte";
  import HomeArb from "./HomeArb.svelte";
  import HomeBuyer from "./HomeBuyer.svelte";
  import HomeSeller from "./HomeSeller.svelte";
  import NewTrade from "./NewTrade.svelte";
  import Offers from "./Offers.svelte";
  import Trade from "./Trade.svelte";

  $effect(() => {
    if (!session.role) {
      go("/start");
    }
  });

  const here = $derived(loc.pathname);
  const role = $derived(session.role);

  const links = $derived(
    role === "buyer"
      ? [
          ["/app", "Home"],
          ["/app/offers", "Offers"],
          ["/app/escrows", "Escrows"],
          ["/app/activity", "Tape"],
        ]
      : role === "seller"
        ? [
            ["/app", "Home"],
            ["/app/new", "New"],
            ["/app/fund", "Fund"],
            ["/app/activity", "Tape"],
          ]
        : [
            ["/app", "Home"],
            ["/app/cases", "Cases"],
            ["/app/activity", "Tape"],
            ["/app/guide", "Guide"],
          ],
  );

  function active(href: string) {
    if (href === "/app") {
      return here === "/app" || here === "/app/home";
    }
    return here === href || here.startsWith(`${href}/`);
  }
</script>

<div class="shell" data-role={session.role ?? "buyer"}>
  <aside class="side">
    <a class="mark" href="#/"><i>P</i> {session.name}</a>
    <nav>
      {#each links as [href, label]}
        <a href="#{href}" class:active={active(href)}>{label}</a>
      {/each}
      {#if role !== "arbitrator"}
        <a href="#/app/guide" class:active={here === "/app/guide"}>Guide</a>
      {/if}
      <a href="#/app/desk" class:active={here === "/app/desk"}>Desk</a>
    </nav>
    <p class="tiny">{session.role} · {session.health === "up" ? "node live" : "node quiet"}</p>
    {#if session.peers[0]}
      <p class="tiny">Also here: {session.peers.map((peer) => peer.name).join(", ")}</p>
    {/if}
  </aside>

  <div>
    <div class="wrap main">
      <header class="topbar">
        <div>
          <div class="kicker">{session.role} desk</div>
          <strong>{session.name}</strong>
        </div>
        <a class="chip" class:on={session.health === "up"} href="#/app/desk">
          {session.health === "up" ? `height ${session.blockHeight}` : "daemon offline"}
        </a>
      </header>

      {#if session.error}
        <p class="banner err">{session.error}</p>
      {/if}
      {#if session.notice}
        <p class="banner">{session.notice}</p>
      {/if}

      {#if here === "/app" || here === "/app/home"}
        {#if role === "buyer"}
          <HomeBuyer />
        {:else if role === "seller"}
          <HomeSeller />
        {:else}
          <HomeArb />
        {/if}
      {:else if here === "/app/offers"}
        <Offers />
      {:else if here === "/app/escrows"}
        <Escrows />
      {:else if here.startsWith("/app/escrow/")}
        <Escrow />
      {:else if here === "/app/new"}
        <NewTrade />
      {:else if here === "/app/fund"}
        <Fund />
      {:else if here === "/app/cases"}
        <Cases />
      {:else if here === "/app/activity"}
        <Activity />
      {:else if here === "/app/guide"}
        <Guide />
      {:else if here === "/app/desk"}
        <Desk />
      {:else if here.startsWith("/app/trade/")}
        <Trade />
      {:else if role === "buyer"}
        <HomeBuyer />
      {:else if role === "seller"}
        <HomeSeller />
      {:else}
        <HomeArb />
      {/if}
    </div>

    <nav class="tabbar">
      {#each links as [href, label]}
        <a href="#{href}" class:active={active(href)}>{label}</a>
      {/each}
    </nav>
  </div>
</div>

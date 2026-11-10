<script lang="ts">
  import { loc } from "../../lib/router.svelte";
  import { wallet } from "../../lib/wallet.svelte";

  const here = $derived(loc.pathname);

  const buyerItems = [
    ["/", "Wallet"],
    ["/swap/new", "Swap"],
    ["/me", "Me"],
  ];

  const agentItems = [
    ["/", "Desk"],
    ["/desk", "Offers"],
    ["/me", "Me"],
  ];

  const items = $derived(wallet.isAgent ? agentItems : buyerItems);

  function current(href: string) {
    if (href === "/") {
      return here === "/";
    }
    return here === href || here.startsWith(`${href}/`);
  }
</script>

<nav class="dock" aria-label="Main">
  {#each items as [href, label]}
    <a href="#{href}" class:active={current(href)}>
      {label}
    </a>
  {/each}
  <a
    href="#/me"
    class:active={here === "/me"}
    style="color:{wallet.isAgent ? 'var(--seller)' : 'var(--buyer)'}; font-size:0.68rem; padding-top:6px"
  >
    {wallet.isAgent ? "Agent" : "Personal"}
  </a>
</nav>
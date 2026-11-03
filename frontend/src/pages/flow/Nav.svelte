<script lang="ts">
  import { loc } from "../../lib/router.svelte";

  const here = $derived(loc.pathname);
  const items = [
    ["/", "Wallet"],
    ["/swap/new", "New"],
    ["/desk", "Desk"],
    ["/me", "Me"],
  ];

  function current(href: string) {
    if (href === "/") {
      return here === "/";
    }
    return here === href || here.startsWith(`${href}/`);
  }
</script>

<nav class="dock" aria-label="Main">
  {#each items as [href, label]}
    <a href="#{href}" class:active={current(href)}>{label}</a>
  {/each}
</nav>

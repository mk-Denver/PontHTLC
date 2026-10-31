<script lang="ts">
  import { tradeSteps } from "../lib/flow";
  import { session } from "../lib/session.svelte";

  let { paymentHash }: { paymentHash: string } = $props();
  const steps = $derived(tradeSteps(session, paymentHash));
</script>

<ol class="timeline">
  {#each steps as step}
    <li class:done={step.done} class:now={step.current}>
      <i></i>
      <span>{step.label}</span>
    </li>
  {/each}
</ol>

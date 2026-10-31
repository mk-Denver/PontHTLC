<script lang="ts">
  import { qrSvg } from "../lib/qr";

  let { bolt11 }: { bolt11: string } = $props();
</script>

<div class="kv">
  <div class="qr">{@html qrSvg(bolt11)}</div>
  <p class="mono tiny break">{bolt11}</p>
  <button class="btn ghost" type="button" onclick={() => navigator.clipboard.writeText(bolt11)}>Copy invoice</button>
</div>

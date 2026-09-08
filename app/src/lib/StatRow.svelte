<script lang="ts">
  let {
    label,
    value,
    tone = "normal",
  }: { label: string; value: string; tone?: "normal" | "ok" | "warn" | "bad" } = $props();
</script>

<div class="row">
  <span class="k">{label}</span>
  <span class="v {tone}">{value}</span>
</div>

<style>
  .row { display: flex; justify-content: space-between; gap: 12px; padding: 5px 0; font-size: 13px; }
  .k { color: var(--dim); }
  .v { font-variant-numeric: tabular-nums; }
  .v.ok { color: var(--ok); }
  .v.warn { color: var(--warn); }
  .v.bad { color: var(--bad); }
</style>

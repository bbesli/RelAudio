<script lang="ts">
  import type { Device, DeviceKind } from "./api";

  let {
    devices,
    kind,
    value = $bindable(),
    label,
    hint = "",
    emptyText = "—",
    defaultText = "default",
  }: {
    devices: Device[];
    kind: DeviceKind;
    value: string;
    label: string;
    hint?: string;
    emptyText?: string;
    defaultText?: string;
  } = $props();

  const options = $derived(devices.filter((d) => d.kind === kind));

  // Seçim boşsa ya da listede yoksa varsayılana düş.
  $effect(() => {
    if (!options.length) return;
    if (!value || !options.some((d) => d.id === value)) {
      value = (options.find((d) => d.is_default) ?? options[0]).id;
    }
  });
</script>

<label class="field">
  <span class="lbl">{label}</span>
  <select bind:value>
    {#each options as d (d.id)}
      <option value={d.id}>{d.name}{d.is_default ? `  ·  ${defaultText}` : ""}</option>
    {/each}
    {#if !options.length}
      <option value="">{emptyText}</option>
    {/if}
  </select>
  {#if hint}<span class="hint">{hint}</span>{/if}
</label>

<style>
  .field { display: block; margin-bottom: 14px; }
  .lbl { display: block; font-size: 12px; color: var(--dim); margin-bottom: 6px; }
  .hint { display: block; font-size: 11px; color: var(--dim); margin-top: 5px; line-height: 1.45; }
</style>

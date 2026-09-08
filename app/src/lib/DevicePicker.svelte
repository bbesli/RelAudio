<script lang="ts">
  import type { Device, DeviceKind } from "./api";

  /**
   * Aygıt seçici.
   *
   * Değer tek yönlü gelir, değişiklik `onselect` ile bildirilir. İki yönlü
   * bağlama kullanılmıyor çünkü seçimin `persist()` üzerinden geçmesi gerek:
   * ayar diske yazılmalı ve çalışan akış yeniden kurulmalı.
   */
  let {
    devices,
    kind,
    value,
    onselect,
    label,
    hint = "",
    emptyText = "—",
    defaultText = "default",
  }: {
    devices: Device[];
    kind: DeviceKind;
    value: string;
    onselect: (id: string) => void;
    label: string;
    hint?: string;
    emptyText?: string;
    defaultText?: string;
  } = $props();

  // Tercih sırası: kanonik uç (rank 0) çok kanallı varyanttan (rank 1) önce.
  // Alfabetik sırada "CABLE In 16ch" < "CABLE Input" olduğu için varsayılan
  // seçim yanlış uca düşüyordu.
  const options = $derived(
    devices
      .filter((d) => d.kind === kind)
      .slice()
      .sort((a, b) => (a.rank ?? 0) - (b.rank ?? 0))
  );

  // Seçim boşsa ya da artık listede yoksa varsayılana düş — ama yalnızca
  // gerçekten değişiyorsa bildir, yoksa sonsuz döngü olur.
  $effect(() => {
    if (!options.length) return;
    if (value && options.some((d) => d.id === value)) return;
    // Varsayılan aygıt ancak tercih edilen sıradaysa seçilsin.
    const preferred = options.filter((d) => (d.rank ?? 0) === (options[0].rank ?? 0));
    const fallback = (preferred.find((d) => d.is_default) ?? preferred[0]).id;
    if (fallback !== value) onselect(fallback);
  });
</script>

<label class="field">
  <span class="lbl">{label}</span>
  <select {value} onchange={(e) => onselect(e.currentTarget.value)}>
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

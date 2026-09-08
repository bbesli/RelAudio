<script lang="ts">
  /**
   * Gelen sesin seviye çubuğu.
   *
   * Paket sayacı "veri geliyor" der ama "ses var" demez — sessizlik de paket
   * üretiyor. Kullanıcının zincirin çalıştığını gözle görmesi için gerek.
   */
  let {
    peak,
    label,
    waitingText,
  }: { peak: number; label: string; waitingText: string } = $props();

  // Logaritmik ölçek: konuşma seviyesi (-30 dB civarı) çubuğun ortasına gelsin.
  // Doğrusal ölçekte normal konuşma çubuğun soluna sıkışıp görünmez oluyor.
  const ratio = $derived.by(() => {
    if (peak <= 0) return 0;
    const db = 20 * Math.log10(peak / 32767);
    return Math.max(0, Math.min(1, (db + 60) / 60));
  });

  const segments = 28;
  const lit = $derived(Math.round(ratio * segments));
</script>

<div class="meter">
  <div class="head">
    <span class="lbl">{label}</span>
    {#if peak <= 0}<span class="waiting">{waitingText}</span>{/if}
  </div>
  <div class="bars" role="meter" aria-valuenow={Math.round(ratio * 100)}
       aria-valuemin="0" aria-valuemax="100" aria-label={label}>
    {#each Array(segments) as _, i}
      <span class="seg" class:on={i < lit} class:hot={i > segments * 0.85}></span>
    {/each}
  </div>
</div>

<style>
  .meter { margin-bottom: 16px; }
  .head { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 6px; }
  .lbl { font-size: 12px; color: var(--dim); }
  .waiting { font-size: 11px; color: var(--dim); font-style: italic; }
  .bars { display: flex; gap: 2px; height: 22px; }
  .seg {
    flex: 1; border-radius: 2px; background: var(--panel-2);
    transition: background 80ms linear;
  }
  .seg.on { background: var(--ok); }
  .seg.hot.on { background: var(--warn); }
</style>

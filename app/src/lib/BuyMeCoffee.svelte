<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";

  let {
    label, title, body, failedText,
  }: { label: string; title: string; body: string; failedText: string } = $props();

  const URL = "https://buymeacoffee.com/bbesli";

  // Tarayıcı açılamazsa adresi göster ki kullanıcı elle kopyalayabilsin.
  // Önceki sürüm hatayı yutuyordu: butona basınca hiçbir şey olmuyor,
  // sebebi de görünmüyordu.
  let failed = $state(false);

  async function open() {
    try {
      await openUrl(URL);
      failed = false;
    } catch (e) {
      console.error("bağlantı açılamadı:", e);
      failed = true;
    }
  }
</script>

<div class="bmc">
  <strong>{title}</strong>
  <p>{body}</p>
  <button onclick={open}>
    <!-- BMC kupası kendi renkleriyle çizildi: dış kaynaktan görsel
         çekmiyoruz, çevrimdışı da doğru görünsün. -->
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
      <path d="M4 6h13v7a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5V6z" fill="#4b3a2a"/>
      <path d="M17 8h1.5a2.5 2.5 0 0 1 0 5H17" fill="none" stroke="#4b3a2a" stroke-width="1.6"/>
      <path d="M6.5 3.2c.8.6.8 1.4 0 2M9.5 2.6c.9.7.9 1.7 0 2.4M12.5 3.2c.8.6.8 1.4 0 2"
            fill="none" stroke="#4b3a2a" stroke-width="1.3" stroke-linecap="round"/>
      <rect x="3" y="19.4" width="15" height="1.9" rx=".95" fill="#4b3a2a"/>
    </svg>
    <span>{label}</span>
  </button>

  {#if failed}
    <p class="fallback">{failedText}<br /><code>{URL}</code></p>
  {/if}
</div>

<style>
  .bmc { border-top: 1px solid var(--line); padding-top: 18px; margin-top: 20px; }
  .bmc strong { display: block; font-size: 14px; }
  .bmc p { margin: 5px 0 12px; color: var(--dim); font-size: 12px; line-height: 1.5; }
  button {
    display: inline-flex; align-items: center; gap: 9px;
    background: #ffdd00; color: #12141a; border: none;
    font-weight: 700; padding: 10px 16px; border-radius: 8px;
  }
  button:hover { background: #ffe74d; }
  .fallback { margin-top: 10px; color: var(--warn); }
  .fallback code { color: var(--text); user-select: text; font-size: 12px; }
</style>

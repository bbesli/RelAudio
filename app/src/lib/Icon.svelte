<script module lang="ts">
  /**
   * Satır içi SVG ikonlar.
   *
   * Dış ikon fontu ya da CDN kullanılmıyor: uygulama çevrimdışı çalışıyor ve
   * içerik güvenlik politikası (CSP) yalnızca kendi kaynaklarımıza izin
   * veriyor. Çizimler 24×24 ızgarada, `currentColor` ile — renk çevreden gelir.
   */
  export type IconName =
    | "headset" | "send" | "receive" | "settings" | "link" | "check"
    | "linux" | "windows" | "monitor" | "refresh" | "stop" | "play"
    | "wifi" | "shield" | "alert" | "arrow-right" | "mic" | "speaker" | "x";
</script>

<script lang="ts">
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
     stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === "headset"}
    <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
    <rect x="3" y="14" width="4" height="6" rx="1.5" />
    <rect x="17" y="14" width="4" height="6" rx="1.5" />
  {:else if name === "send"}
    <path d="M12 19V5" /><path d="m6 11 6-6 6 6" />
  {:else if name === "receive"}
    <path d="M12 5v14" /><path d="m18 13-6 6-6-6" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  {:else if name === "link"}
    <path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7" />
    <path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7" />
  {:else if name === "check"}
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  {:else if name === "linux"}
    <path d="M12 3c-2 0-3 1.7-3 4v2.5c0 1-1.4 2.4-2.3 4.1C5.8 15.3 5 17 6 18.5c.7 1 2 .8 3 1.2 1 .5 1.7 1.3 3 1.3s2-.8 3-1.3c1-.4 2.3-.2 3-1.2 1-1.5.2-3.2-.7-4.9C16.4 11.9 15 10.5 15 9.5V7c0-2.3-1-4-3-4z" />
    <circle cx="10.5" cy="7.5" r=".6" fill="currentColor" /><circle cx="13.5" cy="7.5" r=".6" fill="currentColor" />
  {:else if name === "windows"}
    <path d="M3 5.5 10.5 4.5v7H3zM12.5 4.2 21 3v8.5h-8.5zM3 12.5h7.5v7L3 18.5zM12.5 12.5H21V21l-8.5-1.2z" />
  {:else if name === "monitor"}
    <rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 0 0-14.3-4.9L4 8" /><path d="M4 4v4h4" />
    <path d="M4 13a8 8 0 0 0 14.3 4.9L20 16" /><path d="M20 20v-4h-4" />
  {:else if name === "stop"}
    <rect x="6" y="6" width="12" height="12" rx="2" />
  {:else if name === "play"}
    <path d="M7 5v14l12-7z" />
  {:else if name === "wifi"}
    <path d="M2 8.8a15 15 0 0 1 20 0" /><path d="M5 12.6a10 10 0 0 1 14 0" />
    <path d="M8.5 16.2a5 5 0 0 1 7 0" /><circle cx="12" cy="19.5" r=".8" fill="currentColor" />
  {:else if name === "shield"}
    <path d="M12 3 5 6v5c0 4.5 3 8.3 7 10 4-1.7 7-5.5 7-10V6z" /><path d="m9 12 2 2 4-4" />
  {:else if name === "alert"}
    <path d="M12 4 2.5 20h19z" /><path d="M12 10v4" /><circle cx="12" cy="17" r=".6" fill="currentColor" />
  {:else if name === "arrow-right"}
    <path d="M5 12h14" /><path d="m13 6 6 6-6 6" />
  {:else if name === "mic"}
    <rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0" /><path d="M12 18v3" />
  {:else if name === "speaker"}
    <path d="M4 9h4l5-4v14l-5-4H4z" /><path d="M16.5 8.5a5 5 0 0 1 0 7" /><path d="M19 6a8.5 8.5 0 0 1 0 12" />
  {:else if name === "x"}
    <path d="M6 6l12 12M18 6 6 18" />
  {/if}
</svg>

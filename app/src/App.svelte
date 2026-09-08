<script lang="ts">
  import {
    listDevices, startServer, stopServer, startPlayer, stopPlayer, getStats,
    setMinimizeToTray, getLocalAddress, getPeers, getDeviceName, getMicHint,
    getConfig, setConfig, getConfigPath, getLogPath, setTrayLabels,
    type Device, type Stats, type Peer, type MicHint, type Config,
  } from "./lib/api";
  import { LOCALES, translator, detectLocale, isRtl } from "./lib/i18n";
  import DevicePicker from "./lib/DevicePicker.svelte";
  import LevelMeter from "./lib/LevelMeter.svelte";
  import StatRow from "./lib/StatRow.svelte";
  import BuyMeCoffee from "./lib/BuyMeCoffee.svelte";

  type Tab = "server" | "player" | "settings";

  let tab = $state<Tab>("server");
  let devices = $state<Device[]>([]);
  let stats = $state<Stats | null>(null);
  let peers = $state<Peer[]>([]);
  let micHint = $state<MicHint | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  let myName = $state("");
  let localAddress = $state<string | null>(null);
  let configPath = $state("");
  let logPath = $state("");

  // Ayarlar diskten gelir; her anlamlı değişiklikte geri yazılır.
  let cfg = $state<Config | null>(null);
  let lang = $state("en");
  let selectedPeer = $state("");

  const t = $derived(translator(lang));

  const outputChoices = $derived(
    devices.filter(
      (d) => d.kind === "output" && (cfg?.player_mode !== "mic" || d.virtual_cable)
    )
  );

  async function refresh() {
    try {
      devices = await listDevices();
      localAddress = await getLocalAddress();
      myName = await getDeviceName();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function tick() {
    try {
      stats = await getStats();
      peers = await getPeers();
      if (stats.last_error) error = stats.last_error;
    } catch { /* pencere kapanırken olabilir */ }
  }

  /** Ayarlar okunamazsa kullanılacak taban. */
  function fallbackConfig(): Config {
    return {
      language: "", minimize_to_tray: true, auto_listen: true,
      player_mode: "listen", player_port: 59101, player_device: "", player_buffer: 8,
      server_source: "monitor", server_device_monitor: "", server_device_input: "",
      server_target: "",
    };
  }

  // Açılış: ayarları yükle, dili belirle, aygıtları çek, sayacı başlat.
  $effect(() => {
    (async () => {
      // Ayarlar okunamazsa varsayılanlarla devam et. Aksi hâlde arayüz hiç
      // çizilmiyor ve kullanıcı bomboş bir pencere görüyordu.
      let c: Config;
      try {
        c = await getConfig();
      } catch (e) {
        error = String(e);
        c = fallbackConfig();
      }
      lang = c.language || detectLocale();
      if (!c.language) c.language = lang;
      cfg = c;
      configPath = await getConfigPath().catch(() => "");
      logPath = await getLogPath().catch(() => "");
      await refresh();
      await tick();
    })();
    const id = setInterval(tick, 500);
    return () => clearInterval(id);
  });

  // Arapça sağdan sola.
  $effect(() => {
    document.documentElement.dir = isRtl(lang) ? "rtl" : "ltr";
    document.documentElement.lang = lang;
  });

  // Tepsi menüsü Rust tarafında kuruluyor; etiketleri dil değiştikçe gönder.
  $effect(() => {
    const tr = translator(lang);
    setTrayLabels(tr("tray.show"), tr("tray.stopAll"), tr("tray.quit")).catch(() => {});
  });

  // Seçili eş ağdan kaybolursa seçimi bırak, elle adres alanı geri gelsin.
  // Aksi hâlde açılır liste boş kalıyor ve hedef girilemez hâle geliyordu.
  $effect(() => {
    if (selectedPeer && !peers.some((p) => p.id === selectedPeer)) {
      selectedPeer = "";
    }
  });

  $effect(() => {
    const id = cfg?.player_device;
    if (!id) { micHint = null; return; }
    getMicHint(id).then((h) => (micHint = h)).catch(() => (micHint = null));
  });

  // Akışı yeniden kurmayı gerektiren ayarlar. Bunlar açık bir akışa
  // uygulanamıyor: aygıt/port akış açılırken belirleniyor.
  const PLAYER_RESTART_KEYS = ["player_device", "player_port", "player_buffer"] as const;
  const SERVER_RESTART_KEYS = ["server_device_monitor", "server_device_input", "server_source"] as const;
  let restartTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * Sayı alanından geçerli bir değer çıkar.
   *
   * Alan boşaltıldığında `+""` sıfır veriyordu ve o sıfır diske yazılıyordu;
   * bir sonraki açılışta port 0 ile başlamaya çalışılıyordu. min/max
   * öznitelikleri yalnızca form gönderiminde denetleniyor, oninput'ta değil.
   */
  function clampInt(raw: string, min: number, max: number): number | null {
    if (raw.trim() === "") return null;
    const n = Number(raw);
    if (!Number.isFinite(n)) return null;
    const i = Math.round(n);
    return i < min || i > max ? null : i;
  }

  /** Ayarı güncelle, diske yaz, gerekiyorsa çalışan akışı yeniden kur. */
  function persist(patch: Partial<Config>) {
    if (!cfg) return;
    const before = cfg;
    cfg = { ...cfg, ...patch };
    setConfig(cfg).catch((e) => (error = String(e)));

    // Kullanıcı çalışırken aygıt değiştirdiğinde eskisi çalmaya devam
    // ediyordu ve değişiklik hiçbir şey yapmıyor gibi görünüyordu.
    // Yalnızca gerçekten değişen ve önceden bir değeri olan alanlar akışı
    // yeniden kurmayı hak ediyor. Açılışta aygıt ilk kez doldurulduğunda
    // yeniden başlatmak gereksiz bir ses boşluğu yaratıyordu.
    const changed = (k: keyof Config) =>
      k in patch && before[k] !== "" && before[k] !== cfg![k];
    const touchesPlayer = PLAYER_RESTART_KEYS.some(changed);
    const touchesServer = SERVER_RESTART_KEYS.some(changed);
    if (!touchesPlayer && !touchesServer) return;

    // Port alanına yazarken her tuşta yeniden başlatmamak için bekle.
    clearTimeout(restartTimer);
    restartTimer = setTimeout(() => {
      if (touchesPlayer && stats?.player_running) {
        act(async () => {
          await stopPlayer();
          await startPlayer(cfg!.player_port, cfg!.player_device, cfg!.player_buffer);
        });
      }
      if (touchesServer && stats?.server_running) {
        act(async () => {
          await stopServer();
          const peer = peers.find((p) => p.id === selectedPeer);
          const dest = peer ? `${peer.address}:${peer.port}` : (cfg!.server_target ?? "").trim();
          const device = cfg!.server_source === "input"
            ? cfg!.server_device_input : cfg!.server_device_monitor;
          if (dest) await startServer(dest, device, cfg!.server_source as "monitor" | "input");
        });
      }
    }, 400);
  }

  async function act(fn: () => Promise<void>) {
    busy = true;
    error = null;
    try { await fn(); } catch (e) { error = String(e); }
    busy = false;
    await tick();
  }

  const toggleServer = () =>
    act(async () => {
      if (stats?.server_running) return stopServer();
      const peer = peers.find((p) => p.id === selectedPeer);
      const dest = peer ? `${peer.address}:${peer.port}` : (cfg?.server_target ?? "").trim();
      if (!dest) throw new Error(t("error.needTarget"));
      const device = cfg!.server_source === "input"
        ? cfg!.server_device_input : cfg!.server_device_monitor;
      await startServer(dest, device, cfg!.server_source as "monitor" | "input");
    });

  const togglePlayer = () =>
    act(async () => {
      if (stats?.player_running) return stopPlayer();
      await startPlayer(cfg!.player_port, cfg!.player_device, cfg!.player_buffer);
    });

  const running = $derived(stats?.server_running || stats?.player_running);
</script>

{#if cfg}
<div class="app">
  <nav>
    <div class="brand">RelAudio</div>
    <button class="tab" class:active={tab === "server"} onclick={() => (tab = "server")}>
      {t("nav.server")}</button>
    <button class="tab" class:active={tab === "player"} onclick={() => (tab = "player")}>
      {t("nav.player")}</button>
    <button class="tab" class:active={tab === "settings"} onclick={() => (tab = "settings")}>
      {t("nav.settings")}</button>
    <div class="spacer"></div>
    <div class="live" class:on={running}>
      <span class="dot"></span>{running ? t("status.live") : t("status.idle")}
    </div>
  </nav>

  <main>
    {#if stats?.feedback_loop}
      <div class="err" role="alert">
        <div>
          <strong>{t("player.feedback")}</strong>
          <p class="fb">{t("player.feedbackBody")}</p>
        </div>
      </div>
    {/if}

    {#if error}
      <div class="err" role="alert">
        <strong>{t("error.title")}</strong><span>{error}</span>
        <button class="x" onclick={() => (error = null)} aria-label={t("error.close")}>✕</button>
      </div>
    {/if}

    {#if tab === "server"}
      <section class="card">
        <h2>{t("server.title")}</h2>
        <p class="sub">{t("server.subtitle")}</p>

        <div class="modes">
          <label class="mode" class:sel={cfg.server_source === "monitor"}>
            <input type="radio" checked={cfg.server_source === "monitor"}
                   onchange={() => persist({ server_source: "monitor" })} />
            <div><strong>{t("server.systemAudio")}</strong>
                 <span>{t("server.systemAudioDesc")}</span></div>
          </label>
          <label class="mode" class:sel={cfg.server_source === "input"}>
            <input type="radio" checked={cfg.server_source === "input"}
                   onchange={() => persist({ server_source: "input" })} />
            <div><strong>{t("server.mic")}</strong><span>{t("server.micDesc")}</span></div>
          </label>
        </div>

        <label class="field">
          <span class="lbl">{t("server.target")}</span>
          <select bind:value={selectedPeer} disabled={stats?.server_running}>
            {#each peers as p (p.id)}
              <option value={p.id}>
                {p.name} — {p.address}{p.listening ? "" : `  (${t("server.notListening")})`}
              </option>
            {/each}
            <option value="">{t("server.manual")}</option>
          </select>
          {#if peers.length === 0}<span class="hint">{t("server.noPeers")}</span>{/if}
        </label>

        {#if !selectedPeer}
          <label class="field">
            <span class="lbl">{t("server.address")}</span>
            <input type="text" value={cfg.server_target} placeholder="192.168.1.10"
                   disabled={stats?.server_running}
                   oninput={(e) => persist({ server_target: e.currentTarget.value })} />
            <span class="hint">{t("server.portHint")}</span>
          </label>
        {/if}

        <button class:danger={stats?.server_running} class:primary={!stats?.server_running}
                onclick={toggleServer} disabled={busy}>
          {stats?.server_running ? t("server.stop") : t("server.start")}
        </button>
      </section>

    {:else if tab === "player"}
      <section class="card">
        <h2>{t("player.title")}</h2>
        <p class="sub">{t("player.subtitle")}</p>

        <div class="modes">
          <label class="mode" class:sel={cfg.player_mode === "listen"}>
            <input type="radio" checked={cfg.player_mode === "listen"}
                   onchange={() => persist({ player_mode: "listen" })} />
            <div><strong>{t("player.listen")}</strong><span>{t("player.listenDesc")}</span></div>
          </label>
          <label class="mode" class:sel={cfg.player_mode === "mic"}>
            <input type="radio" checked={cfg.player_mode === "mic"}
                   onchange={() => persist({ player_mode: "mic" })} />
            <div><strong>{t("player.micMode")}</strong><span>{t("player.micModeDesc")}</span></div>
          </label>
        </div>

        <div class="two">
          <label class="field">
            <span class="lbl">{t("player.port")}</span>
            <input type="number" value={cfg.player_port} min="1024" max="65535"
                   disabled={stats?.player_running}
                   oninput={(e) => {
                     const v = clampInt(e.currentTarget.value, 1024, 65535);
                     if (v !== null) persist({ player_port: v });
                   }} />
          </label>
          <label class="field">
            <span class="lbl">{t("player.buffer")} ({cfg.player_buffer * 5} ms)</span>
            <input type="number" value={cfg.player_buffer} min="2" max="60"
                   disabled={stats?.player_running}
                   oninput={(e) => {
                     const v = clampInt(e.currentTarget.value, 2, 60);
                     if (v !== null) persist({ player_buffer: v });
                   }} />
            <span class="hint">{t("player.bufferHint")}</span>
          </label>
        </div>

        {#if stats?.player_running}
          <LevelMeter peak={stats.player_peak} label={t("player.level")}
                      waitingText={t("player.levelWaiting")} />
        {/if}

        <div class="addr">
          <span class="lbl">{t("player.thisDevice")}</span>
          <strong>{myName || "…"}</strong>
          <span class="hint">
            {localAddress ?? t("player.noAddress")}{localAddress ? `:${cfg.player_port}` : ""}
            — {t("player.addressHint")}
          </span>
        </div>

        {#if cfg.player_mode === "mic" && micHint?.paired_input}
          <div class="mic-ok">
            <strong>{t("player.micReady")}</strong>
            <ol class="steps">
              <li class="done">{t("player.micStep1", {
                cable: devices.find((d) => d.id === cfg.player_device)?.name ?? "—" })}</li>
              <li><strong class="pick">{t("player.micStep2", { device: micHint.paired_input })}</strong></li>
            </ol>
            <p class="aside-note">{t("player.micNotHere")}</p>
            <p class="warn-line">{t("player.dontChangeDefault")}</p>
          </div>
        {:else if cfg.player_mode === "mic" && micHint && !micHint.any_virtual}
          <div class="mic-info">
            <strong>{t("player.noCable")}</strong>
            <p>{t("player.noCableBody")}</p>
          </div>
        {/if}

        <div class="peers">
          <span class="lbl">{t("player.peers")}</span>
          {#if peers.length}
            <ul>
              {#each peers as p (p.id)}
                <li><span class="pdot" class:on={p.listening}></span>
                    <strong>{p.name}</strong><span class="paddr">{p.address}</span>
                    <span class="pos">{p.os}</span></li>
              {/each}
            </ul>
          {:else}
            <p class="idle">{t("player.noPeersYet")}</p>
          {/if}
        </div>

        <button class:danger={stats?.player_running} class:primary={!stats?.player_running}
                onclick={togglePlayer} disabled={busy}>
          {stats?.player_running ? t("player.stop") : t("player.start")}
        </button>
      </section>

    {:else}
      <section class="card">
        <h2>{t("settings.title")}</h2>

        <label class="field">
          <span class="lbl">{t("settings.language")}</span>
          <select value={lang} onchange={(e) => {
            lang = e.currentTarget.value; persist({ language: lang });
          }}>
            {#each LOCALES as l (l.code)}<option value={l.code}>{l.label}</option>{/each}
          </select>
          <span class="hint">{t("settings.languageDesc")}</span>
        </label>

        <label class="check">
          <input type="checkbox" checked={cfg.minimize_to_tray}
                 onchange={(e) => {
                   const on = e.currentTarget.checked;
                   persist({ minimize_to_tray: on });
                   setMinimizeToTray(on).catch((err) => (error = String(err)));
                 }} />
          <div><strong>{t("settings.tray")}</strong><span>{t("settings.trayDesc")}</span></div>
        </label>

        <label class="check">
          <input type="checkbox" checked={cfg.auto_listen}
                 onchange={(e) => persist({ auto_listen: e.currentTarget.checked })} />
          <div><strong>{t("settings.autoListen")}</strong><span>{t("settings.autoListenDesc")}</span></div>
        </label>

        <div class="note">
          <strong>{t("settings.limits")}</strong>
          <ul><li>{t("settings.limit1")}</li><li>{t("settings.limit2")}</li>
              <li>{t("settings.limit3")}</li></ul>
        </div>

        <div class="note">
          <strong>{t("settings.files")}</strong>
          <p class="path">{t("settings.configFile")}: <code>{configPath}</code></p>
          <p class="path">{t("settings.logFile")}: <code>{logPath}</code></p>
        </div>

        <BuyMeCoffee label={t("settings.coffee")} title={t("settings.support")}
                     body={t("settings.supportBody")}
                     failedText={t("settings.coffeeFailed")} />
      </section>
    {/if}
  </main>

  <aside>
    {#if tab === "player"}
      <div class="card tight">
        <h3>{cfg.player_mode === "mic" ? t("device.virtualCable") : t("device.output")}</h3>
        <DevicePicker devices={outputChoices} kind="output" value={cfg.player_device}
                      onselect={(id) => persist({ player_device: id })}
                      label={cfg.player_mode === "mic" ? t("device.cableLabel") : t("device.outputLabel")}
                      hint={cfg.player_mode === "mic" ? t("device.cableHint") : ""}
                      emptyText={t("device.none")} defaultText={t("device.default")} />
        <button onclick={refresh}>{t("device.refresh")}</button>
      </div>
    {:else if tab === "server"}
      <div class="card tight">
        <h3>{t("device.source")}</h3>
        {#if cfg.server_source === "monitor"}
          <DevicePicker {devices} kind="monitor" value={cfg.server_device_monitor}
                        onselect={(id) => persist({ server_device_monitor: id })}
                        label={t("device.systemSource")} hint={t("device.systemSourceHint")}
                        emptyText={t("device.none")} defaultText={t("device.default")} />
        {:else}
          <DevicePicker {devices} kind="input" value={cfg.server_device_input}
                        onselect={(id) => persist({ server_device_input: id })}
                        label={t("device.micLabel")}
                        emptyText={t("device.none")} defaultText={t("device.default")} />
        {/if}
        <button onclick={refresh}>{t("device.refresh")}</button>
      </div>
    {/if}

    <div class="card tight">
      <h3>{t("stats.title")}</h3>
      {#if stats?.server_running}
        <div class="grp">{t("stats.sending")} → {stats.server_target}</div>
        <StatRow label={t("stats.sourceDevice")} value={stats.server_device_name} />
        <StatRow label={t("stats.packets")} value={stats.server_packets.toLocaleString()} />
        <StatRow label={t("stats.bitrate")} value={`${stats.server_kbps.toFixed(0)} kbit/s`} />
        <StatRow label={t("stats.silentRatio")} value={`${(stats.server_silent_ratio * 100).toFixed(0)}%`} />
      {/if}
      {#if stats?.player_running}
        <div class="grp">{t("stats.receiving")} · {t("stats.port")} {stats.player_port}</div>
        <StatRow label={t("stats.outputDevice")} value={stats.player_device_name} />
        <StatRow label={t("stats.packets")} value={stats.player_packets.toLocaleString()}
                 tone={stats.player_packets === 0 ? "warn" : "normal"} />
        {#if stats.player_packets === 0}
          <p class="tip">{t("player.noPackets", { address: `${localAddress ?? "?"}:${stats.player_port}` })}</p>
        {/if}
        <StatRow label={t("stats.bitrate")} value={`${stats.player_kbps.toFixed(0)} kbit/s`} />
        <StatRow label={t("stats.buffer")} value={`${stats.player_buffer_ms} ms`} />
        <StatRow label={t("stats.lost")} value={String(stats.player_lost)}
                 tone={stats.player_lost > 0 ? "warn" : "ok"} />
        <StatRow label={t("stats.late")} value={String(stats.player_late)}
                 tone={stats.player_late > 0 ? "warn" : "ok"} />
        <StatRow label={t("stats.underruns")} value={String(stats.player_underruns)}
                 tone={stats.player_underruns > 0 ? "warn" : "ok"} />
        <StatRow label={t("stats.dropped")} value={String(stats.player_dropped)}
                 tone={stats.player_dropped > 0 ? "bad" : "ok"} />
      {/if}
      {#if !running}<p class="idle">{t("stats.idle")}</p>{/if}
    </div>
  </aside>
</div>
{/if}

<style>
  .app { display: grid; grid-template-columns: 190px 1fr 300px; height: 100vh; }
  nav {
    background: var(--panel); border-inline-end: 1px solid var(--line);
    padding: 18px 12px; display: flex; flex-direction: column; gap: 4px;
  }
  .brand { font-weight: 700; font-size: 17px; padding: 0 10px 16px; }
  .tab {
    background: none; border: none; text-align: start; border-radius: 7px;
    padding: 9px 10px; color: var(--dim);
  }
  .tab:hover { background: var(--panel-2); color: var(--text); }
  .tab.active { background: var(--panel-2); color: var(--text); font-weight: 600; }
  .spacer { flex: 1; }
  .live { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--dim); padding: 0 10px; }
  .live .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--dim); }
  .live.on { color: var(--ok); }
  .live.on .dot { background: var(--ok); box-shadow: 0 0 8px var(--ok); }

  main { padding: 22px; overflow-y: auto; }
  aside {
    background: var(--panel); border-inline-start: 1px solid var(--line);
    padding: 18px; overflow-y: auto; display: flex; flex-direction: column; gap: 14px;
  }
  .card { background: var(--panel); border: 1px solid var(--line); border-radius: 11px; padding: 20px; }
  aside .card { background: var(--panel-2); }
  .card.tight { padding: 15px; }
  h2 { margin: 0 0 4px; font-size: 18px; }
  h3 { margin: 0 0 12px; font-size: 13px; color: var(--dim); text-transform: uppercase; letter-spacing: .6px; }
  .sub { margin: 0 0 18px; color: var(--dim); font-size: 13px; }

  .modes { display: grid; gap: 9px; margin-bottom: 18px; }
  .mode {
    display: flex; gap: 11px; align-items: flex-start;
    border: 1px solid var(--line); border-radius: 9px; padding: 12px; cursor: pointer;
  }
  .mode.sel { border-color: var(--accent); background: rgba(29,155,240,.09); }
  .mode strong { display: block; font-size: 14px; }
  .mode span { display: block; color: var(--dim); font-size: 12px; margin-top: 2px; line-height: 1.45; }
  .mode input { margin-top: 3px; accent-color: var(--accent); }

  .field { display: block; margin-bottom: 16px; }
  .lbl { display: block; font-size: 12px; color: var(--dim); margin-bottom: 6px; }
  .hint { display: block; font-size: 11px; color: var(--dim); margin-top: 5px; line-height: 1.45; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }

  .check { display: flex; gap: 11px; align-items: flex-start; margin-bottom: 18px; }
  .check input { margin-top: 3px; accent-color: var(--accent); }
  .check strong { display: block; }
  .check span { display: block; color: var(--dim); font-size: 12px; margin-top: 3px; line-height: 1.45; }

  .note { border-top: 1px solid var(--line); padding-top: 16px; margin-top: 4px; font-size: 12px; color: var(--dim); }
  .note ul { margin: 8px 0 0; padding-inline-start: 18px; }
  .note li { margin: 4px 0; line-height: 1.5; }
  .path { margin: 6px 0 0; }
  .path code { color: var(--text); font-size: 11px; user-select: text; }

  .err {
    display: flex; align-items: center; gap: 10px;
    background: rgba(229,83,75,.12); border: 1px solid var(--bad);
    border-radius: 9px; padding: 11px 14px; margin-bottom: 16px; font-size: 13px;
  }
  .err .x { margin-inline-start: auto; background: none; border: none; padding: 2px 6px; color: var(--dim); }
  .err .fb { margin: 4px 0 0; color: var(--dim); line-height: 1.5; }

  .addr { border: 1px solid var(--line); border-radius: 9px; padding: 12px 14px;
          margin-bottom: 16px; background: var(--panel-2); }
  .addr strong { display: block; font-size: 17px; margin: 4px 0 2px;
                 font-variant-numeric: tabular-nums; user-select: text; }

  .mic-ok, .mic-info { border-radius: 9px; padding: 12px 14px; margin-bottom: 16px; font-size: 12px; }
  .mic-ok { border: 1px solid var(--ok); background: rgba(47,191,113,.10); }
  .mic-info { border: 1px solid var(--line); background: var(--panel-2); }
  .mic-ok p, .mic-info p { margin: 5px 0 0; color: var(--dim); line-height: 1.5; }
  .warn-line { color: var(--warn) !important; }
  .steps { margin: 8px 0 0; padding-inline-start: 20px; }
  .steps li { margin: 5px 0; line-height: 1.5; color: var(--text); }
  .steps li.done { color: var(--dim); }
  .steps .pick { font-weight: 600; }
  .aside-note { font-size: 11px; }

  .peers { margin-bottom: 18px; }
  .peers ul { list-style: none; margin: 6px 0 0; padding: 0; }
  .peers li { display: flex; align-items: center; gap: 9px; padding: 9px 12px;
              border: 1px solid var(--line); border-radius: 8px; margin-bottom: 6px; font-size: 13px; }
  .pdot { width: 7px; height: 7px; border-radius: 50%; background: var(--dim); flex: none; }
  .pdot.on { background: var(--ok); }
  .paddr { color: var(--dim); font-variant-numeric: tabular-nums; }
  .pos { color: var(--dim); margin-inline-start: auto; font-size: 11px; text-transform: uppercase; }

  .grp { font-size: 11px; color: var(--dim); text-transform: uppercase; letter-spacing: .5px; margin: 10px 0 4px; }
  .grp:first-child { margin-top: 0; }
  .idle { color: var(--dim); font-size: 13px; margin: 0; }
  .tip { font-size: 11px; color: var(--warn); margin: 6px 0 10px; line-height: 1.45; }
</style>

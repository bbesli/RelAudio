<script lang="ts">
  import {
    listDevices, startServer, stopServer, startPlayer, stopPlayer,
    getStats, setMinimizeToTray, getLocalAddress, getPeers, getDeviceName,
    type Device, type Stats, type Peer,
  } from "./lib/api";
  import DevicePicker from "./lib/DevicePicker.svelte";
  import StatRow from "./lib/StatRow.svelte";

  type Tab = "server" | "player" | "settings";

  let tab = $state<Tab>("server");
  let devices = $state<Device[]>([]);
  let stats = $state<Stats | null>(null);
  let localAddress = $state<string | null>(null);
  let peers = $state<Peer[]>([]);
  let myName = $state("");
  /// Eş seçimi: boş = elle IP gir
  let selectedPeer = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Sunucu
  let source = $state<"monitor" | "input">("monitor");
  let monitorId = $state("");
  let micId = $state("");
  let target = $state("");

  // Oynatıcı
  let port = $state(59101);
  let outputId = $state("");
  let bufferPackets = $state(8);

  // Ayarlar
  let minimizeToTray = $state(true);

  const running = $derived(stats?.server_running || stats?.player_running);

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
    } catch { /* pencere kapanırken olabilir, yok say */ }
  }

  $effect(() => {
    refresh();
    const id = setInterval(tick, 500);
    return () => clearInterval(id);
  });

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
      const dest = peer ? `${peer.address}:${peer.port}` : target.trim();
      if (!dest) throw new Error("Hedef cihaz seç veya adres gir");
      await startServer(dest, source === "monitor" ? monitorId : micId, source);
    });

  const togglePlayer = () =>
    act(async () => {
      if (stats?.player_running) return stopPlayer();
      await startPlayer(port, outputId, bufferPackets);
    });

  function bufferTone(ms: number) {
    if (ms === 0) return "warn";
    return "normal";
  }
</script>

<div class="app">
  <nav>
    <div class="brand">RelAudio</div>
    {#each [["server","Sunucu"],["player","Oynatıcı"],["settings","Ayarlar"]] as [id, name]}
      <button class="tab" class:active={tab === id} onclick={() => (tab = id as Tab)}>
        {name}
      </button>
    {/each}
    <div class="spacer"></div>
    <div class="live" class:on={running}>
      <span class="dot"></span>
      {running ? "Yayında" : "Boşta"}
    </div>
  </nav>

  <main>
    {#if error}
      <div class="err" role="alert">
        <strong>Hata</strong>
        <span>{error}</span>
        <button class="x" onclick={() => (error = null)} aria-label="Kapat">✕</button>
      </div>
    {/if}

    {#if tab === "server"}
      <section class="card">
        <h2>Sunucu</h2>
        <p class="sub">Bu bilgisayarın sesini ağdaki başka bir cihaza gönder.</p>

        <div class="modes">
          <label class="mode" class:sel={source === "monitor"}>
            <input type="radio" bind:group={source} value="monitor" />
            <div>
              <strong>Sistem sesi</strong>
              <span>Bu bilgisayarda çalan sesi gönder</span>
            </div>
          </label>
          <label class="mode" class:sel={source === "input"}>
            <input type="radio" bind:group={source} value="input" />
            <div>
              <strong>Mikrofon</strong>
              <span>Bağlı mikrofonun girişini gönder</span>
            </div>
          </label>
        </div>

        <label class="field">
          <span class="lbl">Hedef cihaz</span>
          <select bind:value={selectedPeer} disabled={stats?.server_running}>
            {#each peers as p (p.id)}
              <option value={p.id}>
                {p.name} — {p.address}{p.listening ? "" : "  (dinlemiyor)"}
              </option>
            {/each}
            <option value="">Elle adres gir…</option>
          </select>
          {#if peers.length === 0}
            <span class="hint">Ağda başka RelAudio bulunamadı. Diğer cihazda
              uygulamayı aç; birkaç saniye içinde burada görünmeli.</span>
          {/if}
        </label>

        {#if !selectedPeer}
          <label class="field">
            <span class="lbl">Adres</span>
            <input type="text" bind:value={target} placeholder="192.168.1.113"
                   disabled={stats?.server_running} />
            <span class="hint">Port belirtilmezse 59101 kullanılır.</span>
          </label>
        {/if}

        <button class:danger={stats?.server_running} class:primary={!stats?.server_running}
                onclick={toggleServer} disabled={busy}>
          {stats?.server_running ? "Yayını durdur" : "Yayına başla"}
        </button>
      </section>
    {:else if tab === "player"}
      <section class="card">
        <h2>Oynatıcı</h2>
        <p class="sub">Başka bir cihazın sesini bu bilgisayarda çal.</p>

        <div class="two">
          <label class="field">
            <span class="lbl">Dinlenecek port</span>
            <input type="number" bind:value={port} min="1024" max="65535"
                   disabled={stats?.player_running} />
          </label>
          <label class="field">
            <span class="lbl">Tampon ({bufferPackets * 5} ms)</span>
            <input type="number" bind:value={bufferPackets} min="2" max="60"
                   disabled={stats?.player_running} />
            <span class="hint">Ağ kötüyse artır. 1 birim = 5 ms.</span>
          </label>
        </div>

        <div class="addr">
          <span class="lbl">Bu cihaz ağda şöyle görünüyor</span>
          <strong>{myName || "…"}</strong>
          <span class="hint">
            {localAddress ?? "adres bulunamadı"}{localAddress ? `:${port}` : ""}
            — diğer cihazın Sunucu sekmesinde bu adı seçmesi yeterli.
          </span>
        </div>

        <div class="peers">
          <span class="lbl">Ağdaki diğer cihazlar</span>
          {#if peers.length}
            <ul>
              {#each peers as p (p.id)}
                <li>
                  <span class="pdot" class:on={p.listening}></span>
                  <strong>{p.name}</strong>
                  <span class="paddr">{p.address}</span>
                  <span class="pos">{p.os}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="idle">Henüz kimse görünmüyor.</p>
          {/if}
        </div>

        <button class:danger={stats?.player_running} class:primary={!stats?.player_running}
                onclick={togglePlayer} disabled={busy}>
          {stats?.player_running ? "Dinlemeyi durdur" : "Dinlemeye başla"}
        </button>
      </section>
    {:else}
      <section class="card">
        <h2>Ayarlar</h2>
        <label class="check">
          <input type="checkbox" bind:checked={minimizeToTray}
                 onchange={() => setMinimizeToTray(minimizeToTray)} />
          <div>
            <strong>Kapatınca tepsiye küçült</strong>
            <span>Pencereyi kapattığında uygulama arka planda çalışmaya devam eder.
                  Tepsi simgesinden geri açabilirsin.</span>
          </div>
        </label>

        <div class="note">
          <strong>Bilinen sınırlar (v1)</strong>
          <ul>
            <li>Saat kayması telafisi yok — uzun oturumlarda "atılan" sayacı artabilir.</li>
            <li>Kodek yalnızca PCM. Sessiz bloklar yük taşımadan gider.</li>
            <li>Ağdaki cihazlar otomatik bulunmuyor; IP elle giriliyor.</li>
          </ul>
        </div>
      </section>
    {/if}
  </main>

  <aside>
    {#if tab === "player"}
      <div class="card tight">
        <h3>Çıkış aygıtı</h3>
        <DevicePicker {devices} kind="output" bind:value={outputId}
                      label="Sesin çalınacağı yer" />
        <button onclick={refresh}>Aygıtları yenile</button>
      </div>
    {:else if tab === "server"}
      <div class="card tight">
        <h3>Kaynak aygıt</h3>
        {#if source === "monitor"}
          <DevicePicker {devices} kind="monitor" bind:value={monitorId}
                        label="Sistem sesi kaynağı"
                        hint="Bir çıkışın monitörü. Genelde varsayılan doğrudur." />
        {:else}
          <DevicePicker {devices} kind="input" bind:value={micId} label="Mikrofon" />
        {/if}
        <button onclick={refresh}>Aygıtları yenile</button>
      </div>
    {/if}

    <div class="card tight">
      <h3>İstatistikler</h3>
      {#if stats?.server_running}
        <div class="grp">Gönderim → {stats.server_target}</div>
        <StatRow label="Paket" value={stats.server_packets.toLocaleString("tr")} />
        <StatRow label="Bit hızı" value={`${stats.server_kbps.toFixed(0)} kbit/s`} />
        <StatRow label="Sessiz oran"
                 value={`%${(stats.server_silent_ratio * 100).toFixed(0)}`} />
      {/if}
      {#if stats?.player_running}
        <div class="grp">Alım · port {stats.player_port}</div>
        <StatRow label="Paket" value={stats.player_packets.toLocaleString("tr")}
                 tone={stats.player_packets === 0 ? "warn" : "normal"} />
        {#if stats.player_packets === 0}
          <p class="tip">
            Hiç paket gelmiyor. Gönderen cihazdaki hedef adres
            <strong>{localAddress ?? "?"}:{stats.player_port}</strong> mi?
            Değilse güvenlik duvarı bu portu engelliyor olabilir.
          </p>
        {/if}
        <StatRow label="Bit hızı" value={`${stats.player_kbps.toFixed(0)} kbit/s`} />
        <StatRow label="Tampon" value={`${stats.player_buffer_ms} ms`}
                 tone={bufferTone(stats.player_buffer_ms)} />
        <StatRow label="Kayıp" value={String(stats.player_lost)}
                 tone={stats.player_lost > 0 ? "warn" : "ok"} />
        <StatRow label="Geç gelen" value={String(stats.player_late)}
                 tone={stats.player_late > 0 ? "warn" : "ok"} />
        <StatRow label="Underrun" value={String(stats.player_underruns)}
                 tone={stats.player_underruns > 0 ? "warn" : "ok"} />
        <StatRow label="Atılan" value={String(stats.player_dropped)}
                 tone={stats.player_dropped > 0 ? "bad" : "ok"} />
      {/if}
      {#if !running}
        <p class="idle">Yayın yok.</p>
      {/if}
    </div>
  </aside>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 190px 1fr 300px;
    height: 100vh;
  }
  nav {
    background: var(--panel);
    border-right: 1px solid var(--line);
    padding: 18px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .brand { font-weight: 700; font-size: 17px; padding: 0 10px 16px; letter-spacing: 0.2px; }
  .tab {
    background: none; border: none; text-align: left; border-radius: 7px;
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
    background: var(--panel);
    border-left: 1px solid var(--line);
    padding: 18px;
    overflow-y: auto;
    display: flex; flex-direction: column; gap: 14px;
  }

  .card { background: var(--panel); border: 1px solid var(--line); border-radius: 11px; padding: 20px; }
  aside .card { background: var(--panel-2); }
  .card.tight { padding: 15px; }
  h2 { margin: 0 0 4px; font-size: 18px; }
  h3 { margin: 0 0 12px; font-size: 13px; color: var(--dim); text-transform: uppercase; letter-spacing: 0.6px; }
  .sub { margin: 0 0 18px; color: var(--dim); font-size: 13px; }

  .modes { display: grid; gap: 9px; margin-bottom: 18px; }
  .mode {
    display: flex; gap: 11px; align-items: flex-start;
    border: 1px solid var(--line); border-radius: 9px; padding: 12px; cursor: pointer;
  }
  .mode.sel { border-color: var(--accent); background: rgba(29, 155, 240, 0.09); }
  .mode strong { display: block; font-size: 14px; }
  .mode span { display: block; color: var(--dim); font-size: 12px; margin-top: 2px; }
  .mode input { margin-top: 3px; accent-color: var(--accent); }

  .field { display: block; margin-bottom: 16px; }
  .lbl { display: block; font-size: 12px; color: var(--dim); margin-bottom: 6px; }
  .hint { display: block; font-size: 11px; color: var(--dim); margin-top: 5px; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }

  .check { display: flex; gap: 11px; align-items: flex-start; margin-bottom: 20px; }
  .check input { margin-top: 3px; accent-color: var(--accent); }
  .check strong { display: block; }
  .check span { display: block; color: var(--dim); font-size: 12px; margin-top: 3px; }

  .note { border-top: 1px solid var(--line); padding-top: 16px; font-size: 12px; color: var(--dim); }
  .note ul { margin: 8px 0 0; padding-left: 18px; }
  .note li { margin: 4px 0; }

  .err {
    display: flex; align-items: center; gap: 10px;
    background: rgba(229, 83, 75, 0.12); border: 1px solid var(--bad);
    border-radius: 9px; padding: 11px 14px; margin-bottom: 16px; font-size: 13px;
  }
  .err .x { margin-left: auto; background: none; border: none; padding: 2px 6px; color: var(--dim); }

  .grp { font-size: 11px; color: var(--dim); text-transform: uppercase;
         letter-spacing: 0.5px; margin: 10px 0 4px; }
  .grp:first-child { margin-top: 0; }
  .idle { color: var(--dim); font-size: 13px; margin: 0; }

  .addr {
    border: 1px solid var(--line); border-radius: 9px;
    padding: 12px 14px; margin-bottom: 16px; background: var(--panel-2);
  }
  .addr strong {
    display: block; font-size: 17px; margin: 4px 0 2px;
    font-variant-numeric: tabular-nums; user-select: text;
  }
  .tip {
    font-size: 11px; color: var(--warn); margin: 6px 0 10px; line-height: 1.45;
  }

  .peers { margin-bottom: 18px; }
  .peers ul { list-style: none; margin: 6px 0 0; padding: 0; }
  .peers li {
    display: flex; align-items: center; gap: 9px;
    padding: 9px 12px; border: 1px solid var(--line);
    border-radius: 8px; margin-bottom: 6px; font-size: 13px;
  }
  .pdot { width: 7px; height: 7px; border-radius: 50%; background: var(--dim); flex: none; }
  .pdot.on { background: var(--ok); }
  .paddr { color: var(--dim); font-variant-numeric: tabular-nums; }
  .pos { color: var(--dim); margin-left: auto; font-size: 11px; text-transform: uppercase; }
</style>

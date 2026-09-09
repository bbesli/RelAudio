<script lang="ts">
  import {
    listDevices, startServer, stopServer, startPlayer, stopPlayer, getStats,
    setMinimizeToTray, getLocalAddress, getPeers, getDeviceName, getMicHint,
    getConfig, setConfig, getConfigPath, getLogPath, setTrayLabels,
    getHeadsetPlan, startHeadset, stopHeadset, pairWithPeer, cancelPairing, isPaired, unpair,
    type Device, type Stats, type Peer, type MicHint, type Config,
    type HeadsetPlan, type HeadsetRole, type HeadsetStartResult,
  } from "./lib/api";
  import { LOCALES, translator, detectLocale, isRtl } from "./lib/i18n";
  import DevicePicker from "./lib/DevicePicker.svelte";
  import LevelMeter from "./lib/LevelMeter.svelte";
  import StatRow from "./lib/StatRow.svelte";
  import BuyMeCoffee from "./lib/BuyMeCoffee.svelte";

  type Tab = "headset" | "server" | "player" | "settings";

  let tab = $state<Tab>("headset");
  let headsetRole = $state<HeadsetRole>("local");
  /** Çekirdeğin çözdüğü aygıt planı. Uzaktan gelen istek de aynısını
   *  kullanıyor, bu yüzden politika burada değil Rust'ta. */
  let headsetPlan = $state<HeadsetPlan | null>(null);
  /** Son "başlat" denemesinde karşı tarafa ne olduğu. */
  let remoteResult = $state<HeadsetStartResult | null>(null);
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
      server_target: "", headset_role: "local", remote_control: true, paired: {},
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
      headsetRole = c.headset_role === "remote" ? "remote" : "local";
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
  /** Kulaklık modu = iki yön birden çalışıyor. */
  const headsetRunning = $derived(!!stats?.server_running && !!stats?.player_running);

  // — Kulaklık modu —
  //
  // Aygıt seçim politikası **Rust tarafında** (`audio::headset_plan`).
  // Sebebi: uzaktan gelen "başlat" isteğinde arayüz hiç devrede olmuyor ve
  // iki yerde ayrı ayrı yazılan bir politika kaçınılmaz olarak ayrışır.
  // Değişmez kural orada da aynı: yakalanan aygıt ile yazılan aygıt aynı
  // olamaz, yoksa kullanıcı kendini duyar.

  const chosenPeer = $derived(peers.find((p) => p.id === selectedPeer) ?? null);

  // — Eşleştirme —
  //
  // Uzaktan başlatma yalnızca eşleşilmiş cihazlar için açık. Paylaşan makine
  // 6 haneli kodu ekranda gösteriyor, kullanıcı kodu karşı makinede yazıyor,
  // iki taraf bir anahtar paylaşıyor ve bir daha sorulmuyor.
  let peerPaired = $state<boolean | null>(null);
  let codeInput = $state("");
  let pairingBusy = $state(false);
  let pairedWith = $state<string | null>(null);

  $effect(() => {
    const id = selectedPeer;
    void cfg?.paired;
    if (!id) { peerPaired = null; return; }
    isPaired(id).then((v) => (peerPaired = v)).catch(() => (peerPaired = null));
  });

  /** Kodumuz ekranda dururken karşı taraf onu kullanırsa kod kayboluyor.
   *  O an başlatmayı kendiliğinden tekrar deniyoruz: kullanıcı düğmeye
   *  ikinci kez basmak zorunda kalmasın. */
  $effect(() => {
    const showing = !!stats?.pairing_code;
    if (remoteResult?.needs_pairing && !showing && !busy) {
      remoteResult = null;
      toggleHeadset();
    }
  });

  const submitCode = () =>
    act(async () => {
      if (!chosenPeer) throw new Error(t("error.needTarget"));
      pairingBusy = true;
      try {
        pairedWith = await pairWithPeer(chosenPeer.id, codeInput);
        codeInput = "";
        cfg = await getConfig();
        peerPaired = true;
      } catch (e) {
        const code = String(e).replace(/^Error:\s*/, "");
        throw new Error(REMOTE_CODES.has(code) ? t(`remote.${code}`) : code);
      } finally {
        pairingBusy = false;
      }
    });

  const savedCaptureId = $derived(
    headsetRole === "local" ? (cfg?.server_device_input ?? "") : (cfg?.server_device_monitor ?? "")
  );

  $effect(() => {
    const role = headsetRole;
    const play = cfg?.player_device ?? "";
    const capture = savedCaptureId;
    // Aygıt listesi değiştiğinde (kablo kuruldu, kulaklık takıldı) planı
    // tazele; aksi hâlde kullanıcı Yenile'ye bassa da eski planı görüyor.
    void devices.length;
    getHeadsetPlan(role, play, capture)
      .then((p) => (headsetPlan = p))
      .catch((e) => { headsetPlan = null; error = String(e); });
  });

  /** Plan kurulamadıysa çeviri anahtarına eşlenmiş sebep. */
  const headsetProblem = $derived(
    headsetPlan?.problem === "need_cable" ? t("headset.needCable")
      : headsetPlan?.problem === "need_physical" ? t("headset.needPhysical")
      : null
  );

  /** Karşı tarafın verdiği ret sebebi — kodu biliyorsak kendi dilimizde.
   *  `error` metni karşı makinede üretiliyor ve onun dili bizimkiyle aynı
   *  olmak zorunda değil; kod bilinmiyorsa metne düşüyoruz. */
  const REMOTE_CODES = new Set([
    "disabled", "busy", "busy_other", "need_cable", "need_physical",
    "device_open", "version", "needs_pairing",
    "pair_no_code", "pair_expired", "pair_wrong", "pair_too_many",
    "pair_no_random", "pair_bad_reply", "auth_stale", "auth_replay", "auth_bad",
    "peer_gone", "no_control",
  ]);
  const remoteError = $derived(
    !remoteResult?.remote_error ? null
      : remoteResult.remote_error_code && REMOTE_CODES.has(remoteResult.remote_error_code)
        ? t(`remote.${remoteResult.remote_error_code}`)
        : remoteResult.remote_error
  );

  /** Sistem sesi kaynağının kullanıcıya gösterilecek adı.
   *  Linux'ta kaynak "Monitor of Razer …" gibi geliyor; talimatta hoparlörün
   *  kendi adı yazmalı, monitör teknik bir ayrıntı. */
  const speakerName = $derived(
    (headsetPlan?.capture?.name ?? "—").replace(/^Monitor of /i, "")
  );

  /** Toplantı uygulamasında seçilecek mikrofon. Uzak rolü karşı taraf
   *  üstlendiyse adı o bildiriyor; biz üstlendiysek kendi planımızda. */
  const pairedMic = $derived(
    (headsetRole === "local" ? remoteResult?.remote_paired_mic : headsetPlan?.paired_mic) ?? "?"
  );

  const toggleHeadset = () =>
    act(async () => {
      if (headsetRunning) {
        remoteResult = null;
        await stopHeadset();
        return;
      }
      if (headsetProblem) throw new Error(headsetProblem);
      const peer = chosenPeer;
      if (!peer && !(cfg!.server_target ?? "").trim()) throw new Error(t("error.needTarget"));
      persist({ headset_role: headsetRole });
      remoteResult = await startHeadset(
        headsetRole,
        peer?.id ?? "",
        peer ? "" : (cfg!.server_target ?? "").trim(),
      );
      // Rust seçtiği aygıtları diske yazdı. Buradaki kopyayı tazelemezsek
      // bir sonraki persist() eski değerleri geri yazıyor ve seçim sessizce
      // geri alınıyordu.
      cfg = await getConfig().catch(() => cfg!);
    });

</script>

{#if cfg}
<div class="app">
  <nav>
    <div class="brand">RelAudio</div>
    <button class="tab" class:active={tab === "headset"} onclick={() => (tab = "headset")}>
      {t("nav.headset")}</button>
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
    {#if stats?.started_by}
      <div class="remote-banner" role="status">
        <div>
          <strong>{t("headset.startedBy", { device: stats.started_by.name })}</strong>
          <p class="fb">{stats.started_by.address}</p>
        </div>
        <button class="danger" onclick={() => act(stopHeadset)}>{t("headset.stop")}</button>
      </div>
    {/if}

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

    {#if tab === "headset"}
      <section class="card">
        <h2>{t("headset.title")}</h2>
        <p class="sub">{t("headset.subtitle")}</p>

        <div class="modes">
          <label class="mode" class:sel={headsetRole === "local"}>
            <input type="radio" checked={headsetRole === "local"}
                   onchange={() => (headsetRole = "local")} disabled={headsetRunning} />
            <div><strong>{t("headset.local")}</strong><span>{t("headset.localDesc")}</span></div>
          </label>
          <label class="mode" class:sel={headsetRole === "remote"}>
            <input type="radio" checked={headsetRole === "remote"}
                   onchange={() => (headsetRole = "remote")} disabled={headsetRunning} />
            <div><strong>{t("headset.remote")}</strong><span>{t("headset.remoteDesc")}</span></div>
          </label>
        </div>

        <label class="field">
          <span class="lbl">{t("headset.target")}</span>
          <select bind:value={selectedPeer} disabled={headsetRunning}>
            {#each peers as p (p.id)}
              <option value={p.id}>{p.name} — {p.address}</option>
            {/each}
            <option value="">{t("server.manual")}</option>
          </select>
          {#if peers.length === 0}<span class="hint">{t("server.noPeers")}</span>{/if}
        </label>

        {#if !selectedPeer}
          <label class="field">
            <span class="lbl">{t("server.address")}</span>
            <input type="text" value={cfg.server_target} placeholder="192.168.1.10"
                   disabled={headsetRunning}
                   oninput={(e) => persist({ server_target: e.currentTarget.value })} />
          </label>
        {/if}

        <!-- Eş uzaktan başlatmayı desteklemiyorsa bunu düğmeye basmadan
             söylemek gerekiyor: aksi hâlde kullanıcı yarım kurulmuş bir
             oturumla baş başa kalıyor ve sebebini bilmiyor. -->
        {#if chosenPeer && !chosenPeer.can_remote_start && !headsetRunning}
          <div class="mic-info">
            <strong>{t("headset.peerNoRemote", { device: chosenPeer.name })}</strong>
          </div>
        {/if}

        <!-- Bu makine kodu gösteriyor: karşı taraf onu girecek. -->
        {#if stats?.pairing_code}
          <div class="pair-code">
            <strong>{t("pair.showTitle", { device: chosenPeer?.name ?? "—" })}</strong>
            <div class="code">{stats.pairing_code.slice(0, 3)} {stats.pairing_code.slice(3)}</div>
            <p class="fb">{t("pair.showBody", { seconds: String(stats.pairing_seconds) })}</p>
            <button class="link" onclick={() => act(async () => { await cancelPairing(); remoteResult = null; })}>
              {t("pair.cancel")}
            </button>
          </div>

        <!-- Karşı taraf eşleşmemiş: kodu buraya gir. -->
        {:else if chosenPeer && chosenPeer.can_remote_start && peerPaired === false && !headsetRunning}
          <div class="pair-code">
            <strong>{t("pair.enterTitle", { device: chosenPeer.name })}</strong>
            <p class="fb">{t("pair.enterBody", { device: chosenPeer.name })}</p>
            <div class="pair-row">
              <input class="code-input" type="text" inputmode="numeric" maxlength="6"
                     placeholder="000000" bind:value={codeInput}
                     onkeydown={(e) => { if (e.key === "Enter" && codeInput.length === 6) submitCode(); }} />
              <button class="primary" disabled={pairingBusy || codeInput.trim().length !== 6}
                      onclick={submitCode}>{t("pair.submit")}</button>
            </div>
          </div>
        {:else if pairedWith && peerPaired}
          <div class="mic-ok"><strong>{t("pair.done", { device: pairedWith })}</strong></div>
        {/if}

        {#if headsetProblem}
          <div class="mic-info"><strong>{headsetProblem}</strong></div>
        {:else}
          <div class="plan">
            <div class="row"><span>{t("stats.sourceDevice")}</span><strong>{headsetPlan?.capture?.name ?? "—"}</strong></div>
            <div class="row"><span>{t("stats.outputDevice")}</span><strong>{headsetPlan?.play?.name ?? "—"}</strong></div>
          </div>
        {/if}

        {#if headsetRunning}
          {#if stats?.started_by}
            <!-- Bu makine uzaktan başlatıldı: istek göndermedik, dolayısıyla
                 "karşı taraf başlatılamadı" demek yanlış olurdu. -->
            <div class="mic-ok">
              <strong>{t("headset.startedBy", { device: stats.started_by.name })}</strong>
              <ol class="steps">
                {#if headsetRole === "remote"}
                  <li><strong class="pick">{t("headset.setupMic", { device: headsetPlan?.paired_mic ?? "?" })}</strong></li>
                {/if}
              </ol>
              <p class="warn-line">{t("headset.parsec")}</p>
            </div>
          {:else if remoteResult?.remote_started}
            <div class="mic-ok">
              <strong>{t("headset.bothStarted", { device: remoteResult.remote_name })}</strong>
              <p class="fb">{headsetRole === "local" ? t("headset.setupTitle") : t("headset.setupTitleHere")}</p>
              <ol class="steps">
                <li><strong class="pick">{t("headset.setupMic", { device: pairedMic })}</strong></li>
                <!-- Talimat, toplantı uygulamasının çalıştığı makine için.
                     Kulaklık bizdeyse o makine karşı taraf; kulaklık karşıdaysa
                     toplantı bu makinede ve hoparlör bizim gerçek
                     hoparlörümüz — kablo değil, o zaten yayınlanıyor. -->
                <li>{t("headset.setupOut", {
                  device: headsetRole === "local"
                    ? t("headset.theirSpeakers")
                    : speakerName })}</li>
              </ol>
              <p class="warn-line">{t("headset.parsec")}</p>
            </div>
          {:else}
            <div class="mic-info">
              <strong>{t("headset.halfStarted")}</strong>
              {#if remoteError}
                <p class="fb">{remoteError}</p>
              {/if}
              <p class="fb">{t("headset.startTheirSide")}</p>
            </div>
          {/if}
        {/if}

        <button class:danger={headsetRunning} class:primary={!headsetRunning}
                onclick={toggleHeadset} disabled={busy || !!headsetProblem}>
          {headsetRunning ? t("headset.stop") : t("headset.start")}
        </button>
        {#if !headsetRunning}
          <p class="hint">{t("headset.oneButton")}</p>
        {/if}
      </section>

    {:else if tab === "server"}
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
                   oninput={(e) => {
                     const v = clampInt(e.currentTarget.value, 1024, 65535);
                     if (v !== null) persist({ player_port: v });
                   }} />
          </label>
          <label class="field">
            <span class="lbl">{t("player.buffer")} ({cfg.player_buffer * 5} ms)</span>
            <input type="number" value={cfg.player_buffer} min="2" max="60"
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
                cable: devices.find((d) => d.id === cfg!.player_device)?.name ?? "—" })}</li>
              <li><strong class="pick">{t("player.micStep2", { device: micHint.paired_input })}</strong></li>
            </ol>
            <p class="aside-note">{t("player.micNotHere")}</p>
            {#if devices.find((d) => d.id === cfg!.player_device)?.is_default}
              <p class="warn-line">{t("player.cableIsDefault")}</p>
            {:else}
              <p class="warn-line">{t("player.dontChangeDefault")}</p>
            {/if}
            <p class="aside-note">{t("player.listenTabHint")}</p>
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

        <label class="check">
          <input type="checkbox" checked={cfg.remote_control}
                 onchange={(e) => persist({ remote_control: e.currentTarget.checked })} />
          <div><strong>{t("settings.remoteControl")}</strong>
               <span>{t("settings.remoteControlDesc")}</span></div>
        </label>

        <div class="note">
          <strong>{t("settings.paired")}</strong>
          {#if Object.keys(cfg.paired ?? {}).length === 0}
            <p class="fb">{t("settings.pairedNone")}</p>
          {:else}
            <ul class="paired">
              {#each Object.entries(cfg.paired) as [id, p] (id)}
                <li>
                  <span>{p.name || id}</span>
                  <button class="link" onclick={() => act(async () => {
                    await unpair(id);
                    cfg = await getConfig();
                  })}>{t("settings.unpair")}</button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>

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
    {#if tab === "headset"}
      <div class="card tight">
        <h3>{t("headset.devices")}</h3>
        <DevicePicker devices={headsetPlan?.play_choices ?? []} kind="output"
                      value={headsetPlan?.play?.id ?? ""}
                      onselect={(id) => persist({ player_device: id })}
                      label={headsetRole === "remote" ? t("device.cableLabel") : t("device.outputLabel")}
                      emptyText={t("device.none")} defaultText={t("device.default")} />
        {#if headsetRole === "local"}
          <DevicePicker devices={headsetPlan?.capture_choices ?? []} kind="input"
                        value={headsetPlan?.capture?.id ?? ""}
                        onselect={(id) => persist({ server_device_input: id })}
                        label={t("device.micLabel")}
                        emptyText={t("device.none")} defaultText={t("device.default")} />
        {:else}
          <DevicePicker devices={headsetPlan?.capture_choices ?? []} kind="monitor"
                        value={headsetPlan?.capture?.id ?? ""}
                        onselect={(id) => persist({ server_device_monitor: id })}
                        label={t("device.systemSource")}
                        emptyText={t("device.none")} defaultText={t("device.default")} />
        {/if}
        <button onclick={refresh}>{t("device.refresh")}</button>
      </div>

    {:else if tab === "player"}
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
        {#if stats.server_send_errors > 0}
          <StatRow label={t("stats.sendErrors")} value={String(stats.server_send_errors)} tone="bad" />
          <p class="tip">{t("server.notReaching")}</p>
        {/if}
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

  .pair-code {
    background: #1d2a44; border: 1px solid #2f4b7d; border-radius: 8px;
    padding: 12px 14px; margin: 12px 0;
  }
  .pair-code .code {
    font-size: 30px; letter-spacing: 6px; font-weight: 700;
    font-variant-numeric: tabular-nums; margin: 8px 0 4px; user-select: all;
  }
  .pair-row { display: flex; gap: 8px; align-items: center; }
  .pair-row button { width: auto; margin: 0; flex: 0 0 auto; }
  .code-input {
    width: 120px; font-size: 20px; letter-spacing: 4px; text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .paired { margin: 6px 0 0; padding-left: 0; list-style: none; }
  .paired li { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .link {
    width: auto; margin: 0; background: none; border: none; padding: 2px 0;
    color: #7aa2f7; text-decoration: underline; cursor: pointer; font-size: 12px;
  }

  .remote-banner {
    display: flex; align-items: center; gap: 12px; justify-content: space-between;
    background: #1d2a44; border: 1px solid #2f4b7d; border-radius: 8px;
    padding: 10px 14px; margin-bottom: 14px;
  }
  .remote-banner button { width: auto; margin: 0; flex: 0 0 auto; }

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

  .plan {
    border: 1px solid var(--line); border-radius: 9px;
    padding: 10px 14px; margin-bottom: 16px; background: var(--panel-2);
  }
  .plan .row { display: flex; justify-content: space-between; gap: 12px; padding: 4px 0; font-size: 12px; }
  .plan .row span { color: var(--dim); }

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

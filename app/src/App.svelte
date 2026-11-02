<script lang="ts">
  import {
    listDevices, startServer, stopServer, startPlayer, stopPlayer, getStats,
    setMinimizeToTray, getLocalAddress, getPeers, getDeviceName, getMicHint,
    getConfig, setConfig, getConfigPath, getLogPath, setTrayLabels,
    getHeadsetPlan, startHeadset, stopHeadset, pairWithPeer, getPairingCode, verifyPairing,
    cancelPairing, unpair,
    type Device, type Stats, type Peer, type MicHint, type Config,
    type HeadsetPlan, type HeadsetRole, type HeadsetStartResult,
  } from "./lib/api";
  import { getVersion } from "@tauri-apps/api/app";
  import { LOCALES, translator, detectLocale, isRtl } from "./lib/i18n";
  import DevicePicker from "./lib/DevicePicker.svelte";
  import LevelMeter from "./lib/LevelMeter.svelte";
  import StatRow from "./lib/StatRow.svelte";
  import BuyMeCoffee from "./lib/BuyMeCoffee.svelte";
  import Icon, { type IconName } from "./lib/Icon.svelte";
  import PairingCard from "./lib/PairingCard.svelte";

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
  let version = $state("");

  let myName = $state("");
  let localAddress = $state<string | null>(null);
  let configPath = $state("");
  let logPath = $state("");

  // Ayarlar diskten gelir; her anlamlı değişiklikte geri yazılır.
  let cfg = $state<Config | null>(null);
  let lang = $state("en");
  let selectedPeer = $state("");
  /** Kulaklık sekmesinde "adres gir" seçeneği açık mı? */
  let manualTarget = $state(false);

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

  /** Bir tick sürerken yenisi başlamasın. Komutlar ana iş parçacığı dışında
   *  çalışıyor; oturum başlarken bir `stats` çağrısı kilit bekleyebiliyor ve
   *  500 ms'lik zamanlayıcı üst üste çağrı biriktirip eski bir sonucu yenisinin
   *  üstüne yazabiliyordu. */
  let ticking = false;
  async function tick() {
    if (ticking) return;
    ticking = true;
    try {
      stats = await getStats();
      peers = await getPeers();
      if (stats.last_error) error = stats.last_error;
    } catch { /* pencere kapanırken olabilir */ }
    finally { ticking = false; }
  }

  /** Ayarlar okunamazsa kullanılacak taban. */
  function fallbackConfig(): Config {
    return {
      language: "", minimize_to_tray: true, auto_listen: true,
      player_mode: "listen", player_port: 59101, player_device: "", player_buffer: 8,
      server_source: "monitor", server_device_monitor: "", server_device_input: "",
      server_target: "", headset_role: "local",
      headset_local_play: "", headset_local_capture: "",
      headset_remote_play: "", headset_remote_capture: "",
      remote_control: true, paired: {},
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
      version = await getVersion().catch(() => "");
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

  // Seçili eş ağdan kaybolursa seçimi bırak. Aksi hâlde açılır liste boş
  // kalıyor ve hedef girilemez hâle geliyordu.
  $effect(() => {
    if (selectedPeer && !peers.some((p) => p.id === selectedPeer)) {
      selectedPeer = "";
    }
  });

  // Tek eş varsa onu kendiliğinden seç: iki makineli kurulumda kullanıcının
  // her açılışta aynı tek seçeneğe tıklaması anlamsız.
  $effect(() => {
    if (!selectedPeer && !manualTarget && peers.length === 1) selectedPeer = peers[0].id;
  });

  // Yalnızca çıkış aygıtı değişince. `cfg` her ayar değişikliğinde yeni bir
  // nesne oluyor; doğrudan ona bağlı bir etki her tuş vuruşunda aygıt sayımı
  // yapan `mic_hint`'i yeniden çağırıyordu. `$derived` ilkel değer değişmedikçe
  // haber vermiyor.
  const playerDevice = $derived(cfg?.player_device ?? "");
  $effect(() => {
    const id = playerDevice;
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
    // Kulaklık oturumu kendi aygıtlarıyla çalışıyor; Oynatıcı/Sunucu
    // ayarındaki bir değişiklik onu yeniden kurmamalı.
    if (stats?.headset_active) return;

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
    try { await fn(); } catch (e) { error = String(e).replace(/^Error:\s*/, ""); }
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

  const running = $derived(!!stats?.server_running || !!stats?.player_running);
  /** Kulaklık modu açık mı? Rust tarafında açıkça tutuluyor; eskiden
   *  "gönderici ve alıcı çalışıyor" diye tahmin ediliyordu ve `auto_listen`
   *  yüzünden yanlış "Durdur" gösteriliyordu. */
  const headsetRunning = $derived(!!stats?.headset_active);
  /** Şu an ses gelmesi bekleniyor mu? (kulaklık oturumu ya da kullanıcı
   *  Oynatıcı sekmesinde alıma bakıyor) */
  const expectingAudio = $derived(headsetRunning || tab === "player");

  // — Kulaklık modu —
  //
  // Aygıt seçim politikası **Rust tarafında** (`audio::headset_plan`).
  // Değişmez kural orada da aynı: yakalanan aygıt ile yazılan aygıt aynı
  // olamaz, yoksa kullanıcı kendini duyar.

  const chosenPeer = $derived(peers.find((p) => p.id === selectedPeer) ?? null);

  /** Bu eşle eşleştik mi? İstatistiklerden geliyor (her 500 ms). Kodu
   *  *gösteren* makine, karşı taraf kodu girince bunu böyle fark ediyor;
   *  eskiden arayüzdeki config kopyası bayat kalıyordu. */
  const isPaired = (p: Peer | null) =>
    !!p && !!p.device_id && (stats?.paired_ids ?? []).includes(p.device_id);
  const peerPaired = $derived(isPaired(chosenPeer));
  /** Eşleştirme kartı gösterilmeli mi? */
  const needsPairing = $derived(
    !!chosenPeer && chosenPeer.can_remote_start && !!chosenPeer.device_id && !peerPaired
  );

  // Kart görünürken bu makinenin kodu canlı olmalı (süresi dolunca yenisi),
  // görünmezken iptal edilmeli: ekranda olmayan bir kod, kimsenin
  // görmediği bir kapı.
  $effect(() => {
    const showing = tab === "headset" && needsPairing && !headsetRunning;
    const has = !!stats?.pairing_code;
    if (showing && !has) getPairingCode().catch(() => {});
    else if (!showing && has) cancelPairing().catch(() => {});
  });

  // Eşleşmiş sandığımız eşi düzenli doğrula. Eşleşme bir tarafta kalıp
  // öbüründe kaybolabiliyor (cevap yolda kaybolur, biri Ayarlar'dan kaldırır,
  // iki taraf aynı anda eşleşir). O zaman bu taraf kod göstermiyor, öbürü kod
  // bekliyor ve kullanıcı çıkmazda kalıyordu. Karşı taraf bizi tanımıyorsa
  // Rust bayat anahtarı siliyor; sonraki istatistikte kart kendiliğinden çıkıyor.
  //
  // Bağımlılıklar bilerek ilkel: `chosenPeer` her 500 ms'de yeni bir nesne,
  // ona bağlanan bir etki her tick'te yeniden çalışıp ağa istek yağdırırdı.
  const chosenCanRemote = $derived(!!chosenPeer?.can_remote_start);
  $effect(() => {
    const id = selectedPeer;
    if (!id || !peerPaired || !chosenCanRemote || tab !== "headset" || headsetRunning) return;
    const check = () => { verifyPairing(id).catch(() => {}); };
    check();
    const timer = setInterval(check, 10_000);
    return () => clearInterval(timer);
  });

  let pairingBusy = $state(false);
  let pairedWith = $state<string | null>(null);

  const submitCode = (code: string) =>
    act(async () => {
      if (!chosenPeer) throw new Error(t("error.needTarget"));
      pairingBusy = true;
      try {
        pairedWith = await pairWithPeer(chosenPeer.id, code);
        cfg = await getConfig();
      } catch (e) {
        const c = String(e).replace(/^Error:\s*/, "");
        throw new Error(REMOTE_CODES.has(c) ? t(`remote.${c}`) : c);
      } finally {
        pairingBusy = false;
      }
    });

  const savedPlay = $derived(
    (headsetRole === "local" ? cfg?.headset_local_play : cfg?.headset_remote_play) ?? ""
  );
  const savedCapture = $derived(
    (headsetRole === "local" ? cfg?.headset_local_capture : cfg?.headset_remote_capture) ?? ""
  );

  $effect(() => {
    const role = headsetRole;
    const play = savedPlay;
    const capture = savedCapture;
    // Aygıt listesi değiştiğinde (kablo kuruldu, kulaklık takıldı) planı
    // tazele; aksi hâlde kullanıcı Yenile'ye bassa da eski planı görüyor.
    void devices.length;
    getHeadsetPlan(role, play, capture)
      .then((p) => (headsetPlan = p))
      .catch((e) => { headsetPlan = null; error = String(e); });
  });

  /** Kulaklık aygıt seçimi — Oynatıcı/Sunucu ayarlarından ayrı alanlara. */
  function persistHeadsetDevice(which: "play" | "capture", id: string) {
    const key = `headset_${headsetRole}_${which}` as
      "headset_local_play" | "headset_local_capture" | "headset_remote_play" | "headset_remote_capture";
    persist({ [key]: id } as Partial<Config>);
  }

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
    "peer_gone", "no_control", "manual_target", "unreachable", "no_reply",
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

  // Bu makine uzaktan başlatıldığında rol Rust tarafında seçildi ve diske
  // yazıldı; arayüzdeki rol açılıştan kalma ve yanlış olabilir. Eşitlenmezse
  // plan yanlış rol için çözülüyor ve "şu mikrofonu seç" talimatı "?" diyordu.
  let lastOrigin = "";
  $effect(() => {
    const origin = stats?.started_by?.address ?? "";
    if (origin && origin !== lastOrigin) {
      getConfig().then((c) => {
        cfg = c;
        headsetRole = c.headset_role === "remote" ? "remote" : "local";
      }).catch(() => {});
    }
    lastOrigin = origin;
  });

  /** Toplantı uygulamasında seçilecek mikrofon. Uzak rolü karşı taraf
   *  üstlendiyse adı o bildiriyor (Rust saklıyor); biz üstlendiysek kendi
   *  planımızda. */
  const pairedMic = $derived(
    (headsetRole === "local" && !stats?.started_by
      ? stats?.linked_paired_mic
      : headsetPlan?.paired_mic) ?? "?"
  );

  /** Oturumun karşı ucunun adı — kim başlattıysa. Durum Rust'tan geliyor:
   *  arayüzün kendi belleğine güvenmek, pencere yeniden yüklenince çalışan
   *  bir oturumu "yarım başladı" gösteriyordu. */
  const sessionPeer = $derived(stats?.started_by?.name ?? stats?.linked_peer ?? "");
  /** Oturum gerçekten iki taraflı mı? */
  const sessionComplete = $derived(!!stats?.started_by || !!stats?.linked_peer);

  const toggleHeadset = () =>
    act(async () => {
      if (headsetRunning) {
        remoteResult = null;
        await stopHeadset();
        return;
      }
      if (headsetProblem) throw new Error(headsetProblem);
      if (needsPairing) throw new Error(t("headset.pairFirst"));
      const peer = manualTarget ? null : chosenPeer;
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

  function osIcon(os: string): IconName {
    return os === "windows" ? "windows" : os === "linux" ? "linux" : "monitor";
  }

  const NAV: { id: Tab; icon: IconName; key: string }[] = [
    { id: "headset", icon: "headset", key: "nav.headset" },
    { id: "server", icon: "send", key: "nav.server" },
    { id: "player", icon: "receive", key: "nav.player" },
    { id: "settings", icon: "settings", key: "nav.settings" },
  ];
</script>

{#if cfg}
<div class="app">
  <!-- ——— Kenar çubuğu ——— -->
  <nav class="sidebar" aria-label="RelAudio">
    <div class="brand">
      <span class="logo" aria-hidden="true"><Icon name="headset" size={18} /></span>
      <span>RelAudio</span>
    </div>

    <div class="navlist">
      {#each NAV as item (item.id)}
        <button class="nav" class:active={tab === item.id} onclick={() => (tab = item.id)}
                aria-current={tab === item.id ? "page" : undefined}>
          <Icon name={item.icon} size={17} />
          <span>{t(item.key)}</span>
          {#if item.id === "headset" && headsetRunning}<span class="pip" aria-hidden="true"></span>{/if}
        </button>
      {/each}
    </div>

    <div class="spacer"></div>

    <div class="me">
      <span class="me-label">{t("player.thisDevice")}</span>
      <strong title={myName}>{myName || "…"}</strong>
      <span class="me-addr">{localAddress ?? t("player.noAddress")}</span>
    </div>

    <div class="status" class:on={running}>
      <span class="dot" aria-hidden="true"></span>
      <span>{running ? t("status.live") : t("status.idle")}</span>
      {#if version}<span class="ver">v{version}</span>{/if}
    </div>
  </nav>

  <!-- ——— İçerik ——— -->
  <main>
    <div class="content">
      {#if stats?.started_by && tab !== "headset"}
        <div class="banner info" role="status">
          <Icon name="headset" />
          <div class="grow">
            <strong>{t("headset.startedBy", { device: stats.started_by.name })}</strong>
            <span>{stats.started_by.address}</span>
          </div>
          <button class="danger" onclick={() => act(stopHeadset)}>
            <Icon name="stop" size={15} />{t("headset.stop")}
          </button>
        </div>
      {/if}

      {#if stats?.feedback_loop}
        <div class="banner bad" role="alert">
          <Icon name="alert" />
          <div class="grow">
            <strong>{t("player.feedback")}</strong>
            <span>{t("player.feedbackBody")}</span>
          </div>
        </div>
      {/if}

      {#if error}
        <div class="banner bad" role="alert">
          <Icon name="alert" />
          <div class="grow">
            <strong>{t("error.title")}</strong>
            <span class="selectable">{error}</span>
          </div>
          <button class="ghost icon" onclick={() => (error = null)} aria-label={t("error.close")}>
            <Icon name="x" size={16} />
          </button>
        </div>
      {/if}

      {#if tab === "headset"}
        <header class="page">
          <h1>{t("headset.title")}</h1>
          <p>{t("headset.subtitle")}</p>
        </header>

        {#if headsetRunning}
          <!-- ——— Oturum sürüyor ——— -->
          <section class="session" class:partial={!sessionComplete}>
            <div class="session-head">
              <span class="live-dot" aria-hidden="true"></span>
              <div class="grow">
                <strong>
                  {#if stats?.started_by}
                    {t("headset.startedBy", { device: stats.started_by.name })}
                  {:else if stats?.linked_peer}
                    {t("headset.live", { device: sessionPeer })}
                  {:else}
                    {t("headset.halfStarted")}
                  {/if}
                </strong>
                {#if !sessionComplete}
                  {#if remoteError}<span>{remoteError}</span>{/if}
                  <span>{t("headset.startTheirSide")}</span>
                {/if}
              </div>
            </div>

            <div class="flows">
              <div class="flow">
                <span class="flow-icon up"><Icon name="send" size={16} /></span>
                <div class="grow">
                  <span class="flow-label">{t("stats.sending")}</span>
                  <strong title={stats?.server_device_name}>{stats?.server_device_name || "—"}</strong>
                </div>
                <span class="flow-num">{stats?.server_packets.toLocaleString() ?? 0}</span>
              </div>
              <div class="flow">
                <span class="flow-icon down"><Icon name="receive" size={16} /></span>
                <div class="grow">
                  <span class="flow-label">{t("stats.receiving")}</span>
                  <strong title={stats?.player_device_name}>{stats?.player_device_name || "—"}</strong>
                </div>
                <span class="flow-num">{stats?.player_packets.toLocaleString() ?? 0}</span>
              </div>
              {#if stats?.player_running}
                <LevelMeter peak={stats.player_peak} label={t("player.level")}
                            waitingText={t("player.levelWaiting")} />
              {/if}
            </div>

            {#if sessionComplete}
              <div class="howto">
                <strong>{headsetRole === "local" ? t("headset.setupTitle") : t("headset.setupTitleHere")}</strong>
                <ol>
                  <li>{t("headset.setupMic", { device: pairedMic })}</li>
                  <li>{t("headset.setupOut", {
                    device: headsetRole === "local" ? t("headset.theirSpeakers") : speakerName })}</li>
                </ol>
                <p class="note-warn"><Icon name="alert" size={14} />{t("headset.parsec")}</p>
              </div>
            {/if}

            <button class="danger big" onclick={toggleHeadset} disabled={busy}>
              <Icon name="stop" size={16} />{t("headset.stop")}
            </button>
          </section>

        {:else}
          <!-- ——— 1. Kulaklık nerede? ——— -->
          <section class="step">
            <h2><span class="num">1</span>{t("headset.step1")}</h2>
            <div class="choice-grid" role="radiogroup">
              <button class="choice" class:sel={headsetRole === "local"} role="radio"
                      aria-checked={headsetRole === "local"} onclick={() => (headsetRole = "local")}>
                <span class="choice-icon"><Icon name="headset" size={22} /></span>
                <strong>{t("headset.local")}</strong>
                <span class="choice-desc">{t("headset.localDesc")}</span>
              </button>
              <button class="choice" class:sel={headsetRole === "remote"} role="radio"
                      aria-checked={headsetRole === "remote"} onclick={() => (headsetRole = "remote")}>
                <span class="choice-icon"><Icon name="monitor" size={22} /></span>
                <strong>{t("headset.remote")}</strong>
                <span class="choice-desc">{t("headset.remoteDesc")}</span>
              </button>
            </div>
          </section>

          <!-- ——— 2. Karşı makine ——— -->
          <section class="step">
            <h2><span class="num">2</span>{t("headset.step2")}</h2>

            {#if peers.length === 0 && !manualTarget}
              <div class="empty">
                <span class="spinner" aria-hidden="true"></span>
                <span>{t("server.noPeers")}</span>
              </div>
            {/if}

            <div class="peer-list">
              {#each peers as p (p.id)}
                <button class="peer" class:sel={!manualTarget && selectedPeer === p.id}
                        onclick={() => { manualTarget = false; selectedPeer = p.id; }}>
                  <span class="peer-os"><Icon name={osIcon(p.os)} size={18} /></span>
                  <span class="grow">
                    <strong>{p.name}</strong>
                    <span class="peer-addr">{p.address}</span>
                  </span>
                  {#if !p.can_remote_start || !p.device_id}
                    <span class="tag">{t("peer.manualOnly")}</span>
                  {:else if isPaired(p)}
                    <span class="tag ok"><Icon name="check" size={13} />{t("pair.badge")}</span>
                  {:else}
                    <span class="tag">{t("pair.badgeNo")}</span>
                  {/if}
                </button>
              {/each}
              <button class="peer manual" class:sel={manualTarget}
                      onclick={() => { manualTarget = true; selectedPeer = ""; }}>
                <span class="peer-os"><Icon name="wifi" size={18} /></span>
                <span class="grow"><strong>{t("server.manual")}</strong></span>
              </button>
            </div>

            {#if manualTarget}
              <label class="field">
                <span class="lbl">{t("server.address")}</span>
                <input type="text" value={cfg.server_target} placeholder="192.168.1.10"
                       oninput={(e) => persist({ server_target: e.currentTarget.value })} />
                <span class="hint">{t("remote.manual_target")}</span>
              </label>
            {/if}
          </section>

          <!-- ——— Eşleştirme (bir kez) ——— -->
          {#if needsPairing && chosenPeer}
            <PairingCard peerName={chosenPeer.name} code={stats?.pairing_code ?? null}
                         seconds={stats?.pairing_seconds ?? 0} busy={pairingBusy}
                         onsubmit={submitCode} {t} />
          {:else if chosenPeer && !manualTarget && !chosenPeer.can_remote_start}
            <div class="banner warn">
              <Icon name="alert" />
              <div class="grow"><span>{t("headset.peerNoRemote", { device: chosenPeer.name })}</span></div>
            </div>
          {:else if pairedWith && peerPaired}
            <div class="banner ok">
              <Icon name="shield" />
              <div class="grow"><strong>{t("pair.done", { device: pairedWith })}</strong></div>
            </div>
          {/if}

          <!-- ——— Plan ve başlat ——— -->
          {#if headsetProblem}
            <div class="banner warn">
              <Icon name="alert" />
              <div class="grow"><strong>{headsetProblem}</strong></div>
            </div>
          {:else}
            <div class="plan">
              <div class="plan-end">
                <span class="plan-icon"><Icon name={headsetRole === "local" ? "mic" : "speaker"} size={16} /></span>
                <div class="grow">
                  <span class="plan-label">{t("stats.sourceDevice")}</span>
                  <strong title={headsetPlan?.capture?.name}>{headsetPlan?.capture?.name ?? "—"}</strong>
                </div>
              </div>
              <span class="plan-arrow"><Icon name="arrow-right" size={16} /></span>
              <div class="plan-end">
                <span class="plan-icon"><Icon name={headsetRole === "local" ? "headset" : "mic"} size={16} /></span>
                <div class="grow">
                  <span class="plan-label">{t("stats.outputDevice")}</span>
                  <strong title={headsetPlan?.play?.name}>{headsetPlan?.play?.name ?? "—"}</strong>
                </div>
              </div>
            </div>
          {/if}

          <button class="primary big" onclick={toggleHeadset}
                  disabled={busy || !!headsetProblem || needsPairing || (!chosenPeer && !manualTarget)}>
            <Icon name="play" size={16} />
            {needsPairing ? t("headset.pairFirst") : t("headset.start")}
          </button>
          <p class="foot">{t("headset.oneButton")}</p>
        {/if}

      {:else if tab === "server"}
        <header class="page">
          <h1>{t("server.title")}</h1>
          <p>{t("server.subtitle")}</p>
        </header>

        <section class="step">
          <div class="choice-grid">
            <button class="choice" class:sel={cfg.server_source === "monitor"}
                    onclick={() => persist({ server_source: "monitor" })}>
              <span class="choice-icon"><Icon name="speaker" size={22} /></span>
              <strong>{t("server.systemAudio")}</strong>
              <span class="choice-desc">{t("server.systemAudioDesc")}</span>
            </button>
            <button class="choice" class:sel={cfg.server_source === "input"}
                    onclick={() => persist({ server_source: "input" })}>
              <span class="choice-icon"><Icon name="mic" size={22} /></span>
              <strong>{t("server.mic")}</strong>
              <span class="choice-desc">{t("server.micDesc")}</span>
            </button>
          </div>
        </section>

        <section class="step">
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
        </section>

        <button class="big" class:danger={stats?.server_running} class:primary={!stats?.server_running}
                onclick={toggleServer} disabled={busy}>
          <Icon name={stats?.server_running ? "stop" : "play"} size={16} />
          {stats?.server_running ? t("server.stop") : t("server.start")}
        </button>

      {:else if tab === "player"}
        <header class="page">
          <h1>{t("player.title")}</h1>
          <p>{t("player.subtitle")}</p>
        </header>

        <section class="step">
          <div class="choice-grid">
            <button class="choice" class:sel={cfg.player_mode === "listen"}
                    onclick={() => persist({ player_mode: "listen" })}>
              <span class="choice-icon"><Icon name="speaker" size={22} /></span>
              <strong>{t("player.listen")}</strong>
              <span class="choice-desc">{t("player.listenDesc")}</span>
            </button>
            <button class="choice" class:sel={cfg.player_mode === "mic"}
                    onclick={() => persist({ player_mode: "mic" })}>
              <span class="choice-icon"><Icon name="mic" size={22} /></span>
              <strong>{t("player.micMode")}</strong>
              <span class="choice-desc">{t("player.micModeDesc")}</span>
            </button>
          </div>
        </section>

        <div class="addr">
          <span class="lbl">{t("player.thisDevice")}</span>
          <strong class="selectable">{myName || "…"}</strong>
          <span class="hint">
            {localAddress ?? t("player.noAddress")}{localAddress ? `:${cfg.player_port}` : ""}
            — {t("player.addressHint")}
          </span>
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

        {#if cfg.player_mode === "mic" && micHint?.paired_input}
          <div class="howto ok">
            <strong>{t("player.micReady")}</strong>
            <ol>
              <li class="done">{t("player.micStep1", {
                cable: devices.find((d) => d.id === cfg!.player_device)?.name ?? "—" })}</li>
              <li><strong>{t("player.micStep2", { device: micHint.paired_input })}</strong></li>
            </ol>
            <p>{t("player.micNotHere")}</p>
            {#if devices.find((d) => d.id === cfg!.player_device)?.is_default}
              <p class="note-warn"><Icon name="alert" size={14} />{t("player.cableIsDefault")}</p>
            {:else}
              <p class="note-warn"><Icon name="alert" size={14} />{t("player.dontChangeDefault")}</p>
            {/if}
            <p>{t("player.listenTabHint")}</p>
          </div>
        {:else if cfg.player_mode === "mic" && micHint && !micHint.any_virtual}
          <div class="banner warn">
            <Icon name="alert" />
            <div class="grow">
              <strong>{t("player.noCable")}</strong>
              <span>{t("player.noCableBody")}</span>
            </div>
          </div>
        {/if}

        <section class="step">
          <h2>{t("player.peers")}</h2>
          {#if peers.length}
            <div class="peer-list">
              {#each peers as p (p.id)}
                <div class="peer static">
                  <span class="peer-os"><Icon name={osIcon(p.os)} size={18} /></span>
                  <span class="grow">
                    <strong>{p.name}</strong>
                    <span class="peer-addr">{p.address}</span>
                  </span>
                  <span class="tag" class:ok={p.listening}>{p.listening ? t("status.live") : t("status.idle")}</span>
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty"><span class="spinner" aria-hidden="true"></span><span>{t("player.noPeersYet")}</span></div>
          {/if}
        </section>

        <button class="big" class:danger={stats?.player_running} class:primary={!stats?.player_running}
                onclick={togglePlayer} disabled={busy}>
          <Icon name={stats?.player_running ? "stop" : "play"} size={16} />
          {stats?.player_running ? t("player.stop") : t("player.start")}
        </button>

      {:else}
        <header class="page">
          <h1>{t("settings.title")}</h1>
        </header>

        <section class="group">
          <label class="field">
            <span class="lbl">{t("settings.language")}</span>
            <select value={lang} onchange={(e) => {
              lang = e.currentTarget.value; persist({ language: lang });
            }}>
              {#each LOCALES as l (l.code)}<option value={l.code}>{l.label}</option>{/each}
            </select>
            <span class="hint">{t("settings.languageDesc")}</span>
          </label>
        </section>

        <section class="group">
          <label class="toggle">
            <input type="checkbox" checked={cfg.minimize_to_tray}
                   onchange={(e) => {
                     const on = e.currentTarget.checked;
                     persist({ minimize_to_tray: on });
                     setMinimizeToTray(on).catch((err) => (error = String(err)));
                   }} />
            <span class="switch" aria-hidden="true"></span>
            <div><strong>{t("settings.tray")}</strong><span>{t("settings.trayDesc")}</span></div>
          </label>

          <label class="toggle">
            <input type="checkbox" checked={cfg.auto_listen}
                   onchange={(e) => persist({ auto_listen: e.currentTarget.checked })} />
            <span class="switch" aria-hidden="true"></span>
            <div><strong>{t("settings.autoListen")}</strong><span>{t("settings.autoListenDesc")}</span></div>
          </label>

          <label class="toggle">
            <input type="checkbox" checked={cfg.remote_control}
                   onchange={(e) => persist({ remote_control: e.currentTarget.checked })} />
            <span class="switch" aria-hidden="true"></span>
            <div><strong>{t("settings.remoteControl")}</strong>
                 <span>{t("settings.remoteControlDesc")}</span></div>
          </label>
        </section>

        <section class="group">
          <h2><Icon name="shield" size={16} />{t("settings.paired")}</h2>
          {#if Object.keys(cfg.paired ?? {}).length === 0}
            <p class="muted">{t("settings.pairedNone")}</p>
          {:else}
            <ul class="paired">
              {#each Object.entries(cfg.paired) as [id, p] (id)}
                <li>
                  <Icon name="check" size={15} />
                  <span class="grow">{p.name || id}</span>
                  <button class="ghost" onclick={() => act(async () => {
                    await unpair(id);
                    cfg = await getConfig();
                  })}>{t("settings.unpair")}</button>
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        <section class="group muted-group">
          <h2>{t("settings.limits")}</h2>
          <ul class="bullets"><li>{t("settings.limit1")}</li><li>{t("settings.limit2")}</li>
              <li>{t("settings.limit3")}</li></ul>
        </section>

        <section class="group muted-group">
          <h2>{t("settings.files")}</h2>
          <p class="path">{t("settings.configFile")}<code class="selectable">{configPath}</code></p>
          <p class="path">{t("settings.logFile")}<code class="selectable">{logPath}</code></p>
        </section>

        <BuyMeCoffee label={t("settings.coffee")} title={t("settings.support")}
                     body={t("settings.supportBody")}
                     failedText={t("settings.coffeeFailed")} />
      {/if}
    </div>
  </main>

  <!-- ——— Sağ panel ——— -->
  <aside>
    {#if tab === "headset" && !headsetRunning}
      <section class="panel">
        <h3>{t("headset.devices")}</h3>
        <DevicePicker devices={headsetPlan?.play_choices ?? []} kind="output"
                      value={headsetPlan?.play?.id ?? ""}
                      onselect={(id) => persistHeadsetDevice("play", id)}
                      label={headsetRole === "remote" ? t("device.cableLabel") : t("device.outputLabel")}
                      emptyText={t("device.none")} defaultText={t("device.default")} />
        <DevicePicker devices={headsetPlan?.capture_choices ?? []}
                      kind={headsetRole === "local" ? "input" : "monitor"}
                      value={headsetPlan?.capture?.id ?? ""}
                      onselect={(id) => persistHeadsetDevice("capture", id)}
                      label={headsetRole === "local" ? t("device.micLabel") : t("device.systemSource")}
                      emptyText={t("device.none")} defaultText={t("device.default")} />
        <button class="ghost wide" onclick={refresh}><Icon name="refresh" size={15} />{t("device.refresh")}</button>
      </section>

    {:else if tab === "player"}
      <section class="panel">
        <h3>{cfg.player_mode === "mic" ? t("device.virtualCable") : t("device.output")}</h3>
        <DevicePicker devices={outputChoices} kind="output" value={cfg.player_device}
                      onselect={(id) => persist({ player_device: id })}
                      label={cfg.player_mode === "mic" ? t("device.cableLabel") : t("device.outputLabel")}
                      hint={cfg.player_mode === "mic" ? t("device.cableHint") : ""}
                      emptyText={t("device.none")} defaultText={t("device.default")} />
        <button class="ghost wide" onclick={refresh}><Icon name="refresh" size={15} />{t("device.refresh")}</button>
      </section>
    {:else if tab === "server"}
      <section class="panel">
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
        <button class="ghost wide" onclick={refresh}><Icon name="refresh" size={15} />{t("device.refresh")}</button>
      </section>
    {/if}

    <section class="panel">
      <h3>{t("stats.title")}</h3>
      {#if stats?.server_running}
        <div class="grp"><Icon name="send" size={13} />{t("stats.sending")} · {stats.server_target}</div>
        <div class="tiles">
          <div class="tile"><span>{t("stats.packets")}</span><strong>{stats.server_packets.toLocaleString()}</strong></div>
          <div class="tile"><span>{t("stats.bitrate")}</span><strong>{stats.server_kbps.toFixed(0)}<small> kbit/s</small></strong></div>
        </div>
        <StatRow label={t("stats.sourceDevice")} value={stats.server_device_name} />
        <StatRow label={t("stats.silentRatio")} value={`${(stats.server_silent_ratio * 100).toFixed(0)}%`} />
        {#if stats.server_send_errors > 0}
          <StatRow label={t("stats.sendErrors")} value={String(stats.server_send_errors)} tone="bad" />
          <p class="tip">{t("server.notReaching")}</p>
        {/if}
      {/if}
      {#if stats?.player_running}
        <div class="grp"><Icon name="receive" size={13} />{t("stats.receiving")} · {t("stats.port")} {stats.player_port}</div>
        <!-- "Paket gelmiyor" yalnızca ses beklenirken bir sorun. Otomatik
             dinleme boşta beklerken uyarmak, her açılışta kırmızı bir
             alarm göstermek demekti. -->
        <div class="tiles">
          <div class="tile" class:warn={stats.player_packets === 0 && expectingAudio}>
            <span>{t("stats.packets")}</span><strong>{stats.player_packets.toLocaleString()}</strong>
          </div>
          <div class="tile"><span>{t("stats.buffer")}</span><strong>{stats.player_buffer_ms}<small> ms</small></strong></div>
        </div>
        {#if stats.player_packets === 0 && expectingAudio}
          <p class="tip">{t("player.noPackets", { address: `${localAddress ?? "?"}:${stats.player_port}` })}</p>
        {/if}
        <StatRow label={t("stats.outputDevice")} value={stats.player_device_name} />
        <StatRow label={t("stats.bitrate")} value={`${stats.player_kbps.toFixed(0)} kbit/s`} />
        <StatRow label={t("stats.lost")} value={String(stats.player_lost)}
                 tone={stats.player_lost > 0 ? "warn" : "ok"} />
        <StatRow label={t("stats.late")} value={String(stats.player_late)}
                 tone={stats.player_late > 0 ? "warn" : "ok"} />
        <StatRow label={t("stats.underruns")} value={String(stats.player_underruns)}
                 tone={stats.player_underruns > 0 ? "warn" : "ok"} />
        <StatRow label={t("stats.dropped")} value={String(stats.player_dropped)}
                 tone={stats.player_dropped > 0 ? "bad" : "ok"} />
        {#if stats.player_foreign > 0}
          <StatRow label={t("stats.foreign")} value={String(stats.player_foreign)} tone="warn" />
        {/if}
      {/if}
      {#if !running}<p class="muted">{t("stats.idle")}</p>{/if}
    </section>
  </aside>
</div>
{/if}

<style>
  /* ——— Yerleşim ——— */
  .app { display: grid; grid-template-columns: 220px minmax(0, 1fr) 300px; height: 100vh; }

  .sidebar {
    background: var(--surface); border-inline-end: 1px solid var(--line);
    padding: 18px 12px 14px; display: flex; flex-direction: column; gap: 4px; min-width: 0;
  }
  .brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 16px; padding: 2px 8px 18px; }
  .logo {
    display: grid; place-items: center; width: 30px; height: 30px; border-radius: 9px;
    background: linear-gradient(135deg, var(--accent), #8b5cf6); color: #fff;
    box-shadow: 0 4px 14px rgba(91, 140, 255, .35);
  }
  .navlist { display: flex; flex-direction: column; gap: 2px; }
  .nav {
    justify-content: flex-start; background: none; border: 1px solid transparent;
    color: var(--dim); padding: 9px 11px; font-weight: 500; position: relative;
  }
  .nav:hover:not(:disabled) { background: var(--surface-2); color: var(--text); border-color: transparent; }
  .nav.active { background: var(--accent-soft); color: var(--text); font-weight: 600; }
  .nav.active :global(svg) { color: var(--accent); }
  .pip { width: 7px; height: 7px; border-radius: 50%; background: var(--ok); margin-inline-start: auto; box-shadow: 0 0 0 3px var(--ok-soft); }
  .spacer { flex: 1; }

  .me {
    display: flex; flex-direction: column; gap: 1px; padding: 12px;
    border: 1px solid var(--line); border-radius: var(--r-md); background: var(--surface-2); margin-bottom: 10px;
  }
  .me-label { font-size: 11px; color: var(--faint); text-transform: uppercase; letter-spacing: .5px; }
  .me strong { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .me-addr { font-size: 12px; color: var(--dim); font-variant-numeric: tabular-nums; }

  .status { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--dim); padding: 0 8px; }
  .status .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--faint); }
  .status.on { color: var(--ok); }
  .status.on .dot { background: var(--ok); box-shadow: 0 0 0 3px var(--ok-soft); animation: pulse 2s infinite; }
  .ver { margin-inline-start: auto; color: var(--faint); font-variant-numeric: tabular-nums; }

  main { overflow-y: auto; min-width: 0; }
  .content { max-width: 760px; margin: 0 auto; padding: 28px 32px 40px; }

  aside {
    background: var(--surface); border-inline-start: 1px solid var(--line);
    padding: 18px 16px; overflow-y: auto; display: flex; flex-direction: column; gap: 14px; min-width: 0;
  }

  /* ——— Sayfa başlığı ve adımlar ——— */
  .page { margin-bottom: 22px; }
  .page h1 { margin: 0; font-size: 24px; font-weight: 700; }
  .page p { margin: 6px 0 0; color: var(--dim); font-size: 13.5px; line-height: 1.55; }

  .step { margin-bottom: 22px; }
  .step h2, .group h2 {
    display: flex; align-items: center; gap: 9px;
    margin: 0 0 12px; font-size: 14px; font-weight: 650;
  }
  .num {
    display: grid; place-items: center; width: 22px; height: 22px; border-radius: 50%;
    background: var(--surface-3); color: var(--dim); font-size: 12px; font-weight: 700;
  }

  /* Seçim kartları */
  .choice-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 10px; }
  .choice {
    flex-direction: column; align-items: flex-start; justify-content: flex-start; text-align: start;
    gap: 6px; padding: 16px; border-radius: var(--r-md);
    background: var(--surface); border: 1.5px solid var(--line); font-weight: 400;
  }
  .choice:hover:not(:disabled) { border-color: var(--line-strong); background: var(--surface-2); }
  .choice.sel { border-color: var(--accent); background: var(--accent-soft); box-shadow: 0 0 0 3px var(--accent-soft); }
  .choice-icon {
    display: grid; place-items: center; width: 38px; height: 38px; border-radius: 11px;
    background: var(--surface-3); color: var(--dim); margin-bottom: 4px;
  }
  .choice.sel .choice-icon { background: var(--accent); color: var(--on-accent); }
  .choice strong { font-size: 14px; font-weight: 650; }
  .choice-desc { color: var(--dim); font-size: 12.5px; line-height: 1.5; }

  /* Eş listesi */
  .peer-list { display: flex; flex-direction: column; gap: 8px; }
  .peer {
    justify-content: flex-start; text-align: start; gap: 12px;
    padding: 11px 14px; border-radius: var(--r-md); font-weight: 400;
    background: var(--surface); border: 1.5px solid var(--line);
  }
  .peer.static { display: flex; align-items: center; cursor: default; }
  .peer.sel { border-color: var(--accent); background: var(--accent-soft); }
  .peer.manual { border-style: dashed; color: var(--dim); }
  .peer.manual.sel { color: var(--text); }
  .peer-os {
    flex: none; display: grid; place-items: center; width: 34px; height: 34px;
    border-radius: 10px; background: var(--surface-3); color: var(--dim);
  }
  .peer.sel .peer-os { color: var(--accent); }
  .peer strong { display: block; font-size: 14px; font-weight: 600; }
  .peer-addr { display: block; font-size: 12px; color: var(--dim); font-variant-numeric: tabular-nums; }
  .tag {
    flex: none; display: inline-flex; align-items: center; gap: 4px;
    font-size: 11.5px; font-weight: 600; padding: 3px 9px; border-radius: 99px;
    background: var(--surface-3); color: var(--dim);
  }
  .tag.ok { background: var(--ok-soft); color: var(--ok); }

  .empty {
    display: flex; align-items: center; gap: 10px; color: var(--dim); font-size: 13px;
    padding: 14px; border: 1px dashed var(--line); border-radius: var(--r-md); margin-bottom: 8px;
  }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%; flex: none;
    border: 2px solid var(--line-strong); border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
  }

  /* Plan özeti */
  .plan {
    display: flex; align-items: center; gap: 10px; padding: 12px 14px; margin-bottom: 16px;
    border: 1px solid var(--line); border-radius: var(--r-md); background: var(--surface);
  }
  .plan-end { display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0; }
  .plan-icon {
    flex: none; display: grid; place-items: center; width: 30px; height: 30px; border-radius: 9px;
    background: var(--surface-3); color: var(--dim);
  }
  /* Sınıf üzerinden: `.plan-end span` ikon kutusunu da (o da bir span)
     yakalayıp `display: grid`'i eziyordu, ikon ortalanmıyordu. */
  .plan-label { display: block; font-size: 11px; color: var(--faint); text-transform: uppercase; letter-spacing: .4px; }
  .plan-end strong { display: block; font-size: 13px; font-weight: 550; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .plan-arrow { flex: none; color: var(--faint); }

  .foot { text-align: center; color: var(--faint); font-size: 12px; margin: 10px 0 0; }

  /* Oturum */
  .session {
    border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--surface);
    padding: 20px; box-shadow: var(--shadow); display: flex; flex-direction: column; gap: 16px;
  }
  .session-head { display: flex; gap: 12px; align-items: flex-start; }
  .session-head strong { display: block; font-size: 16px; }
  .session-head .grow span { display: block; color: var(--dim); font-size: 12.5px; margin-top: 3px; line-height: 1.5; }
  .live-dot {
    flex: none; width: 12px; height: 12px; border-radius: 50%; margin-top: 5px;
    background: var(--ok); box-shadow: 0 0 0 4px var(--ok-soft); animation: pulse 2s infinite;
  }
  .session.partial .live-dot { background: var(--warn); box-shadow: 0 0 0 4px var(--warn-soft); }

  .flows { display: flex; flex-direction: column; gap: 8px; }
  .flow {
    display: flex; align-items: center; gap: 12px; padding: 10px 12px;
    border-radius: var(--r-md); background: var(--surface-2);
  }
  .flow-icon { flex: none; display: grid; place-items: center; width: 30px; height: 30px; border-radius: 9px; }
  .flow-icon.up { background: var(--accent-soft); color: var(--accent); }
  .flow-icon.down { background: var(--ok-soft); color: var(--ok); }
  .flow-label { display: block; font-size: 11px; color: var(--faint); text-transform: uppercase; letter-spacing: .4px; }
  .flow strong { display: block; font-size: 13px; font-weight: 550; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .flow-num { font-variant-numeric: tabular-nums; color: var(--dim); font-size: 12.5px; }

  .howto {
    border-radius: var(--r-md); padding: 14px 16px; font-size: 13px;
    background: var(--surface-2); border: 1px solid var(--line);
  }
  .howto.ok { background: var(--ok-soft); border-color: transparent; margin-bottom: 18px; }
  .howto strong { display: block; margin-bottom: 4px; }
  .howto ol { margin: 6px 0 0; padding-inline-start: 20px; }
  .howto li { margin: 5px 0; line-height: 1.5; }
  .howto li.done { color: var(--dim); }
  .howto p { margin: 6px 0 0; color: var(--dim); line-height: 1.5; font-size: 12.5px; }
  .note-warn { display: flex; gap: 6px; align-items: flex-start; color: var(--warn) !important; }
  .note-warn :global(svg) { flex: none; margin-top: 2px; }

  /* Bantlar */
  .banner {
    display: flex; align-items: flex-start; gap: 12px; padding: 12px 14px; margin-bottom: 16px;
    border-radius: var(--r-md); font-size: 13px; border: 1px solid transparent;
  }
  .banner > :global(svg) { flex: none; margin-top: 1px; }
  .banner strong { display: block; }
  .banner span { display: block; color: var(--dim); line-height: 1.5; }
  .banner.bad { background: var(--bad-soft); border-color: rgba(240,93,93,.35); }
  .banner.bad > :global(svg) { color: var(--bad); }
  .banner.warn { background: var(--warn-soft); border-color: rgba(240,177,61,.30); }
  .banner.warn > :global(svg) { color: var(--warn); }
  .banner.ok { background: var(--ok-soft); border-color: rgba(52,199,123,.30); }
  .banner.ok > :global(svg) { color: var(--ok); }
  .banner.info { background: var(--accent-soft); border-color: var(--accent-line); align-items: center; }
  .banner.info > :global(svg) { color: var(--accent); }
  .banner button { flex: none; }
  .icon { padding: 6px; }
  .grow { flex: 1; min-width: 0; }

  /* Formlar */
  .field { display: block; margin-bottom: 16px; }
  .lbl { display: block; font-size: 12px; color: var(--dim); margin-bottom: 6px; font-weight: 500; }
  .hint { display: block; font-size: 12px; color: var(--faint); margin-top: 6px; line-height: 1.5; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }

  .addr {
    border: 1px solid var(--line); border-radius: var(--r-md); padding: 14px 16px;
    margin-bottom: 18px; background: var(--surface);
  }
  .addr strong { display: block; font-size: 18px; margin: 4px 0 2px; font-variant-numeric: tabular-nums; }

  /* Ayarlar */
  .group {
    border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--surface);
    padding: 18px; margin-bottom: 14px;
  }
  .group .field:last-child { margin-bottom: 0; }
  .muted-group { background: transparent; }
  .muted { color: var(--dim); font-size: 13px; margin: 0; }
  .bullets { margin: 0; padding-inline-start: 18px; color: var(--dim); font-size: 12.5px; }
  .bullets li { margin: 5px 0; line-height: 1.5; }
  .path { margin: 8px 0 0; font-size: 12px; color: var(--dim); display: flex; flex-direction: column; gap: 3px; }
  .path code { color: var(--text); font-size: 11.5px; word-break: break-all; }

  .toggle { display: flex; gap: 14px; align-items: flex-start; padding: 10px 0; cursor: pointer; position: relative; }
  .toggle + .toggle { border-top: 1px solid var(--line); }
  .toggle input { position: absolute; opacity: 0; width: 1px; height: 1px; }
  .switch {
    flex: none; width: 38px; height: 22px; border-radius: 99px; margin-top: 1px;
    background: var(--surface-3); border: 1px solid var(--line-strong); position: relative;
    transition: background 160ms var(--ease);
  }
  .switch::after {
    content: ""; position: absolute; top: 2px; inset-inline-start: 2px; width: 16px; height: 16px;
    border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgba(0,0,0,.3);
    transition: transform 160ms var(--ease);
  }
  .toggle input:checked + .switch { background: var(--accent); border-color: transparent; }
  .toggle input:checked + .switch::after { transform: translateX(16px); }
  :global([dir="rtl"]) .toggle input:checked + .switch::after { transform: translateX(-16px); }
  .toggle input:focus-visible + .switch { box-shadow: 0 0 0 3px var(--accent-soft), 0 0 0 1px var(--accent); }
  .toggle strong { display: block; font-size: 14px; font-weight: 600; }
  .toggle div span { display: block; color: var(--dim); font-size: 12.5px; margin-top: 2px; line-height: 1.5; }

  .paired { list-style: none; margin: 0; padding: 0; }
  .paired li {
    display: flex; align-items: center; gap: 10px; padding: 8px 4px;
    border-bottom: 1px solid var(--line); font-size: 13px;
  }
  .paired li:last-child { border-bottom: none; }
  .paired li :global(svg) { color: var(--ok); flex: none; }

  /* Sağ panel */
  .panel {
    border: 1px solid var(--line); border-radius: var(--r-md); background: var(--surface-2); padding: 15px;
  }
  .panel h3 {
    margin: 0 0 12px; font-size: 11.5px; color: var(--faint);
    text-transform: uppercase; letter-spacing: .7px; font-weight: 650;
  }
  .wide { width: 100%; }
  .grp {
    display: flex; align-items: center; gap: 6px;
    font-size: 11px; color: var(--dim); text-transform: uppercase; letter-spacing: .5px;
    margin: 14px 0 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .grp:first-of-type { margin-top: 0; }
  .tiles { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-bottom: 8px; }
  .tile {
    background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-sm); padding: 9px 10px;
  }
  .tile.warn { border-color: rgba(240,177,61,.4); }
  .tile span { display: block; font-size: 11px; color: var(--faint); }
  .tile strong { display: block; font-size: 17px; font-variant-numeric: tabular-nums; margin-top: 2px; }
  .tile small { font-size: 11px; color: var(--dim); font-weight: 500; }
  .tip { font-size: 12px; color: var(--warn); margin: 4px 0 10px; line-height: 1.5; }

  .selectable { user-select: text; }

  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: .55; } }

  /* Dar pencere: sağ paneli alta al. */
  @media (max-width: 960px) {
    .app { grid-template-columns: 200px minmax(0, 1fr); grid-template-rows: minmax(0, 1fr) auto; }
    .sidebar { grid-row: 1 / span 2; }
    aside {
      grid-column: 2; border-inline-start: none; border-top: 1px solid var(--line);
      max-height: 38vh; flex-direction: row; flex-wrap: wrap;
    }
    aside .panel { flex: 1 1 260px; }
    .content { padding: 22px; }
  }
</style>

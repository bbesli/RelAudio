# 02 — Mimari

## Süreç modeli

Üç ayrı çalışma birimi var. Ayrım bilinçli: **arayüz çökerse veya kapanırsa ses
yayını devam etmeli.**

```
┌──────────────────────────────────────────────────────────────┐
│ relaudio-app  (Tauri, kullanıcı oturumunda)                  │
│                                                              │
│  ┌────────────────────┐        ┌──────────────────────────┐  │
│  │ WebView (frontend) │◄──IPC─►│ Tauri Rust katmanı       │  │
│  │ TS + React/Svelte  │        │ tray, pencere, autostart │  │
│  └────────────────────┘        └───────────┬──────────────┘  │
└────────────────────────────────────────────┼─────────────────┘
                                             │ yerel IPC
                                             │ (UDS / Named Pipe, JSON-RPC)
                                ┌────────────▼─────────────────┐
                                │ relaudio-core  (ayrı süreç)  │
                                │ Rust, gerçek zamanlı         │
                                │ ┌──────────┬───────────────┐ │
                                │ │ capture  │ playback      │ │
                                │ ├──────────┼───────────────┤ │
                                │ │ codec    │ jitter buffer │ │
                                │ ├──────────┴───────────────┤ │
                                │ │ transport (UDP+TCP)      │ │
                                │ ├──────────────────────────┤ │
                                │ │ discovery (mDNS)         │ │
                                │ └──────────────────────────┘ │
                                └────────────┬─────────────────┘
                                             │ platform ses API
                    ┌────────────────────────┼────────────────────────┐
                    ▼                        ▼                        ▼
              WASAPI (Win)           CoreAudio (macOS)      PipeWire/Pulse (Linux)
                    │                        │                        │
                    └──── opsiyonel ─────────┴──── sanal aygıt ───────┘
                                     relaudio-driver
```

### Neden ayrı süreç?

| Gerekçe | Açıklama |
|---|---|
| Dayanıklılık | WebView çökmesi yayını kesmemeli |
| Arka plan | UI penceresi hiç açılmadan da yayın sürebilmeli |
| Zamanlama | Ses thread'i, UI'ın render/GC yükünden yalıtılmalı |
| Güncelleme | Arayüz yeniden başlatılırken oturum korunabilir |
| Taşınabilirlik | Aynı çekirdek ileride CLI, servis veya mobil köprü olarak kullanılabilir |

**Maliyet:** IPC katmanı yazmak, sidecar'ın yaşam döngüsünü yönetmek (başlatma,
sağlık kontrolü, zombi süreç temizliği, sürüm uyuşmazlığı). Bu maliyet kabul edildi.

### Çekirdek yaşam döngüsü

- UI açıldığında çekirdeği arar; yoksa başlatır (`spawn`, stdio yakalanır).
- Çekirdek tekil olmalı: kilit dosyası + soket varlığı kontrolü.
- Çekirdek **UI kapansa da yaşar**; yalnızca "Çıkış" komutuyla veya oturum
  kapanışında sonlanır.
- UI'dan `ping` gelmemesi çekirdeği durdurmaz. Tersine, çekirdek UI'ı bekleyerek
  başlamaz.
- Sürüm uyuşmazlığında çekirdek `hello` sırasında hata döner; UI kullanıcıya
  "yeniden başlat" der.

## Katmanlar (relaudio-core)

```
core/
  audio/
    device.rs        Aygıt listeleme, hot-plug izleme, varsayılan takibi
    capture/         windows.rs (WASAPI) · linux.rs (PulseAudio API)
    playback/        aynı modüller  —  macOS ertelendi
                     Linux'ta neden Pulse API'si: adr/0004
    resample.rs      Sabit oran + adaptif drift düzeltme
    ring.rs          Kilitlemesiz (lock-free) SPSC halka tampon
  codec/
    pcm.rs           s16le / f32le, isteğe bağlı big-endian yok
    opus.rs          libopus bağlaması, VBR/CBR, FEC
  net/
    discovery.rs     mDNS ilan + tarama, UDP broadcast fallback
    control.rs       TCP kontrol kanalı, oturum durumu, el sıkışma
    rtp.rs           UDP ses taşıma, sıra no, zaman damgası
    jitter.rs        Adaptif jitter buffer, PLC
    clock.rs         Saat kayması kestirimi
  session/
    server.rs        Gönderici oturum makinesi
    player.rs        Alıcı oturum makinesi
    stats.rs         Metrik toplama
  ipc/
    server.rs        UDS / Named Pipe dinleyici
    protocol.rs      JSON-RPC istek/olay tipleri
  config.rs          Kalıcı ayarlar (TOML)
```

### Gerçek zamanlı disiplin

Ses geri çağrımı (audio callback) içinde **yasak**: bellek ayırma, kilit alma,
sistem çağrısı, log yazma, `String` üretimi. İzin verilen: önceden ayrılmış
tamponlarda aritmetik, atomik okuma/yazma, kilitlemesiz kuyruk işlemleri.

Veri akışı:

```
capture callback ──push──► SPSC ring ──► network thread ──► UDP
UDP ──► network thread ──► jitter buffer ──► SPSC ring ──pop──► playback callback
```

Ses thread'i ile ağ thread'i arasındaki tek temas noktası halka tampondur.

## IPC protokolü (UI ↔ core)

Taşıma: Unix Domain Socket (Linux/macOS, `$XDG_RUNTIME_DIR/relaudio.sock`),
Named Pipe (Windows, `\\.\pipe\relaudio`). Mesaj biçimi: satır-ayrımlı JSON-RPC 2.0.

**Neden localhost TCP değil:** güvenlik duvarı uyarısı çıkarmamak ve makinedeki
diğer kullanıcıların erişimini engellemek için. Soket izinleri 0600.

### İstekler (UI → core)

| Metot | Parametre | Döner |
|---|---|---|
| `hello` | `{ ui_version }` | `{ core_version, protocol_version }` |
| `devices.list` | `{ direction: "input"\|"output" }` | Aygıt dizisi |
| `devices.refresh` | — | Aygıt dizisi |
| `peers.list` | — | Keşfedilen eşler |
| `server.start` | `{ mode, device_id, codec, bitrate? }` | `{ session_id }` |
| `server.stop` | `{ session_id }` | — |
| `player.connect` | `{ address, port?, mode, device_id }` | `{ session_id }` |
| `player.disconnect` | `{ session_id }` | — |
| `config.get` / `config.set` | ayar anahtarları | ayar nesnesi |
| `stats.subscribe` | `{ interval_ms }` | — |
| `shutdown` | — | — |

### Olaylar (core → UI)

| Olay | Yük |
|---|---|
| `peer.discovered` / `peer.lost` | eş bilgisi |
| `device.changed` | yeni aygıt listesi (hot-plug) |
| `session.state` | `connecting \| streaming \| degraded \| stopped` + sebep |
| `stats.tick` | gecikme, kayıp %, bitrate, buffer doluluk, underrun sayacı |
| `error` | kod + kullanıcıya gösterilecek mesaj |

## Konfigürasyon ve dosya konumları

| Amaç | Windows | macOS | Linux |
|---|---|---|---|
| Ayarlar | `%APPDATA%\RelAudio\config.toml` | `~/Library/Application Support/RelAudio/` | `$XDG_CONFIG_HOME/relaudio/` |
| Log | `%LOCALAPPDATA%\RelAudio\logs\` | `~/Library/Logs/RelAudio/` | `$XDG_STATE_HOME/relaudio/logs/` |
| Runtime soket | `\\.\pipe\relaudio` | `$TMPDIR/relaudio.sock` | `$XDG_RUNTIME_DIR/relaudio.sock` |

Log: dosyaya döngüsel (rotating), varsayılan `info`, `RELAUDIO_LOG` ile
değiştirilebilir. Ses callback'i içinden log yazılmaz; sayaçlar atomik artırılır,
ayrı thread periyodik raporlar.

## Depo yerleşimi (öneri)

```
relaudio/
  crates/
    relaudio-proto/     Ağ protokolü tipleri (paylaşılan)   ✅ yazıldı
    relaudio-core/      Ses + ağ çekirdeği                  ✅ yazıldı
    relaudio-cli/       Komut satırı sürücüsü               ✅ yazıldı
  app/                  Tauri uygulaması
    src/                Frontend (TS)
    src-tauri/          Tauri Rust katmanı
  drivers/
    windows/            (v1.x) lisanslı üçüncü taraf sürücü paketleme
  docs/
  scripts/              Build, imzalama, test yardımcıları
```

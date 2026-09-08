# 03 — Ses Hattı

## Kanonik format

Motor içinde tek bir dahili format kullanılır; dönüşümler kenarlarda yapılır.

- **Örnekleme hızı:** 48 000 Hz (dahili). Aygıt farklı hızdaysa kenarda yeniden
  örneklenir.
- **Örnek tipi:** `f32` interleaved (işleme), ağa çıkarken `s16le` veya Opus.
- **Kanal:** 2 (stereo). Mono kaynak çoğaltılır, çok kanallı kaynak stereo'ya
  indirgenir (v1'de basit downmix).
- **Blok boyutu:** 480 örnek (10 ms) ağ paketi başına. Aygıt tamponu daha küçük
  olabilir (128/256), halka tampon aradaki farkı emer.

10 ms seçimi: Opus'un doğal çerçevesiyle uyumlu, UDP paketi 1500 baytlık MTU'ya
sığıyor (PCM s16 stereo 480 örnek = 1920 bayt → **sığmıyor**, bu yüzden PCM'de
paket başına 5 ms / 240 örnek = 960 bayt + başlık kullanılır). Kodek bazında:

| Kodek | Paket süresi | Yük boyutu | Bitrate |
|---|---|---|---|
| PCM s16 stereo | 5 ms | 960 B | 1.536 Mbit/s + başlık |
| PCM f32 stereo | 5 ms | 1920 B → MTU riski, kullanılmaz | — |
| Opus stereo 128k | 10 ms | ~160 B | 128 kbit/s |
| Opus stereo 256k | 10 ms | ~320 B | 256 kbit/s |

## Yakalama (capture)

### Windows — WASAPI

- **Sistem sesi:** `IAudioClient::Initialize` ile `AUDCLNT_STREAMFLAGS_LOOPBACK`.
  Render (çıkış) aygıtı loopback modunda açılır. **Ek sürücü gerekmez** — Windows'un
  en büyük avantajı bu.
- Loopback yalnızca **shared mode**'da çalışır; exclusive mode ile birlikte kullanılamaz.
- **Sessizlik davranışı — ÖLÇÜLDÜ (Adım 1b):** Hiçbir uygulama endpoint'i
  tutmuyorken loopback **hiç veri üretmez** — sessizlik verisi bile gelmez, olay
  hiç tetiklenmez. Bir uygulama endpoint'i açık tuttuğu sürece sessiz pasajlarda
  veri akmaya devam eder. Gönderici bu boşluğu tespit edip ya sessizlik bayraklı
  keepalive yollamalı ya da endpoint'e sessiz bir render akışı açmalıdır.
  Alıcı sessizliği kopma sanmamalı. Ayrıntı: `docs/10`, Bulgu 6.
- **Mikrofon:** normal capture akışı + `IMMDeviceEnumerator` ile aygıt seçimi.
- Düşük gecikme için `IAudioClient3::InitializeSharedAudioStream` (Win10 1703+),
  aygıtın desteklediği minimum periyot sorgulanır.
- Aygıt değişimi: `IMMNotificationClient` ile varsayılan aygıt ve hot-plug takibi.

### macOS — CoreAudio *(ertelendi)*

> **Kapsam dışı (ertelendi).** macOS v1 hedefi değil. Bu bölüm ileride
> geri gelmesi için korunuyor; şu an iş yapılmıyor.


Sistem sesi yakalama tarihsel olarak macOS'un zayıf noktası. Üç yol var:

| Yöntem | Sürüm | Not |
|---|---|---|
| **CoreAudio process tap** (`CATapDescription`, `AudioHardwareCreateProcessTap`) | macOS **14.4+** | En temiz yol. Sistem geneli veya süreç bazlı. `NSAudioCaptureUsageDescription` gerekir. |
| **ScreenCaptureKit** (`SCStream` audio-only) | macOS **13+** | Çalışır ama ekran kaydı izni ister; kullanıcıya "neden ekranımı kaydediyor?" sorusu doğurur. |
| **Sanal aygıt** (kendi AudioServerPlugin'imiz veya BlackHole) | Tümü | Kullanıcı çıkışı sanal aygıta çevirmek zorunda; UX zayıf ama en uyumlu. |

**Plan:** 14.4+ ise process tap; 13.x ise ScreenCaptureKit; ikisi de yoksa/izin
verilmediyse sanal aygıta yönlendir. Bu üçlü fallback [04](04-sanal-ses-aygitlari.md)
ile birlikte tasarlanır.

- **Mikrofon:** `AudioUnit` (kAudioUnitSubType_HALOutput, input enabled) veya
  `AVAudioEngine`. `NSMicrophoneUsageDescription` + TCC izni zorunlu.
- Aygıt değişimi: `kAudioHardwarePropertyDefaultOutputDevice` property listener.

### Linux — PipeWire (öncelik) / PulseAudio (fallback)

- **En kolay platform.** Her çıkış aygıtının otomatik bir `.monitor` kaynağı vardır;
  sistem sesi ek kurulum olmadan yakalanır.
- PipeWire ile doğrudan `pw-stream` (capture, `PW_KEY_STREAM_CAPTURE_SINK=true`)
  kullanılır; kuantum (buffer) uygulama başına ayarlanabilir → en iyi gecikme kontrolü.
- PulseAudio-only sistemlerde `pa_stream` + monitor source adı.
- ALSA doğrudan kullanılmaz (paylaşımlı erişim sorunları); yalnızca son çare.
- **Ekran görüntüsündeki aygıt listesi bunu doğruluyor:** "AudioRelay Speaker",
  "AudioRelay Mic&Sink" — bunlar uygulamanın çalışma anında yarattığı null-sink'ler.
- Flatpak dağıtımında PipeWire erişimi portal üzerinden; `--socket=pipewire` gerekir.

## Çalma (playback)

| Platform | API | Not |
|---|---|---|
| Windows | WASAPI shared, event-driven | `IAudioClient3` ile düşük periyot |
| Linux | PipeWire `pw-stream` playback | Kuantum `RELAUDIO` node özelliğiyle ayarlanır |

Aygıt kaybolduğunda (kulaklık çıkarıldı) akış otomatik varsayılana taşınır;
kullanıcı açıkça bir aygıt seçtiyse hata gösterilir ve yeniden bağlanma denenir.

## Gecikme bütçesi

Hedef: uçtan uca ≤ 80 ms (kablolu LAN).

| Aşama | Tipik | En iyi | Notlar |
|---|---|---|---|
| Yakalama tamponu | 5–10 ms | 3 ms | Ekran görüntüsünde AudioRelay 6 ms gösteriyor |
| Kodlama (Opus) | 10 ms | 2.5 ms | PCM'de ~0; Opus çerçeve süresi kadar |
| Ağ (kablolu LAN) | 0.5–2 ms | 0.3 ms | Wi-Fi'da 3–30 ms, tepe noktalar daha yüksek |
| Jitter buffer | 20–40 ms | 10 ms | **Ana kalem.** Adaptif; kayıp arttıkça büyür |
| Kod çözme | 1–5 ms | 1 ms | |
| Çalma tamponu | 5–10 ms | 3 ms | |
| **Toplam** | **~45–80 ms** | **~20 ms** | |

Wi-Fi'da gerçekçi hedef 100–150 ms. Kullanıcıya "düşük gecikme / dengeli /
kararlı" şeklinde üç profil sunulur; bu profiller jitter buffer hedefini ve
kodek/paket süresini değiştirir.

## Saat kayması (clock drift)

Gönderici ve alıcı ses kartları nominal 48 kHz'i birebir aynı üretmez; tipik
sapma ±10–100 ppm. 50 ppm sapmada saatte ~180 ms birikir — telafi edilmezse
her birkaç dakikada bir kopma duyulur.

**Çözüm:** alıcıda **adaptif yeniden örnekleme.**

1. Jitter buffer doluluğu sürekli ölçülür, uzun pencerede (ör. 10 s) ortalaması alınır.
2. Ortalama hedeften sapıyorsa yeniden örnekleme oranı çok küçük bir katsayıyla
   düzeltilir (ör. 1.000_02).
3. Düzeltme yavaş ve sınırlı olmalı (< 200 ppm/s), aksi hâlde perde kayması duyulur.

Alternatif — örnek ekleme/atma (drop/insert) — basit ama duyulabilir; yalnızca
acil durum düzeltmesi olarak (buffer taşarken) kullanılır.

Uygulama: `rubato` (Rust) veya `libsoxr` ile değişken oranlı resampler.

## Kayıp gizleme (PLC)

- **Opus:** yerleşik PLC + isteğe bağlı in-band FEC (`OPUS_SET_INBAND_FEC`).
  Kayıp %5'e kadar kabul edilebilir kalitede tolere edilir.
- **PCM:** yerleşik PLC yok. Basit strateji: son çerçeveyi 5 ms rampa ile
  söndür, ardından sessizlik. Tek paket kaybı (5 ms) çoğunlukla duyulmaz.
- Ardışık kayıp eşiği aşılırsa oturum `degraded` durumuna geçer ve UI uyarır.

## Test yöntemleri

**Gecikme ölçümü (döngü testi):** A makinesinde bilinen bir darbe (klik) üretilir,
B'de çalınır, B'nin çıkışı fiziksel kabloyla A'nın girişine bağlanır; A darbeler
arası süreyi ölçer. Kablo yoksa iki makinenin sesi tek bir kayıt cihazına alınıp
ofset ölçülür.

**Kalite:** 20 Hz–20 kHz sweep gönderilip alınan sinyalle karşılaştırma; THD+N
ve frekans yanıtı. PCM'de bit-exact karşılaştırma (kayıpsız olmalı).

**Dayanıklılık:** 8 saatlik sürekli yayın, underrun/overrun sayacı 0 kalmalı.
`tc netem` ile yapay gecikme/kayıp/jitter enjekte ederek jitter buffer davranışı.

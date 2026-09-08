# 10 — Riskler ve Açık Sorular

## Riskler

Etki × olasılık sırasına göre.

### 1. macOS sistem sesi yakalama — ÇÖZÜLDÜ (kapsam dışı)

Bu risk, macOS'un v1 kapsamından çıkarılmasıyla ortadan kalktı. Sorun gerçekti:
sürüme bağlı üç ayrı yakalama yolu, hiçbiri temiz değil, 14.4 altında ekran
kaydı izni gerekiyor. macOS geri kapsama girerse bu risk de geri gelir —
analiz [03](03-ses-hatti.md) ve [04](04-sanal-ses-aygitlari.md)'te duruyor.

### 2. Windows sanal mikrofon — sürücü satıcısına ticari bağımlılık — ORTA

Teknik risk büyük ölçüde çözüldü: kendi sürücümüzü yazmıyoruz, imzalı bir
üçüncü taraf sürücüyü lisanslayıp paketliyoruz ([ADR-0003](adr/0003-v1de-kendi-surucumuz-yok.md)).
EV sertifika, WDK ve attestation signing masadan kalktı.

Kalan risk **ticari**: lisans şartları ve ücreti henüz bilinmiyor, satıcıya
bağımlı hâle geliyoruz, ve lisans açık kaynak dağıtımla uyumsuz olabilir.

**Bu, uygulama binary'sinin imzalanmasıyla ilgili değildir.** Uygulamayı imzasız
dağıtmak sorunsuz; sadece SmartScreen uyarısı çıkar. İki konu ayrıdır.

**Azaltma:** Lisans şartları Faz 5'ten önce netleştirilmeli (aşağıdaki
doğrulama listesi). Anlaşma olmazsa geri düşüş yolu v1 davranışıdır:
kullanıcı VB-CABLE'ı kendisi kurar — çalışır, sadece UX zayıftır.
Ana senaryo ("Send audio") her koşulda etkilenmiyor.

### 3. Linux'ta GNOME tepsi yokluğu — ORTA, ama kritik hata potansiyeli

GNOME'da eklenti olmadan tepsi simgesi görünmez. "Çarpı → gizle" davranışı
bu durumda uygulamayı **erişilemez** hâle getirir.

**Azaltma:** Tepsi yaratılamadığında minimize-to-tray otomatik kapatılır.
Bu, Faz 4'ün kabul kriterine dahil.

### 4. Saat kayması — ORTA

Telafi edilmezse birkaç dakikada bir duyulur kopma. Adaptif resampler'ı doğru
ayarlamak deneysel bir iştir; agresif düzeltme perde kaymasına yol açar.

**Azaltma:** Faz 2'de uzun süreli otomatik test; buffer doluluk grafiği ile
görsel doğrulama.

### 5. Wi-Fi güvenilmezliği — ORTA

Ev Wi-Fi'ında jitter tepe noktaları 100 ms'yi geçebilir. Kullanıcı bunu
uygulamanın hatası sanar.

**Azaltma:** Ağ kalitesi göstergesi; kötüleştiğinde açık uyarı ve
"Kararlı" profiline geçiş önerisi.

### 6. İki dilli build zinciri — DÜŞÜK (Tauri ile), ORTA (Electron ile)

Rust + TS'i üç platformda derleyip paketlemek CI kurulumu gerektirir.
Tauri seçimi bunu azaltıyor.

### 7. WebKitGTK farklılıkları — DÜŞÜK

Linux'ta arayüz Chromium'dan farklı render edilebilir.

**Azaltma:** Muhafazakâr CSS, üç platformda görsel test.

### 8. Antivirüs / güvenlik duvarı yanlış pozitifi — DÜŞÜK

Sesi yakalayıp ağa gönderen bir program, davranış olarak casus yazılıma benzer.

**Azaltma:** Kod imzalama, açık kaynak olma, VirusTotal takibi, gerekirse
satıcılara whitelist başvurusu.

---

## Karar bekleyenler

Bunlar cevaplandıkça bu dosya güncellenir ve gerekirse `adr/` altına kayıt açılır.

| # | Soru | Etki |
|---|---|---|
| 1 | Frontend: React mi Svelte mi? | Düşük — istenildiği zaman verilebilir |
| 2 | Şifreleme varsayılan açık mı? CPU maliyeti ölçülmeli | Orta |
| 3 | Cihaz adı çakışmasında ne olacak (aynı ağda iki "Linux")? | Düşük |
| 4 | Windows'ta sessizlik anında loopback davranışı sürüme göre değişiyor mu? | Orta — Faz 0'da ölçülmeli |
| 5 | Mobil istemci gerçekten kapsam dışı mı, yoksa protokol baştan ona göre mi tasarlanmalı? | Yüksek — protokolü etkiler |
| 6 | Lisans ne olacak (GPL / MIT / kapalı)? | **Yüksek** — sürücü OEM lisansı açık kaynak dağıtımla uyumsuz olabilir |
| 9 | ~~Windows sürücü lisansı hangi satıcıdan?~~ **Karar: VB-Audio, paketimize gömülü** | Kapandı |
| 10 | VB-Audio'ya yapılacak bağış tutarı ne olsun? (500–2000 USD öneriliyor) | Düşük — çıkış öncesi |
| 7 | Aygıt hot-plug sırasında oturum korunacak mı, düşürülecek mi? | Düşük |
| 8 | "Kararlı" profilde maksimum buffer 400 ms mı olmalı, daha fazla mı? | Düşük |

## Adım 1a ölçüm sonuçları (Linux, 2026-09-08)

Ortam: CachyOS, KDE/Wayland, PipeWire 1.6.8, Razer BlackShark V2 HS 2.4 (USB).

**Kavram doğrulandı.** Sistem sesi monitor'den yakalandı, UDP ile taşındı, çıkışa
geri çalındı ve duyuldu. Hat: `pw-record --target <sink>.monitor` → `socat UDP` →
`pw-play`.

| Test | Sonuç |
|---|---|
| `pw-play` / `paplay` ile çalma | ✓ çalışıyor |
| UDP taşıma (socat, 960 baytlık paket) | ✓ **kayıpsız** — 576000/576000 bayt, sinyal bozulmadı |
| Monitor'den yakalama | ✓ (koşullu — aşağıya bak) |
| Tam hat (yakala → UDP → çal) | ✓ ses uçtan uca geçti |

### Bulgu 1 — Monitor'e bağlanma sırası önemli

Sink boştayken (`SUSPENDED`) monitor kaynağına bağlanan `pw-record` **sessizlik
yakalıyor**; sink sonradan aktifleşse bile link kurulmuyor. Aynı yakalama, sink
zaten aktifken başlatıldığında sorunsuz çalışıyor.

**Implementasyona etkisi:** `audio/capture/linux_pipewire.rs`, monitor kaynağına
bağlanırken sink'in durumunu kontrol etmeli ve sink `SUSPENDED` → `RUNNING`
geçişinde akışı **yeniden kurmalı**. Aksi hâlde "ses gönderiyorum ama karşı taraf
sessizlik duyuyor" hatası çıkar — sessiz başarısızlık olduğu için en kötü türden.

### Bulgu 2 — Null sink'ler suspend oluyor

`module-null-sink` ile yaratılan sink'ler grafikte hiç aktifleşmedi (hepsi `S`
durumunda kaldı, QUANT 0). Sanal aygıt tarafında (`docs/04`) bunu ayrıca çözmek
gerekecek — muhtemelen `node.always-process=true` veya benzeri bir özellik.

### Bulgu 3 — Grafik kuantumu 21.3 ms, istenen 5.3 ms yok sayıldı

`--latency 256` (5.3 ms) istendi ama grafiğin gerçek kuantumu **1024 (21.3 ms)**
kaldı. Sebep muhtemelen aynı anda çalışan diğer istemciler: `parsecd` 1024,
`VoiceEngine` **1440 (30 ms)**.

**Bu, `docs/03`'teki "yakalama gecikmesi ≤ 10 ms" hedefini doğrudan tehdit ediyor.**
Yakalama + çalma birlikte ~42 ms eder, jitter buffer bunun üstüne biner.

**Sebep bulundu — PipeWire kısıtı değil, aygıt kısıtı.** Ayarlar:

```
clock.quantum      = 1024      ← varsayılan
clock.min-quantum  = 32        ← 256 fazlasıyla izin dahilinde
clock.force-quantum= 0         ← zorlanmamış
api.alsa.period-size = 512     ← aygıtta
api.alsa.headroom    = 512     ← aygıtta
```

`period-size 512 + headroom 512 = 1024`. Yani 21.3 ms bu **USB kablosuz
kulaklığın** (Razer BlackShark V2 HS 2.4) karakteristiği; PipeWire yapılandırması
32'ye kadar izin veriyor. Kablolu analog çıkış veya HDMI farklı davranabilir.

Test edilecek: `pw-metadata -n settings 0 clock.force-quantum 256` ile grafik
zorla düşürüldüğünde USB aygıt yetişiyor mu, yoksa xrun mu veriyor?
Referans ürünün gösterdiği 6 ms, farklı bir aygıtta ölçülmüş olabilir.

### Bulgu 4 — AudioRelay grafiği kirletiyor

Çalışırken `audiorelay_source` ve `audiorelay_Speaker` düğümleri `rate 0` ile
duruyor ve hata sayaçları artıyor (ERR 1–2). Ölçüm sırasında kapatılmalı.

### Bulgu 5 — `pw-record --target` / `pw-play --target` aygıt adını çözemiyor

**Bu turun en pahalı bulgusu.** PipeWire 1.6.8'de:

| Komut | Sonuç |
|---|---|
| `pw-record --target <sink>.monitor dosya.raw` | **tepe = 0** (sessizlik) |
| `parecord --device=<sink>.monitor --raw dosya.raw` | tepe = 27655 ✓ |

Aynı kaynak, aynı anda, aynı ses. `pw-cat` ailesi adı çözemeyip sessiz
yakalıyor — hata vermiyor, **sessizce başarısız oluyor.** Null sink'lerin de
ölü kalması (Bulgu 2) aynı sebepten: `pw-play --target <nullsink>` bağlanamıyordu.

Bulgu 2 böylece **geçersiz** — null sink'ler pulse araçlarıyla sorunsuz çalışıyor.

**Implementasyona etkisi:** Bu bir CLI aracı sorunu, kütüphane sorunu değil.
Rust tarafında `pipewire-rs` ile node'lara `object.serial` / `node.name`
üzerinden bağlanacağız. Ama ders genel: **aygıt eşleştirmesi isimle yapılınca
sessizce başarısız olabiliyor.** `docs/04`'te Windows için de aynı endişeyi
yazmıştık (görünen ad yerine donanım kimliği). Aynı disiplin Linux'ta da gerekli,
ve bağlantı kurulduktan sonra **sinyal geliyor mu diye doğrulanmalı.**

### Ölçüm sonuçları — uçtan uca hat gecikmesi

Yöntem: null sink içinde yankı treni. Tek patlama çalınır, hat onu geri getirir,
ardışık yankılar arası mesafe = hat gecikmesi. Fiziksel çıkışa ses gitmez.

| Grafik kuantumu | Ölçülen hat gecikmesi | Ölçüm sayısı | Sapma |
|---|---|---|---|
| 1024 (21.3 ms) — varsayılan | **21.3 ms** | 189 | 0 |
| 240 (5.0 ms) — zorlanmış | **15.0 ms** | 269 | 0 |
| 144 (3.0 ms) — zorlanmış | **12.0 ms** | 336 | 0 |

Sapma sıfır — üç ölçümde de min = maks = medyan.

**Sonuç: gecikme hedefi ulaşılabilir.** Kaba bir kabuk hattı (`parecord | tee |
socat | paplay`) bile 12 ms'ye iniyor. `docs/03`'teki 40–80 ms uçtan uca hedefi
rahat görünüyor; asıl payı jitter buffer alacak.

**Uyarılar:**
- Bu ölçüm null sink + localhost içinde. Gerçek yol fiziksel ALSA tamponu ve
  gerçek ağ ekler.
- 12 ms'lik taban, spike'ın `socat` + `tee` + pipe yükünü içeriyor; Rust
  implementasyonunda bu kalem olmayacak.
- **İstemcinin latency isteği grafiği düşürmedi.** `--latency-msec=10` istendi,
  kuantum 1024'te kaldı; yalnızca global `clock.force-quantum` işe yaradı.
  Adım 2'de `pipewire-rs` ile `node.latency` doğru şekilde ayarlanabiliyor mu,
  yeniden ölçülmeli. Uygulama **global ayarı değiştirmemeli.**

## Adım 1b ölçüm sonuçları (Windows, 2026-09-08)

Ortam: Windows, Speakers (Realtek(R) Audio), `wasapi` crate 0.24, shared mode
loopback (`Direction::Render` aygıtı + `Direction::Capture` yönü).

| Ölçüm | Sonuç |
|---|---|
| Aygıt periyodu | varsayılan **10.0 ms**, minimum **3.0 ms** |
| Gerçekleşen tampon | 1056 kare = **22.0 ms** (min istendi, verilmedi) |
| Ses akışı | çalarken kusursuz, 48000 kare/s |
| Kopukluk (discontinuity) | 46 s'de 23 — çoğu başlat/durdur geçişlerinde |
| **Ağ aktarımı** | **8.870.400 bayt gönderildi → 8.870.400 bayt alındı** |
| Paket kaybı | **%0** (gerçek LAN, iki fiziksel makine) |

Linux tarafında ses duyuldu ve kaydedildi: 46.20 s, tepe 32768, %64 sıfır olmayan.

### Bulgu 6 — Loopback, hiçbir uygulama çalmıyorken HİÇ veri üretmiyor

**Açık sorulardan biri kesin cevaplandı.** İlk 18 saniye hiçbir şey çalınmadı:

```
[ 18.1s] kare 0 (0.00s ses)  veri-tamponu 10  SESSİZ-tampon 0  tepe 0
```

- Yakalanan kare: **0**
- `AUDCLNT_BUFFERFLAGS_SILENT` bayraklı tampon: **0**
- Olay 2 saniyede bir zaman aşımına uğradı

Yani sessizlikte "sessizlik verisi" gelmiyor — **hiçbir şey gelmiyor**, olay hiç
tetiklenmiyor. Müzik açılınca aynı anda 48000 kare/s'ye çıktı.

**Önemli nüans:** Bir uygulama endpoint'i açık tuttuğu sürece, sessiz pasajlarda
bile veri akmaya devam ediyor (`tepe 1` iken kare sayacı artmaya devam etti).
Yani kapı "ses var mı" değil, **"herhangi bir uygulama endpoint'i tutuyor mu"**.

**Implementasyona etkisi — iki seçenek:**

1. **Zaman aşımını tespit et, sessizlik bayraklı keepalive gönder.** Protokolde
   bu bayrak zaten var (`docs/05`, Flags bit1). Bant genişliği tasarrufu sağlar.
2. **Endpoint'e sessiz bir render akışı aç.** Loopback'in sürekli veri üretmesini
   zorlar. Birçok loopback kaydedicisinin yaptığı budur; daha basit ama sürekli
   CPU/uyanma maliyeti var.

Alıcı taraf her hâlükârda **sessizliği bağlantı kopması sanmamalı** —
`docs/05`'teki oturum durum makinesi buna göre yazılmalı.

### Bulgu 7 — Shared mode istenen tamponu vermiyor

`buffer_duration_hns: min_time` (3 ms) istendi, 1056 kare (22 ms) verildi.
Shared mode'da WASAPI motor periyodunu dayatıyor. Daha düşüğü için
`IAudioClient3::InitializeSharedAudioStream` gerekiyor (Win10 1703+).
`wasapi` crate'inin bunu destekleyip desteklemediği Adım 2'de bakılacak.

Linux'takiyle simetrik bir durum: orada da istemci isteği yok sayılmış,
yalnızca global `force-quantum` işe yaramıştı.

### Bulgu 8 — WebKitGTK DMABUF renderer'ı Wayland'da çökertiyor

Tauri uygulaması KDE/Wayland'da pencere açılır açılmaz çöküyordu:

```
Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display.
```

Sebep WebKitGTK'nın DMABUF renderer'ı. Üç ayrı geçici çözüm de işe yarıyor
(`GDK_BACKEND=x11`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`,
`WEBKIT_DISABLE_DMABUF_RENDERER=1`) ama sonuncusu **yerel Wayland'ı koruyor**,
X11'e düşmüyor.

Uygulama bunu `main()` içinde kendisi ayarlıyor (`app/src-tauri/src/main.rs`),
kullanıcıdan ortam değişkeni beklenmiyor. Kullanıcı açıkça bir değer verdiyse
ona dokunulmuyor.

`docs/01`'de "Linux'ta WebKitGTK sürprizleri" diye kabul edilen riskin ilk
somut örneği bu.

### Bulgu 9 — PulseAudio olmayan aygıt adında hata vermiyor, varsayılana düşüyor

`pa_simple_new` bilinmeyen bir aygıt adıyla çağrıldığında **başarı dönüyor** ve
sessizce varsayılan aygıtı kullanıyor. Testle yakalandı
(`app/src-tauri/src/session.rs`, `server_start_reports_bad_device...`).

Kullanıcıya etkisi: HDMI monitörünü seçersin, o aygıt kaybolmuştur, uygulama
hiçbir şey söylemeden kulaklıktan yayın yapar. Yanlış aygıttan yayın, üstelik
sessizce — Bulgu 5'teki tuzağın daha kötü hâli.

**Çözüm:** `audio/linux.rs::resolve` artık aygıtı açmadan önce listede var mı
**ve türü doğru mu** diye bakıyor. Windows tarafı (`find_device`) zaten
koleksiyonda arayıp `DeviceNotFound` döndürüyordu.

**Genel ders:** Ses API'lerinde "aygıt seçimi" güvenilmez. Açmadan önce
doğrula, açtıktan sonra sinyal geldiğini doğrula.

### Bulgu 10 — WASAPI nesneleri `Send` değil

`cargo check --target x86_64-pc-windows-msvc` ile yakalandı (linker gerekmiyor,
tip kontrolü yeterli). COM arayüzleri (`IAudioClient`, `HANDLE`) `Send`
uygulamıyor, ama `Capture`/`Playback` trait'lerinde `Send` bound'u vardı.

Tasarım zaten `Send` gerektirmiyordu: her akış onu kullanan thread'in içinde
açılıp orada tüketiliyor, thread sınırını geçmiyor. Bound kaldırıldı.

**Yöntem notu:** Windows kodunu Linux'ta doğrulamanın yolu
`cargo check --target x86_64-pc-windows-msvc`. Derler, link etmez; MSVC
toolchain gerekmez. Windows'a özgü kodda regresyon olmaması için CI'da da
çalıştırılmalı.

### Bulgu 11 — Windows → Linux ses aktarımı doğrulandı

Gerçek iki makine, gerçek ağ: Windows sistem sesi → Linux hoparlörü. Çalışıyor.

### Bulgu 12 — Windows çalma yolunda kısmi yazma hatası

Linux → Windows denendiğinde **paketler ulaştı ama ses çıkmadı.**

Sebep `windows.rs::WasapiPlayback::write` içindeydi: yalnızca kuyrukta **tam bir
cihaz tamponu kadar** veri varken yazıyordu (`queue.len() < space * blockalign`
ise hiç yazmıyordu). Bize 5 ms'lik (960 bayt) bloklar geliyor, cihaz tamponu ise
22 ms (~4224 bayt). Yani çoğu çağrıda hiçbir şey yazılmadı, cihaz tamponu sürekli
aç kaldı.

**Düzeltme:** ne kadar yer varsa o kadar yaz — kısmi de olsa. Cihaz tamponu
dolduğunda olayı bekle; pacing oradan gelir.

**Ders:** Bu, Linux tarafında Pulse `tlength` parametresini vermediğimizde
yaşadığımız salınımın (bkz. üstteki jitter buffer notları) Windows'taki karşılığı.
Her iki platformda da **pacing'i ses saatine bırakmak** doğru olan; tampon
doluluğuna göre kendi mantığını kurmak hataya açık.

Tip kontrolü bunu yakalayamazdı — mantık hatası. Windows çalma yolu hâlâ gerçek
donanımda doğrulanmayı bekliyor.

**Henüz test edilmeyenler:**
- Linux → Windows yönü — bir hata bulunup düzeltildi (Bulgu 12), yeniden
  test edilmeyi bekliyor.
- Mikrofon aktarımı (her iki yönde).
- Sanal mikrofon senaryosu (uzak mikrofonu Windows'ta yerel mikrofon gibi
  kullanma). `docs/04`'teki strateji hâlâ **varsayım**, doğrulanmadı.

## Adım 1 KAPANDI

Her iki platformda ses yakalanıyor, taşınıyor ve çalınıyor. Gecikme ölçüldü,
paket kaybı sıfır. Mimariye bağlanmak için yeterli veri var.

### Henüz ölçülmedi

- İki makine arasında **sayısal** uçtan uca gecikme (Linux içi ölçüm yapıldı;
  gerçek ağ ölçümü Adım 2'de çekirdek zaman damgalarıyla yapılacak).

## Doğrulanması gereken teknik varsayımlar

Bu dokümanlardaki şu iddialar Faz 0'da **koda dökülerek** doğrulanmalıdır:

- [x] ~~Windows WASAPI loopback, hiç ses çalmıyorken de akış üretiyor mu?~~
      **HAYIR — hiç veri gelmiyor, olay bile tetiklenmiyor.** Bkz. Bulgu 6.
- [ ] PipeWire'da uygulamanın kendi `Audio/Source` node'unu kaydettirmesi,
      harici modül olmadan sanal mikrofon için yeterli mi?
- [ ] Hedeflenen yakalama gecikmesi (≤ 10 ms) gerçek donanımda tutuyor mu?
- [ ] mDNS, yaygın ev yönlendiricilerinde (client isolation kapalıyken) güvenilir mi?
- [x] ~~VB-Audio redistribution izni var mı?~~ **Evet, doğrulandı** — paketleme
      açıkça serbest, bağış lisans ücreti bekleniyor, yalnızca temel VB-CABLE.
- [ ] VB-CABLE'ın sessiz kurulum bayrakları neler? Resmî belge yok; test edilmeli.
- [ ] Kurulum sonrası **yeniden başlatma** zorunlu mu, yoksa aygıt hemen görünüyor mu?
- [ ] Aygıtı görünen ad yerine donanım kimliğinden tespit etme deseni nedir?
      (`PKEY_Device_InstanceId` içeriği — kullanıcı aygıtı yeniden adlandırırsa
      isim eşleştirmesi kırılır.)
- [ ] Aygıt yeniden adlandırma ("Virtual Mic for RelAudio") mümkün mü?
      Standart izne dahil değil; istenirse VB-Audio'ya ayrıca sorulmalı.
- [ ] Virtual Audio Cable (VAC) redistribution şartları — yalnızca VB-CABLE
      yetersiz kalırsa gerekli.
- [ ] Microsoft `sysvad` örneğinden türetilen bir sürücüyü attestation ile
      imzalatmanın gerçek süreci ne kadar sürüyor? (Kod MS-PL ile bedava;
      darboğaz yalnızca imzalama.)
- [ ] Lisanslı sürücü kurulumu yönetici yetkisi / yeniden başlatma istiyor mu?

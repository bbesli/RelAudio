# RelAudio

Yerel ağ üzerinden bilgisayarlar arasında ses aktarımı — sistem sesi veya
mikrofon, iki yönde de, düşük gecikmeyle.

**Windows ve Linux.** Ücretsiz ve açık kaynak (MIT).

*[English README](README.md)*

---

## Ne yapıyor

| | |
|---|---|
| **Sistem sesini gönder** | Bilgisayarının sesini başka bir bilgisayarın hoparlöründen çal |
| **Mikrofon gönder** | Bir makinedeki mikrofonu diğerine aktar |
| **Dinle** | Gelen sesi bu bilgisayarın hoparlöründen duy |
| **Mikrofon olarak kullan** | Uzaktaki mikrofonu Discord, Zoom, OBS'ye mikrofon olarak tanıt |

Cihazlar ağda birbirini otomatik buluyor — IP yazmak yok. Uygulama sistem
tepsisinde yaşıyor, pencereyi kapatsan da yayın sürüyor. Arayüz 10 dilde.

**Gecikme.** Yazılım yolu, Linux'ta yerel bir döngü testinde ses grafiği
kuantumuna göre **12–21 ms** ölçüldü ([docs/10-riskler.md](docs/10-riskler.md)).
İki makine arasında, varsayılan ayarlarla uçtan uca kabaca **60–90 ms** bekle —
jitter buffer varsayılanda 40 ms, her makinenin aygıt tamponu da 20 ms civarı
ekliyor. Daha azı gerekiyorsa tamponu ve aygıt periyodunu düşür.

---

## Kurulum

### Linux

`rustup`, Node.js ve PulseAudio/PipeWire geliştirme başlıkları gerekiyor.

```bash
# Arch / CachyOS
sudo pacman -S --needed base-devel rustup nodejs npm libpulse webkit2gtk-4.1 libayatana-appindicator
rustup default stable

# Debian / Ubuntu
sudo apt install build-essential curl libpulse-dev libwebkit2gtk-4.1-dev \
                 libayatana-appindicator3-dev librsvg2-dev nodejs npm

# Fedora
sudo dnf install @development-tools pulseaudio-libs-devel webkit2gtk4.1-devel \
                 libappindicator-gtk3-devel librsvg2-devel nodejs
```

Sonra derle ve kur:

```bash
git clone https://github.com/bbesli/RelAudio.git
cd RelAudio/app && npm install && npx tauri build --no-bundle
cd .. && ./scripts/install-linux.sh
```

`~/.local/bin/relaudio` olarak kurulur ve uygulama menüsüne **RelAudio**
girdisi eklenir. Yönetici yetkisi gerekmez. Kaldırmak için
`./scripts/uninstall-linux.sh`.

> `relaudio` komutu bulunamıyorsa `~/.local/bin` `PATH`'inde değildir.
> Ya uygulama menüsünden aç, ya da şunu ekle:
> `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc`

`.deb` ve `.rpm` paketleri `npx tauri build` ile üretilebilir (AppImage
üretimi şu an başarısız — [Bilinen sınırlar](#bilinen-sınırlar)).

### Windows

Gerekenler:

1. **[Rust](https://rustup.rs)** — `rustup-init.exe` çalıştır, standart kurulumu seç.
2. **Visual C++ Build Tools** ve Windows SDK. `rustup` kurmayı teklif etmezse:
   ```
   winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
   `--add ...VCTools` kısmı önemli — onsuz Build Tools kurulur ama C++
   derleyicisi gelmez, derleme `link.exe not found` ile düşer.
3. **[Node.js](https://nodejs.org)**

Sonra:

```
git clone https://github.com/bbesli/RelAudio.git
cd RelAudio
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

Betik gereksinimleri kontrol eder, Visual Studio ortamını yükler (Insiders
sürümleri dahil — Rust bunları tek başına bulamıyor), npm bağımlılıklarını
kurar ve derler. Çıktı:
`app\src-tauri\target\release\relaudio-app.exe`

**Güvenlik duvarı:** ilk kez ses alırken Windows izin soracak, özel ağlar için
onayla. Elle kural eklemek istersen, yönetici PowerShell'de:

```
New-NetFirewallRule -DisplayName "RelAudio UDP 59101" -Direction Inbound -Protocol UDP -LocalPort 59101 -Action Allow
```

---

## Kullanım

Her iki bilgisayarda RelAudio'yu aç. Her biri otomatik olarak dinlemeye başlar
ve kendini ağa ilan eder.

### Bir bilgisayarın sesini diğerinde çal

1. **Alan makine** — *Oynatıcı* sekmesi, **Hoparlörden dinle**, çıkış aygıtı
   olarak hoparlörünü seç.
2. **Gönderen makine** — *Sunucu* sekmesi, **Sistem sesi**, **Hedef cihaz**
   listesinden alan makineyi seç, **Yayına başla**.

Linux'ta "sistem sesi" bir çıkış aygıtının *monitor*'ü demektir. Varsayılan
genelde doğrudur; ses gelmiyorsa medya oynatıcının hangi çıkışı kullandığına
bak (`pactl list sink-inputs`).

### Mikrofon gönder

Aynısı, ama gönderen makinede **Mikrofon**'u seç ve sağ panelden mikrofonu belirle.

### Kulaklık modu — bir makinenin kulaklığını diğerinde kullan

RelAudio'nun asıl yazılma sebebi bu: B makinesini kullanıyorsun (Parsec, RDP,
Sunshine, ne olursa) ama kulaklığın A makinesinde takılı ve onun B'nin
kulaklığı gibi davranmasını istiyorsun — hem mikrofon hem hoparlör.

İki makinede de **Kulaklık** sekmesini aç ve rol seç:

| Makine | Rol | Ne yapar |
|---|---|---|
| Kulaklığın takılı olduğu | **Kulaklık bu makinede** | Mikrofonunu gönderir, geleni bu kulaklıkta çalar |
| Kullandığın makine | **Uzak makine** | Gelen mikrofonu sanal kabloya yazar, sistem sesini geri gönderir |

Karşı cihazı hedef seç, iki tarafta da başlat. Aygıtları RelAudio kendisi
seçiyor ve asla bozmadığı tek bir kural var: **bir makinede yakaladığı aygıt
ile yazdığı aygıt asla aynı olmaz.** Bu kural bozulursa ses kendi kuyruğunu
yer ve kendi sesini duyarsın.

Uzak makinede toplantı uygulamasına hangi mikrofonu kullanacağını yine sen
söylüyorsun — uygulama adı ekranda yazıyor.

> **Uzak masaüstünün ses aktarımını kapat.** Parsec, RDP ve benzerleri
> makinenin varsayılan çıkışını yakalayıp sana gönderiyor. RelAudio da aynı
> sesi taşıyorsa iki kez alırsın — ve uzak masaüstü RelAudio'nun yazdığı
> kabloyu yakalıyorsa kendi sesini duyarsın. Sesi RelAudio taşısın, uzak
> masaüstü görüntü ve klavye/fare ile ilgilensin.

### Uzaktaki mikrofonu yerel mikrofon olarak kullan

Biraz kurulum gerektiren senaryo bu. Windows'ta da Linux'ta da bir uygulamanın
"mikrofon olması" için yerleşik bir yol yok; **sanal ses kablosu** gerekiyor:
bir ucu hoparlör, öbür ucu mikrofon olan bir sürücü. Hoparlör ucuna yazılan,
mikrofon ucundan çıkar.

**Sanal kablo kur:**

| Platform | Ne kurulacak |
|---|---|
| **Windows** | [VB-CABLE](https://vb-audio.com/Cable/) — ücretsiz (bağış usulü). ZIP'i aç, `VBCABLE_Setup_x64.exe`'yi **yönetici olarak** çalıştır, sonra yeniden başlat. |
| **Linux** | Kurulum gerekmez, şununla yarat:<br>`pactl load-module module-null-sink sink_name=relaudio media.class=Audio/Sink sink_properties=device.description=RelAudio-Cable` |

**Sonra:**

1. **Alan makine** — *Oynatıcı*, **Mikrofon olarak kullan**. Aygıt listesi
   kendiliğinden sanal kablolara süzülür. **Hoparlör ucunu** seç (Windows'ta
   `CABLE Input`). Yeşil kutu diğer uygulamalarda hangi mikrofonu seçeceğini
   söyler.
2. **Gönderen makine** — *Sunucu*, **Mikrofon**, hedef olarak alan makineyi
   seç, yayına başla.
3. **Discord / Zoom / OBS'de** — mikrofon olarak **mikrofon ucunu** seç
   (Windows'ta `CABLE Output`).

> **Sistem çıkışını kabloya çevirme.** O kabloya yalnızca RelAudio yazmalı.
> Windows'un varsayılan çıkışını `CABLE Input` yaparsan bilgisayarın çaldığı
> her şey de mikrofona karışır ve kendi sesini duyarsın.

> **Kendi sesini duyuyorsan:** Windows Ses ayarları → Kayıt → `CABLE Output` →
> Özellikler → **Dinle** sekmesi → *"Bu aygıtı dinle"* işaretini kaldır.

---

## Sorun giderme

| Belirti | Sebep ve çözüm |
|---|---|
| **Paket sayacı 0'da duruyor** | Yanlış hedef adres ya da güvenlik duvarı. Oynatıcı sekmesi bu makinenin adresini gösteriyor; gönderenin oraya baktığından emin ol. |
| **Paket geliyor ama ses yok** | Yanlış çıkış aygıtı. İstatistikler panelindeki **Çıkış** satırı akışın gerçekte açtığı aygıtı gösterir — açılır listede seçili olanı değil. Sağ panelden değiştir, akış kendiliğinden yeniden kurulur. Çıkışı tek başına sınamak için `relaudio-cli tone`. |
| **Kendi sesini duyuyorsun** | Bir şey kabloyu dinliyor. Yukarıdaki iki nota bak. |
| **Uygulama açılmıyor, hiçbir şey olmuyor** | Zaten çalışıyordur — sistem tepsisine bak. RelAudio tek örneğe izin veriyor. |
| **GNOME'da tepsi simgesi yok** | GNOME'da varsayılan tepsi yok. *AppIndicator and KStatusNotifierItem Support* eklentisini kur. Tepsi yoksa RelAudio "tepsiye küçült"ü kapatıyor ki uygulama erişilemez hâle gelmesin. |
| **Tampon zamanla büyüyor** | İki makine arasında saat kayması. Telafi henüz yok; akışı yeniden başlat. |
| **Başka bir şey** | Log'a bak: `%LOCALAPPDATA%\RelAudio\relaudio.log` (Windows) veya `~/.local/state/RelAudio/relaudio.log` (Linux). Ayarlar sekmesi tam yolu gösteriyor. |

### Teşhis komutları

Uygulamayla birlikte bir komut satırı aracı da derleniyor:

```bash
relaudio-cli devices                  # aygıtları id'leriyle listele
relaudio-cli tone                     # test tonu çal — çıkışı sınar, ağ gerekmez
relaudio-cli level --mic --device ID  # canlı giriş seviyesi ölçer
relaudio-cli send 192.168.1.10        # arayüzsüz gönder
relaudio-cli recv                     # arayüzsüz al
```

`relaudio` uygulamayı açar; `relaudio-cli` teşhis aracıdır.

`relaudio-cli level` zincirin neresinin koptuğunu bulmanın en hızlı yolu: kablonun
mikrofon ucuna doğrult ve sesin gerçekten ulaşıp ulaşmadığını gör.

---

## Bilinen sınırlar

- **Saat kayması telafisi yok.** İki makinenin ses kartları birebir aynı hızda
  çalışmaz. Uzun oturumlarda tampon kayar, "atılan" sayacı artar. Bir tavan
  gecikmeyi sınırlıyor; adaptif yeniden örnekleme planda.
- **Yalnızca PCM.** ~1.5 Mbit/s, ama sessiz bloklar yük taşımadan gidiyor,
  yani sessiz pasajlar neredeyse bedava. Opus planda.
- **Şifreleme yok.** Güvendiğin ağlarda kullan.
- **Kayıp gizleme yok.** Kaybolan paketler kısa sessizliğe dönüşür.
- **macOS desteklenmiyor** — bilinçli olarak ertelendi,
  [docs/00-genel-bakis.md](docs/00-genel-bakis.md).
- **AppImage üretimi başarısız** (`linuxdeploy` hatası). `.deb` ve `.rpm` çalışıyor.

---

## Nasıl çalışıyor

```
yakala ──► paketle ──► UDP ──► jitter buffer ──► çal
(WASAPI / PulseAudio)  5 ms                    (WASAPI / PulseAudio)
```

- **Arayüz:** Tauri v2 + Svelte 5 — ~11 MB binary, işletim sisteminin webview'ı
- **Çekirdek:** Rust. Ses ve ağ kendi thread'lerinde
- **Linux arka ucu:** PulseAudio API (PipeWire bunu yerel sağlıyor)
- **Windows arka ucu:** WASAPI, loopback için render aygıtı capture yönünde açılıyor
- **Keşif:** mDNS (`_relaudio._udp`)

Tasarım notları, ölçümler ve karar kayıtları [docs/](docs/) altında.
Mimari: [docs/02-mimari.md](docs/02-mimari.md).
Karar kayıtları: [docs/adr/](docs/adr/).

### Derleme ve test

```bash
cargo test                                    # çekirdek testleri
cd app/src-tauri && cargo test                # oturum katmanı testleri
rustup target add x86_64-pc-windows-msvc      # bir kez
cargo check --target x86_64-pc-windows-msvc   # Windows kodunu Linux'ta doğrula
node scripts/check-i18n.mjs                   # çeviri bütünlüğü
```

Üçüncüsü önemli: Windows'a özgü kod normal Linux derlemesinde `cfg` ile
dışlanıyor, yani istemedikçe hiç kontrol edilmiyor.

---

## Lisans

MIT — [LICENSE](LICENSE).

## Destek

RelAudio işine yaradıysa: [buymeacoffee.com/bbesli](https://buymeacoffee.com/bbesli)

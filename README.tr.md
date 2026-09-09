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

## Bu proje neden var

RelAudio, **[AudioRelay](https://audiorelay.net)** kullandıktan sonra yazıldı —
bu problemi yıllardır iyi çözen, cilalı, telefonları da destekleyen bir uygulama.
Hakkını teslim edelim: bu iş akışının mümkün olduğunu bana o gösterdi. Olgun ve
destekli bir ürün, Android/iOS istemcileriyle birlikte istiyorsan onu kullan ve
parasını öde.

Bu proje aynı fikrin açık kaynak bir okuması ve bilinçli olarak daha dar:
Windows ↔ Linux masaüstleri, ve özellikle tek bir iş akışı — kulaklığın önündeki
makinede takılıyken uzaktaki makineyi kullanmak. Var olma sebebi kaynağı
istemem, o senaryonun uçtan uca çözülmesini istemem, ve zor kısımların (hangi
kablo ucu, hangi varsayılan aygıt, neden kendini duyuyorsun) deneme yanılmaya
bırakılmak yerine anlatılmasını istemem.

Farklı kapsam, rakip değil. AudioRelay senin işini görüyorsa onu kullan.

## Kurulum

### Linux

Rust, Node.js, git ve PulseAudio/PipeWire + WebKitGTK geliştirme başlıkları
gerekiyor. Dağıtımına uyan bloğu çalıştır:

```bash
# Arch / CachyOS
sudo pacman -S --needed base-devel git rustup nodejs npm libpulse webkit2gtk-4.1 libayatana-appindicator
rustup default stable

# Debian / Ubuntu
sudo apt install build-essential curl git libpulse-dev libwebkit2gtk-4.1-dev \
                 libayatana-appindicator3-dev librsvg2-dev nodejs npm
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust apt'ta yok
. "$HOME/.cargo/env"

# Fedora
sudo dnf install @development-tools git pulseaudio-libs-devel webkit2gtk4.1-devel \
                 libappindicator-gtk3-devel librsvg2-devel nodejs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust varsayılan depolarda yok
. "$HOME/.cargo/env"
```

Sonra derle ve kur. Üç derleme komutu da gerekli — ikincisi aşağıdaki bütün
Sorun giderme komutlarının kullandığı teşhis aracını üretiyor:

```bash
git clone https://github.com/bbesli/RelAudio.git
cd RelAudio
cargo build --release --bin relaudio-cli          # teşhis: devices, tone, level
cd app && npm install && npx tauri build --no-bundle
cd .. && ./scripts/install-linux.sh
```

Uygulama `~/.local/bin/relaudio`, teşhis aracı `~/.local/bin/relaudio-cli`
olarak kurulur ve uygulama menüsüne **RelAudio** girdisi eklenir. Yönetici
yetkisi gerekmez. Kaldırmak için `./scripts/uninstall-linux.sh`.

> `relaudio` komutu bulunamıyorsa `~/.local/bin` `PATH`'inde değildir.
> Ya uygulama menüsünden aç, ya da kabuğunun açılış dosyasına ekle — çoğu
> dağıtımda `~/.bashrc`, zsh kullanıyorsan `~/.zshrc`:
> ```bash
> echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc && exec $SHELL
> ```

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
4. **[Git](https://git-scm.com/download/win)** — ya da `winget install --id Git.Git -e`

Sonra:

```
git clone https://github.com/bbesli/RelAudio.git
cd RelAudio
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

Betik gereksinimleri kontrol eder, Visual Studio ortamını yükler (Insiders
sürümleri dahil — Rust bunları tek başına bulamıyor), npm bağımlılıklarını
kurar ve derler. İki program üretir:

| Dosya | Nedir |
|---|---|
| `app\src-tauri\target\release\relaudio-app.exe` | çift tıklayacağın uygulama |
| `target\release\relaudio-cli.exe` | [Sorun giderme](#sorun-giderme) bölümündeki teşhis aracı |

İkisi de `PATH`'te değil. Bir Sorun giderme komutunu çalıştırmak için klonladığın
`RelAudio` klasöründe PowerShell aç ve başına `.\` koy:

```
cd C:\yol\RelAudio
.\target\release\relaudio-cli.exe devices
```

Aşağıda okunurluk için `relaudio-cli` yolsuz yazıldı — Windows'ta yerine
`.\target\release\relaudio-cli.exe` yaz.

**Güvenlik duvarı:** ilk kez ses alırken Windows izin soracak, özel ağlar için
onayla. Elle kural eklemek istersen, yönetici PowerShell'de:

```
New-NetFirewallRule -DisplayName "RelAudio UDP 59101" -Direction Inbound -Protocol UDP -LocalPort 59101 -Action Allow
```

---

## Başlamadan önce

İki şeyi bilmen lazım — insanların takıldığı sorunların neredeyse tamamı
bu ikisinden çıkıyor.

**1. Bir uygulamanın "mikrofon olması" için sanal ses kablosu gerekiyor.**
Ne Windows ne Linux, normal bir programın başka bir programın mikrofon
listesinde görünmesine izin veriyor. Sanal kablo iki uçlu bir sürücü: bir ucu
**hoparlör** gibi görünür, öbür ucu **mikrofon** gibi. Hoparlör ucuna yazılan,
mikrofon ucundan çıkar.

```
RelAudio buraya yazar              toplantı uygulaman buradan okur
        │                                        │
        ▼                                        ▼
  CABLE Input  ══════ kablo ══════════►  CABLE Output
  (bir hoparlör)                          (bir mikrofon)
```

İsimler kafa karıştırıcı çünkü **kablonun** bakış açısıyla konulmuş, Windows'un
değil. Bu yüzden Windows Ses panelinde `CABLE Input` **Kayıttan yürütme**
sekmesinde, `CABLE Output` ise **Kayıt** sekmesinde. `CABLE Output`'u
hoparlör listesinde ararsan bulamazsın.

**2. Bir makine, yazdığı aygıtı asla yakalamamalı.**
Yakalarsa ses bir çember çizip kendine döner ve kendi sesini duyarsın.
RelAudio'nun Kulaklık modu aygıtları senin için seçiyor ve bu kuralı asla
bozmuyor; elle ayarlarsan aklında tutman gereken kural bu.

---

## Kullanım

**İki bilgisayarda da** RelAudio'yu aç. Her biri kendiliğinden dinlemeye başlar
ve kendini ağa ilan eder; birbirlerini bulurlar, IP yazmazsın.

Aşağıda "A makinesi" ve "B makinesi" sadece iki bilgisayarın demek.

---

### Senaryo 1 — Bir bilgisayarın sesini diğerinde çal

*Örnek: masaüstünde müzik çalıyor, mutfaktaki dizüstünden duymak istiyorsun.*

**Sesi çıkaracak makinede (B):**

1. RelAudio'yu aç.
2. **Oynatıcı** sekmesine geç.
3. **Hoparlörden dinle** seçili olsun.
4. Sağda **Çıkış aygıtı** altında hoparlörünü veya kulaklığını seç.
5. **Dinlemeye başla**'ya bas. (Genelde kendiliğinden başlamıştır.)

**Sesini göndermek istediğin makinede (A):**

1. RelAudio'yu aç.
2. **Sunucu** sekmesine geç.
3. **Sistem sesi**'ni seç.
4. **Hedef cihaz** listesinden B makinesini seç.
   *Liste boşsa birkaç saniye bekle. Hâlâ boşsa Sorun giderme'ye bak.*
5. **Yayına başla**'ya bas.

A'da bir şey çal. Saniyesinde B'den duyman lazım.

> Linux'ta "sistem sesi" bir çıkış aygıtının *monitor*'ü demek. RelAudio
> varsayılanı seçiyor, çoğu durumda doğrusu odur. Ses gelmiyorsa medya
> oynatıcının hangi çıkışı kullandığına bak — bazı uygulamalar belirli bir
> aygıta sabitlenmiş olabiliyor.

---

### Senaryo 2 — Mikrofonu başka bilgisayarın hoparlörüne gönder

Senaryo 1'in aynısı; tek fark, A makinesindeki 3. adımda **Sistem sesi**
yerine **Mikrofon** seçmen ve sağdan hangi mikrofonu göndereceğini belirlemen.

Bu, mikrofonu B'de sadece **duyulur** yapar. B'deki uygulamaların (Discord,
Zoom…) onu gerçek bir mikrofon sanmasını istiyorsan Senaryo 4'e bak.

---

### Senaryo 3 — Kulaklık modu (asıl olay)

*Örnek: Linux makinenin başında oturuyorsun, Parsec/RDP/Sunshine ile Windows
makinesini kullanıyorsun. Kulaklığın Linux'a takılı. Onun Windows'un kulaklığı
gibi çalışmasını istiyorsun — sen konuşursun toplantı duyar, toplantı konuşur
sen duyarsın.*

Bunun için **iki yön birden** gerekiyor ve RelAudio ikisini de senin için kuruyor.

#### Adım 1 — Uzak makineye sanal ses kablosu kur

Yalnızca kulaklığın **takılı olmadığı** makinede gerekiyor.

**Windows:**

1. <https://vb-audio.com/Cable/> adresine git.
2. Soldaki Windows başlığı altından **VBCABLE_Driver_Pack** ZIP'ini indir.
3. ZIP'i **bir klasöre çıkar.** ZIP'in içinden çalıştırma.
4. `VBCABLE_Setup_x64.exe` dosyasına sağ tıkla → **Yönetici olarak çalıştır**.
5. **Install Driver**'a bas, Windows'un sorduğu izni onayla.
6. **Yeniden başlat.** Bunu yapmadan sürücü tam kullanılabilir olmuyor.

Yeniden başlattıktan sonra hoparlör listende `CABLE Input`, mikrofon listende
`CABLE Output` görünmeli.

**Linux:** kurulum gerekmez. Şunu bir kez çalıştır (yeniden başlatana kadar kalır):

```bash
pactl load-module module-null-sink sink_name=relaudio \
  media.class=Audio/Sink sink_properties=device.description=RelAudio-Cable
```

> `invalid argument` hatası alırsan PipeWire değil klasik PulseAudio
> kullanıyorsun demektir; `media.class` kısmını at:
> ```bash
> pactl load-module module-null-sink sink_name=relaudio \
>   sink_properties=device.description=RelAudio-Cable
> ```

Kablonun iki ucunun adı Windows'takinden farklı. Hoparlör listene
**RelAudio-Cable**, mikrofon listene **Monitor of RelAudio-Cable** gelir —
toplantı uygulamasında seçeceğin ikincisi. Yeniden başlatmalarda kalıcı olması
için aynı satırı (`pactl` olmadan)
`~/.config/pipewire/pipewire-pulse.conf.d/relaudio.conf` dosyasına koy, ya da
her açılışta tekrar çalıştır.

#### Adım 2 — Uzak masaüstünün sesini kapat

**Bunu mutlaka yap.** Parsec, RDP, AnyDesk ve benzerleri uzak makinenin
varsayılan hoparlörünü yakalayıp sana gönderiyor. RelAudio da ses taşıyorsa
her şey sana iki kez gelir — ve uzak masaüstü tam da RelAudio'nun yazdığı
kabloyu yakalıyorsa kendi sesini duyarsın, RelAudio'da ne değiştirirsen
değiştir geçmez.

- **Parsec:** Settings → Host (veya Client) → **Audio** → kapat.
- **Windows RDP:** bağlantı ayarlarında Yerel Kaynaklar → Uzak ses →
  **Çalma**.

RelAudio o ses kanalının yerini alıyor, üstelik daha düşük gecikmeyle.

#### Adım 3 — Uzak makinenin varsayılan hoparlörü *gerçek* hoparlör olsun

Uzak makinede ses ayarlarını aç ve varsayılan çıkışın normal hoparlörün
olduğundan emin ol (örn. `Speakers (Realtek(R) Audio)`) — **kablo olmasın**.

Neden: RelAudio sistem sesini gerçek bir çıkış aygıtından yakalayıp sana
gönderiyor. Varsayılan kablo olursa makinenin sesi kabloya gider, aktarılan
mikrofonla karışır ve kendini duyarsın.

- **Windows:** <kbd>Win</kbd>+<kbd>R</kbd> → `mmsys.cpl` → Enter.
  **Kayıttan Yürütme** (*Playback*) sekmesi → hoparlörüne tıkla →
  **Varsayılan Yap** (*Set Default*).
- **Linux:** Sistem Ayarları → Ses → çıkış aygıtını gerçek hoparlörün yap,
  `RelAudio-Cable` olmasın. Terminalden:
  `pactl set-default-sink <gerçek sink>` (listesi: `pactl list short sinks`).

#### Adım 4 — İki makinede de Kulaklık modunu başlat

**Kulaklığın takılı olduğu makinede:**

1. **Kulaklık** sekmesi.
2. **Kulaklık bu makinede**'yi seç.
3. **Karşı cihaz** → uzak makineyi seç.
4. **Kulaklık modunu başlat**'a bas.

RelAudio bu makinenin **varsayılan** mikrofonunu ve **varsayılan** hoparlörünü
seçiyor. Bu yüzden başlatmadan önce kulaklığı bu makinede varsayılan yap —
yoksa dizüstünün dahili mikrofonunu gönderir, dahili hoparlöründen dinlersin.
Sistem varsayılanını değiştirmek istemiyorsan sağdaki panelde **Aygıtlar**'ı
aç ve kulaklığın mikrofonunu/hoparlörünü elle seç; seçimin kaydedilir ve
otomatik seçimin önüne geçer.

**Uzak makinede:**

1. **Kulaklık** sekmesi.
2. **Uzak makine**'yi seç.
3. **Karşı cihaz** → kulaklığın olduğu makineyi seç.
4. **Kulaklık modunu başlat**'a bas.

Başlatmadan önce RelAudio hangi iki aygıtı seçtiğini gösteriyor. Uzak makinede
şöyle görünmeli:

```
Kaynak   Speakers (Realtek(R) Audio)          ← sistem sesi, sana gidiyor
Çıkış    CABLE Input (VB-Audio Virtual Cable) ← mikrofonun, kabloya yazılıyor
```

İki farklı aygıt. Sayfanın başındaki kural bu, uygulanmış hâli.

#### Adım 5 — Toplantı uygulamasına ne kullanacağını söyle

**Uzak** makinede, Discord / Zoom / Teams / Meet içinde:

- **Mikrofon:** kablonun yakalama ucu — uzak makine Windows'sa
  `CABLE Output (VB-Audio Virtual Cable)`, Linux'sa `Monitor of RelAudio-Cable`.
  Başlata bastıktan sonra RelAudio tam adı ekranda yazıyor; onu kullan.
- **Hoparlör / çıkış:** normal hoparlörün kalsın. Kabloyu seçme.
  Onun sesi zaten sana aktarılıyor.

Bu kadar. Kulaklığına konuş — toplantı seni duyar. Toplantı konuşur — sen
kulaklığından duyarsın.

---

### Senaryo 4 — Uzaktaki mikrofonu yerel mikrofon olarak kullan (tek yön)

Senaryo 3'ün aynısı, ama yalnızca mikrofon yönü seni ilgilendiriyor (sesi
başka bir yoldan zaten alıyorsun).

1. Alan makineye sanal kablo kur (yukarıdaki Adım 1).
2. **Alan makine:** *Oynatıcı* → **Mikrofon olarak kullan** → aygıt listesi
   kendiliğinden kablolara süzülür → `CABLE Input`'u seç. **Dinlemeye başla**.
   Yeşil bir kutu çıkıp diğer uygulamada hangi mikrofonu seçeceğini söyler.
3. **Gönderen makine:** *Sunucu* → **Mikrofon** → hedefi seç → **Yayına başla**.
4. **Uygulamanda:** mikrofon olarak `CABLE Output`'u seç.

> `CABLE Output` RelAudio'nun listesinde **görünmez**. RelAudio hoparlörleri
> gösteriyor; `CABLE Output` bir mikrofon. Diğer uygulamanın mikrofon
> listesinde çıkar. Bu neredeyse herkesi bir kez yanıltıyor.

## Sorun giderme

Sırayla ilerle. Her adım zincirin neresinin koptuğunu söylüyor.

### "Kendi sesimi duyuyorum"

En sık gelen şikâyet ve neredeyse hiçbir zaman RelAudio sesi sana geri çalmıyor.
Şu sırayla kontrol et:

1. **Uzak masaüstünün sesi hâlâ açık mı?** Parsec/RDP/AnyDesk uzak makinenin
   hoparlör çıkışını sana gönderiyor. RelAudio da aktarıyorsa aynı ses sana iki
   kez ulaşır — ve uzak masaüstü RelAudio'nun yazdığı kabloyu yakalıyorsa
   RelAudio'da ne değiştirirsen değiştir kendini duyarsın.
   → Sesini kapat (Senaryo 3, Adım 2).

2. **Uzak makinenin varsayılan hoparlörü kablo mu?** O zaman o makinenin
   çaldığı her şey kabloya gider, aktarılan mikrofonla karışır ve geri gelir.
   → `mmsys.cpl` → **Kayıttan Yürütme** (*Playback*) → gerçek hoparlörünü seç
   → **Varsayılan Yap** (*Set Default*).

3. **Kablonun mikrofon ucunda "Bu aygıtı dinle" açık mı?** O ayar kabloyu
   doğrudan hoparlörüne bağlıyor.
   → `mmsys.cpl` → **Kayıt** (*Recording*) sekmesi → `CABLE Output` →
   **Özellikler** (*Properties*) → **Dinle** (*Listen*) sekmesi →
   *"Bu aygıtı dinle"* işaretini kaldır.

4. **Aynı toplantı iki makinede birden açık mı?** Discord ikisinde de aynı
   kanaldaysa biri aktardığın mikrofonu yayınlar, diğeri sana geri çalar.
   Aktarım yapmayan makinede görüşmeden çık.

5. **RelAudio kendi içinde döngü kurmuş olabilir mi?** Sunucu, Oynatıcı'nın
   yazdığı aygıtı yakalıyorsa RelAudio kırmızı **Geri besleme döngüsü** uyarısı
   gösteriyor. Kulaklık modu bunu engelliyor; elle ayarda karşılaşılabilir.

### "Paket sayacı 0'da duruyor"

Hiçbir şey ulaşmıyor. Ya gönderen yanlış adrese bakıyor ya da güvenlik duvarı
araya giriyor.

- **Oynatıcı** sekmesi bu makinenin adresini gösteriyor. Gönderenin tam olarak
  ona baktığından emin ol.
- Gönderen tarafta kırmızı **Gönderilemeyen** sayacı varsa paketler
  reddediliyor demektir — adres yanlış. Birden çok ağ adaptörü olan bir makine
  (VPN, WSL, Hyper-V) birden fazla adres ilan ediyor; hedefi tekrar seç ya da
  adresi elle yaz.
- Windows güvenlik duvarı: sorduğunda izin ver, ya da kuralı elle ekle:
  ```
  New-NetFirewallRule -DisplayName "RelAudio UDP 59101" -Direction Inbound -Protocol UDP -LocalPort 59101 -Action Allow
  ```

### "Paket geliyor ama ses duymuyorum"

Ses dinlemediğin bir yere gidiyor.

- İstatistikler panelindeki **Çıkış** satırına bak. Orası akışın *gerçekte
  açtığı* aygıt — açılır listede seçili olan değil. Yanlış yazıyorsa sağdan
  aygıtı değiştir; akış kendiliğinden yeniden kurulur.
- Çıkışı ağdan bağımsız sına:
  ```
  relaudio-cli tone
  ```
  440 Hz ton duymuyorsan sorun çıkış aygıtında, ağda değil.
- Kulaklık/mikrofon modunda ses bir **kabloya** gidiyor, yani duymaman normal.
  Karşı uca bak:
  ```
  # Windows — CABLE Output gerçek bir mikrofon, --mic doğru
  relaudio-cli level --mic --device "<CABLE Output id>"

  # Linux — kablonun yakalama ucu bir monitor, --mic koyma
  relaudio-cli level --device "<Monitor of RelAudio-Cable id>"
  ```
  Id'yi `relaudio-cli devices` ile al. Karşı taraf konuşurken çubuk oynamalı.
  (Linux'ta `--mic` o adda gerçek bir mikrofon arar ve `aygıt bulunamadı` der.)

### "Toplantı uygulamam kabloyu mikrofon olarak göstermiyor"

- VB-CABLE kurduktan sonra **yeniden başlattın mı?** Öncesinde tam kayıtlı olmuyor.
- Muhtemelen hoparlör listesine bakıyorsun. Kablonun yakalama ucu —
  Windows'ta `CABLE Output`, Linux'ta `Monitor of RelAudio-Cable` —
  mikrofon/giriş ayarlarında çıkar, hoparlörlerde asla.
- Bazı uygulamalar aygıt listesini önbelleğe alıyor — toplantı uygulamasını
  yeniden başlat.

### Diğer

| Belirti | Sebep |
|---|---|
| **Uygulama açılmıyor, hiçbir şey olmuyor** | Zaten çalışıyordur — sistem tepsisine bak. RelAudio tek örneğe izin veriyor. |
| **GNOME'da tepsi simgesi yok** | GNOME'da varsayılan tepsi yok. *AppIndicator and KStatusNotifierItem Support* eklentisini kur. Tepsi yoksa RelAudio "tepsiye küçült"ü kapatıyor ki uygulama erişilemez hâle gelmesin. |
| **Tampon zamanla büyüyor / "Atılan" sayacı artıyor** | İki makine arasında saat kayması. Telafi henüz yok; akışı yeniden başlat. |
| **Ses kesik kesik** | Oynatıcı sekmesinde **Tampon**'u artır. Her birim 5 ms; varsayılan 8 = 40 ms. Wi-Fi genelde 15–20 ister. |
| **Başka bir şey** | Log'a bak. Ayarlar sekmesi tam yolu gösteriyor — Windows'ta `%LOCALAPPDATA%\RelAudio\relaudio.log`, Linux'ta `~/.local/state/RelAudio/relaudio.log`. |

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

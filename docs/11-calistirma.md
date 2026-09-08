# 11 — Derleme ve Çalıştırma

## Kurulum

### Linux — sisteme kur (önerilen)

```bash
cd app && npx tauri build --no-bundle
cd .. && ./scripts/install-linux.sh
```

Uygulamayı `~/.local/bin/relaudio` olarak kurar ve uygulama menüsüne
**RelAudio** girdisi ekler. Yönetici yetkisi gerektirmez.
Geri almak için `./scripts/uninstall-linux.sh`.

### Linux — dağıtım paketleri

`npx tauri build` (bundle'lı hâli) `.deb` ve `.rpm` üretir:

```
app/src-tauri/target/release/bundle/deb/RelAudio_0.1.0_amd64.deb
app/src-tauri/target/release/bundle/rpm/RelAudio-0.1.0-1.x86_64.rpm
```

**AppImage üretimi başarısız oluyor** (`failed to run linuxdeploy`).
Arch/CachyOS'ta deb/rpm zaten kullanılmadığı ve kapsam kişisel kullanım olduğu
için kovalanmadı; `install-linux.sh` aynı sonucu veriyor. Dağıtım gündeme
gelirse buraya bakılmalı.

### Windows

Kaynağı Windows'a kopyala, proje kökünde:

```
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

Betik gereksinimleri kontrol eder, Visual Studio geliştirme ortamını kendisi
yükler (VS Insiders dahil — Rust bunu tek başına bulamıyor), npm bağımlılıklarını
kurar ve derler. Çıktı:
`app\src-tauri\target\release\relaudio-app.exe`

## Uygulama (arayüz)

```bash
cd app
npm install          # ilk seferde
npx tauri build --no-bundle
./src-tauri/target/release/relaudio-app
```

Geliştirirken canlı yeniden yükleme için: `npx tauri dev`

Arayüz üç sekmeden oluşur:

| Sekme | İş |
|---|---|
| **Sunucu** | Bu makinenin sesini gönder. Sistem sesi veya mikrofon seçilir, hedef IP girilir. |
| **Oynatıcı** | Gelen sesi bu makinede çal. Port, çıkış aygıtı ve tampon ayarlanır. |
| **Ayarlar** | Kapatınca tepsiye küçültme, bilinen sınırlar. |

Sağ sütun bağlama duyarlı: seçili sekmeye göre kaynak veya çıkış aygıtını
gösterir, altında canlı istatistikler akar (500 ms'de bir).

**Tepsi davranışı:** Pencereyi kapattığında uygulama kapanmaz, tepsiye iner ve
yayın devam eder. Tepsi menüsünde *Göster*, *Tüm yayınları durdur* ve *Çıkış*
var. Gerçekten çıkmak için **Çıkış**'ı kullan.

> Linux'ta tepsi simgesi StatusNotifierItem üzerinden çalışır. GNOME'da
> varsayılan olarak tepsi yoktur; uygulama bunu algılayıp "kapatınca küçült"
> davranışını otomatik kapatır (docs/06). KDE, XFCE, Cinnamon'da sorunsuz.

## Komut satırı (`relaudio`)

Arayüzsüz test ve hata ayıklama için. Aynı çekirdeği kullanır.

## Gereksinimler

| Platform | Gerekenler |
|---|---|
| Linux | `rustup` + stable, `libpulse` (dev başlıkları), PipeWire veya PulseAudio |
| Windows | `rustup` + stable-msvc, Visual C++ Build Tools (SDK dahil) |

Arch/CachyOS'ta: `sudo pacman -S --needed rustup libpulse && rustup default stable`

## Derleme

```bash
cargo build --release
```

Çıktı: `target/release/relaudio` (Windows'ta `relaudio.exe`).

## Kullanım

### Aygıtları listele

```bash
relaudio devices
```

Üç grup gösterir: **ÇIKIŞ**, **MİKROFON**, **SİSTEM SESİ (loopback)**.
`*` varsayılanı işaretler. Aygıt seçerken görünen adı değil `id` alanını kullan.

### Ses gönder

```bash
# Sistem sesi (varsayılan)
relaudio send 192.168.1.113

# Mikrofon
relaudio send 192.168.1.113 --mic

# Belirli bir aygıttan
relaudio send 192.168.1.113 --device <id>
```

### Ses al ve çal

```bash
relaudio recv
relaudio recv --port 59101 --device <id> --buffer 8
```

`--buffer` jitter buffer hedefidir, paket cinsinden. 1 paket = 5 ms.
Varsayılan 8 = 40 ms. Ağ kötüyse artır.

## Sağlıklı çıktı nasıl görünür

```
alınan 2797 paket  kayıp 0  geç 0  underrun 0  atılan 0  tampon 6 paket (30 ms)
```

| Alan | Anlamı | Sorun işareti |
|---|---|---|
| kayıp | Hiç gelmeyen paket | > %1 → ağ sorunu |
| geç | Oynatma noktasını geçmiş | Sürekli artıyorsa jitter yüksek, `--buffer` artır |
| underrun | Tamponun tamamen boşalması | > 0 ve artıyorsa `--buffer` artır |
| **atılan** | Tampon tavanı aşıldı | **Artıyorsa saat kayması birikiyor** — bkz. aşağı |
| tampon | Anlık doluluk | Sabit kalmalı; sürekli büyüyorsa drift var |

## Geliştirme

```bash
cargo test                                          # çekirdek testleri
cd app/src-tauri && cargo test                      # oturum katmanı testleri
cargo check --target x86_64-pc-windows-msvc         # Windows kodunu Linux'ta doğrula
```

Sonuncusu önemli: Windows'a özgü kod (`audio/windows.rs`) Linux'ta `cfg` ile
dışlandığı için normal derlemede hiç kontrol edilmiyor. Bu komut linker
gerektirmeden tip kontrolü yapar ve `windows.rs`'teki hataları yakalar.

## Bilinen sınırlar (v1)

- **Saat kayması telafisi yok.** İki makinenin ses kartları birebir aynı hızda
  çalışmaz. Uzun oturumlarda tampon yavaşça büyür veya küçülür; tavan
  aşıldığında paket atılır (`atılan` sayacı). Kalıcı çözüm adaptif yeniden
  örnekleme (docs/03) — sonraki fazda.
- **Kayıp gizleme (PLC) yok.** Kayıp paket sessizlikle doldurulur.
- **Kodek yalnızca PCM.** 1.5 Mbit/s; sessiz bloklar yük taşımadan gider,
  bu yüzden sessizlikte bant genişliği ~50 kbit/s'e düşer. Opus sonraki fazda.
- **Keşif yok.** IP elle verilir. mDNS sonraki fazda.
- **Ctrl+C ile düzgün kapanma yok.** Süreç sonlandırılıyor; ses aygıtları
  işletim sistemi tarafından serbest bırakılıyor. IPC entegrasyonunda düzelecek.

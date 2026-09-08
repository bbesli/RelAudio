# 08 — Derleme, İmzalama ve Dağıtım

> **Büyük ölçüde ertelendi.** Hedef kişisel kullanım olduğu için paketleme,
> imzalama, updater ve dağıtım v1 kapsamında değil. Geliştirme derlemesi
> (`cargo tauri dev` / `cargo build --release`) yeterli.
>
> Bu doküman, yayımlamaya karar verilirse hazır olsun diye korunuyor.
> Şu an geçerli olan tek bölüm: **Derleme zinciri**.

## Derleme zinciri

| Bileşen | Araç | Çıktı |
|---|---|---|
| `relaudio-core` | `cargo build --release` | Platforma özel binary |
| Frontend | Vite | Statik varlıklar |
| Uygulama | `cargo tauri build` | Kurulum paketi |

Çekirdek, Tauri paketine **sidecar** olarak gömülür (`tauri.conf.json` →
`bundle.externalBin`). Tauri sidecar'ları hedef üçlüsü (target triple) ekiyle
adlandırır: `relaudio-core-x86_64-unknown-linux-gnu` gibi.

Electron seçilirse karşılığı: `electron-builder` + `extraResources` ve
`process.resourcesPath` üzerinden `spawn`.

## Paket formatları

| Platform | Format | Not |
|---|---|---|
| Windows | NSIS (`.exe`), MSI opsiyonel | Kullanıcı bazlı kurulum tercih; yönetici gerektirmez |
| Linux | AppImage, `.deb`, `.rpm` | AppImage birincil; dağıtım paketleri ikincil |
| Linux (opsiyonel) | Flatpak | `--socket=pipewire`, `--share=network` izinleri |

**Linux bağımlılıkları** (deb/rpm için mutlaka bildirilmeli):
`libwebkit2gtk-4.1`, `libayatana-appindicator3-1` (tepsi için — bu unutulursa
tepsi simgesi hiç görünmez), `libasound2` / PipeWire istemci kütüphaneleri.

## İmzalama

| Platform | Zorunlu mu? | Maliyet | İmzasızsa ne olur |
|---|---|---|---|
| Windows | **Hayır** | Authenticode ~$200–600/yıl | SmartScreen "Bilinmeyen yayımcı" uyarısı; kullanıcı "Yine de çalıştır" der ve program çalışır |
| Linux | Hayır | — (opsiyonel GPG) | Bir şey olmaz; depo yayınında anlamlı |

macOS kapsam dışı olduğu için **v1'de hiçbir platformda imzalama zorunlu değil.**
Bu, çıkışın önündeki tüm sertifika engelini kaldırıyor.

**Windows'ta sertifika v1 için gerekli değildir.** İmzasız dağıtım geçerli bir
yoldur; tek bedeli ilk çalıştırmadaki SmartScreen penceresidir. Kullanıcı sayısı
arttıkça sertifika alınabilir — bu, sonradan verilebilecek bir karardır.

**macOS geri kapsama girerse zorunlu olur.** İmzasız/notarize edilmemiş bir
uygulama Gatekeeper tarafından engellenir; kullanıcının bunu aşması makul bir
dağıtım yolu değildir. Developer ID $99/yıl.

**Notarization akışı** (macOS geri gelirse): imzala → `notarytool submit --wait` → `stapler staple`.
Hardened Runtime açık olmalı; ses için `com.apple.security.device.audio-input`
entitlement'ı ve `Info.plist` içinde `NSMicrophoneUsageDescription` ile
(ScreenCaptureKit yolu kullanılıyorsa) `NSAudioCaptureUsageDescription` gerekir.

**Sürücü imzalama bambaşka bir konudur.** Yukarıdaki tablo yalnızca uygulama
binary'sini kapsar. Windows'ta bir çekirdek modu sürücü (sanal ses aygıtı)
dağıtmak isterseniz imzalama isteğe bağlı değildir: imzasız sürücü hiç yüklenmez.
Bkz. [04 — Windows](04-sanal-ses-aygitlari.md#windows--en-zor). v1'de kendi
sürücümüz olmadığı için bu bizi ilgilendirmiyor.

## CI

GitHub Actions matris:

```
windows-latest   → x86_64-pc-windows-msvc
ubuntu-22.04     → x86_64-unknown-linux-gnu
```

Ubuntu 22.04 seçimi bilinçli: daha yeni glibc ile derlenen binary eski
dağıtımlarda çalışmaz. AppImage için mümkün olan en eski desteklenen taban.

Adımlar: lint (`clippy`, `eslint`) → birim testler → derleme → paketleme →
imzalama (yalnızca etiketli sürümlerde) → sürüm taslağı.

İmzalama sırları GitHub Secrets'ta; PR'lardan erişilemez.

## Güncelleme

- **Tauri updater** (`tauri-plugin-updater`) — imzalı manifest ile.
- Güncelleme **yayın sürerken uygulanmaz**; bir sonraki boşta kalma anına ertelenir.
- Çekirdek ile UI aynı sürümde olmalı; güncelleme ikisini birlikte değiştirir.
- Linux'ta AppImage kendi kendini güncelleyebilir; dağıtım paketlerinde
  güncelleme paket yöneticisine bırakılır (updater kapatılır).

## Sürümleme

SemVer. Ayrıca ayrı bir **protokol sürümü** vardır ve uygulama sürümünden
bağımsız artar. `v1.4.0` uygulaması ile `v1.2.0` uygulaması aynı protokol
sürümündeyse konuşabilmelidir — kullanıcıların iki makineyi aynı anda
güncellemesi beklenemez.

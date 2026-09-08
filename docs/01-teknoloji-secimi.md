# 01 — Teknoloji Seçimi

## Soru

"Bunu ElectronJS ile yazabilir miyiz? Ya da neyle yazmak lazım?"

## Kısa cevap

Electron ile **arayüzü** yazabilirsin. **Ses motorunu yazamazsın.** Bu, Electron'a
özgü bir kısıt değil; herhangi bir web-tabanlı çerçeve için geçerli. Dolayısıyla
soru "Electron mı, başka bir şey mi" değil, **"arayüz katmanı için hangisi"**
sorusudur — çünkü alt katman her koşulda native olacak.

## Neden ses motoru JavaScript'te olamaz

1. **Node.js'in ses donanımına erişimi yok.** Standart kütüphanede WASAPI /
   CoreAudio / PipeWire karşılığı bir API bulunmuyor. Erişim ancak native
   eklenti (N-API) veya harici süreç ile mümkün.
2. **Web Audio API sistem sesini yakalayamaz.** Renderer'da `getDisplayMedia({audio:true})`
   Windows'ta yalnızca sekme/pencere sesini verir, Linux'ta portal davranışına bağlıdır,
   macOS'ta hiç çalışmaz. Aygıt seçimi ve loopback kaynağı üzerinde kontrol yok.
3. **Gerçek zamanlı garanti yok.** Ses geri çağrımı (callback) 48 kHz / 128 örnek
   tamponda ~2.7 ms'de bir kesintisiz dönmek zorundadır. V8'in GC duraklamaları ve
   Electron'un arka plan kısıtlamaları bunu ihlal eder; sonuç duyulabilir çıtırtıdır.
4. **Sanal ses aygıtı zaten native.** Windows'ta sürücü, macOS'ta AudioServerPlugin,
   Linux'ta PipeWire node'u — hiçbiri JS'ten yazılamaz.

Sonuç: **native bir ses/ağ çekirdeği zorunlu.** Dil olarak Rust veya C++.
Rust önerilir: `cpal`/`coreaudio-rs`/`windows-rs`/`pipewire-rs` olgun, bellek
güvenliği gerçek zamanlı kodda değerli, cross-compile ve paketleme kolay.

## Arayüz katmanı için alternatifler

| Seçenek | Artılar | Eksiler | Paket boyutu |
|---|---|---|---|
| **Tauri v2 + Rust** | Motor ile aynı dil; tray/autostart/single-instance resmi eklentiler; küçük paket; bellek düşük | Linux'ta WebKitGTK render farklılıkları; ekosistem Electron'dan küçük | ~8–15 MB |
| **Electron + Rust sidecar** | En olgun ekosistem, en iyi belgelenmiş tray davranışı, Chromium her platformda aynı | ~150 MB paket, ~200 MB RAM; iki dilli build zinciri; sidecar süreç yönetimi elle | ~120–180 MB |
| **Electron + N-API eklentisi** | Tek süreç, sidecar yönetimi yok | `node-gyp`/prebuild üç platformda acı; native çökme tüm uygulamayı düşürür; ses thread'i Electron ile aynı süreçte | ~120–180 MB |
| **Qt 6 (C++/QML)** | Tek dil, olgun, en düşük gecikme kontrolü, tray birinci sınıf | Lisans (LGPL yükümlülükleri / ticari), UI geliştirme yavaş, web ekosistemi yok | ~30–60 MB |
| **Flutter Desktop** | Tek kod tabanı, iyi UI DX, ileride mobil | Ses için yine FFI + Rust/C++; Linux desktop olgunluğu orta; tray topluluk paketi | ~25–40 MB |
| **.NET 8 + Avalonia** | Güçlü Windows ses desteği (NAudio), tek dil | macOS/Linux ses tarafı yine P/Invoke; runtime boyutu | ~60–90 MB |
| **JavaFX (AudioRelay'in yolu)** | Referans ürünün kanıtlanmış yolu | JVM bağımlılığı, ses için yine JNI, modern UI zahmetli | ~60–100 MB |

## Karar

**Birincil öneri: Tauri v2 (arayüz) + Rust (çekirdek).**

Gerekçe:

- Çekirdek zaten Rust olacak. Tauri seçmek **tek toolchain, tek build, tek CI**
  demek. Electron seçmek Node + Rust ikilisini paralel yürütmek demek.
- Tepsi/arka plan gereksinimi Tauri v2'de resmi eklentilerle karşılanıyor:
  `tray-icon` (çekirdekte), `tauri-plugin-autostart`, `tauri-plugin-single-instance`,
  `tauri-plugin-window-state`.
- Sürekli arka planda duran bir ses uygulaması için 200 MB RAM'lik Chromium
  gereksiz bir maliyet. Tauri'de pencere kapalıyken bellek ayak izi belirgin düşük.
- Frontend yine React/Svelte + TypeScript; UI geliştirme deneyimi Electron'a yakın.

**Kabul edilen risk:** Linux'ta WebKitGTK, Chromium'dan farklı davranır (CSS
farklılıkları, DevTools daha zayıf). Arayüz görece basit olduğu için bu risk
yönetilebilir. Karşılaşılırsa çözüm: CSS'i muhafazakâr tut, üç platformda da
görsel test et.

**Electron ne zaman tercih edilmeli:** Ekip yalnızca JS/TS biliyorsa ve Rust'a
yatırım yapılamayacaksa. Bu durumda Rust çekirdek **sidecar süreç** olarak
paketlenir (`extraResources` + `child_process.spawn`), UI ile yerel soket
üzerinden konuşur. Mimari aynı kalır, yalnızca kabuk değişir — bkz.
[02-mimari.md](02-mimari.md). Bu yüzden karar geri döndürülebilirdir: çekirdek
her iki durumda da aynıdır.

Karar kaydı: [adr/0001-tauri-ve-rust-cekirdek.md](adr/0001-tauri-ve-rust-cekirdek.md)

## "Tauri v2 + Rust çekirdek" tam olarak ne demek?

Bu cümlede **iki ayrı Rust parçası** var; karıştırılmaması önemli.

### Tauri nedir?

Electron'un yaptığı işi yapan bir çerçeve: web teknolojileriyle yazılmış bir
arayüzü masaüstü uygulamasına çevirir. İki temel farkı var:

1. **Chromium paketlemez.** İşletim sisteminde zaten kurulu olan web görüntüleyiciyi
   kullanır: Windows'ta WebView2 (Edge motoru), macOS'ta WKWebView, Linux'ta
   WebKitGTK. Paketin ~150 MB yerine ~10 MB olmasının sebebi bu.
2. **Arka plan süreci Node.js değil, Rust'tır.** Pencere yönetimi, tepsi, dosya
   erişimi, ayarlar gibi işler Rust tarafında yapılır.

Karşılık tablosu:

| Electron | Tauri |
|---|---|
| main process (Node.js) | Rust backend (`src-tauri/`) |
| renderer (paketlenmiş Chromium) | işletim sisteminin webview'ı |
| `ipcMain` / `ipcRenderer` | `#[tauri::command]` + `invoke()` |
| `preload.js` | Tauri'nin izin (capability) sistemi |
| `electron-builder` | `cargo tauri build` |
| `Tray`, `Menu` | `TrayIconBuilder`, `Menu` |
| ~150 MB paket, ~200 MB RAM | ~10 MB paket, belirgin daha az RAM |

**Frontend tarafı ikisinde de aynı:** HTML + CSS + TypeScript, React/Svelte,
Vite ile derlenir. Arayüz kodu yazarken Electron'dan farkını hissetmezsin.

**"v2"** ise Tauri'nin güncel ana sürümü (2024 sonunda çıktı). v1'e göre tepsi
API'si, eklenti sistemi ve izin modeli yeniden yazıldı; buradaki
[06-arkaplan-ve-tepsi.md](06-arkaplan-ve-tepsi.md) örnekleri v2 API'sine göre.

### "Rust çekirdek" nedir?

Tauri'nin kendi Rust katmanından **ayrı**, bizim yazacağımız bağımsız bir
program: `relaudio-core`. Ses yakalama, kodlama, ağ, jitter buffer burada.
Tauri paketinin içinde değil, **yanında** çalışır (bkz.
[ADR-0002](adr/0002-ayri-cekirdek-sureci.md)).

### İkisi bir arada

```
┌─────────────────────────────────────────────┐
│ relaudio-app  (Tauri uygulaması)            │
│                                             │
│   Frontend            Tauri'nin Rust katmanı│
│   TS + React     ←→   src-tauri/            │
│   (webview)           tepsi, pencere,       │
│                       ayarlar, autostart    │
│                              │              │
└──────────────────────────────┼──────────────┘
                               │ yerel soket
                               │ (JSON-RPC)
                    ┌──────────▼───────────┐
                    │ relaudio-core        │   ← ayrı süreç, ayrı binary
                    │ ses + ağ motoru      │     asıl iş burada
                    └──────────────────────┘
```

- `src-tauri/` **ince** kalır: UI'dan gelen çağrıyı çekirdeğe iletir, çekirdekten
  gelen olayı UI'ya yayar, tepsi ve pencereyi yönetir. Ses koduna dokunmaz.
- `relaudio-core` **kalın**tır: projenin gerçek karmaşıklığı burada.

İkisinin de Rust olması pratik faydalar veriyor: protokol tipleri tek bir
crate'te (`relaudio-proto`) tanımlanıp her ikisi tarafından kullanılır, tek
`cargo build` zinciri, tek CI matrisi.

**Electron seçilseydi** kutunun üst kısmı Node.js olurdu, alt kısmı (çekirdek)
aynı kalırdı. Mimarinin geri döndürülebilir olmasının sebebi bu.


## Sanal sürücü katmanının dili

Bu katman seçilebilir değil, platform dikte eder:

| Platform | Teknoloji | Dil |
|---|---|---|
| Windows | APO / AVStream sürücüsü veya hazır çözüme yönlendirme | C/C++ (WDK) |
| macOS | AudioServerPlugin (kullanıcı alanı, kext değil) | C++ / Objective-C / Swift |
| Linux | PipeWire node veya `module-null-sink` | Gerekmiyor — çalışma anında yaratılır |

Detay: [04-sanal-ses-aygitlari.md](04-sanal-ses-aygitlari.md)

## Frontend bağımlılık önerisi

- **TypeScript** — zorunlu.
- **React** veya **Svelte** — ikisi de uygun; ekip tercihine bırakılır.
- **Durum yönetimi:** hafif bir store (Zustand / Svelte store). Motor durumu
  zaten çekirdekte; UI yalnızca yansıtır.
- **Stil:** CSS değişkenleriyle tema; ağır UI kütüphanesinden kaçın (WebKitGTK
  uyumu için).

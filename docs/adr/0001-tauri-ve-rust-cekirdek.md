# 0001 — Tauri v2 arayüz + Rust çekirdek

- **Durum:** Kabul edildi
- **Tarih:** 2026-09-08

## Bağlam

Cross-platform (Windows/Linux/macOS) düşük gecikmeli bir ses aktarım uygulaması
yazılacak. İlk akla gelen seçenek Electron'du.

Kritik teknik gerçek: **ses motoru JavaScript'te yazılamaz.** Node.js'in ses
donanımına erişimi yok, Web Audio API sistem sesini güvenilir yakalayamıyor ve
V8'in GC duraklamaları gerçek zamanlı ses geri çağrımının 2–10 ms'lik son
teslim tarihini ihlal ediyor. Dolayısıyla native bir çekirdek her koşulda
zorunlu; soru yalnızca **arayüz kabuğunun** ne olacağı.

## Karar

Arayüz **Tauri v2 + TypeScript**, çekirdek **Rust**.

## Sonuçlar

**Kolaylaştırdıkları:**
- Tek toolchain (cargo), tek CI matrisi, tek dilde paylaşılan tip tanımları
  (`relaudio-proto` crate'i hem çekirdek hem Tauri katmanı tarafından kullanılır).
- Paket boyutu ~10 MB, bellek ayak izi belirgin düşük. Sürekli arka planda duran
  bir uygulama için anlamlı.
- Tepsi, otomatik başlatma, tek örnek, pencere durumu için resmi eklentiler mevcut.

**Zorlaştırdıkları:**
- Linux'ta WebKitGTK, Chromium'dan farklı render eder; DevTools daha zayıf.
- Ekip Rust bilmiyorsa öğrenme eğrisi. (Ama çekirdek zaten Rust olacağı için
  bu maliyet her hâlükârda ödeniyor.)
- Electron'a göre daha küçük ekosistem, daha az hazır çözüm.

**Geri döndürülebilirlik:** Yüksek. Çekirdek ayrı süreç olduğu ve IPC üzerinden
konuştuğu için (bkz. [0002](0002-ayri-cekirdek-sureci.md)), kabuğu Electron'a
çevirmek yalnızca UI katmanını değiştirmek demek. Çekirdeğe dokunulmaz.

## Değerlendirilen alternatifler

- **Electron + Rust sidecar:** Geçerli. Ekip JS ağırlıklıysa tercih edilmeli.
  Reddedilme sebebi tek: iki paralel toolchain ve 150 MB paket/200 MB RAM
  maliyeti, bu kullanım için gereksiz.
- **Electron + N-API eklentisi:** Reddedildi. `node-gyp`/prebuild üç platformda
  kırılgan; native çökme tüm uygulamayı düşürür; ses thread'i UI süreciyle
  aynı yerde kalır.
- **Qt 6 (C++/QML):** Teknik olarak en güçlü seçenek ama lisans yükümlülükleri
  ve yavaş UI geliştirme nedeniyle reddedildi.
- **Flutter Desktop:** Ses için yine FFI + Rust gerekir; tepsi desteği topluluk
  paketlerine bağlı. Ek katman kazanç sağlamıyor.

# 0002 — Ses motoru ayrı süreçte çalışır

- **Durum:** Kısmen değiştirildi → [0005](0005-cekirdek-tauri-icinde.md)
- **Tarih:** 2026-09-08

## Bağlam

Uygulamanın pencere kapatıldıktan sonra da yayına devam etmesi gerekiyor.
Ayrıca gerçek zamanlı ses thread'inin, arayüzün render/GC yükünden yalıtılması
gerekiyor.

Ses motoru UI süreciyle aynı yerde çalışırsa: WebView çökmesi yayını keser,
UI güncellemesi ses akışını riske atar, arayüz olmadan çalışan bir mod kurmak
zorlaşır.

## Karar

`relaudio-core` ayrı bir işletim sistemi sürecidir. UI onu başlatır ama sahibi
değildir; UI kapansa da çekirdek yaşamaya devam eder. İletişim yerel soket
(Unix Domain Socket / Named Pipe) üzerinden satır-ayrımlı JSON-RPC ile yapılır.

## Sonuçlar

**Kolaylaştırdıkları:**
- Arayüz çökse/yeniden başlasa da ses kesilmez.
- Ses thread'i tamamen yalıtık; zamanlama garantisi daha güçlü.
- Çekirdek ileride CLI, sistem servisi veya mobil köprü olarak yeniden kullanılabilir.
- Arayüzü Electron'a çevirmek çekirdeği etkilemez.

**Zorlaştırdıkları:**
- IPC katmanı yazılmalı ve sürümlenmeli.
- Süreç yaşam döngüsü elle yönetilmeli: başlatma, sağlık kontrolü, zombi
  temizliği, sürüm uyuşmazlığı, tekillik kilidi.
- Hata ayıklama iki süreçte yapılır; loglar ilişkilendirilmeli.

Bu maliyet bilinçli olarak kabul edildi.

**Not:** Localhost TCP yerine yerel soket seçildi — güvenlik duvarı uyarısı
çıkarmamak ve makinedeki diğer kullanıcıların erişimini engellemek için
(soket izinleri 0600).

## Değerlendirilen alternatifler

- **Tek süreç (motor UI ile birlikte):** Basit ama gereksinimi karşılamıyor;
  UI kapanınca yayın kesilir.
- **Sistem servisi / daemon (kullanıcı oturumundan bağımsız):** Ses aygıtlarına
  erişim oturum bağlamı gerektiriyor (özellikle PipeWire ve CoreAudio).
  Ayrıca kurulumda yönetici yetkisi ister. Reddedildi.

# 00 — Genel Bakış

## Problem

Aynı yerel ağdaki iki bilgisayar arasında sesi düşük gecikmeyle taşımak. Tipik
kullanımlar:

- Masaüstünün sesini salondaki dizüstünün hoparlörlerinden çalmak.
- Kulaklığın takılı olduğu makineyi "ses kartı" gibi kullanmak.
- Bir makinedeki mikrofonu başka bir makinede mikrofon olarak göstermek
  (toplantı/oyun makinesi ayrımı).
- Linux masaüstünü ses çıkışı olmayan bir sunucudan/VM'den beslemek.

## Roller

Uygulama iki rolü de barındırır ve ikisi aynı anda aktif olabilir.

### Server (gönderici)

| Mod | Kaynak | Sanal aygıt gerekir mi? |
|---|---|---|
| Send audio | Sistem sesi (loopback) | Platforma göre — bkz. [04](04-sanal-ses-aygitlari.md) |
| Send mic input | Fiziksel mikrofon | Hayır |

### Player (alıcı)

| Mod | Hedef | Sanal aygıt gerekir mi? |
|---|---|---|
| Receive audio | Fiziksel çıkış aygıtı | Hayır |
| Receive mic input | Sisteme sanal mikrofon olarak sunma | **Evet, her platformda** |

## Hedef platformlar ve minimum sürümler

| Platform | Minimum | Ses altyapısı | Not |
|---|---|---|---|
| Windows | 10 (1809) | WASAPI | Loopback yerleşik, sanal aygıt sürücü ister |
| Linux | PipeWire 0.3.60+ / PulseAudio 15+ | PipeWire (öncelik), PulseAudio (fallback) | En kolay platform |

Mimari: x86_64 (Windows on ARM ve arm64 Linux opsiyonel).

### macOS ertelendi

macOS bilinçli olarak kapsam dışı bırakıldı. Sebep: sistem sesi yakalama
macOS'ta en sorunlu konu — sürüme bağlı üç ayrı yol var, hiçbiri temiz
(bkz. [03](03-ses-hatti.md)), ve 14.4 altında ekran kaydı izni istemek gerekiyor.
İki platformla çıkıp macOS'u sonra eklemek, üçünü birden zorlamaktan daha hızlı.

Dokümanlardaki macOS bölümleri **silinmedi**; araştırma değerli ve macOS
geri geldiğinde hazır olacak. Ama kapsam dışı oldukları açıkça işaretli.
macOS'un mimariye etkisi zaten sınırlı: `audio/capture` ve `audio/playback`
altında bir platform implementasyonu daha demek.

## Hedef: kişisel kullanım

Bu bir yayın ürünü değil; kullanıcının kendi iki makinesi arasında çalışması
hedefleniyor. Bunun planlamaya etkisi:

**Kapsam dışı kalanlar:** NSIS/AppImage paketleme, kod imzalama, otomatik
güncelleme, i18n (tek dil yeterli), onboarding sihirbazı, VB-CABLE'ı kurulum
paketine gömme ve buna bağlı bağış/lisans yükümlülüğü.

Bunlar mimariyi kısıtlamıyor; yayımlamaya karar verilirse geri eklenir.
İlgili dokümanlar (`08`, `09`) silinmedi, ertelendi olarak işaretlendi.

## Kapsam içi (v1)

- LAN keşfi (mDNS) + IP ile manuel bağlanma
- PCM ve Opus kodek desteği
- Aygıt seçimi (giriş/çıkış), örnekleme hızı ve kanal seçimi
- Adaptif jitter buffer, saat kayması telafisi
- Sistem tepsisi / menü çubuğu, arka planda çalışma, oturum açılışında başlatma
- Karanlık/aydınlık tema
- Anlık istatistikler: gecikme, paket kaybı, bitrate

## Kapsam dışı (v1)

Bunlar bilinçli olarak ertelendi; v2+ değerlendirilecek:

- **macOS.** Yukarıda gerekçelendirildi. Mimari onu sonradan kabul edecek
  şekilde kurulur.
- **Mobil istemci (Android/iOS).** AudioRelay'in ana kullanım senaryosu bu; ancak
  kullanıcı talebi masaüstü. Protokol mobil eklenebilecek şekilde tasarlanır.
- **İnternet üzerinden bağlantı** (NAT traversal, relay sunucu).
- **Çoklu alıcıya eşzamanlı yayın.** Protokol izin verir, v1 UI'ı tek eşe kilitler.
- **Ses işleme:** EQ, gürültü bastırma, kanal karıştırma.
- **Video/senkron.** Bu bir ses uygulaması; A/V lip-sync garantisi verilmez.
- **Kendi imzalı Windows sanal ses sürücüsü.** v1'de mevcut sürücülere
  (VB-CABLE vb.) yönlendirme yapılır — bkz. [04](04-sanal-ses-aygitlari.md).

## Başarı kriterleri

| Metrik | Hedef | Ölçüm |
|---|---|---|
| Uçtan uca gecikme | ≤ 80 ms (kablolu LAN), ≤ 150 ms (Wi-Fi 5 GHz) | Döngü testi, bkz. [03](03-ses-hatti.md) |
| Yakalama gecikmesi | ≤ 10 ms | Motor içi ölçüm, UI'da gösterilir |
| Kesintisiz çalışma | 8 saat, glitch sayısı = 0 | Uzun süreli test |
| Bant genişliği (PCM 48k/16/stereo) | ~1.6 Mbit/s | Sayaç |
| Bant genişliği (Opus 128k) | ~0.14 Mbit/s | Sayaç |
| CPU (motor, 48k stereo PCM) | < %3 tek çekirdek | Profil |
| RAM (toplam, UI dahil) | < 250 MB | Profil |


## Terminoloji

- **Loopback / monitor** — sistemin çaldığı sesin yakalanması.
- **Sanal ses aygıtı** — işletim sistemine gerçekmiş gibi görünen, yazılımla
  beslenen giriş/çıkış aygıtı.
- **Jitter buffer** — ağdan düzensiz gelen paketleri düzgün akışa çeviren tampon.
- **Saat kayması (clock drift)** — gönderici ve alıcı ses kartlarının nominal
  48 kHz'i birebir aynı hızda üretmemesi; telafi edilmezse periyodik kopma yaratır.
- **Underrun / overrun** — tamponun boşalması / taşması.

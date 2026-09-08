# 09 — Yol Haritası

Fazlar sırayla ilerler; her fazın somut bir kabul kriteri vardır. Erken fazlar
UI'sız, doğrulanabilir çıktılar üretir — en riskli parça (ses + ağ) önce çözülür.

## Faz 0 — Doğrulama (spike)  ✅ *(tamamlandı, docs/10)*

**Amaç:** "Bu gerçekten yapılabilir mi?" sorusunu koda dökmek. Atılacak kod.

- Linux'ta PipeWire monitor'den yakala → UDP'ye yaz → ikinci makinede oku → çal.
- Ölçüm: gerçek uçtan uca gecikme ne çıkıyor?
- Windows WASAPI loopback ile aynısı.

**Kabul:** Her iki platformda da ses akıyor, ölçülen gecikme < 100 ms.
**Bu faz geçilmeden mimariye bağlanmayın.**

## Faz 1 — Çekirdek iskeleti  ✅ *(büyük ölçüde tamam)*

- `relaudio-core` süreç yapısı, IPC sunucusu, JSON-RPC şeması.
- Aygıt listeleme (Windows + Linux), hot-plug olayları.
- Halka tampon, gerçek zamanlı disiplin, sayaçlar.
- Yakalama ve çalma soyutlamaları + iki platform implementasyonu.
  Soyutlama macOS'u sonradan kabul edecek şekilde tasarlanır.
- PCM kodek.

**Kabul:** CLI ile `relaudio-core` başlatılıp iki makine arasında PCM yayın
yapılabiliyor. Underrun sayacı 1 saatlik testte 0.

## Faz 2 — Ağ olgunluğu

- mDNS keşif + broadcast fallback + manuel IP.
- TCP kontrol kanalı, el sıkışma, format anlaşması.
- Adaptif jitter buffer.
- Saat kayması telafisi (adaptif resampler).
- Opus kodek.
- `tc netem` ile kayıp/jitter testleri.

**Kabul:** %3 paket kaybı ve 20 ms jitter altında duyulur bozulma yok.
8 saatlik test: kopma yok, drift telafi ediliyor.

## Faz 3 — Arayüz

- Tauri kabuğu, üç sekme, aygıt seçimi, eş listesi.
- Canlı istatistikler, durum göstergeleri.
- Ayarlar kalıcılığı, tema, i18n (TR/EN).
- Hata mesajları ve kurtarma akışları.

**Kabul:** Teknik olmayan bir kullanıcı yardım almadan iki makine arasında
yayın başlatabiliyor.

## Faz 4 — Arka plan ve tepsi

- Tepsi simgesi, menü, durum yansıtma.
- Çarpı → gizle, tek örnek, çıkış onayı.
- Otomatik başlatma, `--hidden` argümanı.
- [06](06-arkaplan-ve-tepsi.md)'daki 12 maddelik test listesinin tamamı.

**Kabul:** Test listesi iki platformda ve en az üç Linux masaüstü ortamında geçer.
GNOME'da tepsisiz senaryo düzgün ele alınıyor.

## Faz 5 — Sanal aygıtlar

- Linux: PipeWire node yaratma (sanal hoparlör + sanal mikrofon).
- Windows: tespit + rehberli yönlendirme akışı.
- "Receive mic input" senaryosunun uçtan uca çalışması.

**Kabul:** Linux'ta harici kurulum olmadan sanal mikrofon çalışıyor.
Windows'ta rehber izlenerek çalışıyor.

## Faz 5.5 — VB-CABLE'ı pakete gömme *(ertelendi — kişisel kullanım)*

Dağıtım izni doğrulandı, ayrı bir anlaşma beklemek gerekmiyor:

- VB-CABLE kurulum paketimize gömülür; sessiz kurulum akışı.
- Yeniden başlatma gereksinimi kullanıcıya düzgün anlatılır.
- Kaldırma akışı: uygulamayı kaldırırken sürücü de kaldırılsın mı? (Kullanıcı
  başka bir şey için kullanıyor olabilir — sormalı, sessizce silmemeli.)
- Donationware görünürlüğü: kullanıcı ürünün VB-Audio'ya ait olduğunu görebilmeli
  ve bağış bağlantısına ulaşabilmeli. **Lisans şartı, atlanamaz.**
- VB-Audio'ya lisans bağışı yapılır.

**Kabul:** Kullanıcı hiçbir harici indirme yapmadan "Receive mic input"
kullanabiliyor.

## Faz 6 — Paketleme ve yayın *(ertelendi — kişisel kullanım)*

- İki platform CI. İmzalama v1'de zorunlu değil.
- Updater.
- Kurulum belgeleri, sorun giderme rehberi.

**Kabul:** Temiz makinelere kurulup çalışıyor. Windows'ta uygulama imzasız
dağıtılabilir (SmartScreen uyarısı kabul edilir).

## Sonrası (v2 adayları)

Öncelik sırası henüz belirlenmedi:

- **macOS desteği** (ertelendi, mimari hazır)
- Android/iOS istemci (asıl kullanım senaryosunun büyük kısmı burada)
- Çoklu alıcıya eşzamanlı yayın
- Uygulama bazlı ses yakalama (yalnızca Spotify'ın sesi gibi)
- FLAC (kayıpsız sıkıştırma) kodek
- İnternet üzerinden bağlantı

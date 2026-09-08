# 0003 — Kendi sanal ses sürücümüzü yazmıyoruz, üçüncü tarafı lisanslıyoruz

- **Durum:** Kabul edildi
- **Tarih:** 2026-09-08

## Bağlam

"Receive mic input" (başka cihazı bu makinede mikrofon olarak kullanma) özelliği
her platformda bir sanal ses aygıtı gerektiriyor.

Maliyetler:
- **Windows:** Çekirdek modu sürücü. Windows'ta kullanıcı alanında ses uç noktası
  yaratmanın desteklenen bir yolu yok. İmzasız sürücü yüklenmez (Secure Boot
  açıkken OS reddeder) — bu, uygulama binary'sindeki SmartScreen uyarısından
  farklı, sert bir kısıttır. Microsoft attestation signing gerekiyor; portala
  bağlanmak için EV kod imzalama sertifikası zorunlu (~$250–600/yıl). Asıl
  maliyet parada değil, WDK öğrenme eğrisi ve sürüm bakımında.
- **macOS:** AudioServerPlugin (kernel extension değil, bu iyi). Ama Developer ID
  imzalama + notarization + root kurulum akışı + `coreaudiod` yeniden başlatma
  UX'i gerekiyor.
- **Linux:** Maliyet yok; PipeWire node'u çalışma anında yaratılabiliyor.

## Karar

Kendi Windows sürücümüzü **hiçbir zaman** yazmıyoruz. Bunun yerine, imzalı bir
üçüncü taraf sürücüyü yeniden dağıtım lisansıyla alıp kendi kurulum paketimize
gömüyoruz.

**Windows:** VB-CABLE kurulum paketimize gömülür. VB-Audio'nun dağıtım
şartları bunu açıkça serbest bırakıyor (doğrulandı, Eylül 2026):
uygulamanla birlikte dağıtabilir ve kurulum paketine gömebilirsin.

Şartlar: donationware modeli korunmalı (kullanıcı ürünü VB-Audio ürünü olarak
tanıyabilmeli ve bağış yapabilmeli), dağıtım için anlamlı bir bağış lisans
ücreti olarak bekleniyor (500–2000 USD örnekleniyor), ve **yalnızca temel
VB-CABLE** paketlenebilir — A+B / C+D paketlenemez.

**Aygıt yeniden adlandırma bu izne dahil değil.** Aygıt
`CABLE Input (VB-Audio Virtual Cable)` olarak görünür. "Virtual Mic for RelAudio"
istiyorsak ayrıca sorulmalı; v1 için gerekli değil.

Bu, referans üründeki "Virtual Speaker for AudioRelay" isminin standart izinle
açıklanamadığı anlamına geliyor — onların özel bir anlaşması, başka bir satıcısı
veya kendi sürücüsü var. Bizi bağlamıyor.

**Linux:** Uygulama kendi PipeWire node'larını çalışma anında yaratır.
Kullanıcıdan hiçbir kurulum istenmez.

**macOS:** Kapsam dışı (ertelendi).

## Sonuçlar

**Kolaylaştırdıkları:**
- v1 çıkışı sertifika ve imzalama sürecine bağlı kalmaz.
- Ana senaryo ("Send audio" — sistem sesini gönder) hiç etkilenmez; Windows'ta
  WASAPI loopback, Linux'ta PipeWire monitor sürücüsüz çalışır.
- **EV sertifika, WDK ve attestation signing tamamen masadan kalkar.** Sürücü
  satıcı tarafından imzalanmış gelir. Windows sürüm uyumluluğunun bakımı da
  satıcıda kalır.
- Aygıt adı bize ait olur; "CABLE Input" gibi kafa karıştırıcı isimler yerine
  "Virtual Mic for RelAudio" görünür.

**Zorlaştırdıkları:**
- v1'de "Receive mic input" Windows ve macOS'ta **harici bir kuruluma bağımlı.**
  Bu bir UX zaafı ve kullanıcıya dürüstçe açıklanmalı.
- Rehberli akışın kendisi de iş; tespit mantığı (aygıt adı + yetenek eşleştirmesi)
  kırılgan olabilir.
- **Ticari bağımlılık:** Sürücü satıcısına bağlı hâle geliriz. Satıcı desteği
  keserse veya şartlarını değiştirirse alternatif gerekir.
- **Aygıt adı bize ait değil.** Kullanıcı ses ayarlarında "CABLE Input" görür,
  "RelAudio" değil. Bir UX pürüzü; kurulum rehberinde açıklanmalı.
- **Kurulum yeniden başlatma istiyor.** VB-CABLE kurulumu sonrası reboot
  gerekiyor; akışın bunu düzgün anlatması gerek.
- Tek kablo ile sınırlıyız (A+B paketlenemez); kullanıcı VB-CABLE'ı başka bir
  şey için zaten kullanıyorsa çakışma olur.

**Lisans notu:** BlackHole GPL-3.0'dır. Yönlendirme (kullanıcı kendi kurar)
lisans yükümlülüğü doğurmaz; paketleyip dağıtmak doğurur. Paketlemiyoruz.

## Değerlendirilen alternatifler

- **Kendi Windows sürücümüzü yazmak:** Reddedildi. Lisans yolu aynı sonucu,
  EV sertifika + WDK + her sürümde attestation turu olmadan veriyor. Bu
  seçeneği v2'ye ertelemek yerine tamamen kapattık.
- **Sadece kullanıcı kurulumuna dayanmak (kalıcı olarak):** Ücretsiz ama UX
  zayıf; aygıt adları kafa karıştırıcı, kurulum adımı kullanıcıyı kaybettiriyor.
  v1 için kabul edilebilir, kalıcı çözüm değil.
- **"Receive mic input" özelliğini tamamen çıkarmak:** Reddedildi — Linux'ta
  bedavaya çalışıyor ve gerçek bir kullanım senaryosu.

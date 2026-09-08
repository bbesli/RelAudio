# 04 — Sanal Ses Aygıtları

Bu, projenin **en zor ve en maliyetli** parçasıdır. Ekran görüntüsündeki
"Creating a virtual audio device could be necessary" ve "A virtual audio device
is necessary" uyarıları tam olarak bu konuya işaret ediyor.

## Ne zaman gerekir?

| Senaryo | Sanal aygıt | Neden |
|---|---|---|
| Send audio (sistem sesi gönder) | Windows: **hayır** · Linux: **hayır** | WASAPI loopback ve PipeWire monitor yerleşik |
| Send mic input | **Hayır** | Fiziksel mikrofon doğrudan okunur |
| Receive audio (sesi çal) | **Hayır** | Normal çıkışa yazılır |
| **Receive mic input** (sanal mikrofon) | **Evet, her platformda** | İşletim sistemine "mikrofon" olarak görünecek bir aygıt yaratmaktan başka yol yok |

İkinci bir kullanım daha var: kullanıcı sadece **belirli bir uygulamanın** sesini
göndermek isterse, o uygulamanın çıkışını sanal bir hoparlöre yönlendirip onun
monitor'ünü yakalamak gerekir.

## Linux — kolay

PipeWire/PulseAudio çalışma anında modül yükleyerek aygıt yaratabilir; **kurulum,
sürücü, imza, yeniden başlatma gerekmez.**

- **Sanal hoparlör:** `module-null-sink` — bir sink yaratır, monitor'ü okunur.
- **Sanal mikrofon:** null-sink + `module-remap-source` ile monitor'ü kaynak
  olarak sunmak; ya da PipeWire'da doğrudan bir `Audio/Source` node yaratmak.
- PipeWire ile en temizi: uygulamanın kendi `pw-stream`'ini `Audio/Source` medya
  sınıfıyla kaydettirmesi — böylece **harici modüle hiç gerek kalmaz** ve
  aygıt uygulama kapanınca kendiliğinden kaybolur.
- Ekran görüntüsündeki "AudioRelay Speaker" ve "AudioRelay Mic&Sink" girdileri
  bu yöntemle yaratılmış node'lar.

**Karar:** Linux'ta node'lar çalışma anında uygulama tarafından yaratılır,
kullanıcıdan hiçbir kurulum istenmez.

## macOS — orta zorluk *(ertelendi)*

> **Kapsam dışı (ertelendi).** macOS v1 hedefi değil. Bu bölüm ileride
> geri gelmesi için korunuyor; şu an iş yapılmıyor.


- Sanal aygıt bir **AudioServerPlugin**'dir: `coreaudiod` içine yüklenen,
  `/Library/Audio/Plug-Ins/HAL/` altına kurulan bir bundle. **Kernel extension
  değildir** — bu iyi haber; kext imzalama/onay çilesi yok.
- Kurulum root yetkisi ister ve `coreaudiod` yeniden başlatılmalıdır
  (`sudo killall coreaudiod`) — bu sırada tüm sistem sesi kısa süre kesilir.
- Bundle **Developer ID ile imzalanmalı ve notarize edilmelidir**, aksi hâlde
  Gatekeeper engeller.
- Referans uygulama: BlackHole (GPL) ve Apple'ın `SimpleAudioDriver` örneği.

**v1 kararı:** Kendi eklentimizi yazmak yerine, "Receive mic input" özelliği için
kullanıcıyı **BlackHole kurulumuna yönlendir** ve uygulama içi rehber göster.
Kendi eklentimiz v2'de. Gerekçe: notarization + kurulum akışı + coreaudiod
yeniden başlatma UX'i tek başına bir sprint.

**Not:** Lisans dikkat — BlackHole GPL-3.0'dır; onu paketleyip dağıtmak lisans
yükümlülüğü doğurur. Yönlendirme (kullanıcı kendi kurar) bu sorunu doğurmaz.

## Windows — en zor

### Önce bir ayrım: iki farklı "imzalama" var

Bunlar sık karıştırılır ama tamamen farklı şeylerdir:

| | Uygulama imzalama (Authenticode) | Sürücü imzalama |
|---|---|---|
| Neyi kapsar | `relaudio.exe`, kurulum paketi | `.sys` çekirdek modu sürücü |
| Zorunlu mu? | **Hayır** | **Evet** |
| İmzasızsa ne olur | SmartScreen "Bilinmeyen yayımcı" uyarısı çıkar; kullanıcı "Yine de çalıştır" der ve program çalışır | Sürücü **hiç yüklenmez**; işletim sistemi reddeder |
| Engel türü | Uyarı penceresi | Sert OS kısıtı |

**Uygulamanın kendisi için sertifikaya ihtiyaç yok.** İmzasız dağıtım tamamen
geçerli bir yoldur; sayısız açık kaynak masaüstü uygulaması böyle dağıtılıyor.
Sertifika yalnızca kullanıcı deneyimini iyileştirir (uyarı penceresini kaldırır)
ve v1 için gerekli değildir.

**Sürücü tarafı farklıdır.** Windows 10 1607'den itibaren, Secure Boot açık bir
sistemde çekirdek modu sürücüler Microsoft tarafından imzalanmış olmak
zorundadır. Self-signed bir `.sys` dosyası yüklenmez — kullanıcı "yine de
yükle" diyemez. Tek alternatif `bcdedit /set testsigning on` ile test imza
moduna geçmek; bu Secure Boot'un kapatılmasını gerektirir, masaüstünde kalıcı
bir filigran bırakır ve son kullanıcıya sunulabilecek bir şey değildir.

### Windows'ta sanal ses aygıtı neden sürücü gerektiriyor?

Windows'ta ses uç noktaları (endpoint), çekirdek modundaki KS (Kernel
Streaming) filtrelerinden türer. macOS'un `AudioServerPlugin`'i veya
PipeWire'ın node'ları gibi **kullanıcı alanında yeni bir ses aygıtı yaratmanın
desteklenen bir yolu yoktur.** APO (Audio Processing Object) kullanıcı
alanındadır ama var olan bir uç noktaya takılır, yenisini yaratmaz.

Bu, Windows'un gerçek bir mimari boşluğudur ve VB-CABLE gibi araçların neden
var olduğunun cevabıdır.

### Seçenekler

| Yol | Zorluk | Gereken |
|---|---|---|
| Mevcut sürücüye yönlendirme (VB-CABLE, VAC) | Düşük | Hiçbir şey — kullanıcı kendi kurar |
| Kendi AVStream/portcls sürücümüz | Çok yüksek | WDK + EV kod imzalama sertifikası + Microsoft Partner Center üzerinden **attestation signing** |
| `SYSVAD` örneğini temel alma | Yüksek | Aynı imzalama gereksinimleri |

**Attestation signing** (WHQL'in hafif hâli): HLK test paketini çalıştırmayı
gerektirmez, sürücüyü Partner Center'a yükleyip Microsoft'un imzalamasını
istemektir. Kayıt ücretsizdir ama **portala bağlanmak için bir EV kod imzalama
sertifikası zorunludur** — sertifikanın işlevi burada uyarı penceresini
kaldırmak değil, kimlik doğrulamaktır.

Gerçekçi maliyet: EV sertifika ~$250–600/yıl (satıcıya ve bulut HSM kullanımına
göre değişir). Daha önce bu doküman "$1.000+" diyordu; bu abartılıydı.
Asıl maliyet parada değil, **mühendislik süresinde ve sürüm bakımında**:
WDK öğrenme eğrisi, x64 + ARM64 build, her Windows sürümünde regresyon testi,
her sürücü güncellemesinde yeniden imzalatma turu.

### AudioRelay ne yapıyor? — muhtemelen lisanslı, yeniden markalanmış sürücü

Windows'ta aygıt adları **"Virtual Speaker for AudioRelay"** ve
**"Virtual Mic for AudioRelay"** şeklinde görünüyor.

Bu isimlendirme kalıbı belirleyici: bağımsız bir üçüncü taraf kendi sürücüsünü
"for AudioRelay" diye adlandırmaz. İki ihtimalden birine işaret ediyor —
ya sürücüyü kendileri yazıp imzalattılar, ya da **yeniden markalama hakkı olan
bir üçüncü taraf sürücüyü lisansladılar.** İkincisi bu sektörde yaygın ve çok
daha ucuz olan yoldur.

### VB-Audio'nun dağıtım şartları — doğrulandı

`vb-audio.com/Services/licensing.htm` sayfasından, Eylül 2026 itibarıyla:

**Paketleme açıkça serbest.** Sayfa şunu diyor: VB-CABLE paketini kendi
uygulamanla birlikte (ücretsiz veya ticari) dağıtabilir ve kurulum paketine
gömebilirsin. Yani **ayrı bir OEM anlaşması beklemeden başlayabiliriz.**

Şartlar:

| Şart | Sonuç |
|---|---|
| Donationware modeli korunmalı; son kullanıcı ürünü **VB-Audio ürünü olarak tanıyabilmeli** ve bağış yapabilmeli | Aygıtı sessizce yeniden adlandırmak bu şartla çelişir |
| Dağıtım için "anlamlı bağış" lisans ücreti olarak bekleniyor (sayfada 500 / 1000 / 2000 USD örnekleniyor) | Tek seferlik, makul |
| **Yalnızca temel VB-CABLE dağıtılabilir.** A+B ve C+D paketlenemez | Tek kablo ile yetinmemiz gerekiyor |
| Profesyonel/kurumsal bağlamda kullanımda lisans ödenmeli | Son kullanıcının yükümlülüğü |

**Yeniden adlandırma bu sayfada geçmiyor.** Yani standart izin bize
"Virtual Mic for RelAudio" ismini vermiyor; aygıt
`CABLE Input (VB-Audio Virtual Cable)` olarak görünür. Zaten "kullanıcı bunu
VB-Audio ürünü olarak tanıyabilmeli" şartıyla da çelişirdi.

**Bu, AudioRelay hakkında bir şey söylüyor:** "Virtual Speaker for AudioRelay"
ismi standart izinle açıklanamıyor. Ya VB-Audio ile özel bir anlaşmaları var,
ya başka bir satıcı kullanıyorlar, ya da kendi sürücüleri. Yeniden adlandırma
bizim için gerekliyse ayrıca sorulmalı — ama v1 için gerekli değil.

### Hangi VB-Audio ürünü gerekiyor?

**Sadece temel VB-CABLE.** Sağladığı şey bir çift uç nokta:

- `CABLE Input (VB-Audio Virtual Cable)` → sistemde **hoparlör** olarak görünür
- `CABLE Output (VB-Audio Virtual Cable)` → sistemde **mikrofon** olarak görünür

Birine yazılan, diğerinden okunur. Bizim akışımız:

```
ağdan gelen ses → relaudio-core → "CABLE Input"e yazar
                                        ↓
              Discord/Zoom mikrofon olarak "CABLE Output"u seçer
```

Tek kablo, "Receive mic input" özelliği için yeterli.

Diğer ürünler neden gerekmiyor:

| Ürün | Neden gerekmiyor |
|---|---|
| VB-CABLE A+B / C+D | Ek kablo çiftleri. Yalnızca eşzamanlı birden fazla bağımsız akış gerekirse (v1 kapsamı dışı) veya kullanıcı VB-CABLE'ı başka bir şey için zaten kullanıyorsa (çakışma) |
| HIFI-CABLE & ASIO Bridge | ASIO/yüksek çözünürlük senaryoları için; 48 kHz stereo'da kazancı yok |
| Voicemeeter (tüm sürümler) | Tam bir sanal mikser uygulaması. Aşırı; gömülemez, kullanıcıya ayrı bir program dayatır |
| VB-Audio Matrix / Coconut | Yönlendirme matrisi. Aynı sebeple aşırı |
| Macro-Buttons, VBAN-*, Spectralissime, MT* | Konuyla ilgisiz |

**Not:** VB-CABLE'ın donationware lisansı ticari kullanımda bağış/lisans
bekliyor. Kullanıcı kendi kurduğunda bu yükümlülük kullanıcıdadır, bizde değil.

### Windows için tüm seçenekler

| Seçenek | Sürücü kodu | İmzalama | Aygıt adı | Durum |
|---|---|---|---|---|
| **VB-CABLE, kullanıcı kurar** | — | — | "CABLE Input/Output" | **v1 kararı.** Ücretsiz, bugün çalışır, UX zayıf |
| **VB-CABLE paketimize gömülü** | — | Satıcıda | "CABLE Input/Output" | **Şartlar doğrulandı, serbest.** Bağış + donationware görünürlüğü |
| Virtual Audio Cable (VAC) | — | Satıcıda | Bize ait | Ticari ürün, lisanslama modeli daha net; şartlar karşılaştırılmalı |
| Microsoft `sysvad` / Scream temel alma | Var, hazır, MS-PL | **Bizde** | Bize ait | Kod bedava; imzalama yükü bizde kalır |
| Sıfırdan kendi sürücümüz | Bizde | Bizde | Bize ait | Reddedildi — `sysvad` varken sıfırdan yazmak anlamsız |

**Scream** (`github.com/duncanthrax/scream`) ilginç bir referans: Windows için
sanal ses sürücüsü + ağ üzerinden ses aktarımı yapan açık kaynak bir proje,
yani bizim yapmak istediğimizin küçük bir örneği. Kendi dağıtımı imzasız olduğu
için son kullanıcı test imza modu açmak zorunda.

### Sürücü kaynak kodu zaten bedava — sorun o değil

Bu noktada bir yanılgıyı kapatmak gerekiyor: **sanal ses sürücüsünün kaynak
kodunu bulmak bir problem değil.** Microsoft, tam da bu işi yapan örnekleri
kendi deposunda yayımlıyor:

`github.com/microsoft/Windows-driver-samples` → `audio/`

| Örnek | Ne |
|---|---|
| `sysvad` | Sanal ses aygıtı sürücüsü — modern, bakımı yapılan referans |
| `simpleaudiosample` | Daha sade bir başlangıç |
| `Acx/` | Windows 11'in yeni ACX modeliyle yazılmış örnekler |

Depo **MS-PL** lisanslı (izin verici; ticari kullanıma ve dağıtıma açık) ve
aktif olarak güncelleniyor.

**Darboğaz kod değil, imza.** Bu ikisi bağımsız şeylerdir ve sürekli
karıştırılıyor:

| | Lisans | İmza |
|---|---|---|
| Ne verir | Kodu **kullanma ve dağıtma hakkı** | İşletim sisteminin binary'yi **yükleme izni** |
| Kim verir | Kodun sahibi (Microsoft, MS-PL ile) | Microsoft'un imzalama servisi |
| Neyi kapsar | Kaynak kodu | Senin derlediğin tekil `.sys` dosyası |
| Bedava mı | Evet | Hayır |

Microsoft'un `sysvad` örneğini kullanmak **lisans** tarafını çözer, **imza**
tarafını çözmez. Sebebi şu: GitHub'daki şey kaynak kodudur (`.cpp`, `.h`,
`.inf`) — binary değil. Sen onu derlediğinde ortaya **senin** ürettiğin, daha
önce dünyada var olmamış bir `sysvad.sys` çıkar.

Windows sürücü yüklerken "bu kod nereden gelmiş?" diye sormaz — soramaz, kaynak
koduna erişimi yok. Sorduğu tek şey: *"bu dosyanın üzerinde, güvendiğim bir
zincire bağlanan geçerli bir imza var mı?"* Senin derlemenin üzerinde böyle bir
imza yok. Microsoft'un kodu yayımlamış olması, senin derlemene imza atmış
olduğu anlamına gelmiyor.

Benzetme: Microsoft yemek tarifini yayımlamış. Tarifi kullanman serbest. Ama
kendi mutfağında pişirdiğin yemeği restoranda satmak için gıda sertifikası
gerekiyor — tarifin ünlü bir şeften gelmesi bu sertifikayı sağlamıyor.

**Neden bu kadar katı?** Çekirdek modu sürücü, sistemde tam yetkiyle çalışır.
İmzasız sürücü yüklenebilseydi her rootkit kendini sürücü olarak kurardı.
Microsoft bu yüzden 2016'da (Win10 1607) bunu sert bir kapıya çevirdi ve
Secure Boot'a bağladı.

**EV sertifikanın buradaki rolü uyarı kaldırmak değil, kimlik doğrulamaktır.**
Süreç şöyle işliyor:

```
sysvad kaynağı (MS-PL, bedava)
        ↓ derle
  senin sysvad.sys      ← imzasız, Windows yüklemez
        ↓ Partner Center'a yükle          ← girmek için EV sertifika gerekiyor
Microsoft attestation imzası
        ↓
  imzalı sysvad.sys     ← artık yüklenir
```

EV sertifika, Partner Center kapısındaki kimlik kartı. Sürücünün üzerine
sonunda düşen imza Microsoft'un.

**Bu, uygulamamızın kendisi için geçerli değil.** `relaudio.exe` kullanıcı
modunda çalışır; hiçbir imzaya ihtiyacı yok, imzasız dağıtılabilir.
Sadece çekirdek modu `.sys` dosyaları bu kapıdan geçmek zorunda.

### Bu yüzden OEM lisansı aslında "imza kiralamak"

Üçüncü taraf lisansının cazip olmasının sebebi kod değil. VB-Audio'nun kodunu
istemiyoruz — Microsoft'unki zaten bedava. İstediğimiz şey **onların zaten
imzalanmış binary'si** ve o imzayı her Windows sürümünde güncel tutma yükünü
üstlenmeleri.

| Yol | Kod | İmza | Bize maliyet |
|---|---|---|---|
| `sysvad`'dan kendimiz türetiriz | Bedava (MS-PL) | **Bizde** | EV sertifika + her sürümde attestation turu + WDK emeği |
| OEM lisans | Satıcıda | **Satıcıda** | Lisans ücreti |
| Kullanıcı VB-CABLE'ı kurar | Satıcıda | Satıcıda | **Sıfır** |

### `HSpear/virtual-audio-wire` — kullanılamaz

Değerlendirildi, elenedi. Repo bir **proje değil, bir istek listesi**:

- README'si 12 maddelik bir gereksinim listesi ve katkıda bulunan arayan bir
  çağrı — "Here we provide MSVAD sample program as an initial code."
- İçindeki tek kod Microsoft'un eski **MSVAD** örneğinin birebir kopyası.
  Özgün satır yok.
- Toplam 10 commit, hepsi README düzenlemesi. **Son kod push'u: Eylül 2013.**
- Deponun kendi lisansı yok (yalnızca Microsoft örneğinin `license.rtf`'i var).
  Lisanssız kod = tüm haklar saklı; kullanılamaz.
- 241 yıldız, fikri beğenenler; kullananlar değil.

Kısacası: içinde alınacak bir şey yok. Aynı MSVAD kodunun güncel, bakımlı ve
MS-PL lisanslı hâli zaten Microsoft'un kendi deposunda — oraya gitmek her
açıdan daha iyi.

### VB-CABLE'ı kullanmak için SDK yok — düz WASAPI

Sık sorulan soru: "VB-CABLE'ın SDK'sini nasıl kullanacağız?" Cevap: **SDK yok,
gerekmiyor da.**

VB-CABLE bir kütüphane değil; kurulduğunda Windows'a iki normal ses uç noktası
ekleyen bir sürücü. Bizim açımızdan diğer bütün ses aygıtlarından farkı yok:

```
CABLE Input  (VB-Audio Virtual Cable)   → sistemde HOPARLÖR olarak görünür
CABLE Output (VB-Audio Virtual Cable)   → sistemde MİKROFON olarak görünür
```

Yani entegrasyon işi **sıfır**. Zaten yazacağımız WASAPI çalma kodu, aygıt
olarak "CABLE Input"u seçtiğinde iş biter:

```
ağdan gelen ses → relaudio-core → WASAPI render → "CABLE Input"
                                                        ↓ (sürücü içinden)
                          Discord/Zoom mikrofon olarak "CABLE Output"u seçer
```

Yapılacaklar listesi entegrasyon değil, **ayrıntı yönetimi**:

1. **Aygıtı güvenilir tespit et.** Görünen ad ("CABLE Input (VB-Audio Virtual
   Cable)") kırılgan — kullanıcı yeniden adlandırabilir, yerelleştirme değişebilir.
   `IMMDeviceEnumerator` ile listeleyip `PKEY_Device_InstanceId` içindeki
   donanım kimliğine bakmak daha sağlam. Kesin kimlik deseni Faz 0'da ölçülmeli.
2. **Format uyumu.** Kablonun iki ucunun formatı Windows ses denetim masasından
   ayrı ayrı ayarlanır ve **uyuşmazsa ses bozulur.** Motor, uç noktanın
   `GetMixFormat` değerini okuyup ona uymalı; 48 kHz'i dayatmamalı.
3. **Yokluğunu tespit et** ve kurulum rehberini göster.
4. **Kurulum akışı.** VB-CABLE `VBCABLE_Setup_x64.exe` ile kuruluyor, yönetici
   yetkisi istiyor ve **kurulum sonrası yeniden başlatma gerektiriyor.** Sessiz
   kurulum bayrakları resmî olarak belgelenmemiş — doğrulanmalı.

### Sürücüsüz bir yol var mı? — Hayır

Bu soruyu kapatalım: Windows'ta ses uç noktaları çekirdek modundaki KS
filtrelerinden türer. Kullanıcı alanından yeni bir uç nokta yaratmanın
desteklenen bir yolu yoktur. APO var olan uç noktaya takılır, yenisini
yaratmaz. DirectShow filtreleri yalnızca DirectShow kullanan uygulamalara
görünür (bugün neredeyse hiç kimse). Windows 11'in ACX modeli de kernel modudur.

### Ama bu yalnızca 4 özellikten 1'ini etkiliyor

| Özellik | Windows'ta sürücü gerekir mi? |
|---|---|
| Send audio (sistem sesi gönder) | **Hayır** — WASAPI loopback |
| Send mic input | **Hayır** |
| Receive audio (sesi çal) | **Hayır** |
| Receive mic input (sanal mikrofon) | Evet |

Sanal mikrofon, masaüstünden masaüstüne bir araçta görece niş bir senaryo.
**Bu konunun v1'i bloke etmesine izin verilmemeli.**

### macOS'ta durum farklı — kendi eklentimiz makul *(ertelendiğinde geçerli)*

Windows'un aksine macOS'ta sanal aygıt **kullanıcı alanında** bir
`AudioServerPlugin`'dir. Gereken tek şey zaten sahip olduğumuz Developer ID
($99/yıl) ve notarization. Kernel sürücü yok, attestation turu yok, EV
sertifika yok.

Yani macOS'ta kendi eklentimizi yazmak gerçekçi bir seçenek; maliyet
mühendislik süresi (kurulum akışı + `coreaudiod` yeniden başlatma UX'i) ile
sınırlı. Üçüncü taraf gerekirse BlackHole'un ticari/beyaz etiket lisansı da
araştırılabilir — GPL-3.0 sürümünü paketlemek mümkün ama lisans yükümlülükleri
(kaynak sunma, lisans metni) doğar.

## Karar: üçüncü taraf sürücü lisanslanır

**v1:** Kullanıcı VB-CABLE'ı kendisi kurar (ücretsiz, hemen çalışır, UX zayıf).
**v1.x / v2:** Lisans şartları uygunsa yeniden markalanmış sürücü kurulum
paketine gömülür; aygıtlar "Virtual Speaker for RelAudio" / "Virtual Mic for
RelAudio" olarak görünür.

Kendi Windows sürücümüzü yazmak **tamamen masadan kalktı.** Lisans yolu aynı
sonucu, imzalama ve bakım yükü olmadan veriyor.

## Özet karar tablosu (v1)

| Platform | Sanal hoparlör | Sanal mikrofon | Sonraki adım |
|---|---|---|---|
| Linux | Uygulama yaratır (PipeWire node) | Uygulama yaratır (PipeWire node) | — zaten çözülmüş |
| Windows | Gerekmiyor (loopback) | VB-CABLE yönlendirmesi | Lisanslı, yeniden markalanmış sürücüyü pakete göm |
| ~~macOS~~ | *ertelendi* | *ertelendi* | Kapsama girerse kendi AudioServerPlugin'imiz |

Bu, "Receive mic input" özelliğinin Windows'ta v1'de **harici bir kuruluma
bağımlı** olması demektir. Kullanıcıya bu durum dürüstçe, kurulum
adımlarıyla birlikte gösterilir. Lisanslı sürücü devreye girdiğinde bu bağımlılık
Windows'ta ortadan kalkar.

## Uygulama içi rehber akışı

"Receive mic input" seçildiğinde:

1. Sistemde uygun bir sanal aygıt var mı diye bak (isim + yetenek eşleştirmesi).
2. Varsa doğrudan kullan, kullanıcıya "X aygıtı üzerinden çalışacak" de.
3. Yoksa platforma özel rehberi göster: ne kurulacak, nereden, kurulduktan
   sonra ne yapılacak. **Uygulama kullanıcı adına indirme/kurulum yapmaz.**
4. Kurulum sonrası "Yenile" ile aygıt listesi tazelenir.

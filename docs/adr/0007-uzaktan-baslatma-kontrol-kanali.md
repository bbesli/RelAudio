# 0007 — Kulaklık modu tek düğmeyle: kontrol kanalı ve 6 haneli eşleştirme

- **Durum:** Kabul edildi
- **Tarih:** 2026-09-09

## Bağlam

Kulaklık modu iki yarıdan oluşuyor ve ikisi de kurulmadan çalışmıyor: kulaklığın
takılı olduğu makine mikrofonunu gönderip geleni hoparlöründen çalıyor, karşı
makine geleni sanal kabloya yazıp kendi sistem sesini gönderiyor.

İlk sürümde kullanıcı **iki makinede ayrı ayrı** düğmeye basıyordu. Asıl kullanım
senaryosunda bu tuhaf: kullanıcı Windows makinesini Parsec üzerinden sürüyor,
kulaklık Linux'ta. Yani zaten iki ekran arasında gidip geliyor ve her oturumda
aynı iki tıklamayı tekrarlıyor.

İstenen: kulaklığı paylaşan taraf hedefi seçip **bir kez** bassın, karşı taraf
kendi yarısını kursun.

Bu bir kontrol kanalı gerektiriyor. `docs/05-ag-protokolu.md` TCP 59100'ü bunun
için zaten ayırmıştı ama hiçbir kısmı yazılmamıştı.

### Asıl gerilim

"Ağdaki bir makineye sesini yakalatıp gönderten uç" bir dinleme cihazı
ilkelidir. Kimlik doğrulaması olmadan açık bırakılırsa, aynı yerel ağdaki
herhangi biri uygulaman açıkken sesini kendi makinesine akıtabilir. Bu, evdeki
iki makinesi için özelliği açan kullanıcı için kabul edilebilir; otelde,
ofiste, kafede ağa giren kullanıcı için değil — ve ikincisi böyle bir uç
çalıştırdığını bilmiyor bile.

Üç bağımsız tasarım incelendi (asgari değişiklik / güvenlik / hata modları) ve
her biri çapraz eleştiriye sokuldu. Üçü de kimlik doğrulamasız bir ucun
varsayılan açık bırakılmasına karşı çıktı. İlk uygulama bu yüzden varsayılanı
kapalı tuttu; ardından eşleştirme eklenince kapalı tutmanın gerekçesi düştü.

## Karar

**Kontrol kanalı yazılıyor, kapsamı dar ve 6 haneli kodla eşleştirme zorunlu.**

### Kanal

- TCP 59100 (`docs/05` tablosundaki port), doluysa 59107'ye kadar deneniyor.
  Fiilen bağlanılan port mDNS TXT'de `cport` olarak ilan ediliyor.
- Satır ayrımlı JSON, bağlantı başına bir istek + bir cevap, sonra kapanış.
  `docs/05`'teki tam el sıkışma (HELLO/OFFER/ANSWER, format anlaşması)
  **yazılmadı**; yalnızca `start_headset`, `stop_headset`, `ping` ve `pair` var.
- Kalıcı kiralı bağlantı ve PING/PONG kasten **yok**: mesaj kimliği olmayan bir
  satır akışında iki yönlü trafik desenkronize oluyor. Yetim akış sorunu
  bunun yerine bir sayaç gözcüsüyle çözüldü.

### Eşleştirme

Uzaktan başlatma **yalnızca eşleşilmiş cihazlar** için açık:

1. Paylaşan makine düğmeye basınca, eşleşme yoksa ekranda **6 haneli bir kod**
   çıkıyor (3 dakika geçerli, 5 yanlış denemede yanıyor).
2. Kullanıcı kodu karşı makinede yazıyor; o makine kodu sahibine gönderiyor.
3. Kod tutarsa iki taraf 32 baytlık rastgele bir anahtarı paylaşıp diske
   yazıyor. Kod tükeniyor.
4. Bundan sonra her istek o anahtarla imzalanıyor (HMAC-SHA256, nonce + zaman
   damgası). Kod bir daha sorulmuyor — tek düğme.

Anahtar telde tekrarlanmıyor; imza rolü ve portu da kapsıyor, dolayısıyla
yakalanan bir "sistem sesi gönder" isteği "mikrofonu gönder"e çevrilemiyor.
Tekrar oynatma penceresi ±120 sn ve görülen nonce'lar hatırlanıyor.

### Korunan değişmezler

1. **Hedef adres telde yok.** Ses her zaman TCP bağlantısının kaynak IP'sine
   gidiyor. İsteyen taraf üçüncü bir makineyi hedef gösteremiyor — bu uç bir
   yansıtma aracı değil.
2. **Aygıtı isteyen seçemiyor.** Hangi mikrofon/hoparlör kullanılacağına
   yalnızca isteği alan makinenin kendi kayıtlı ayarları ve
   `audio::headset_plan` karar veriyor. Telde aygıt kimliği geçmiyor, aygıt
   listesi hiç dönmüyor.
3. **Kaynak yerel ağla sınırlı.** `accept()` anında, thread açılmadan önce:
   yalnızca loopback, RFC1918 ve link-local adresler. Port yönlendirmesiyle
   dışarıdan gelen bağlantı reddediliyor.
4. **Eşleşme zorunlu.** İmzasız ya da tanınmayan anahtarla imzalı istek
   reddediliyor. Eşleşmemiş bir makinenin elinde yalnızca bir ret kalıyor.
5. **Ret bir cevaptır**, düşürülmüş bağlantı değil — kullanıcı sebebi görüyor.
6. **Çalışan oturum çalınamaz.** Kullanıcının kendi kurduğu ya da başka bir
   eşin kurduğu oturumun üstüne yazılmıyor.

### Varsayılan: açık, ama yetkisiz

`remote_control = true`. Açık olması tek başına kimseye yetki vermiyor:
eşleşmemiş her istek reddediliyor. Bu, ilk tasarımdaki "varsayılan kapalı"
kararının yerini aldı — o karar, kimlik doğrulaması olmadığı için gerekliydi;
eşleştirme geldiğinde gerekçesi düştü ve kullanıcıya yüklediği bir kerelik
ayar bulma yükü de gereksizleşti.

Kullanıcı yine de kapatabiliyor. Kapalıyken sunucu **hiç bağlanmıyor**:
dinlenen port yok, güvenlik duvarı sorusu yok, mDNS'te `cport` ilan
edilmiyor — eşler bu makineyi "uzaktan başlatılamaz" görüp kullanıcıyı
düğmeye basmadan önce uyarıyor.

## Sonuçlar

**İyi:**
- Tek düğme çalışıyor; ikinci makineye dokunmak gerekmiyor.
- Simetrik: düğmeye hangi taraftan basılırsa basılsın karşı taraf karşıt rolü
  üstleniyor.
- Aygıt seçim politikası tek yerde (`audio::headset_plan`) — arayüz de,
  uzaktan gelen istek de aynı fonksiyona iniyor. Daha önce politika yalnızca
  `App.svelte` içindeydi ve uzak istek onu kullanamazdı.
- Ret sebepleri makine okunur kodlarla dönüyor (`code` alanı), arayüz kendi
  dilinde gösteriyor. Karşı makinenin dili bizimkiyle aynı olmak zorunda değil.

**Kötü / kabul edilen:**
- **Anahtar değişimi yok.** Eşleştirme anındaki tek alışverişi dinleyebilen
  biri paylaşılan anahtarı görür ve o eşleşmeyi devralabilir. Diffie-Hellman
  eklemek bunu kapatırdı; ses akışının kendisi de şifresiz olduğu için tek
  başına kontrol kanalını şifrelemek yanıltıcı bir güven verirdi. Koruma,
  ağa *bağlanabilen* birine karşı; trafiği *dinleyebilen* birine karşı değil.
- Kullanıcı bir kez 6 haneli kodu girmek zorunda.
- Eşleştirme kimliği mDNS örnek adı. Cihaz adı değişirse eşleşme kopar ve
  kod yeniden istenir; sessizce yanlış davranmıyor ama kullanıcı şaşırabilir.
- Yetim akış koruması sayaç tabanlı (12 sn paket gelmezse kapat), anlık değil.

## Değerlendirilen alternatifler

**Kimlik doğrulamasız bırakmak.** İlk tasarım buydu ve varsayılanı kapalı
tutmayı gerektiriyordu. Reddedildi: kullanıcı ayarı açtığı anda ağdaki herkes
o makineyi başlatabiliyordu ve bunu görünürlük dışında hiçbir şey
sınırlamıyordu.

**X25519 anahtar değişimi.** Dinleyen saldırganı da kapatırdı. Ertelendi: ses
akışı zaten şifresiz olduğu için tek başına kontrol kanalını korumak asimetrik
bir güvence olurdu. `hmac-sha256` + `getrandom` dışında bağımlılık eklenmedi;
ikisi de Tauri üzerinden zaten ağaçtaydı.

**Eşleştirme diyaloğunu istek üzerine açmak** (karşı makinede "X eşleşmek
istiyor" penceresi). Reddedildi: ağdaki herkese kurbanın ekranını öne
getirtme ilkeli verirdi. Kod, isteği **alan** değil **paylaşan** tarafta ve
yalnızca kullanıcı düğmeye bastığında çıkıyor.

**Kalıcı kiralı TCP bağlantısı.** Bağlantının kopması yetim akışı anında
kapatırdı. Reddedildi: mesaj kimliği olmayan satır akışında STOP ile keepalive
birbirine karışıyor. Sayaç gözcüsü aynı işi daha az parça ile yapıyor.

**mDNS TXT üzerinden komut.** Yeni port açmazdı. Reddedildi: TXT ilanı
gecikmeli ve idempotent değil; "başlat" gibi bir kenar tetikleyici için yanlış
taşıma.

# 05 — Ağ Protokolü

Protokol sıfırdan tasarlanır; AudioRelay ile uyumluluk hedefi yoktur.
Sürüm alanı ilk baytta taşınır, ileri uyumluluk için.

## Portlar

| Amaç | Taşıma | Varsayılan port |
|---|---|---|
| Keşif (mDNS) | UDP çoklu yayın | 5353 (standart) |
| Keşif (broadcast fallback) | UDP yayın | 59102 |
| Kontrol | TCP | 59100 |
| Ses verisi | UDP | 59101 |

Portlar çakışma hâlinde +1 artırılarak denenir; gerçek port mDNS kaydında ilan
edilir. Güvenlik duvarı istisnası kurulum sırasında istenir (Windows'ta ilk
çalıştırmada sistem sorar).

## 1. Keşif

**Birincil: mDNS / DNS-SD.**

- Servis tipi: `_relaudio._tcp.local`
- TXT kayıtları:
  ```
  v=1                     protokol sürümü
  name=BURAKBESLI         kullanıcıya gösterilen ad
  os=windows|macos|linux
  caps=send,recv,mic      desteklenen roller
  codecs=pcm16,opus
  cport=59100             kontrol portu
  aport=59101             ses portu
  id=<uuid>               kalıcı cihaz kimliği
  ```
- Rust tarafında `mdns-sd` veya `zeroconf` kullanılabilir. Windows'ta Bonjour
  kurulumuna bağımlı olmayan saf Rust implementasyonu tercih edilir.

**İkincil: UDP broadcast.** mDNS'in engellendiği ağlar için 59102'ye periyodik
(2 s) ilan; aynı TXT alanları JSON olarak.

**Üçüncül: manuel IP.** Ekran görüntüsündeki "Connect by address" alanı.
Hostname de kabul edilir.

Keşfedilen eşler 30 s TTL ile tutulur; ilan gelmezse `peer.lost` yayılır.

## 2. Kontrol kanalı (TCP)

Satır-ayrımlı JSON. Oturum kurma, aygıt/format anlaşması, canlı tutma, kapatma.

### El sıkışma

```
Player → Server   HELLO      { v, id, name, os }
Server → Player   HELLO_ACK  { v, id, name, caps, codecs, auth_required }

(auth_required ise)
Player → Server   AUTH       { pin }          ← kullanıcı Server ekranındaki PIN'i girer
Server → Player   AUTH_OK    { session_key }  ← veya AUTH_FAIL

Player → Server   OFFER      { mode: "audio"|"mic",
                               codecs: ["opus","pcm16"],
                               sample_rates: [48000, 44100],
                               channels: 2,
                               aport: 59101 }
Server → Player   ANSWER     { codec, sample_rate, channels,
                               packet_ms, ssrc, aport }
Server → Player   START
```

Anlaşma kuralı: **alıcı tercih sırasını verir, gönderici seçer.** Ortak nokta
yoksa `pcm16 / 48000 / 2` zorunlu tabandır — her uç bunu desteklemek zorundadır.

### Canlı tutma ve kapanış

- Her 2 saniyede `PING` / `PONG`. 3 kayıp PONG → oturum düşürülür.
- `BYE { reason }` düzgün kapanış.
- TCP koptuğunda UDP akışı da durdurulur (yetim akış bırakılmaz).

## 3. Ses taşıma (UDP)

RTP'ye benzer ama basitleştirilmiş, 12 baytlık sabit başlık:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Ver=1 | Codec |     Flags     |        Sequence (16)          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Timestamp (32, örnek cinsinden)           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          SSRC (32)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          Payload ...                          |
```

- **Ver** (4 bit): protokol sürümü.
- **Codec** (4 bit): `0=pcm_s16le`, `1=opus`, `2=flac` (v2).
- **Flags** (8 bit): bit0 = marker (akış başı/atlama sonrası), bit1 = sessizlik
  (payload boş, alıcı sessizlik üretir — bant genişliği tasarrufu), bit2 = şifreli.
- **Sequence:** 16 bit, sarmalanır; kayıp ve sıra bozukluğu tespiti.
- **Timestamp:** gönderici örnek sayacı; jitter ve drift hesabı için.
- **SSRC:** oturum kimliği; eski oturumun geciken paketlerini ayıklamak için.

Neden RTP'nin kendisi değil: RTP'nin tam uyumluluğu (RTCP, profil, uzantılar)
gereksiz karmaşıklık getirir. Ancak başlık RTP'ye kasten benzetildi ki
gerekirse geçiş kolay olsun.

### Paketleme kuralları

- Payload **her zaman** MTU'ya sığmalı; parçalanma (IP fragmentation) yasak.
  Hedef: toplam paket ≤ 1200 bayt (VPN/tünel payı bırakılır).
- PCM'de 5 ms (240 örnek × 2 kanal × 2 bayt = 960 B), Opus'ta 10 ms.
- Gönderici düzenli aralıkla yollar; burst yapmaz (pacing).

## 4. Jitter buffer

Alıcı tarafındaki en kritik bileşen.

**Yapı:** sıra numarasına göre indekslenen sabit boyutlu pencere.
Geç gelen paket, oynatma noktasını geçmediyse yerine konur; geçtiyse atılır.

**Adaptif hedef gecikme:**

```
hedef = clamp(ortalama_jitter × 3 + paket_süresi × 2,  min_ms,  max_ms)
```

- Profil "Düşük gecikme": min 10 ms, max 60 ms
- Profil "Dengeli": min 20 ms, max 120 ms
- Profil "Kararlı": min 50 ms, max 400 ms

Hedef büyürken hızlı (kayıp yaşandığında hemen), küçülürken yavaş (sessiz
anlarda kademeli) ayarlanır. Hedefe uyum, örnek atma/ekleme ile değil,
[03](03-ses-hatti.md)'teki adaptif resampler ile yapılır — böylece duyulmaz.

**Underrun:** buffer boşalırsa PLC devreye girer, sayaç artar, hedef büyütülür.
**Overrun:** buffer taşarsa en eski paketler atılır ve resampler hızlandırılır.

## 5. Güvenlik

Varsayılan olarak yalnızca LAN. Yine de:

- **Eşleştirme:** Server ekranında 6 haneli PIN gösterilir; ilk bağlantıda
  Player girer. Başarılı eşleşme cihaz kimliğiyle birlikte saklanır, sonraki
  bağlantılarda PIN sorulmaz.
- **Şifreleme (opsiyonel, varsayılan açık):** PIN'den türetilen anahtarla
  (Noise_NK veya basit X25519 + HKDF) oturum anahtarı; ses paketleri
  ChaCha20-Poly1305 ile şifrelenir. Her paket bağımsız şifrelenir (kayıp
  toleransı için), nonce = SSRC + sequence.
- Şifreleme kapatılabilir (CPU tasarrufu için); kapatıldığında UI açıkça uyarır.
- Kontrol kanalı **hiçbir zaman** kod/komut yürütmez; yalnızca sabit bir şema
  ile doğrulanan mesajlar kabul edilir. Bilinmeyen alanlar yok sayılır,
  bilinmeyen mesaj tipi bağlantıyı kapatır.
- Bind adresi varsayılan olarak yalnızca özel ağ arayüzleri; kullanıcı isterse
  `0.0.0.0` seçebilir ve bu açıkça uyarılır.

## 6. Çoklu alıcı (v2)

Protokol SSRC ve ayrı UDP hedefleriyle çoklu alıcıya izin verir. Gönderici tek
kodlama yapıp N hedefe unicast gönderir (çoklu yayın/multicast Wi-Fi'da
güvenilmez olduğu için tercih edilmez). v1'de UI bunu tek eşe kilitler.

## 7. Hata durumları ve UI'a yansıması

| Durum | Tespit | UI |
|---|---|---|
| Eş bulunamadı | mDNS boş | "Ağda cihaz yok" + manuel IP önerisi |
| Bağlantı reddedildi | TCP RST / AUTH_FAIL | "PIN yanlış" veya "Karşı taraf reddetti" |
| Ağ yavaş | Kayıp > %2 veya jitter > 30 ms | `degraded` rozeti + "Kodek/profil değiştir" önerisi |
| Sürüm uyuşmazlığı | HELLO_ACK'te v farkı | "Diğer cihazı güncelleyin" |
| Ses aygıtı kayboldu | Platform olayı | "Aygıt çıkarıldı", varsayılana geçiş |

# RelAudio

Yerel ağ (LAN) üzerinden bilgisayarlar arasında **düşük gecikmeli ses aktarımı** yapan
bir masaüstü uygulaması. Hedef platformlar: **Windows ve Linux.**
macOS ertelendi (bkz. [docs/00-genel-bakis.md](docs/00-genel-bakis.md)).

> **Durum: Uygulama çalışıyor.** Arayüz (Tauri + Svelte) ve çekirdek hazır;
> iki makine arasında sistem sesi ve mikrofon aktarılabiliyor, tepsi/arka plan
> davranışı yerinde.
> Kurulum ve kullanım: [docs/11-calistirma.md](docs/11-calistirma.md).
> Ölçüm sonuçları ve bilinen sınırlar: [docs/10-riskler.md](docs/10-riskler.md).

---

## Ne yapacak?

İki temel rol var; her cihaz aynı anda ikisini birden üstlenebilir:

| Rol | Açıklama |
|---|---|
| **Server (gönderici)** | Bu bilgisayarda çalan sesi (system audio / loopback) veya bir mikrofonun girişini ağdaki başka cihaza yollar. |
| **Player (alıcı)** | Ağdaki başka bir cihazın sesini bu bilgisayarda çalar veya o cihazı sanal mikrofon olarak sisteme tanıtır. |

Dört senaryo:

1. **Send audio** — PC'nin sesini başka cihazın hoparlöründen dinle.
2. **Send mic input** — PC'ye bağlı mikrofonu ağa yolla.
3. **Receive audio** — Başka cihazın sesini bu PC'de çal.
4. **Receive mic input** — Başka cihazı bu PC'de sanal mikrofon olarak kullan.

Ek gereksinimler:

- **Arka planda çalışma.** Pencere kapatılınca uygulama sonlanmaz; sistem tepsisi /
  menü çubuğu simgesinden geri açılır. (Discord davranışı.)
- **Otomatik keşif.** Aynı ağdaki cihazlar listede kendiliğinden görünür; IP ile
  manuel bağlanma da mümkün.
- **Düşük gecikme.** Hedef uçtan uca 40–80 ms (LAN, kablolu/iyi Wi-Fi).

---

## Kısa cevap: Electron ile yazılır mı?

**Kısmen.** Arayüzü Electron ile yazabilirsin, ama **ses motorunu JavaScript'te
yazamazsın.** Node.js'in ses donanımına erişimi yok; Web Audio API ise sistem sesini
(loopback) güvenilir biçimde yakalayamaz ve GC duraklamaları nedeniyle gerçek zamanlı
ses için uygun değil.

Dolayısıyla mimari her hâlükârda iki katmanlı olmak zorunda:

```
[ Arayüz: web teknolojileri ]  ←→  [ Ses + ağ motoru: native kod (Rust/C++) ]
                                          ↓
                        [ Sanal ses aygıtı — Linux: kendi PipeWire node'umuz,
                          Windows: üçüncü taraf sürücü ]
```

**Önerilen yığın: Tauri v2 + Rust çekirdek.** Gerekçe ve alternatiflerin karşılaştırması
[docs/01-teknoloji-secimi.md](docs/01-teknoloji-secimi.md) içinde. Electron + Rust
sidecar da geçerli bir alternatif; ekip JS ağırlıklıysa o tercih edilebilir.

---

## Dokümantasyon

| Doküman | İçerik |
|---|---|
| [00-genel-bakis.md](docs/00-genel-bakis.md) | Ürün kapsamı, kullanıcı senaryoları, kapsam dışı bırakılanlar |
| [01-teknoloji-secimi.md](docs/01-teknoloji-secimi.md) | Electron / Tauri / Qt / Flutter karşılaştırması ve karar |
| [02-mimari.md](docs/02-mimari.md) | Süreç modeli, katmanlar, IPC, modül sınırları |
| [03-ses-hatti.md](docs/03-ses-hatti.md) | Yakalama ve çalma API'leri, platform bazında; gecikme bütçesi |
| [04-sanal-ses-aygitlari.md](docs/04-sanal-ses-aygitlari.md) | Sanal sürücü ihtiyacı, üçüncü taraf lisanslama, imzalama gerçekleri |
| [05-ag-protokolu.md](docs/05-ag-protokolu.md) | Keşif, kontrol kanalı, ses taşıma, jitter buffer, saat kayması |
| [06-arkaplan-ve-tepsi.md](docs/06-arkaplan-ve-tepsi.md) | **Tray / arka plan davranışı — platform bazında, kod örnekleriyle** |
| [07-arayuz.md](docs/07-arayuz.md) | Ekran yapısı, durum modeli, tasarım notları |
| [08-derleme-ve-paketleme.md](docs/08-derleme-ve-paketleme.md) | Build, imzalama, notarization, dağıtım, güncelleme |
| [09-yol-haritasi.md](docs/09-yol-haritasi.md) | Fazlar, her fazın çıktısı ve kabul kriterleri |
| [10-riskler.md](docs/10-riskler.md) | Riskler, **ölçüm sonuçları**, karar bekleyen konular |
| [11-calistirma.md](docs/11-calistirma.md) | **Kurulum, derleme, kullanım, sağlıklı çıktı nasıl görünür** |
| [adr/](docs/adr/) | Mimari karar kayıtları (ADR) |

---

## Lisans ve isimlendirme notu

"AudioRelay" üçüncü taraf bir ürünün adıdır. Bu proje bağımsızdır; ismi, ikonu veya
protokolü ile uyumluluk iddiası taşımaz. Protokol sıfırdan tasarlanır.

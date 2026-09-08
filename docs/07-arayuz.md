# 07 — Arayüz

Referans ürünün düzeni sade ve işe yarıyor; benzer bir iskelet benimsenir.

## Ekran yapısı

```
┌────────────┬──────────────────────────────────┬────────────────────┐
│ Sunucu     │  Rol kartı (açıklama + modlar)   │  Bu cihaz          │
│ Oynatıcı   │                                  │  ad + IP           │
│ Ayarlar    │  ┌────────────────────────────┐  ├────────────────────┤
│            │  │ Bağlantılar / Sunucular    │  │  Ses aygıtı        │
│            │  │  • BURAKBESLI  192.168...  │  │  seçim + yenile    │
│            │  │  • ...                     │  ├────────────────────┤
│            │  └────────────────────────────┘  │  İstatistikler     │
│            │                                  │  gecikme, kayıp    │
└────────────┴──────────────────────────────────┴────────────────────┘
```

Üç sekme:

**Sunucu** — Bu cihazdan ses gönderme. İki mod (sistem sesi / mikrofon),
bağlı istemciler listesi, PIN gösterimi.

**Oynatıcı** — Başka cihazdan ses alma. İki mod (ses / mikrofon), keşfedilen
sunucular listesi, "IP ile bağlan" alanı.

**Ayarlar** — Cihaz adı, tema, dil, gecikme profili, kodek, otomatik başlatma,
"kapatınca tepsiye küçült", güvenlik, loglar.

Sağ sütun bağlama duyarlı: seçili sekmeye göre giriş veya çıkış aygıtını gösterir,
altında canlı istatistikler.

## Durum modeli

Tüm gerçek durum çekirdektedir. Arayüz **onu yansıtır, sahiplenmez.** UI yeniden
başlatıldığında `hello` + `stats.subscribe` ile mevcut duruma yeniden bağlanır.

```
idle ──► discovering ──► connecting ──► streaming
                              │              │
                              │              ├──► degraded ──► streaming
                              ▼              ▼
                            error ◄──────── stopped
```

Her durum için UI'da tek bir baskın gösterge olmalı: renkli nokta + tek satır
metin. Kullanıcı "çalışıyor mu?" sorusunu bir bakışta yanıtlayabilmeli.

## Gösterilecek istatistikler

| Metrik | Nerede | Not |
|---|---|---|
| Yakalama gecikmesi | Sağ sütun | Referans ürün "6 ms" gösteriyor |
| Uçtan uca gecikme (tahmini) | Sağ sütun | Yakalama + buffer + çalma |
| Paket kaybı % | Sağ sütun | > %1 sarı, > %5 kırmızı |
| Bitrate | Sağ sütun | Anlık |
| Buffer doluluk | Ayrıntı görünümü | Geliştirici modunda grafik |
| Underrun sayacı | Ayrıntı görünümü | 0 olmalı |

Güncelleme sıklığı 500 ms; UI'ı gereksiz render etmemek için throttle edilir.

## Tasarım notları

- **Tema:** karanlık varsayılan, aydınlık seçenek, "sistemi takip et" üçüncü
  seçenek. CSS değişkenleriyle; her renk `:root` üzerinde tanımlı olmalı.
- **Erişilebilirlik:** durum yalnızca renkle anlatılmaz (ikon + metin de var).
  Klavye ile tam gezinme. Odak halkaları görünür.
- **Dil:** TR/EN, i18n altyapısı baştan kurulur (sonradan eklemek pahalı).
  Tepsi menüsü de çevrilir.
- **WebKitGTK uyumu:** ağır CSS özelliklerinden (backdrop-filter, karmaşık
  grid subgrid, `:has()` zincirleri) kaçın; üç platformda görsel test yap.
- **Pencere boyutu:** minimum 900×600. Pencere konumu/boyutu kalıcı
  (`window-state` eklentisi).

## İlk çalıştırma akışı

1. Cihaz adı önerilir (hostname), değiştirilebilir.
2. Ağda cihaz aranır; bulunursa doğrudan listelenir.
3. Kullanılacak rol sorulmaz — kullanıcı sekmelerden seçer.
4. Güvenlik duvarı izni gerekiyorsa açıklama gösterilir.
5. Tepsi davranışı ilk çarpıda anlatılır (bkz. [06](06-arkaplan-ve-tepsi.md)).

## Hata mesajları

Her hata için üç parça: **ne oldu**, **neden**, **ne yapmalı**. Teknik ayrıntı
katlanabilir bir alanda. Örnek:

> **Bağlantı kurulamadı**
> BURAKBESLI (192.168.1.103) yanıt vermedi.
> Karşı cihazda RelAudio açık mı? Aynı ağda mısınız?
> <sub>Ayrıntı: TCP connect timeout after 5000 ms — 59100</sub>

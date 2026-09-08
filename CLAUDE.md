# CLAUDE.md

Bu dosya, bu depoda çalışan Claude Code oturumları için kalıcı bağlamdır.

## Projenin durumu

**Çalışan uygulama.** Arayüz, çekirdek, keşif, tepsi ve i18n yerinde.
Açık kaynak yayımlandı: https://github.com/bbesli/RelAudio (MIT).

`spike/` altındaki Adım 1 doğrulama kodu **atılacak** — kalıcı mimariye örnek
alınmamalı. Ölçüm sonuçları `docs/10-riskler.md` içinde.

## Proje nedir

RelAudio — LAN üzerinden düşük gecikmeli ses aktarımı yapan masaüstü uygulaması.
Hedef platformlar: **Windows ve Linux.** macOS bilinçli olarak ertelendi —
dokümanlardaki macOS bölümleri korunuyor ama kapsam dışı; macOS'a iş yapma,
sorulmadıkça önerme. Referans ürün: AudioRelay.

**Hedef: açık kaynak, MIT.** Katkı kabul ediliyor.

Hâlâ kapsam dışı olanlar: kod imzalama, otomatik güncelleme, VB-CABLE'ı kurulum
paketine gömme (kullanıcı kendisi kuruyor — ADR-0003), macOS (ADR yok, docs/00).
Bunlar istenirse tartışılabilir ama varsayılan kapsam bu değil.

i18n **yapıldı**: 10 dil, `app/src/lib/i18n.ts`. Yeni dil eklerken
`node scripts/check-i18n.mjs` eksik anahtarı yakalıyor — CI'da çalıştırılmalı.

Test makineleri: Linux `192.168.1.113` (bu makine), Windows `192.168.1.103`.
İki rol: **Server** (ses gönderir) ve **Player** (ses alır). Her cihaz ikisini
aynı anda üstlenebilir.

Ek zorunlu davranış: pencere kapatılınca uygulama sonlanmaz, sistem tepsisine iner,
tepsi simgesinden geri açılır (Discord davranışı).

## Seçilen mimari (özet)

Detay: `docs/02-mimari.md`, karar gerekçesi: `docs/01-teknoloji-secimi.md`.

```
app/                     Tauri v2 + Svelte 5      Arayüz, tepsi, ayarlar, i18n
crates/relaudio-core     Rust kütüphane           Ses yakalama, çalma, ağ, keşif, config
crates/relaudio-proto    Rust kütüphane           Ağ protokolü tipleri
crates/relaudio-cli      Rust binary `relaudio`   Teşhis: devices, tone, level, send, recv
spike/                   Atılacak ölçüm kodu (Adım 1)
scripts/                 Kurulum, Windows derleme, i18n denetimi

Çekirdek **ayrı süreç değil**, Tauri'ye kütüphane olarak bağlı (ADR-0005).
```

Kritik kısıt: **ses motoru JavaScript'te yazılamaz.** Node'un ses donanımına
erişimi yok, Web Audio loopback yakalayamaz, GC gerçek zamanlı ses için uygunsuz.
Arayüz web teknolojisi olabilir, motor native olmak zorunda.

İkincil kritik kısıt: **ses motoru UI sürecinden ayrı bir süreçte çalışmalı.**
Arayüz kapandığında/çöktüğünde yayın kesilmemeli.

## Doküman haritası

| Dosya | Ne zaman bak |
|---|---|
| `docs/00-genel-bakis.md` | Kapsam sorusu, "bu özellik dahil mi?" |
| `docs/01-teknoloji-secimi.md` | Framework / dil kararı tartışılıyorsa |
| `docs/02-mimari.md` | Süreç sınırları, IPC, modül yerleşimi |
| `docs/03-ses-hatti.md` | Ses API'leri, gecikme, format, resampling |
| `docs/04-sanal-ses-aygitlari.md` | Sanal mikrofon / sanal hoparlör konuları |
| `docs/05-ag-protokolu.md` | Paket formatı, keşif, jitter buffer, saat kayması |
| `docs/06-arkaplan-ve-tepsi.md` | Tray, arka plan, otomatik başlatma |
| `docs/07-arayuz.md` | Ekran/akış tasarımı |
| `docs/08-derleme-ve-paketleme.md` | Build, imzalama, dağıtım |
| `docs/09-yol-haritasi.md` | Sıradaki iş ne? |
| `docs/10-riskler.md` | Açık sorular, karar bekleyenler |
| `docs/adr/` | Verilmiş mimari kararların gerekçeleri |

## Çalışma kuralları

- **Dil:** Kullanıcı Türkçe yazıyor. `docs/` ve kod yorumları Türkçe.
  Tanımlayıcılar ve commit mesajları İngilizce/Türkçe karışabilir.
  **Kullanıcıya görünen metinler:** arayüzde i18n üzerinden (10 dil),
  çekirdek hata mesajlarında İngilizce — Türkçe hata İspanyolca arayüzde
  tuhaf kaçıyordu.
- **README İngilizce** (`README.md`); Türkçesi `README.tr.md`.
- **Doküman biçimi:** Dosya adları `NN-konu.md` (Türkçe, ASCII, tireli). Başlıklarda
  ASCII olmayan karakter serbest, dosya adlarında değil.
- **Yeni karar verildiğinde** `docs/adr/` altına ADR ekle ve ilgili dokümanı güncelle.
  ADR formatı: `NNNN-baslik.md`, içinde Bağlam / Karar / Sonuçlar / Alternatifler.
- **Platform iddialarını doğrula.** "macOS'ta şu API var" gibi bir şey yazmadan önce
  hangi sürümden itibaren geçerli olduğunu belirt. Minimum hedef sürümler
  `docs/00-genel-bakis.md` içinde.
- Kullanıcı geliştirme ortamı: CachyOS (Arch tabanlı) Linux, PipeWire. Linux tarafı
  birincil geliştirme platformu; Windows test hedefi.

## Derleme ve denetim

```bash
cargo test                                    # çekirdek
cd app/src-tauri && cargo test                # oturum katmanı
rustup target add x86_64-pc-windows-msvc      # bir kez
cargo check --target x86_64-pc-windows-msvc   # Windows kodu Linux'ta cfg ile dışlanıyor
node scripts/check-i18n.mjs                   # çeviri bütünlüğü
```

Sondan ikincisi önemli: `audio/windows.rs` normal Linux derlemesinde hiç
kontrol edilmiyor.

## Bilinmeyenler

`docs/10-riskler.md` içindeki "Karar bekleyenler" bölümü canlıdır. Oradaki bir soru
konuşmada cevaplanırsa, dosyayı güncelle ve gerekiyorsa ADR yaz.

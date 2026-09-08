# CLAUDE.md

Bu dosya, bu depoda çalışan Claude Code oturumları için kalıcı bağlamdır.

## Projenin durumu

**Implementasyon başladı.** Plan onaylandı: `~/.claude/plans/shimmying-jingling-beaver.md`

Sıra: Ortam → doküman güncellemeleri → **Adım 1 spike + ölçüm (durak)** → çekirdek →
ağ → arayüz → tepsi.

Adım 1'in sonunda ölçüm sonuçları değerlendirilecek; kötü çıkarsa plan gözden geçirilir.
Spike kodu `spike/` altında ve **atılacak** — kalıcı mimariye örnek alınmamalı.

## Proje nedir

RelAudio — LAN üzerinden düşük gecikmeli ses aktarımı yapan masaüstü uygulaması.
Hedef platformlar: **Windows ve Linux.** macOS bilinçli olarak ertelendi —
dokümanlardaki macOS bölümleri korunuyor ama kapsam dışı; macOS'a iş yapma,
sorulmadıkça önerme. Referans ürün: AudioRelay.

**Hedef: kişisel kullanım.** Yayımlanacak bir ürün değil. Paketleme (NSIS/AppImage),
kod imzalama, updater, i18n, onboarding akışı ve VB-CABLE'ı pakete gömme
**kapsam dışı**. Bunları planlama veya önerme; kullanıcı yayımlamaya karar verirse
geri gelirler.

Test makineleri: Linux `192.168.1.113` (bu makine), Windows `192.168.1.103`.
İki rol: **Server** (ses gönderir) ve **Player** (ses alır). Her cihaz ikisini
aynı anda üstlenebilir.

Ek zorunlu davranış: pencere kapatılınca uygulama sonlanmaz, sistem tepsisine iner,
tepsi simgesinden geri açılır (Discord davranışı).

## Seçilen mimari (özet)

Detay: `docs/02-mimari.md`, karar gerekçesi: `docs/01-teknoloji-secimi.md`.

```
app/              Tauri v2 + Svelte + TypeScript   Arayüz, tepsi, ayarlar
crates/relaudio-core     Rust kütüphane            Ses yakalama, çalma, ağ, keşif
crates/relaudio-coremon  Rust binary (ayrı süreç)  Çekirdeği çalıştırır
crates/relaudio-proto    Rust kütüphane            IPC + ağ protokolü tipleri
spike/            Atılacak ölçüm kodu (Adım 1)
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

- **Dil:** Kullanıcı Türkçe yazıyor. Dokümanlar ve açıklamalar Türkçe. Kod, kod
  yorumları, commit mesajları, tanımlayıcılar İngilizce olacak.
- **Doküman biçimi:** Dosya adları `NN-konu.md` (Türkçe, ASCII, tireli). Başlıklarda
  ASCII olmayan karakter serbest, dosya adlarında değil.
- **Yeni karar verildiğinde** `docs/adr/` altına ADR ekle ve ilgili dokümanı güncelle.
  ADR formatı: `NNNN-baslik.md`, içinde Bağlam / Karar / Sonuçlar / Alternatifler.
- **Platform iddialarını doğrula.** "macOS'ta şu API var" gibi bir şey yazmadan önce
  hangi sürümden itibaren geçerli olduğunu belirt. Minimum hedef sürümler
  `docs/00-genel-bakis.md` içinde.
- Kullanıcı geliştirme ortamı: CachyOS (Arch tabanlı) Linux, PipeWire. Linux tarafı
  birincil geliştirme platformu; Windows test hedefi.

## Bilinmeyenler

`docs/10-riskler.md` içindeki "Karar bekleyenler" bölümü canlıdır. Oradaki bir soru
konuşmada cevaplanırsa, dosyayı güncelle ve gerekiyorsa ADR yaz.

# Mimari Karar Kayıtları (ADR)

Her önemli teknik karar buraya kaydedilir. Amaç, altı ay sonra "bunu neden
böyle yapmıştık?" sorusunu cevaplayabilmek.

**Dosya adı:** `NNNN-kisa-baslik.md` (sıralı numara, ASCII, tireli)

**Şablon:**

```markdown
# NNNN — Başlık

- **Durum:** Önerildi | Kabul edildi | Reddedildi | Değiştirildi (→ NNNN)
- **Tarih:** YYYY-MM-DD

## Bağlam
Hangi problem, hangi kısıtlar altında?

## Karar
Ne yapmaya karar verildi?

## Sonuçlar
Bu karar neyi kolaylaştırıyor, neyi zorlaştırıyor? Hangi maliyet kabul edildi?

## Değerlendirilen alternatifler
Neden diğerleri seçilmedi?
```

## Kayıtlar

| # | Başlık | Durum |
|---|---|---|
| [0001](0001-tauri-ve-rust-cekirdek.md) | Tauri v2 arayüz + Rust çekirdek | Kabul edildi |
| [0002](0002-ayri-cekirdek-sureci.md) | Ses motoru ayrı süreçte çalışır | Kabul edildi |
| [0003](0003-v1de-kendi-surucumuz-yok.md) | Kendi sürücümüzü yazmıyoruz, üçüncü tarafı lisanslıyoruz | Kabul edildi |
| [0004](0004-linux-pulse-api.md) | Linux'ta PipeWire API'si değil, PulseAudio API'si | Kabul edildi |
| [0005](0005-cekirdek-tauri-icinde.md) | Çekirdek ayrı süreç değil, Tauri sürecinde | Kabul edildi |
| [0006](0006-mit-lisansi.md) | MIT lisansı | Kabul edildi |
| [0007](0007-uzaktan-baslatma-kontrol-kanali.md) | Tek düğmeyle iki taraf: kontrol kanalı, varsayılan kapalı | Kabul edildi |

# 0005 — Çekirdek ayrı süreç değil, Tauri sürecinde

- **Durum:** Kabul edildi
- **Tarih:** 2026-09-08
- **Değiştirir:** [0002](0002-ayri-cekirdek-sureci.md) (kısmen)

## Bağlam

[ADR-0002](0002-ayri-cekirdek-sureci.md) ses motorunu ayrı bir işletim sistemi
sürecinde çalıştırmayı, aralarında JSON-RPC IPC kurmayı öngörüyordu. Gerekçesi
üç maddeydi:

1. Pencere kapatıldığında ses devam etmeli
2. Arayüz çökerse ses kesilmemeli
3. Ses thread'i UI yükünden yalıtık olmalı

Bu arada iki şey değişti:

- **Kapsam kişisel kullanıma indi** (docs/00). IPC katmanı, süreç yaşam
  döngüsü, sürüm uyuşmazlığı, zombi temizliği — hepsi yazılacak ve bakılacak kod.
- **Tepsi davranışı zaten süreci ayakta tutuyor.** Pencere kapandığında süreç
  ölmüyor, sadece pencere gizleniyor (docs/06). Yani 1. madde ayrı süreç
  olmadan da sağlanıyor.

## Karar

`relaudio-core` doğrudan Tauri sürecine kütüphane olarak bağlanır. Ses
oturumları ayrı **thread**'lerde çalışır (`app/src-tauri/src/session.rs`),
ayrı süreçte değil.

## Sonuçlar

**Kolaylaştırdıkları:**
- IPC katmanı, süreç yönetimi ve sürümleme tamamen ortadan kalktı.
- Hata yolu tek: `Result` doğrudan komut dönüşünde UI'a ulaşıyor.
- Tek binary, tek derleme, tek dağıtım.

**Kaybedilenler — dürüstçe:**
- **Arayüz çökerse ses de kesilir.** ADR-0002'nin 2. maddesi karşılanmıyor.
  Kişisel kullanımda kabul edilebilir bir risk; yayımlanacak bir üründe olmazdı.
- Çekirdeği CLI/servis olarak yeniden kullanma esnekliği azaldı —
  ama `relaudio-cli` zaten aynı kütüphaneyi kullanıyor, o yol açık kaldı.

**Hâlâ korunanlar:**
- Ses thread'leri UI thread'inden ayrı; render/GC yükü ses yolunu etkilemiyor.
- Pencere kapandığında yayın sürüyor (tepsi süreci ayakta tutuyor).
- `relaudio-core` arayüzden bağımsız bir kütüphane olarak duruyor; ayrı sürece
  taşımak istenirse `session.rs` değişir, çekirdek değişmez.

## Değerlendirilen alternatifler

- **ADR-0002'yi olduğu gibi uygulamak:** Reddedildi. Kişisel kullanım kapsamında
  maliyeti faydasından büyük.
- **Yayımlamaya karar verilirse:** Bu karar yeniden değerlendirilmeli. Çekirdek
  zaten ayrı crate olduğu için taşıma maliyeti sınırlı.

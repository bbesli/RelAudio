# 0006 — MIT lisansı

- **Durum:** Kabul edildi
- **Tarih:** 2026-09-09

## Bağlam

Proje açık kaynak yayımlanacak. `docs/10`'da lisans açık soru olarak duruyordu
ve iki şeyi etkiliyordu: katkı kabul etme biçimi ve üçüncü taraf bileşenlerle
ilişki.

## Karar

MIT.

## Sonuçlar

- **Katkı kolay.** Bu ölçekteki araçlar için fiili standart; katkıda bulunacak
  kişi lisans metnini okumak zorunda kalmıyor.
- **Sanal sürücü bileşenleriyle sorun yok.** VB-CABLE'ı paketlemiyoruz,
  kullanıcı kendisi kuruyor (ADR-0003); MIT bu ilişkiyi kısıtlamıyor.
- **Kaybedilen:** Türev çalışmaların açık kalması garanti değil. Kod kapalı
  bir üründe kullanılabilir. Bu tür bir yardımcı araç için kabul edilebilir.

## Değerlendirilen alternatifler

- **Apache-2.0:** Patent maddesi ve değişiklik bildirimi ekliyor. Kurumsal
  kullanıcılar için değerli ama bu proje bireysel kullanıma yönelik.
- **GPL-3.0:** Türevleri açık tutar ama katkı ve kullanım sürtünmesini artırır.

# 0004 — Linux'ta PipeWire API'si değil, PulseAudio API'si

- **Durum:** Kabul edildi
- **Tarih:** 2026-09-08

## Bağlam

`docs/02-mimari.md` başlangıçta Linux arka ucu için `pipewire-rs` öngörüyordu.
Adım 1 spike'ı bu varsayımı sarstı (docs/10, Bulgu 5):

| Araç | Monitor kaynağından yakalama |
|---|---|
| `pw-record --target <sink>.monitor` | **tepe 0 — sessizlik** |
| `parecord --device=<sink>.monitor` | tepe 27655 ✓ |

PipeWire 1.6.8'de `pw-cat` ailesi aygıt adını çözemedi ve **hata vermeden
sessizce sessizlik yakaladı.** Aynı sunucuya karşı Pulse istemcileri kusursuz
çalıştı. Bu, spike aşamasında birkaç saat kaybettiren tek sorundu.

## Karar

Linux ses arka ucu `libpulse-binding` + `libpulse-simple-binding` kullanır.

- Aygıt listeleme: Pulse introspection (`get_sink_info_list`,
  `get_source_info_list`, `get_server_info`)
- Yakalama/çalma: `libpulse_simple_binding::Simple` (blocking API)

## Sonuçlar

**Kolaylaştırdıkları:**
- PipeWire, PulseAudio API'sini yerel olarak sağlıyor — PipeWire kullanan
  sistemlerde de, saf PulseAudio kullanan sistemlerde de tek kod yolu çalışır.
  Kapsamımız genişledi, daralmadı.
- Blocking `Simple` API'si thread-per-stream tasarımımıza doğrudan oturuyor;
  callback/mainloop karmaşıklığı yok.
- Monitor kaynakları ve mikrofonlar aynı API'den, aynı biçimde geliyor.
  `monitor_of_sink` alanı ikisini ayırt etmeye yetiyor.

**Zorlaştırdıkları:**
- Kuantum/gecikme üzerinde PipeWire'ın yerel API'sindeki kadar ince kontrol yok.
  Pratikte kayıp değil: spike'ta **istemci latency isteği zaten yok sayılıyordu**
  (docs/10, Bulgu 3); yalnızca global `clock.force-quantum` işe yaradı.
- PipeWire'a özgü özellikler (node özellikleri, `node.always-process`) doğrudan
  erişilebilir değil. Sanal aygıt yaratma (docs/04) gerektiğinde bu konu
  yeniden açılacak.

**Geri döndürülebilirlik:** Yüksek. `audio::Capture` / `audio::Playback`
trait'leri arkasında; `linux.rs` değiştirilir, üst katmanlar etkilenmez.

## Değerlendirilen alternatifler

- **`pipewire-rs`:** Reddedildi. Daha karmaşık, ve spike'ta güvenilirlik sorunu
  yaşadığımız yol bu. İhtiyaç doğarsa trait arkasında geri gelebilir.
- **`cpal`:** Cross-platform tek API çekiciydi ama Linux'ta ALSA üzerinden
  gidiyor (paylaşımlı erişim sorunları) ve **Windows'ta loopback desteklemiyor** —
  bizim ana senaryomuz o.

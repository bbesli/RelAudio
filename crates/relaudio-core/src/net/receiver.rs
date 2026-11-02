//! Alıcı: UDP → jitter buffer → çalma.
//!
//! İki thread: soket okuyan ve çalan. Aralarında Mutex'li jitter buffer var.
//! Ses geri çağrımı burada blocking Pulse/WASAPI yazımı olduğu için gerçek
//! zamanlı callback disiplini (docs/02) bu tasarımda gerekmiyor — kilit
//! yalnızca kısa bir kuyruk işlemi için tutuluyor.

use relaudio_proto::{PacketHeader, HEADER_LEN, MAX_PACKET_LEN, PCM_PAYLOAD_LEN};
use std::net::{IpAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{JitterBuffer, Stopper};
use crate::audio;
use crate::error::Result;

#[derive(Default)]
pub struct ReceiverStats {
    pub packets: AtomicU64,
    pub bytes: AtomicU64,
    pub lost: AtomicU64,
    pub late: AtomicU64,
    pub underruns: AtomicU64,
    pub buffer_depth: AtomicU64,
    /// Tampon tavanı aşıldığı için atılanlar (gecikme birikimi göstergesi).
    pub dropped: AtomicU64,
    /// Çalınan sesin son tepe değeri (0–32767). Arayüzdeki seviye çubuğu için:
    /// "paket geliyor ama ses sessiz" durumunu paket sayacı gösteremiyor.
    pub peak: AtomicU64,
    /// Oturumun karşı ucundan gelmediği için atılan paketler. Sıfırdan
    /// büyükse ya biri araya girmeye çalışıyor ya da karşı taraf beklenenden
    /// farklı bir arayüzden gönderiyor — iki durum da görünür olmalı.
    pub foreign: AtomicU64,
}

/// `port`'u dinler ve `device_id` aygıtında çalar.
/// `target_packets` jitter buffer hedefi (paket sayısı; 1 paket = 5 ms).
///
/// `allow_from` boş değilse yalnızca o adreslerden gelen paketler kabul
/// ediliyor. Birden fazla adres olabilir: aynı ağa hem kablolu hem Wi-Fi bağlı
/// bir eşin paketleri ilan ettiği adreslerin herhangi birinden gelebilir.
///
/// Bu kilit olmadan ağdaki herkes bu porta paket atabiliyor ve yeni bir SSRC
/// görülünce oturum ona devrediliyordu. Kulaklık modunda alınan ses sanal
/// kabloya — yani toplantı uygulamasının **mikrofonuna** — yazıldığı için bu,
/// yabancı birinin toplantıda senin yerine konuşabilmesi demekti. Eşleştirme
/// yalnızca kontrol kanalını koruyordu, ses yolunu değil.
///
/// Sınır: UDP kaynak adresi yerel ağda sahtelenebilir. Bu kilit, ağa paket
/// atabilen herkese karşı korur; kaynak adres sahteleyebilen birine karşı
/// paket başına imza gerekir (yok).
pub fn receive_loop(
    port: u16,
    device_id: &str,
    target_packets: usize,
    allow_from: Vec<IpAddr>,
    stop: Stopper,
    stats: Arc<ReceiverStats>,
) -> Result<()> {
    let sock = UdpSocket::bind(("0.0.0.0", port))?;
    sock.set_read_timeout(Some(Duration::from_millis(200)))?;
    if allow_from.is_empty() {
        log::info!("dinleniyor: 0.0.0.0:{port}");
    } else {
        log::info!("dinleniyor: 0.0.0.0:{port} — yalnızca {allow_from:?} kabul ediliyor");
    }

    let mut out = audio::open_playback(device_id)?;
    let jitter = Arc::new(Mutex::new(JitterBuffer::new(target_packets)));

    // Ağ thread'i
    let net_jitter = jitter.clone();
    let net_stop = stop.clone();
    let net_stats = stats.clone();
    let net = std::thread::Builder::new()
        .name("relaudio-net".into())
        .spawn(move || {
            let mut buf = [0u8; MAX_PACKET_LEN];
            let mut current_ssrc: Option<u32> = None;
            while !net_stop.stopped() {
                let (n, src) = match sock.recv_from(&mut buf) {
                    Ok(v) => v,
                    Err(_) => continue, // zaman aşımı; stop bayrağını kontrol et
                };
                // Kaynak denetimi ayrıştırmadan ÖNCE: yabancı paketin baytları
                // hiçbir koda dokunmasın.
                if !allow_from.is_empty() {
                    if !allow_from.contains(&src.ip()) {
                        let n_foreign = net_stats.foreign.fetch_add(1, Ordering::Relaxed);
                        // Her paketi loglamak diski doldururdu (saniyede 200
                        // paket); ilkini ve sonra her 1000'de birini yaz.
                        if n_foreign % 1000 == 0 {
                            log::warn!(
                                "oturum dışı kaynaktan paket atıldı: {src} (beklenen {allow_from:?}, toplam {})",
                                n_foreign + 1
                            );
                        }
                        continue;
                    }
                }
                let header = match PacketHeader::parse(&buf[..n]) {
                    Ok(h) => h,
                    Err(e) => {
                        log::debug!("bozuk paket atlandı: {e}");
                        continue;
                    }
                };
                // Yeni oturum: tamponu sıfırla, eski paketleri karıştırma.
                if current_ssrc != Some(header.ssrc) {
                    log::info!("yeni oturum (ssrc={})", header.ssrc);
                    current_ssrc = Some(header.ssrc);
                    *net_jitter.lock().unwrap() = JitterBuffer::new(target_packets);
                }
                // Yük uzunluğu beklenen blok boyutunda değilse paketi at.
                // Aksi hâlde tek bir bozuk datagram çalma kuyruğunun kare
                // hizasını kalıcı olarak kaydırıyor ve ses o andan sonra
                // gürültüye dönüyor. Ağdaki başıboş bir paket bile yeter.
                let payload = if header.flags.silence {
                    Vec::new()
                } else {
                    let body = &buf[HEADER_LEN..n];
                    if body.len() != PCM_PAYLOAD_LEN {
                        log::debug!(
                            "beklenmeyen yük boyutu {} (beklenen {PCM_PAYLOAD_LEN}), paket atlandı",
                            body.len()
                        );
                        continue;
                    }
                    body.to_vec()
                };
                net_stats.packets.fetch_add(1, Ordering::Relaxed);
                net_stats.bytes.fetch_add(n as u64, Ordering::Relaxed);
                net_jitter.lock().unwrap().push(header.seq, payload);
            }
        })
        .expect("thread başlatılamadı");

    // Çalma döngüsü (bu thread).
    //
    // Kapanış her yoldan geçmeli: `?` ile erken dönüldüğünde ağ thread'i
    // durdurulmuyordu ve bağlı UDP portunu sonsuza dek tutuyordu. Aygıt
    // kaybolduğunda (kulaklık çıkarıldı, pipewire yeniden başladı) port bir
    // daha açılamıyor, kullanıcı uygulamayı yeniden başlatmak zorunda
    // kalıyordu — her olayda bir thread daha sızıyordu.
    let result = playback_loop(&mut out, &jitter, &stop, &stats);

    stop.stop();
    let _ = net.join();
    log::info!("alım durdu");
    return result;
}

/// Jitter buffer'dan okuyup çalar. Hata durumunda çağıran temizliği yapar.
fn playback_loop(
    out: &mut Box<dyn crate::audio::Playback>,
    jitter: &Arc<Mutex<JitterBuffer>>,
    stop: &Stopper,
    stats: &Arc<ReceiverStats>,
) -> Result<()> {
    let silence = vec![0u8; PCM_PAYLOAD_LEN];

    /// Bir blokun tepe genliği. Ses yolunda değil, çalma öncesi tek geçiş.
    fn block_peak(buf: &[u8]) -> u64 {
        buf.chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs() as u64)
            .max()
            .unwrap_or(0)
    }

    while !stop.stopped() {
        let block = {
            let mut j = jitter.lock().unwrap();
            stats.buffer_depth.store(j.len() as u64, Ordering::Relaxed);
            stats.lost.store(j.lost, Ordering::Relaxed);
            stats.late.store(j.late, Ordering::Relaxed);
            stats.underruns.store(j.starved(), Ordering::Relaxed);
            stats.dropped.store(j.dropped_count(), Ordering::Relaxed);
            j.pop()
        };
        match block {
            Some(Some(p)) if !p.is_empty() => {
                stats.peak.store(block_peak(&p), Ordering::Relaxed);
                out.write(&p)?
            }
            // Kayıp paket veya sessizlik bayraklı paket. PLC v1'de yok.
            Some(Some(_)) | Some(None) => {
                stats.peak.store(0, Ordering::Relaxed);
                out.write(&silence)?
            }
            None => {
                // Tampon hedefe ulaşmadı. Sessizlik yazmaya devam et — ses
                // saatini beslemezsek çalma akışının kendisi underrun verir.
                out.write(&silence)?;
            }
        }
    }

    Ok(())
}

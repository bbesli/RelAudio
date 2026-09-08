//! Alıcı: UDP → jitter buffer → çalma.
//!
//! İki thread: soket okuyan ve çalan. Aralarında Mutex'li jitter buffer var.
//! Ses geri çağrımı burada blocking Pulse/WASAPI yazımı olduğu için gerçek
//! zamanlı callback disiplini (docs/02) bu tasarımda gerekmiyor — kilit
//! yalnızca kısa bir kuyruk işlemi için tutuluyor.

use relaudio_proto::{PacketHeader, HEADER_LEN, MAX_PACKET_LEN, PCM_PAYLOAD_LEN};
use std::net::UdpSocket;
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
}

/// `port`'u dinler ve `device_id` aygıtında çalar.
/// `target_packets` jitter buffer hedefi (paket sayısı; 1 paket = 5 ms).
pub fn receive_loop(
    port: u16,
    device_id: &str,
    target_packets: usize,
    stop: Stopper,
    stats: Arc<ReceiverStats>,
) -> Result<()> {
    let sock = UdpSocket::bind(("0.0.0.0", port))?;
    sock.set_read_timeout(Some(Duration::from_millis(200)))?;
    log::info!("dinleniyor: 0.0.0.0:{port}");

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
                let n = match sock.recv(&mut buf) {
                    Ok(n) => n,
                    Err(_) => continue, // zaman aşımı; stop bayrağını kontrol et
                };
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
                let payload = if header.flags.silence {
                    Vec::new()
                } else {
                    buf[HEADER_LEN..n].to_vec()
                };
                net_stats.packets.fetch_add(1, Ordering::Relaxed);
                net_stats.bytes.fetch_add(n as u64, Ordering::Relaxed);
                net_jitter.lock().unwrap().push(header.seq, payload);
            }
        })
        .expect("thread başlatılamadı");

    // Çalma döngüsü (bu thread)
    let silence = vec![0u8; PCM_PAYLOAD_LEN];
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
            Some(Some(p)) if !p.is_empty() => out.write(&p)?,
            // Kayıp paket veya sessizlik bayraklı paket. PLC v1'de yok.
            Some(Some(_)) | Some(None) => out.write(&silence)?,
            None => {
                // Tampon hedefe ulaşmadı. Sessizlik yazmaya devam et — ses
                // saatini beslemezsek çalma akışının kendisi underrun verir.
                out.write(&silence)?;
            }
        }
    }

    let _ = net.join();
    log::info!("alım durdu");
    Ok(())
}

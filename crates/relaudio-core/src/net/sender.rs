//! Gönderici: yakala → paketle → UDP.

use relaudio_proto::{
    Codec, Flags, PacketHeader, HEADER_LEN, PCM_FRAMES_PER_PACKET, PCM_PACKET_LEN, PCM_PAYLOAD_LEN,
};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::Stopper;
use crate::audio::{self, DeviceKind};
use crate::error::Result;

#[derive(Default)]
pub struct SenderStats {
    pub packets: AtomicU64,
    pub bytes: AtomicU64,
    pub silent_packets: AtomicU64,
}

impl SenderStats {
    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.packets.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed),
            self.silent_packets.load(Ordering::Relaxed),
        )
    }
}

/// Yakalar ve `target`'a yollar. `stop` set edilene kadar bloklar.
pub fn send_loop(
    device_id: &str,
    kind: DeviceKind,
    target: &str,
    stop: Stopper,
    stats: Arc<SenderStats>,
) -> Result<()> {
    let mut cap = audio::open_capture(device_id, kind)?;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect(target)?;
    log::info!("gönderim başladı → {target}");

    let ssrc: u32 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);

    let mut payload = vec![0u8; PCM_PAYLOAD_LEN];
    let mut packet = vec![0u8; PCM_PACKET_LEN];
    let mut seq: u16 = 0;
    let mut timestamp: u32 = 0;
    let mut first = true;

    while !stop.stopped() {
        let n = cap.read(&mut payload)?;
        if n == 0 {
            break;
        }

        // Tamamen sessiz bloklar yük taşımadan gider; alıcı sessizlik üretir.
        // Bant genişliği tasarrufu, ve Windows loopback'inin sessizlikte hiç
        // veri üretmemesiyle (docs/10, Bulgu 6) simetrik davranış.
        let silent = payload.iter().all(|&b| b == 0);

        let header = PacketHeader {
            codec: Codec::PcmS16,
            flags: Flags { marker: first, silence: silent },
            seq,
            timestamp,
            ssrc,
        };
        first = false;
        header.write_to(&mut packet);

        let len = if silent {
            HEADER_LEN
        } else {
            packet[HEADER_LEN..].copy_from_slice(&payload);
            PCM_PACKET_LEN
        };

        match sock.send(&packet[..len]) {
            Ok(sent) => {
                stats.packets.fetch_add(1, Ordering::Relaxed);
                stats.bytes.fetch_add(sent as u64, Ordering::Relaxed);
                if silent {
                    stats.silent_packets.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(e) => log::warn!("paket gönderilemedi: {e}"),
        }

        seq = seq.wrapping_add(1);
        timestamp = timestamp.wrapping_add(PCM_FRAMES_PER_PACKET as u32);
    }

    log::info!("gönderim durdu");
    Ok(())
}

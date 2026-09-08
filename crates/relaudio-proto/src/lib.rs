//! RelAudio ağ protokolü — paylaşılan tipler.
//!
//! Paket biçimi `docs/05-ag-protokolu.md` içinde tanımlı.

mod packet;
pub use packet::{Codec, Flags, PacketHeader, ProtoError, HEADER_LEN, VERSION};

/// Dahili kanonik format. `docs/03-ses-hatti.md`.
pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
pub const BYTES_PER_SAMPLE: usize = 2; // s16le

/// PCM'de paket başına 5 ms — 240 kare × 2 kanal × 2 bayt = 960 bayt.
/// MTU'ya rahat sığar (960 + 12 başlık = 972).
pub const PCM_FRAMES_PER_PACKET: usize = 240;
pub const PCM_PAYLOAD_LEN: usize =
    PCM_FRAMES_PER_PACKET * CHANNELS as usize * BYTES_PER_SAMPLE;

/// Bir paketin toplam boyutu (PCM).
pub const PCM_PACKET_LEN: usize = HEADER_LEN + PCM_PAYLOAD_LEN;

/// Alıcı tamponu — en büyük olası paketten büyük olmalı.
pub const MAX_PACKET_LEN: usize = 1500;

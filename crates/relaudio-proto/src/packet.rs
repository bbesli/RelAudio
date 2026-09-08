//! 12 baytlık paket başlığı. RTP'ye kasten benzetildi; bkz. docs/05.
//!
//! ```text
//! 0                   1                   2                   3
//! | Ver=1 | Codec |     Flags     |        Sequence (16)          |
//! |                     Timestamp (32, örnek cinsinden)           |
//! |                          SSRC (32)                            |
//! ```

use thiserror::Error;

pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 12;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtoError {
    #[error("paket çok kısa: {0} bayt, en az {HEADER_LEN} gerekli")]
    TooShort(usize),
    #[error("desteklenmeyen protokol sürümü: {0}")]
    BadVersion(u8),
    #[error("bilinmeyen kodek: {0}")]
    BadCodec(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Codec {
    PcmS16 = 0,
    Opus = 1,
}

impl Codec {
    fn from_bits(b: u8) -> Result<Self, ProtoError> {
        match b {
            0 => Ok(Codec::PcmS16),
            1 => Ok(Codec::Opus),
            other => Err(ProtoError::BadCodec(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// Akış başı veya atlama sonrası ilk paket.
    pub marker: bool,
    /// Yük boş; alıcı bu süre kadar sessizlik üretir.
    /// Windows loopback'i sessizlikte hiç veri üretmiyor (docs/10, Bulgu 6),
    /// bu bayrak o boşluğu doldurmak için var.
    pub silence: bool,
}

impl Flags {
    const MARKER: u8 = 1 << 0;
    const SILENCE: u8 = 1 << 1;

    fn to_bits(self) -> u8 {
        (if self.marker { Self::MARKER } else { 0 }) | (if self.silence { Self::SILENCE } else { 0 })
    }

    fn from_bits(b: u8) -> Self {
        Flags {
            marker: b & Self::MARKER != 0,
            silence: b & Self::SILENCE != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub codec: Codec,
    pub flags: Flags,
    /// Sarmalanan sıra numarası; kayıp ve sıra bozukluğu tespiti.
    pub seq: u16,
    /// Gönderici örnek sayacı; jitter ve saat kayması hesabı.
    pub timestamp: u32,
    /// Oturum kimliği; eski oturumun geciken paketlerini ayıklamak için.
    pub ssrc: u32,
}

impl PacketHeader {
    pub fn write_to(&self, out: &mut [u8]) {
        debug_assert!(out.len() >= HEADER_LEN);
        out[0] = (VERSION << 4) | (self.codec as u8 & 0x0F);
        out[1] = self.flags.to_bits();
        out[2..4].copy_from_slice(&self.seq.to_be_bytes());
        out[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        out[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
    }

    pub fn parse(buf: &[u8]) -> Result<Self, ProtoError> {
        if buf.len() < HEADER_LEN {
            return Err(ProtoError::TooShort(buf.len()));
        }
        let version = buf[0] >> 4;
        if version != VERSION {
            return Err(ProtoError::BadVersion(version));
        }
        Ok(PacketHeader {
            codec: Codec::from_bits(buf[0] & 0x0F)?,
            flags: Flags::from_bits(buf[1]),
            seq: u16::from_be_bytes([buf[2], buf[3]]),
            timestamp: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
            ssrc: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let h = PacketHeader {
            codec: Codec::PcmS16,
            flags: Flags { marker: true, silence: false },
            seq: 0xBEEF,
            timestamp: 0x1234_5678,
            ssrc: 0xDEAD_C0DE,
        };
        let mut buf = [0u8; HEADER_LEN];
        h.write_to(&mut buf);
        assert_eq!(PacketHeader::parse(&buf).unwrap(), h);
    }

    #[test]
    fn seq_wraps_are_preserved() {
        for seq in [0u16, 1, 65534, 65535] {
            let h = PacketHeader {
                codec: Codec::Opus,
                flags: Flags { marker: false, silence: true },
                seq,
                timestamp: 0,
                ssrc: 7,
            };
            let mut buf = [0u8; HEADER_LEN];
            h.write_to(&mut buf);
            assert_eq!(PacketHeader::parse(&buf).unwrap().seq, seq);
        }
    }

    #[test]
    fn rejects_short_and_bad_version() {
        assert_eq!(PacketHeader::parse(&[0u8; 4]), Err(ProtoError::TooShort(4)));
        let mut buf = [0u8; HEADER_LEN];
        buf[0] = 9 << 4;
        assert_eq!(PacketHeader::parse(&buf), Err(ProtoError::BadVersion(9)));
    }
}

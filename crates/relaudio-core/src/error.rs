//! Hata tipleri.
//!
//! Mesajlar **İngilizce**: arayüz 10 dile çevrili ve bu metinler doğrudan
//! kullanıcıya gösteriliyor. Türkçe bir hata, İspanyolca arayüzde tuhaf
//! kaçıyordu. Çekirdek tarafında tam çeviri için hata kodları gerekir;
//! şimdilik nötr dil kullanılıyor.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("audio device not found: {0}")]
    DeviceNotFound(String),
    #[error("could not open audio device ({device}): {source_msg}")]
    DeviceOpen { device: String, source_msg: String },
    #[error("audio stream error: {0}")]
    Stream(String),
    #[error("could not list audio devices: {0}")]
    Enumerate(String),
    #[error("network error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Proto(#[from] relaudio_proto::ProtoError),
    #[error("not supported on this platform: {0}")]
    Unsupported(&'static str),
    /// Kontrol kanalında karşı makineye bağlanılamadı (kapalı, güvenlik
    /// duvarı, yanlış adres). Arayüz bunu "güvenlik duvarına bak" diye
    /// çeviriyor; ayrı tür olmasının sebebi bu.
    #[error("could not reach {0}")]
    Unreachable(String),
    /// Bağlantı kuruldu ama cevap gelmedi. Ulaşılamamaktan farklı bir
    /// sorun: karşı taraf ayakta ama takılmış ya da uyumsuz bir sürüm.
    #[error("{0} did not reply in time")]
    NoReply(String),
}

pub type Result<T> = std::result::Result<T, Error>;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("ses aygıtı bulunamadı: {0}")]
    DeviceNotFound(String),
    #[error("ses aygıtı açılamadı ({device}): {source_msg}")]
    DeviceOpen { device: String, source_msg: String },
    #[error("ses akışı hatası: {0}")]
    Stream(String),
    #[error("aygıtlar listelenemedi: {0}")]
    Enumerate(String),
    #[error("ağ hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("protokol hatası: {0}")]
    Proto(#[from] relaudio_proto::ProtoError),
    #[error("bu platformda desteklenmiyor: {0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

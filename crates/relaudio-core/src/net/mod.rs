//! Ağ katmanı: gönderici, alıcı, jitter buffer.

mod control;
mod discovery;
mod jitter;
mod pairing;
mod receiver;
mod sender;

pub use control::{
    Auth as ControlAuth,
    send as control_send, serve as control_serve, ControlServer, Reply as ControlReply,
    Request as ControlRequest, CONTROL_PORT, CONTROL_VERSION,
};
pub use discovery::{device_name, Discovery, Peer, SERVICE_TYPE};
pub use jitter::JitterBuffer;
pub use pairing::{sign as sign_request, PairError, Pairing, CODE_TTL};
pub use receiver::{receive_loop, ReceiverStats};
pub use sender::{send_loop, SenderStats};

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Bu makinenin yerel ağdaki IP adresi.
///
/// Yöntem: UDP soketini uzak bir adrese "bağla" ve yerel ucunu oku. UDP'de
/// `connect` **paket göndermez**, yalnızca işletim sistemine rota seçtirir —
/// yani internet bağlantısı olmasa da doğru arayüzü verir.
///
/// Karşı tarafa hangi adresi yazacağını bilmek için gerekiyor; yanlış IP
/// girildiğinde belirti "hiç paket gelmiyor" oluyor ve sebebi anlaşılmıyor.
pub fn local_address() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.168.1.1:9").ok().or_else(|| sock.connect("8.8.8.8:80").ok())?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_unspecified() || ip.is_loopback() {
        None
    } else {
        Some(ip.to_string())
    }
}

/// Çalışan bir oturumu dışarıdan durdurmak için.
#[derive(Clone, Default)]
pub struct Stopper(Arc<AtomicBool>);

impl Stopper {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

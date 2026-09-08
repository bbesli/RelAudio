//! Platformdan bağımsız ses aygıtı soyutlaması.
//!
//! Üç aygıt türü var ve ayrım kullanıcı için anlamlı:
//!   - [`DeviceKind::Output`]   — sesin çalınacağı yer
//!   - [`DeviceKind::Input`]    — fiziksel mikrofon
//!   - [`DeviceKind::Monitor`]  — sistem sesi (Linux'ta sink monitor, Windows'ta loopback)
//!
//! macOS ertelendi (docs/00); soyutlama onu sonradan kabul edecek şekilde duruyor.

use crate::error::Result;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as backend;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceKind {
    Output,
    Input,
    Monitor,
}

impl DeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::Output => "output",
            DeviceKind::Input => "input",
            DeviceKind::Monitor => "monitor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Kararlı tanıtıcı. Görünen ada göre eşleştirme yapma — kullanıcı adı
    /// değiştirebilir ve eşleşme sessizce başarısız olur (docs/10, Bulgu 5).
    pub id: String,
    /// Kullanıcıya gösterilecek ad.
    pub name: String,
    pub kind: DeviceKind,
    pub is_default: bool,
}

/// Yakalama akışı. `read` bir blok dolana kadar bloklar.
///
/// **`Send` bilinçli olarak yok.** Windows'ta WASAPI nesneleri COM arayüzleri
/// (`IAudioClient`, `HANDLE`) ve bunlar `Send` değil. Tasarım zaten bunu
/// gerektirmiyor: her akış, onu kullanan thread'in içinde açılır ve orada
/// tüketilir — thread sınırını hiç geçmez. Bkz. `net::sender::send_loop` ve
/// `net::receiver::receive_loop`.
pub trait Capture {
    /// `buf` tamamen dolana kadar okur. Dönen değer okunan bayt sayısıdır;
    /// akış kapandıysa 0 döner.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

/// Çalma akışı. `Send` yok — gerekçe [`Capture`] ile aynı.
pub trait Playback {
    fn write(&mut self, buf: &[u8]) -> Result<()>;
}

/// Sistemdeki tüm ses aygıtlarını listeler.
pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let mut v = backend::list_devices()?;
    v.sort_by(|a, b| {
        (a.kind, !a.is_default, a.name.to_lowercase())
            .cmp(&(b.kind, !b.is_default, b.name.to_lowercase()))
    });
    Ok(v)
}

/// Yakalama akışı açar. `id` boşsa türün varsayılan aygıtı kullanılır.
pub fn open_capture(id: &str, kind: DeviceKind) -> Result<Box<dyn Capture>> {
    backend::open_capture(id, kind)
}

/// Çalma akışı açar. `id` boşsa varsayılan çıkış kullanılır.
pub fn open_playback(id: &str) -> Result<Box<dyn Playback>> {
    backend::open_playback(id)
}

/// Verilen tür için varsayılan aygıtı bulur.
pub fn default_device(kind: DeviceKind) -> Result<Option<DeviceInfo>> {
    Ok(list_devices()?
        .into_iter()
        .find(|d| d.kind == kind && d.is_default))
}

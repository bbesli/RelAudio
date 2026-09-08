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

/// Sanal ses kablosu (virtual audio cable) satıcılarını tanıyan belirteçler.
///
/// Bir sanal kablo, bir **çıkış** (hoparlör) ile bir **giriş** (mikrofon)
/// ucundan oluşur: çıkışa yazılan, girişten okunur. "Uzaktaki mikrofonu bu
/// makinede mikrofon olarak kullanma" senaryosu bununla çözülüyor —
/// kendi sürücümüzü yazmadan (docs/04, ADR-0003).
const VIRTUAL_TOKENS: &[&str] = &[
    "cable",       // VB-CABLE: "CABLE Input" / "CABLE Output"
    "vb-audio",
    "voicemeeter",
    "virtual",     // "Virtual Speaker for X" / "Virtual Mic for X"
    "audiorelay",
    "relaudio",
    "blackhole",
];

/// Seçilen çıkış bir sanal kablonun hoparlör ucuysa, aynı kabloya ait
/// mikrofon ucunu bulur.
///
/// Kullanıcıya "diğer uygulamalarda mikrofon olarak şunu seç" diyebilmek için.
/// Eşleştirme satıcı belirtecine göre yapılıyor; isim kalıpları satıcıdan
/// satıcıya değiştiği için tam eşleşme aramıyoruz.
pub fn paired_virtual_input(output_name: &str, devices: &[DeviceInfo]) -> Option<String> {
    let out = output_name.to_lowercase();
    let matched: Vec<&str> = VIRTUAL_TOKENS
        .iter()
        .copied()
        .filter(|t| out.contains(t))
        .collect();
    if matched.is_empty() {
        return None;
    }
    devices
        .iter()
        .filter(|d| d.kind == DeviceKind::Input)
        .find(|d| {
            let n = d.name.to_lowercase();
            matched.iter().any(|t| n.contains(t))
        })
        .map(|d| d.name.clone())
}

/// Sistemde herhangi bir sanal kablo hoparlörü var mı?
pub fn has_virtual_output(devices: &[DeviceInfo]) -> bool {
    devices.iter().any(|d| {
        d.kind == DeviceKind::Output && {
            let n = d.name.to_lowercase();
            VIRTUAL_TOKENS.iter().any(|t| n.contains(t))
        }
    })
}

/// Verilen tür için varsayılan aygıtı bulur.
pub fn default_device(kind: DeviceKind) -> Result<Option<DeviceInfo>> {
    Ok(list_devices()?
        .into_iter()
        .find(|d| d.kind == kind && d.is_default))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, kind: DeviceKind) -> DeviceInfo {
        DeviceInfo { id: name.into(), name: name.into(), kind, is_default: false }
    }

    #[test]
    fn pairs_vb_cable_speaker_with_its_microphone() {
        let devices = vec![
            dev("Speakers (Realtek(R) Audio)", DeviceKind::Output),
            dev("CABLE Input (VB-Audio Virtual Cable)", DeviceKind::Output),
            dev("Microphone Array (Intel)", DeviceKind::Input),
            dev("CABLE Output (VB-Audio Virtual Cable)", DeviceKind::Input),
        ];
        assert_eq!(
            paired_virtual_input("CABLE Input (VB-Audio Virtual Cable)", &devices),
            Some("CABLE Output (VB-Audio Virtual Cable)".into())
        );
    }

    #[test]
    fn pairs_vendor_named_devices() {
        let devices = vec![
            dev("Virtual Speaker for AudioRelay", DeviceKind::Output),
            dev("Virtual Mic for AudioRelay", DeviceKind::Input),
        ];
        assert_eq!(
            paired_virtual_input("Virtual Speaker for AudioRelay", &devices),
            Some("Virtual Mic for AudioRelay".into())
        );
    }

    #[test]
    fn real_speakers_have_no_paired_microphone() {
        let devices = vec![
            dev("Speakers (Realtek(R) Audio)", DeviceKind::Output),
            dev("Microphone Array (Intel)", DeviceKind::Input),
        ];
        assert_eq!(paired_virtual_input("Speakers (Realtek(R) Audio)", &devices), None);
    }

    #[test]
    fn detects_whether_any_virtual_cable_exists() {
        let none = vec![dev("Speakers", DeviceKind::Output)];
        assert!(!has_virtual_output(&none));
        let some = vec![dev("CABLE Input (VB-Audio Virtual Cable)", DeviceKind::Output)];
        assert!(has_virtual_output(&some));
    }
}

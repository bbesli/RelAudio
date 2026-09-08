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
/// Sanal ses kablosu satıcılarını tanıyan belirteçler ve ağırlıkları.
///
/// Ağırlık şart: iki farklı satıcının kablosu aynı anda kurulu olabiliyor ve
/// "virtual" gibi genel bir kelime ikisinde de geçiyor. Gerçek bir makinede
/// hem VB-CABLE hem AudioRelay kuruluyken, "CABLE Input (VB-Audio Virtual
/// Cable)" çıkışı yanlışlıkla "Virtual Mic for AudioRelay" ile eşleşiyordu.
/// Satıcıya özgü belirteçler genel olanları yenmeli.
const VIRTUAL_TOKENS: &[(&str, u32)] = &[
    // Satıcıya özgü — güçlü kanıt
    ("vb-audio", 10),
    ("voicemeeter", 10),
    ("audiorelay", 10),
    ("blackhole", 10),
    ("relaudio", 10),
    // Genel — yalnızca destekleyici
    ("cable", 3),
    ("virtual", 1),
];

fn token_score(name: &str, against: &str) -> u32 {
    let (a, b) = (name.to_lowercase(), against.to_lowercase());
    VIRTUAL_TOKENS
        .iter()
        .filter(|(t, _)| a.contains(t) && b.contains(t))
        .map(|(_, w)| w)
        .sum()
}

/// Ad bir sanal ses kablosuna mı işaret ediyor?
pub fn looks_virtual(name: &str) -> bool {
    let n = name.to_lowercase();
    VIRTUAL_TOKENS.iter().any(|(t, _)| n.contains(t))
}

/// Seçilen çıkış bir sanal kablonun hoparlör ucuysa, aynı kabloya ait
/// mikrofon ucunu bulur.
///
/// Bir sanal kablo bir **çıkış** (hoparlör) ile bir **giriş** (mikrofon)
/// ucundan oluşur: çıkışa yazılan, girişten okunur. "Uzaktaki mikrofonu bu
/// makinede mikrofon olarak kullanma" senaryosu bununla çözülüyor — kendi
/// sürücümüzü yazmadan (docs/04, ADR-0003).
///
/// Birden fazla satıcının kablosu kuruluysa en çok belirteç paylaşan seçilir.
pub fn paired_virtual_input(output_name: &str, devices: &[DeviceInfo]) -> Option<String> {
    if !looks_virtual(output_name) {
        return None;
    }
    devices
        .iter()
        .filter(|d| d.kind == DeviceKind::Input)
        .map(|d| (token_score(output_name, &d.name), d))
        .filter(|(score, _)| *score > 0)
        // Eşit puanda kanonik (stereo) ucu seç.
        .max_by_key(|(score, d)| (*score, u8::MAX - virtual_output_rank(&d.name)))
        .map(|(_, d)| d.name.clone())
}

/// Çok kanallı sanal kablo varyantlarını tanıyan desenler.
///
/// VB-CABLE "CABLE Input" (2 kanal) yanında "CABLE In 16ch" gibi çok kanallı
/// uçlar da sunuyor. Bizim akışımız stereo; 16 kanallı uca yazmak eşleşen
/// mikrofon ucuna düzgün ulaşmayabiliyor. Alfabetik sıralamada "cable in
/// 16ch" < "cable input" olduğu için varsayılan seçim yanlış uca düşüyordu.
const MULTICHANNEL_HINTS: &[&str] = &["16ch", "8ch", "6ch", " in 16", " out 16"];

/// Sanal kablo uçlarını tercih sırasına göre puanlar; küçük olan önce gelir.
/// Stereo/kanonik uç, çok kanallı varyantı yenmeli.
pub fn virtual_output_rank(name: &str) -> u8 {
    let n = name.to_lowercase();
    if MULTICHANNEL_HINTS.iter().any(|h| n.contains(h)) {
        1
    } else {
        0
    }
}

/// Sistemde herhangi bir sanal kablo hoparlörü var mı?
pub fn has_virtual_output(devices: &[DeviceInfo]) -> bool {
    devices
        .iter()
        .any(|d| d.kind == DeviceKind::Output && looks_virtual(&d.name))
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

    /// Gerçek bir Windows makinesinden alınan aygıt listesi: hem VB-CABLE
    /// hem AudioRelay kurulu. Genel "virtual" kelimesi ikisinde de geçiyor.
    fn real_windows_devices() -> Vec<DeviceInfo> {
        vec![
            dev("CABLE Input (VB-Audio Virtual Cable)", DeviceKind::Output),
            dev("CABLE In 16ch (VB-Audio Virtual Cable)", DeviceKind::Output),
            dev("Speakers (Realtek(R) Audio)", DeviceKind::Output),
            dev("Virtual Speakers (Virtual Speakers for AudioRelay)", DeviceKind::Output),
            dev("Virtual Mic (Virtual Mic for AudioRelay)", DeviceKind::Input),
            dev("CABLE Output (VB-Audio Virtual Cable)", DeviceKind::Input),
            dev("Microphone Array (Intel)", DeviceKind::Input),
        ]
    }

    #[test]
    fn picks_the_right_vendor_when_two_cables_are_installed() {
        let d = real_windows_devices();
        assert_eq!(
            paired_virtual_input("CABLE Input (VB-Audio Virtual Cable)", &d),
            Some("CABLE Output (VB-Audio Virtual Cable)".into()),
            "VB-CABLE hoparlörü AudioRelay mikrofonuyla eşleşmemeli"
        );
        assert_eq!(
            paired_virtual_input("Virtual Speakers (Virtual Speakers for AudioRelay)", &d),
            Some("Virtual Mic (Virtual Mic for AudioRelay)".into()),
            "AudioRelay hoparlörü VB-CABLE mikrofonuyla eşleşmemeli"
        );
    }

    #[test]
    fn real_speakers_have_no_paired_microphone() {
        let d = real_windows_devices();
        assert_eq!(paired_virtual_input("Speakers (Realtek(R) Audio)", &d), None);
    }

    #[test]
    fn prefers_the_stereo_cable_over_multichannel_variants() {
        assert_eq!(virtual_output_rank("CABLE Input (VB-Audio Virtual Cable)"), 0);
        assert_eq!(virtual_output_rank("CABLE In 16ch (VB-Audio Virtual Cable)"), 1);
        assert_eq!(virtual_output_rank("Speakers (Realtek)"), 0);
    }

    #[test]
    fn detects_whether_any_virtual_cable_exists() {
        let none = vec![dev("Speakers", DeviceKind::Output)];
        assert!(!has_virtual_output(&none));
        let some = vec![dev("CABLE Input (VB-Audio Virtual Cable)", DeviceKind::Output)];
        assert!(has_virtual_output(&some));
    }
}

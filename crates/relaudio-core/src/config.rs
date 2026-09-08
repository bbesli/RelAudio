//! Kalıcı ayarlar.
//!
//! Uygulama her açıldığında kullanıcının aynı seçimleri tekrar yapmasını
//! istemiyoruz: dil, aygıtlar, mod, tampon. Konum `docs/02-mimari.md`'deki
//! tabloya uygun.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Ayar dosyasının yeri.
pub fn config_path() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
    .unwrap_or_else(std::env::temp_dir);
    base.join("RelAudio").join("config.toml")
}

fn default_port() -> u16 {
    59101
}
fn default_buffer() -> usize {
    8
}
fn default_true() -> bool {
    true
}
fn default_language() -> String {
    // Boş = sistem dilini algıla. Kullanıcı seçim yapınca somut kod yazılır.
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// BCP-47 dil kodu ("tr", "en", ...). Boşsa sistem dili kullanılır.
    #[serde(default = "default_language")]
    pub language: String,

    /// Kapatma düğmesi tepsiye küçültsün mü?
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,

    /// Uygulama açılışında oynatıcı kendiliğinden dinlemeye başlasın mı?
    #[serde(default = "default_true")]
    pub auto_listen: bool,

    // — Oynatıcı —
    /// "listen" (hoparlörden dinle) veya "mic" (sanal kabloya yaz)
    pub player_mode: String,
    #[serde(default = "default_port")]
    pub player_port: u16,
    pub player_device: String,
    #[serde(default = "default_buffer")]
    pub player_buffer: usize,

    // — Sunucu —
    /// "monitor" (sistem sesi) veya "input" (mikrofon)
    pub server_source: String,
    pub server_device_monitor: String,
    pub server_device_input: String,
    /// Son seçilen hedef; eş kaybolursa geri düşmek için.
    pub server_target: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: default_language(),
            minimize_to_tray: true,
            auto_listen: true,
            player_mode: "listen".into(),
            player_port: default_port(),
            player_device: String::new(),
            player_buffer: default_buffer(),
            server_source: "monitor".into(),
            server_device_monitor: String::new(),
            server_device_input: String::new(),
            server_target: String::new(),
        }
    }
}

impl Config {
    /// Diskten okur. Dosya yoksa veya bozuksa varsayılanlara döner —
    /// bozuk bir ayar dosyası yüzünden uygulama açılmamalı.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("ayar dosyası okunamadı ({}): {e} — varsayılanlar kullanılıyor", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Diske yazar. Hata durumunda yalnızca loglar; ayar kaydedilememesi
    /// uygulamayı durdurmamalı.
    pub fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::warn!("ayar dizini oluşturulamadı: {e}");
                return;
            }
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    log::warn!("ayarlar yazılamadı: {e}");
                }
            }
            Err(e) => log::warn!("ayarlar serileştirilemedi: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // Eski sürümden kalan eksik alanlı dosya uygulamayı bozmamalı.
        let c: Config = toml::from_str(r#"language = "tr""#).unwrap();
        assert_eq!(c.language, "tr");
        assert_eq!(c.player_port, 59101);
        assert!(c.minimize_to_tray);
        assert_eq!(c.player_mode, "listen");
    }

    #[test]
    fn round_trips_through_toml() {
        let mut c = Config::default();
        c.language = "de".into();
        c.player_mode = "mic".into();
        c.player_buffer = 20;
        let text = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.language, "de");
        assert_eq!(back.player_mode, "mic");
        assert_eq!(back.player_buffer, 20);
    }
}

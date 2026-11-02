//! Kalıcı ayarlar.
//!
//! Uygulama her açıldığında kullanıcının aynı seçimleri tekrar yapmasını
//! istemiyoruz: dil, aygıtlar, mod, tampon. Konum `docs/02-mimari.md`'deki
//! tabloya uygun.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Bu kurulumun kalıcı kimliği. Eşleştirme anahtarları buna göre
    /// saklanıyor ve mDNS'te `pid` olarak ilan ediliyor.
    ///
    /// Boşsa ilk açılışta üretilip yazılıyor. **Makine adından bağımsız:**
    /// aynı ada sahip iki makine eskiden aynı kimliği alıyor ve eşleştirme
    /// hiç kurulamıyordu.
    #[serde(default)]
    pub device_id: String,

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

    // — Kulaklık modu —
    /// Son seçilen rol: "local" (kulaklık burada) veya "remote".
    #[serde(default = "default_headset_role")]
    pub headset_role: String,

    /// Kulaklık modunun aygıt seçimleri — Oynatıcı/Sunucu sekmelerinden
    /// **ayrı**.
    ///
    /// Eskiden kulaklık modu seçtiği aygıtları `player_device` ve
    /// `player_mode`'a yazıyordu. Uzak rolde bu, oynatıcının sanal kabloya
    /// "mic" modunda ayarlanması demekti; oturum bitince `auto_listen`
    /// oynatıcıyı **bu ayarlarla ve kaynak kilidi olmadan** yeniden açıyordu.
    /// Makine böylece ağdaki herkesin toplantı mikrofonuna ses basabileceği
    /// bir durumda kalıyor, bir sonraki açılışta da öyle başlıyordu.
    #[serde(default)]
    pub headset_local_play: String,
    #[serde(default)]
    pub headset_local_capture: String,
    #[serde(default)]
    pub headset_remote_play: String,
    #[serde(default)]
    pub headset_remote_capture: String,

    /// Bu makine, **eşleşilmiş** cihazların uzaktan kulaklık modu
    /// başlatmasına izin versin mi?
    ///
    /// Varsayılan açık. Açık olması tek başına kimseye yetki vermiyor:
    /// eşleşmemiş bir istek reddediliyor (bkz. [`crate::net::Pairing`]).
    /// Kapatmak, eşleşmiş cihazlar dahil her şeyi reddeder ve kontrol
    /// sunucusu hiç bağlanmaz — dinlenen port, güvenlik duvarı sorusu ve
    /// ilan edilen `cport` oluşmaz.
    #[serde(default = "default_true")]
    pub remote_control: bool,

    /// Eşleşilmiş cihazlar: kalıcı kimlik → paylaşılan anahtar (hex).
    ///
    /// Kullanıcı bir kez 6 haneli kodu girdiğinde buraya yazılıyor ve bir
    /// daha sorulmuyor. Silmek eşleştirmeyi bozar, güvenlik açığı yaratmaz.
    #[serde(default)]
    pub paired: BTreeMap<String, PairedPeer>,
}

/// Eşleşilmiş bir cihaz.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedPeer {
    /// Paylaşılan anahtar, hex. Bu dosyayı okuyabilen biri eşleştirmeyi
    /// devralabilir — dosya kullanıcının kendi ayar dizininde.
    pub key: String,
    /// Kullanıcıya gösterilecek ad; kimlik teknik ve okunaksız.
    #[serde(default)]
    pub name: String,
}

fn default_headset_role() -> String {
    "local".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_id: String::new(),
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
            headset_role: default_headset_role(),
            headset_local_play: String::new(),
            headset_local_capture: String::new(),
            headset_remote_play: String::new(),
            headset_remote_capture: String::new(),
            remote_control: true,
            paired: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Diskten okur. Dosya yoksa veya bozuksa varsayılanlara döner —
    /// bozuk bir ayar dosyası yüzünden uygulama açılmamalı.
    pub fn load() -> Self {
        let path = config_path();
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Self>(&text) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("ayar dosyası okunamadı ({}): {e} — varsayılanlar kullanılıyor", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        };
        // Kimlik yoksa (ilk açılış ya da eski sürümden gelen dosya) üret ve
        // hemen yaz: eşleştirme buna bağlı ve her açılışta değişirse
        // kullanıcı kodu tekrar tekrar girmek zorunda kalır.
        if cfg.device_id.is_empty() {
            cfg.device_id = crate::net::new_device_id();
            if cfg.device_id.is_empty() {
                log::error!("cihaz kimliği üretilemedi — eşleştirme çalışmayacak");
            } else {
                log::info!("bu kuruluma kimlik atandı: {}", cfg.device_id);
                // Eski, makine adına dayalı eşleşmeler yeni kimlikle
                // eşleşmiyor; bırakılsalar kullanıcı "eşleşmiş görünüyor ama
                // çalışmıyor" durumunda kalırdı.
                if !cfg.paired.is_empty() {
                    log::warn!(
                        "{} eski eşleşme temizlendi — kimlik biçimi değişti, yeniden eşleşmek gerekiyor",
                        cfg.paired.len()
                    );
                    cfg.paired.clear();
                }
                cfg.save();
            }
        }
        cfg
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
                if let Err(e) = write_private(&path, text.as_bytes()) {
                    log::warn!("ayarlar yazılamadı: {e}");
                }
            }
            Err(e) => log::warn!("ayarlar serileştirilemedi: {e}"),
        }
    }
}

/// Dosyayı yalnızca sahibinin okuyabileceği şekilde yazar.
///
/// Ayar dosyası eşleştirme anahtarlarını içeriyor: okuyabilen biri o
/// eşleşmeyi devralıp karşı makineye "sesini gönder" diyebilir. Linux'ta
/// `std::fs::write` umask'a göre genelde 0644 açıyor, yani makinedeki diğer
/// kullanıcılar dosyayı okuyabiliyordu. Windows'ta `%APPDATA%` zaten
/// kullanıcıya özel ACL'li.
///
/// Önce geçici dosyaya yazıp yerine taşınıyor: yazma ortasında çökme ya da
/// eşzamanlı ikinci yazma dosyayı yarım bırakmasın.
fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("toml.tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    // Daha önce geniş izinlerle yazılmış bir dosya varsa onu da daralt.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eşleştirme anahtarları bu dosyada; başka kullanıcılar okuyamamalı.
    #[cfg(unix)]
    #[test]
    fn the_config_file_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("relaudio-cfg-perm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        // Önce geniş izinli bir dosya bırak: eski sürümün yazdığı gibi.
        std::fs::write(&path, b"x").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, b"language = \"tr\"").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "ayar dosyası 0600 olmalı, {mode:o} bulundu");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Yerine taşıma iki platformda da var olan dosyanın üstüne yazabilmeli.
    #[test]
    fn writing_twice_replaces_the_file() {
        let dir = std::env::temp_dir().join(format!("relaudio-cfg-twice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        write_private(&path, b"a = 1").unwrap();
        write_private(&path, b"a = 2").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // Eski sürümden kalan eksik alanlı dosya uygulamayı bozmamalı.
        let c: Config = toml::from_str(r#"language = "tr""#).unwrap();
        assert_eq!(c.language, "tr");
        assert_eq!(c.player_port, 59101);
        assert!(c.minimize_to_tray);
        assert_eq!(c.player_mode, "listen");
        // Yeni alanlar eski dosyada yok; varsayılanları gelmeli.
        assert_eq!(c.headset_role, "local");
        // Uzaktan başlatma varsayılan açık ama tek başına yetki vermiyor:
        // eşleşmemiş istek reddediliyor.
        assert!(c.remote_control);
        assert!(c.paired.is_empty(), "hiçbir cihaz kendiliğinden eşleşmiş olmamalı");
    }

    /// Kullanıcı uzaktan kontrolü kapattıysa bu açılışta geri açılmamalı.
    #[test]
    fn remote_control_stays_off_once_turned_off() {
        let c: Config = toml::from_str("remote_control = false").unwrap();
        assert!(!c.remote_control);
        let back: Config = toml::from_str(&toml::to_string_pretty(&c).unwrap()).unwrap();
        assert!(!back.remote_control);
    }

    /// Eşleştirmeler diskte kalmalı; aksi hâlde kullanıcı her açılışta kodu
    /// tekrar giriyor ve özelliğin anlamı kalmıyor.
    #[test]
    fn pairings_survive_a_round_trip() {
        let mut c = Config::default();
        c.paired.insert(
            "windows-pc-windows".into(),
            PairedPeer { key: "ab".repeat(32), name: "WINDOWS-PC".into() },
        );
        let back: Config = toml::from_str(&toml::to_string_pretty(&c).unwrap()).unwrap();
        assert_eq!(back.paired.len(), 1);
        assert_eq!(back.paired["windows-pc-windows"].key, "ab".repeat(32));
        assert_eq!(back.paired["windows-pc-windows"].name, "WINDOWS-PC");
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

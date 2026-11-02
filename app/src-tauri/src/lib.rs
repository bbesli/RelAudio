//! RelAudio masaüstü uygulaması — Tauri katmanı.
//!
//! Bu katman **ince** tutulur (docs/02): pencere, tepsi, ayarlar ve çekirdeğe
//! köprü. Ses koduna dokunmaz; onun tamamı `relaudio-core` içinde.

mod session;
mod tray;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, WindowEvent};

use relaudio_core::audio::{self, DeviceKind, HeadsetRole};
use relaudio_core::net::{
    self as net, ControlAuth, ControlReply, ControlRequest, ControlServer, Discovery, Pairing,
};
use relaudio_core::{Config, Error as CoreError, PairedPeer};
use session::{PlayerSession, ServerSession};
use std::sync::Mutex;

pub const DEFAULT_PORT: u16 = 59101;

/// Ulaşılamayan bir makineyi ne kadar bekleyeceğiz. Kullanıcı düğmeye
/// bastıktan sonra arayüz bu süreden uzun donmamalı.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
/// Cevabı ne kadar bekleyeceğiz. Karşı taraf cevap vermeden önce aygıtları
/// sayıyor ve iki kez 300 ms bekliyor (`session::start`); Windows'ta WASAPI
/// sayımı yavaş olabiliyor. Komut `async` olduğu için bu süre arayüzü
/// dondurmuyor, yalnızca düğme "meşgul" kalıyor.
const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);
/// Eşleştirme cevabını ne kadar bekleyeceğiz. Eşleştirme ses aygıtı açmıyor,
/// karşı taraf milisaniyeler içinde cevap veriyor; başlatmanın 12 sn'si burada
/// kullanıcıyı boşuna bekletiyordu.
const PAIR_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);
/// Eşleşme doğrulamasının imzasında rol alanına yazılan değer. Gerçek bir
/// rol değil: imzalı bir doğrulama, imzalı bir "başlat" isteği yerine
/// kullanılamasın diye ayrı.
const VERIFY_ROLE: &str = "verify";
/// Uzaktan başlatılan bir oturumda bu kadar süre hiç paket gelmezse, başlatan
/// makine gitmiş demektir; akış kapatılır.
const ORPHAN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);

pub struct AppState {
    pub server: ServerSession,
    pub player: PlayerSession,
    /// mDNS ilanı ve taraması. Uygulama açılışında başlar, kapanana kadar yaşar.
    pub discovery: Mutex<Option<Discovery>>,
    /// Tepsi menüsündeki "Çıkış" bunu set eder; yalnızca o zaman gerçekten kapanırız.
    pub is_quitting: AtomicBool,
    /// Kapatma düğmesi tepsiye küçültsün mü? Tepsi yaratılamazsa kapatılır.
    pub minimize_to_tray: AtomicBool,
    /// Diskte saklanan kullanıcı ayarları.
    pub config: Mutex<Config>,
    /// Uzaktan gelen "kulaklık modunu başlat" isteklerini dinleyen TCP sunucusu.
    pub control: Mutex<Option<ControlServer>>,
    /// Bu makine uzaktan başlatıldıysa: kim başlattı. Arayüz bunu gösteriyor
    /// ve yetim akış gözcüsü buna bakıyor.
    pub started_by: Mutex<Option<RemoteOrigin>>,
    /// Bu makine karşı tarafı başlattıysa: onu durdurmak için gereken adres.
    pub started_peer: Mutex<Option<StartedPeer>>,
    /// Ekranda duran eşleştirme kodu ve imza doğrulama durumu.
    pub pairing: Pairing,
    /// Başlat/durdur bileşik işlemlerini sıraya sokar.
    ///
    /// Oturum başlatmak iki adım: önce alıcı, sonra gönderici. Araya giren bir
    /// durdurma, henüz başlamamış göndericiyi göremeyip yalnızca alıcıyı
    /// kapatıyor ve gönderici ondan sonra açılıyordu — kullanıcı Durdur'a
    /// bastıktan sonra mikrofon yayında kalıyordu. Kontrol kanalı ayrı bir
    /// thread'de çalıştığı için bu artık kuramsal değil.
    pub session_lock: Mutex<()>,
    /// Kulaklık modu açık mı?
    ///
    /// Eskiden arayüz bunu "gönderici VE alıcı çalışıyor" diye tahmin
    /// ediyordu. `auto_listen` yüzünden alıcı hep açık olduğu için Sunucu
    /// sekmesinden elle yayın başlatmak Kulaklık sekmesinde yanlışlıkla
    /// "Durdur" gösteriyordu; tersine, uzaktan başlatılan makinede de durum
    /// yalnızca tahmindi. Artık açıkça tutuluyor.
    pub headset_active: AtomicBool,
    /// Bu kurulumun kimliği. Açılışta bir kez yazılıyor ve bir daha
    /// değişmiyor; o yüzden ayar kilidinin arkasında değil.
    ///
    /// Eskiden `own_id` her çağrıda ayar kilidini alıyordu. Eşleştirme
    /// işleyicisi o kilidi zaten elinde tutarken `own_id`'yi çağırınca
    /// iş parçacığı kendini sonsuza kadar kilitledi: kodu gösteren makinenin
    /// arayüzü tamamen dondu (her 500 ms'de istatistik de o kilidi bekliyor),
    /// kodu yazan makine 12 sn bekleyip zaman aşımı hatası aldı.
    pub device_id: std::sync::OnceLock<String>,
}

/// Bu makinede kulaklık modunu uzaktan başlatan taraf.
#[derive(Clone, Serialize)]
pub struct RemoteOrigin {
    pub name: String,
    pub address: String,
}

/// Bu makinenin uzaktan başlattığı taraf — durdururken haber vermek için.
#[derive(Clone)]
pub struct StartedPeer {
    pub name: String,
    pub control: std::net::SocketAddr,
    /// Karşı tarafta toplantı uygulamasında seçilecek mikrofon (uzak rolü o
    /// üstlendiyse). Arayüz yeniden yüklense de talimat kaybolmasın diye
    /// burada tutuluyor, arayüzün belleğinde değil.
    pub paired_mic: Option<String>,
    /// Eşin bilinen bütün adresleri. Durdurma isteği bunların herhangi
    /// birinden gelebilir: kablolu + Wi-Fi bağlı bir makine, mDNS'in seçtiği
    /// adresten değil, işletim sisteminin yönlendirdiği adresten bağlanıyor.
    /// Yalnızca tek adres karşılaştırılınca durdurma sessizce yok sayılıyor
    /// ve bu makine yayına devam ediyordu.
    pub addresses: Vec<std::net::IpAddr>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            server: ServerSession::default(),
            player: PlayerSession::default(),
            discovery: Mutex::new(None),
            is_quitting: AtomicBool::new(false),
            minimize_to_tray: AtomicBool::new(true),
            config: Mutex::new(Config::default()),
            control: Mutex::new(None),
            started_by: Mutex::new(None),
            started_peer: Mutex::new(None),
            pairing: Pairing::default(),
            session_lock: Mutex::new(()),
            headset_active: AtomicBool::new(false),
            device_id: std::sync::OnceLock::new(),
        }
    }
}

#[derive(Serialize)]
pub struct PeerDto {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub os: String,
    /// Bu eş şu anda ses alabiliyor mu?
    pub listening: bool,
    /// Eş uzaktan başlatmayı destekliyor mu? (kontrol portunu ilan ediyorsa)
    pub can_remote_start: bool,
    /// Eşin kalıcı kurulum kimliği; eşleştirme bunun üzerinden yapılıyor.
    /// mDNS örnek adı (`id`) çakışmada değişebildiği için ona güvenilmiyor.
    pub device_id: String,
}

#[derive(Serialize)]
pub struct DeviceDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub is_default: bool,
    /// Sanal ses kablosunun bir ucu mu? Arayüz "mikrofon olarak kullan"
    /// modunda listeyi buna göre süzüyor.
    pub virtual_cable: bool,
    /// Sanal kablolar arasında tercih sırası (0 = kanonik stereo uç,
    /// 1 = çok kanallı varyant). Varsayılan seçim buna göre yapılıyor.
    pub rank: u8,
}

#[derive(Serialize, Default)]
pub struct StatsDto {
    pub server_running: bool,
    pub server_target: String,
    pub server_packets: u64,
    pub server_kbps: f64,
    pub server_silent_ratio: f64,
    /// Gönderilemeyen paket sayısı — yanlış hedef adresin tek görünür işareti.
    pub server_send_errors: u64,

    pub player_running: bool,
    pub player_port: u16,
    pub player_packets: u64,
    pub player_kbps: f64,
    pub player_lost: u64,
    pub player_late: u64,
    pub player_underruns: u64,
    pub player_dropped: u64,
    /// Oturumun karşı ucundan gelmediği için atılan paketler.
    pub player_foreign: u64,
    pub player_buffer_ms: u64,
    /// Çalınan sesin tepe genliği (0–32767). Seviye çubuğu için.
    pub player_peak: u64,
    /// Akışın **gerçekte** açtığı çıkış aygıtının adı. Seçili olan değil,
    /// çalan olan: ikisi ayrışabiliyor ve "ses nereden çıkıyor?" sorusu
    /// cevapsız kalıyordu.
    pub player_device_name: String,
    /// Gönderilen sesin kaynağı — aynı sebeple.
    pub server_device_name: String,
    /// Sunucu, oynatıcının yazdığı aygıtı yakalıyor mu? Öyleyse ses kendini
    /// besliyor ve kullanıcı kendi sesini duyuyor.
    pub feedback_loop: bool,
    /// Karşılaştırma için ham kimlikler (arayüzde gösterilmiyor).
    pub player_device_id: String,
    pub server_device_id: String,

    /// Kulaklık modu açık mı? Arayüz "Durdur" düğmesini buna göre gösteriyor.
    pub headset_active: bool,
    /// Bu makinenin uzaktan başlattığı karşı taraf (adı ve oradaki mikrofon).
    /// Oturumun iki taraflı olup olmadığını arayüz buradan okuyor; eskiden
    /// kendi belleğindeki son "başlat" sonucuna bakıyordu ve pencere yeniden
    /// yüklenince çalışan bir oturum "yarım başladı" görünüyordu.
    pub linked_peer: Option<String>,
    pub linked_paired_mic: Option<String>,
    /// Bu makinede kulaklık modu uzaktan mı başlatıldı, kim tarafından?
    pub started_by: Option<RemoteOrigin>,
    /// Ekranda gösterilmesi gereken eşleştirme kodu ve kalan saniye.
    /// Karşı taraf kodu kullanınca kayboluyor — arayüz bunu görüp
    /// başlatmayı kendiliğinden tekrar deniyor.
    pub pairing_code: Option<String>,
    pub pairing_seconds: u64,
    /// Eşleşilmiş cihazların kurulum kimlikleri.
    ///
    /// Arayüz "bu eşle eşleştik mi?" sorusunu buradan yanıtlıyor. Eskiden
    /// ayrı bir çağrı ve arayüzdeki config kopyası kullanılıyordu; kodu
    /// *gösteren* makine, karşı taraf kodu girdiğinde bunu hiç fark
    /// etmiyordu çünkü anahtar Rust tarafında yazılıyor, kopya bayat kalıyordu.
    pub paired_ids: Vec<String>,

    /// Kullanıcıya gösterilecek son hata; okununca temizlenir.
    pub last_error: Option<String>,
}

#[derive(Deserialize)]
pub struct StartServerArgs {
    pub target: String,
    pub device_id: String,
    /// "monitor" = sistem sesi, "input" = mikrofon
    pub source: String,
}

#[derive(Deserialize)]
pub struct StartPlayerArgs {
    pub port: u16,
    pub device_id: String,
    pub buffer_packets: usize,
}

// Komutların neredeyse hepsi `async`. İşaretsiz bir `#[tauri::command]` ana
// (olay döngüsü) iş parçacığında çalışıyor; orada bir kilit beklemek, ses
// aygıtı saymak ya da diske yazmak pencereyi donduruyor. Arayüz `stats` ve
// `peers`'ı 500 ms'de bir çağırıyor ve oturum başlarken tutulan kilitleri
// bekliyorlardı: her Başlat/Durdur'da pencere takılıyordu. Ana iş parçacığında
// yalnızca anlık olanlar ve tepsi menüsüne dokunan `set_tray_labels` kaldı.
#[tauri::command(async)]
fn list_devices() -> Result<Vec<DeviceDto>, String> {
    audio::list_devices()
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

impl From<audio::DeviceInfo> for DeviceDto {
    fn from(d: audio::DeviceInfo) -> Self {
        DeviceDto {
            virtual_cable: audio::looks_virtual(&d.name),
            rank: audio::virtual_output_rank(&d.name),
            id: d.id,
            name: d.name,
            kind: d.kind.as_str().to_string(),
            is_default: d.is_default,
        }
    }
}

/// Kulaklık modunun aygıt planı — arayüz ve uzaktan gelen istek aynı
/// politikayı kullansın diye çekirdekte hesaplanıyor.
#[derive(Serialize)]
pub struct HeadsetPlanDto {
    pub capture: Option<DeviceDto>,
    pub play: Option<DeviceDto>,
    /// "input" (mikrofon) ya da "monitor" (sistem sesi).
    pub capture_kind: String,
    /// Çeviri anahtarına eşlenecek sorun kodu: "need_physical" | "need_cable".
    pub problem: Option<String>,
    pub capture_choices: Vec<DeviceDto>,
    pub play_choices: Vec<DeviceDto>,
    /// Uzak rolde toplantı uygulamasında seçilecek mikrofonun adı.
    pub paired_mic: Option<String>,
}

impl From<audio::HeadsetPlan> for HeadsetPlanDto {
    fn from(p: audio::HeadsetPlan) -> Self {
        HeadsetPlanDto {
            capture: p.capture.map(Into::into),
            play: p.play.map(Into::into),
            capture_kind: p.capture_kind.as_str().to_string(),
            problem: p.problem.map(|e| e.as_str().to_string()),
            capture_choices: p.capture_choices.into_iter().map(Into::into).collect(),
            play_choices: p.play_choices.into_iter().map(Into::into).collect(),
            paired_mic: p.paired_mic,
        }
    }
}

/// Bir rol için kayıtlı seçimleri de dikkate alarak planı çözer.
fn plan_for(cfg: &Config, role: HeadsetRole) -> audio::HeadsetPlan {
    let devices = audio::list_devices().unwrap_or_default();
    let (saved_play, saved_capture) = saved_headset_devices(cfg, role);
    audio::headset_plan(role, &devices, saved_play, saved_capture)
}

/// Bir rol için kayıtlı kulaklık aygıtları (çalma, yakalama).
/// Oynatıcı/Sunucu sekmelerinin ayarlarından bağımsız — bkz.
/// `Config::headset_local_play`.
fn saved_headset_devices(cfg: &Config, role: HeadsetRole) -> (&str, &str) {
    match role {
        HeadsetRole::Local => (&cfg.headset_local_play, &cfg.headset_local_capture),
        HeadsetRole::Remote => (&cfg.headset_remote_play, &cfg.headset_remote_capture),
    }
}

/// Arayüz için plan. Kayıtlı seçimler argüman olarak geliyor, diskten
/// okunmuyor: kullanıcı bir aygıt seçtiğinde `set_config`'in tamamlanmasını
/// beklemeden doğru planı görmeli.
#[tauri::command(async)]
fn headset_plan(
    role: String,
    saved_play: String,
    saved_capture: String,
) -> Result<HeadsetPlanDto, String> {
    let role = HeadsetRole::parse(&role).ok_or_else(|| format!("bilinmeyen rol: {role}"))?;
    let devices = audio::list_devices().map_err(|e| e.to_string())?;
    Ok(audio::headset_plan(role, &devices, &saved_play, &saved_capture).into())
}

// ————————————————————————————————————————————————————————————————
//  Kulaklık modu — tek düğmeyle iki taraf
// ————————————————————————————————————————————————————————————————
//
// Eskiden kullanıcı iki makinede ayrı ayrı düğmeye basıyordu. Artık kulaklığı
// paylaşan taraf hedefi seçip bir kez basıyor; karşı taraf kontrol kanalından
// (`net::control`) kendi yarısını kuruyor.
//
// Sıra önemli: **önce kendi alıcımızı** açıyoruz, sonra karşı tarafa haber
// veriyoruz. Tersi olsaydı karşı taraf biz dinlemeye başlamadan yayına
// geçerdi ve ilk paketler boşa giderdi.

/// Karşı tarafın bildirdiği adı ekrana ve loga koymadan önce sınırla.
///
/// `name` ağdan gelen, denetlenmemiş metin: 8 KiB'a kadar olabiliyor ve
/// kontrol karakteri içerebiliyor. Loga ham yazmak satır uydurmaya, arayüze
/// ham vermek bandı taşırmaya yarardı.
fn sanitize_peer_name(raw: &str) -> String {
    // Keşifle aynı kural: kontrol ve yön denetim karakterleri atılıyor. Eskiden
    // yalnızca kontrol karakterleri atılıyordu; U+202E ile bir ad ekranda
    // başka bir cihazın adı gibi okunabiliyordu.
    let cleaned = net::sanitize_display(raw, 48);
    if cleaned.is_empty() { "?".into() } else { cleaned }
}

/// Karşı tarafın bildirdiği kalıcı kimlik. Ad gibi bu da denetimsiz metin ve
/// ayar dosyasında **anahtar** olarak kullanılıyor; dar tutuluyor.
fn sanitize_peer_id(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(64)
        .collect();
    if cleaned.is_empty() { "?".into() } else { cleaned }
}

/// Bu kurulumun kalıcı kimliği (ayar dosyasından).
///
/// Eskiden `{hostname}-{os}` hesaplanıyordu. İki makinenin adı aynıysa
/// kimlikleri de aynı oluyordu; üstelik mDNS çakışmada örnek adını
/// "... (2)" diye değiştirdiği için ilan edilen ad ile hesaplanan kimlik
/// ayrışıyordu. Eşleştirme bu yüzden sessizce hiç kurulmuyordu.
///
/// **Kilit almıyor** — ve almamalı: ayar kilidini tutan kod (eşleştirme
/// işleyicisi gibi) bunu çağırabiliyor, `Mutex` de yeniden girilebilir değil.
pub fn own_id(state: &AppState) -> String {
    state.device_id.get().cloned().unwrap_or_default()
}

/// Eşin eşleştirme kimliği. Boşsa eş eski bir sürüm ya da henüz TXT
/// kaydını yaymamış demektir.
fn peer_key_id(peer: &relaudio_core::net::Peer) -> Option<String> {
    let id = sanitize_peer_id(&peer.device_id);
    if peer.device_id.is_empty() || id == "?" { None } else { Some(id) }
}

/// Bir rolü bu makinede fiilen başlatır. Hem yerel düğme hem uzaktan gelen
/// istek buradan geçiyor — iki yol arasında davranış farkı olmasın diye.
/// Hata, `(kod, metin)` çifti olarak dönüyor: kod karşı tarafta çevrilebilsin,
/// metin bilinmeyen bir kodda yedek kalsın.
///
/// `peer_addrs`: karşı ucun bilinen bütün adresleri. Alıcı yalnızca bunlardan
/// (ve hedefin kendisinden) gelen sesi kabul ediyor.
fn start_headset_locally(
    state: &AppState,
    role: HeadsetRole,
    target: &str,
    peer_addrs: &[std::net::IpAddr],
) -> Result<audio::HeadsetPlan, (&'static str, String)> {
    let _serialized = state.session_lock.lock().unwrap();
    let cfg = state.config.lock().unwrap().clone();
    let plan = plan_for(&cfg, role);
    if let Some(problem) = plan.problem {
        return Err((problem.as_str(), problem.message().to_string()));
    }
    let (capture, play) = match (&plan.capture, &plan.play) {
        (Some(c), Some(p)) => (c.clone(), p.clone()),
        _ => {
            let p = audio::HeadsetProblem::NeedPhysical;
            return Err((p.as_str(), p.message().to_string()));
        }
    };

    // Kulaklık oturumunda alıcı yalnızca karşı uçtan ses kabul ediyor.
    // Kilitlenmezse ağdaki herkes bu porta paket atıp oturumu devralabiliyor;
    // uzak rolde bu ses toplantının mikrofonuna gidiyordu. Hedef bir ad ise
    // (elle yazılmış makine adı) çözümlüyoruz; çözülemezse kilitsiz açmak
    // yerine başlatmayı reddediyoruz.
    let target_ip = {
        use std::net::ToSocketAddrs;
        target
            .to_socket_addrs()
            .ok()
            .and_then(|mut it| it.next())
            .map(|a| a.ip())
            .ok_or(("device_open", format!("could not resolve {target}")))?
    };
    // Tek adres yetmiyor: aynı ağa hem kablolu hem Wi-Fi bağlı bir eş birden
    // fazla adres ilan ediyor ve paketleri hangisinden göndereceğini işletim
    // sistemi seçiyor. Yalnızca birine kilitlemek sesi sessizce kesiyordu.
    let mut allow: Vec<std::net::IpAddr> = peer_addrs.to_vec();
    if !allow.contains(&target_ip) {
        allow.push(target_ip);
    }

    state
        .player
        .start(cfg.player_port, &play.id, cfg.player_buffer, allow)
        .map_err(|e| ("device_open", e))?;
    announce_port(state, cfg.player_port, true);

    // Alıcı ayakta ama gönderici açılamadıysa yarım bir oturum bırakmıyoruz:
    // kullanıcı "çalışıyor" görüp neden ses gitmediğini aramasın.
    if let Err(e) = state.server.start(&capture.id, plan.capture_kind, target) {
        state.player.stop();
        announce(state, false);
        return Err(("device_open", e));
    }

    // Seçilen aygıtlar diske yazılıyor ki bir dahakine (ve uzaktan gelen
    // istekte) aynı seçim tekrarlansın. **Yalnızca kulaklık alanlarına**:
    // Oynatıcı sekmesinin ayarlarına dokunmak, oturum bitince oynatıcının
    // sanal kabloya kilitsiz açılmasına yol açıyordu.
    {
        let mut c = state.config.lock().unwrap();
        c.headset_role = role.as_str().to_string();
        match role {
            HeadsetRole::Local => {
                c.headset_local_play = play.id.clone();
                c.headset_local_capture = capture.id.clone();
            }
            HeadsetRole::Remote => {
                c.headset_remote_play = play.id.clone();
                c.headset_remote_capture = capture.id.clone();
            }
        }
        c.save();
    }
    state.headset_active.store(true, Ordering::Relaxed);
    Ok(plan)
}

/// İki yarıyı da durdurur ve uzaktan başlatma durumunu temizler.
///
/// Durdurduktan sonra `auto_listen` açıksa oynatıcı normal ayarlarıyla geri
/// açılıyor. Aksi hâlde uzaktan gelen tek bir "dur", makinenin ağdaki
/// dinleyici rolünü kalıcı olarak kapatıyordu ve kullanıcı bunu ancak
/// "hiç ses gelmiyor" diye fark ediyordu.
fn stop_headset_locally(state: &AppState) {
    let _serialized = state.session_lock.lock().unwrap();
    state.headset_active.store(false, Ordering::Relaxed);
    state.server.stop();
    state.player.stop();
    *state.started_by.lock().unwrap() = None;

    let cfg = state.config.lock().unwrap().clone();
    if cfg.auto_listen {
        match state.player.start(cfg.player_port, &cfg.player_device, cfg.player_buffer, Vec::new()) {
            Ok(()) => {
                announce_port(state, cfg.player_port, true);
                return;
            }
            Err(e) => log::warn!("dinlemeye geri dönülemedi: {e}"),
        }
    }
    announce(state, false);
}

/// Tepsideki "Tümünü durdur": kulaklık oturumu dahil her şeyi durdurur.
///
/// [`stop_headset_locally`]'den farkı: otomatik dinlemeye geri **dönmüyor** —
/// kullanıcı "hepsini durdur" dedi. Oturum kilidini alıyor; eskiden tepsi
/// kilidi atlıyordu ve o anda süren bir uzaktan başlatma, durdurmanın ardından
/// yayını geri açabiliyordu.
pub fn stop_everything(state: &AppState) {
    let _serialized = state.session_lock.lock().unwrap();
    state.headset_active.store(false, Ordering::Relaxed);
    state.server.stop();
    state.player.stop();
    *state.started_by.lock().unwrap() = None;
    *state.started_peer.lock().unwrap() = None;
    // Eşler bu makineyi hâlâ "dinliyor" görüyordu; ilanı düzelt.
    announce(state, false);
}

/// Bağlı olduğumuz tarafın adı ve kontrol adresi; okurken `started_peer`
/// tüketiliyor çünkü bağlantı bitiyor.
fn take_link_target(state: &AppState) -> Option<(String, std::net::SocketAddr)> {
    let from_peer = state.started_peer.lock().unwrap().take().map(|p| (p.name, p.control));
    from_peer.or_else(|| {
        state.started_by.lock().unwrap().as_ref().and_then(|o| {
            o.address
                .parse::<std::net::IpAddr>()
                .ok()
                .map(|ip| (o.name.clone(), std::net::SocketAddr::new(ip, net::CONTROL_PORT)))
        })
    })
}

fn send_stop(name: &str, addr: std::net::SocketAddr) {
    let quick = std::time::Duration::from_millis(700);
    match net::control_send(addr, &ControlRequest::StopHeadset, quick, quick) {
        // `ok:false` da bir cevap: karşı taraf bizi tanımadı demek, başarı değil.
        Ok(r) if r.ok => log::info!("karşı tarafa durduğumuz bildirildi: {name}"),
        Ok(r) => log::info!(
            "karşı taraf durdurmayı kabul etmedi ({name}): {}",
            r.error.unwrap_or_default()
        ),
        Err(e) => log::info!("karşı tarafa bildirilemedi ({name}): {e}"),
    }
}

/// Bağlı olduğumuz tarafa "ben durdum" der.
///
/// Durdurma iki yönde de yayılmalı: kullanıcı hangi makinede Durdur'a basarsa
/// bassın diğeri de dursun. Aksi hâlde uzaktan başlatılan makinede durdurunca
/// karşı taraf mikrofonu göndermeye devam ediyordu ve bunu ancak gözcü 12 sn
/// sonra fark ediyordu. Kısa zaman aşımı: karşı taraf kapanmış olabilir.
/// Ağ beklediği için ana iş parçacığından çağrılmamalı.
pub fn notify_peer_stopped(state: &AppState) {
    if let Some((name, addr)) = take_link_target(state) {
        send_stop(&name, addr);
    }
}

/// `host` ile `from` aynı makine mi? Aynı adres ya da keşfedilen bir eşin
/// ilan ettiği iki ayrı adres olabilirler (kablolu + Wi-Fi).
///
/// Keşif kilidi kendi ifadesinde alınıp bırakılıyor: çağıran başka bir kilit
/// tutuyor olabilir, burada kilit sırası tersine dönmesin.
fn same_peer(state: &AppState, host: &str, from: std::net::IpAddr) -> bool {
    if host == from.to_string() {
        return true;
    }
    let peers = state
        .discovery
        .lock()
        .unwrap()
        .as_ref()
        .map(|d| d.peers())
        .unwrap_or_default();
    let from_s = from.to_string();
    peers
        .iter()
        .any(|p| p.addresses.iter().any(|a| a == host) && p.addresses.contains(&from_s))
}

/// Uzaktan gelen kontrol isteğini işler.
///
/// Sesin gideceği adres **istekten değil**, TCP bağlantısının kaynak IP'sinden
/// alınıyor: bu uç üçüncü bir makineye ses yansıtmak için kullanılamasın.
fn handle_control(state: &AppState, req: &ControlRequest, from: std::net::IpAddr) -> ControlReply {
    let me = net::device_name();
    match req {
        ControlRequest::Ping => ControlReply::ok(me, None),

        ControlRequest::Verify { auth } => {
            // Anahtar kopyalanıp kilit hemen bırakılıyor; doğrulama sırasında
            // ayar kilidi tutulmuyor.
            let key = state.config.lock().unwrap().paired.get(&auth.id).map(|p| p.key.clone());
            let Some(key) = key else {
                return ControlReply::failed(me, "needs_pairing", "not paired with this machine");
            };
            match state.pairing.verify(&key, &auth.nonce, auth.ts, &auth.mac, VERIFY_ROLE, 0) {
                Ok(()) => ControlReply::ok(me, None),
                Err(e) => ControlReply::failed(me, e.code(), e.message()),
            }
        }

        ControlRequest::StartHeadset { role, audio_port, name, auth } => {
            let cfg_now = state.config.lock().unwrap().clone();
            if !cfg_now.remote_control {
                log::warn!("uzaktan başlatma isteği reddedildi (ayar kapalı): {from}");
                return ControlReply::failed(
                    me,
                    "disabled",
                    "the other machine does not allow being started remotely \
                     (Settings -> Allow remote start)",
                );
            }
            let Some(role) = HeadsetRole::parse(role) else {
                return ControlReply::failed(me, "unknown_role", format!("unknown role: {role}"));
            };

            // Eşleştirme denetimi. Bu olmadan ağdaki herkes bu makineye
            // "sesini yakala ve bana gönder" diyebilirdi.
            let Some(auth) = auth else {
                log::warn!("imzasız başlatma isteği reddedildi: {from}");
                return ControlReply::failed(
                    me,
                    "needs_pairing",
                    "not paired with this machine — pair with the 6-digit code first",
                );
            };
            let Some(peer) = cfg_now.paired.get(&auth.id) else {
                log::warn!("eşleşmemiş cihazdan başlatma isteği: {} ({from})", auth.id);
                return ControlReply::failed(
                    me,
                    "needs_pairing",
                    "not paired with this machine — pair with the 6-digit code first",
                );
            };
            if let Err(e) = state.pairing.verify(
                &peer.key,
                &auth.nonce,
                auth.ts,
                &auth.mac,
                role.as_str(),
                *audio_port,
            ) {
                log::warn!("başlatma imzası geçersiz ({from}): {}", e.code());
                return ControlReply::failed(me, e.code(), e.message());
            }

            // Çalışan bir oturumun üstüne yazmıyoruz: kullanıcı kendi
            // kurduğu bağlantıyı bir başkasının kesmesini beklemez.
            //
            // Ölçüt **gönderici**, alıcı değil: `auto_listen` açıkken oynatıcı
            // uygulama açılır açılmaz dinlemeye başlıyor, dolayısıyla "alıcı
            // çalışıyor mu" diye bakmak her uzaktan başlatmayı reddederdi.
            let origin = state.started_by.lock().unwrap().clone();
            match &origin {
                Some(o) if o.address != from.to_string() => {
                    return ControlReply::failed(
                        me,
                        "busy_other",
                        format!("the other machine is already in a session with {}", o.name),
                    );
                }
                // Zaten *bu eşle* ve *istenen rolde* bir kulaklık oturumundaysak
                // meşgul değiliz: iki makinede aynı anda düğmeye basılmış
                // demektir ve istenen sonuç kurulmuş.
                //
                // İki koşul da şart. Rol denetlenmezse iki taraf da "kulaklık
                // burada" seçip aynı anda basınca ikisi de "tamam" alıyor ve
                // hiçbir taraf sanal kabloya yazmıyordu. Adres tek tek
                // karşılaştırılırsa kablolu + Wi-Fi bağlı bir eş, mDNS'in
                // seçtiği adresten başka bir adresten bağlanınca "meşgul"
                // alıyordu.
                None if state.headset_active.load(Ordering::Relaxed)
                    && state.config.lock().unwrap().headset_role == role.as_str()
                    && state.server.target().is_some_and(|t| {
                        let host = t.rsplit_once(':').map_or(t.as_str(), |(h, _)| h);
                        same_peer(state, host, from)
                    }) =>
                {
                    log::info!("uzaktan başlatma isteği: zaten {from} ile oturum var, korunuyor");
                    *state.started_by.lock().unwrap() = Some(RemoteOrigin {
                        name: sanitize_peer_name(name),
                        address: from.to_string(),
                    });
                    let cfg = state.config.lock().unwrap().clone();
                    return ControlReply::ok(me, plan_for(&cfg, role).paired_mic);
                }
                None if state.server.is_running() => {
                    return ControlReply::failed(
                        me,
                        "busy",
                        "the other machine is already in a session started locally",
                    );
                }
                _ => {}
            }

            let who = sanitize_peer_name(name);
            let target = format!("{from}:{audio_port}");
            // Bizi başlatan taraf bize TCP ile bağlandı; o bağlantının kaynağı,
            // UDP'sinin de çıkacağı adres (aynı yönlendirme).
            match start_headset_locally(state, role, &target, &[from]) {
                Ok(plan) => {
                    log::info!(
                        "kulaklık modu uzaktan başlatıldı: {who} ({from}) — rol {}",
                        role.as_str()
                    );
                    *state.started_by.lock().unwrap() = Some(RemoteOrigin {
                        name: who,
                        address: from.to_string(),
                    });
                    ControlReply::ok(me, plan.paired_mic)
                }
                Err((code, e)) => {
                    log::warn!("uzaktan başlatma başarısız ({from}): {e}");
                    ControlReply::failed(me, code, e)
                }
            }
        }

        ControlRequest::Pair { code, id, name } => {
            // Kodu **gösteren** taraf burası. Kullanıcı kodu okuduğu makinede
            // değil, yazdığı makinede giriyor; istek bize oradan geliyor.
            //
            // Kimlik, kodu harcamadan önce denetleniyor: geçersiz bir kimlikle
            // gelen istek doğru kodu tüketip işe yaramaz bir eşleşme
            // bırakmasın. Kimlik 32 hex (bkz. `new_device_id`); başka biçim,
            // eski bir sürüm ya da uydurma bir istek demek.
            if id.len() != 32 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
                return ControlReply::failed(
                    me,
                    "version",
                    "the other machine runs an older RelAudio — update both sides",
                );
            }
            match state.pairing.redeem(code) {
                Ok(key) => {
                    let who = sanitize_peer_name(name);
                    let peer_id = sanitize_peer_id(id);
                    log::info!("eşleştirildi: {who} [{peer_id}] ({from})");
                    // Kilit yalnızca bu blokta. Cevap kilit bırakıldıktan
                    // sonra kuruluyor: içinde başka bir kilit alan bir çağrı
                    // olursa kendi kendini kilitlemesin.
                    {
                        let mut cfg = state.config.lock().unwrap();
                        cfg.paired.insert(
                            peer_id,
                            PairedPeer { key: key.clone(), name: who },
                        );
                        cfg.save();
                    }
                    ControlReply::paired(me, own_id(state), key)
                }
                Err(e) => {
                    log::warn!("eşleştirme reddedildi ({from}): {}", e.code());
                    ControlReply::failed(me, e.code(), e.message())
                }
            }
        }

        ControlRequest::StopHeadset => {
            // Yalnızca oturumun karşı ucu durdurabilir — bizi başlatan taraf
            // ya da bizim başlattığımız taraf. Aksi hâlde ağdaki herkes
            // çalışan bir oturumu kesebilirdi.
            let from_s = from.to_string();
            let origin = state.started_by.lock().unwrap().clone();
            let linked = state.started_peer.lock().unwrap().clone();
            let started_us = origin.as_ref().is_some_and(|o| o.address == from_s);
            let we_started_them = linked
                .as_ref()
                .is_some_and(|p| p.control.ip() == from || p.addresses.contains(&from));

            if started_us || we_started_them {
                let who = origin.map(|o| o.name).or(linked.map(|p| p.name)).unwrap_or_default();
                log::info!("kulaklık modu karşı taraftan durduruldu: {who} ({from})");
                *state.started_peer.lock().unwrap() = None;
                stop_headset_locally(state);
                ControlReply::ok(me, None)
            } else if origin.is_some() {
                ControlReply::failed(me, "not_yours", "this session was started by someone else")
            } else {
                // Bizimle oturumu olmayan birinin "dur"u zaten bir şey
                // değiştirmiyor; hata döndürmek yanıltıcı olurdu.
                ControlReply::ok(me, None)
            }
        }
    }
}

#[derive(Deserialize)]
pub struct StartHeadsetArgs {
    /// Bu makinenin üstleneceği rol: "local" | "remote".
    pub role: String,
    /// Keşfedilen eşin kimliği; elle adres girildiyse boş.
    pub peer_id: String,
    /// Elle girilen adres ("192.168.1.103" ya da "host:port").
    pub address: String,
}

#[derive(Serialize, Default)]
pub struct HeadsetStartResult {
    /// Karşı taraf da başlatılabildi mi?
    pub remote_started: bool,
    pub remote_name: String,
    /// Karşı taraf uzak rolü üstlendiyse, orada seçilecek mikrofonun adı.
    pub remote_paired_mic: Option<String>,
    /// Karşı taraf başlatılamadıysa sebebi. Bu makine yine de çalışıyor —
    /// kullanıcı karşı tarafa gidip elle başlatabilsin diye durdurmuyoruz.
    pub remote_error: Option<String>,
    /// Sebebin çevrilebilir karşılığı ("disabled", "needs_pairing", ...).
    /// Arayüz bunu bilirse kendi dilinde gösteriyor; bilmezse `remote_error`.
    pub remote_error_code: Option<String>,
    /// Karşı tarafla henüz eşleşmedik: aşağıdaki kod ekranda gösterilmeli.
    pub needs_pairing: bool,
    /// Karşı makinede yazılacak 6 haneli kod.
    pub pairing_code: Option<String>,
}

/// Tek düğme: bu makineyi başlat, sonra karşı tarafı başlat.
///
/// `async` işareti şart. İşaretsiz bir `#[tauri::command]` olay döngüsü
/// thread'inde çalışıyor; bu gövde ise ses aygıtı açıyor, iki kez 300 ms
/// bekliyor ve ulaşılamayabilecek bir makineye TCP açıyor. Senkron olsaydı
/// kullanıcı düğmeye bastığında pencere saniyelerce donardı.
#[tauri::command(async)]
fn start_headset(
    state: tauri::State<'_, AppState>,
    args: StartHeadsetArgs,
) -> Result<HeadsetStartResult, String> {
    let role = HeadsetRole::parse(&args.role).ok_or_else(|| format!("bilinmeyen rol: {}", args.role))?;

    // Hedefi çöz: keşfedilen eş varsa onun ilan ettiği adres, yoksa elle girilen.
    // Elle yazılan adres keşfedilen bir eşinkiyle aynıysa o eşi kullanıyoruz:
    // aksi hâlde kullanıcı aynı makineyi IP'siyle yazdı diye eşleştirme ve
    // uzaktan başlatma kayboluyordu.
    let peers = state
        .discovery
        .lock()
        .unwrap()
        .as_ref()
        .map(|d| d.peers())
        .unwrap_or_default();
    let typed_host = args.address.trim().split(':').next().unwrap_or("").to_string();
    let peer = if !args.peer_id.is_empty() {
        peers.into_iter().find(|p| p.id == args.peer_id)
    } else if !typed_host.is_empty() {
        peers.into_iter().find(|p| p.address == typed_host)
    } else {
        None
    };
    let audio_target = match &peer {
        Some(p) => format!("{}:{}", p.address, p.port),
        None => {
            let a = args.address.trim();
            if a.is_empty() {
                return Err("hedef yok".into());
            }
            if a.contains(':') { a.to_string() } else { format!("{a}:{DEFAULT_PORT}") }
        }
    };

    // Karşı tarafın kontrol portunu yalnızca mDNS'ten biliyoruz. Elle adres
    // girildiyse varsayılan portu deniyoruz — çalışmazsa kullanıcıya karşı
    // tarafta elle başlatması söyleniyor.
    let control_addr = match &peer {
        Some(p) if p.control_port != 0 => format!("{}:{}", p.address, p.control_port),
        Some(p) => format!("{}:{}", p.address, net::CONTROL_PORT),
        None => {
            let host = audio_target.rsplit_once(':').map(|(h, _)| h).unwrap_or(&audio_target);
            format!("{host}:{}", net::CONTROL_PORT)
        }
    };

    // Uzaktan başlatılabilecek bir eş mi? Keşfedilmiş, kontrol portu ilan
    // eden ve kurulum kimliği bilinen bir eş olmalı — anahtar o kimliğe göre
    // saklanıyor. Keşfedilmemiş bir adreste eşleşmeyi bulmanın yolu yok.
    let remote_capable = peer
        .as_ref()
        .is_some_and(|p| p.control_port != 0 && peer_key_id(p).is_some());

    let (my_port, paired_key) = {
        let cfg = state.config.lock().unwrap();
        let key = peer
            .as_ref()
            .and_then(peer_key_id)
            .and_then(|id| cfg.paired.get(&id))
            .map(|p| p.key.clone());
        (cfg.player_port, key)
    };

    // **Eşleşme denetimi başlatmadan önce.** Eskiden önce kendi yarımız
    // başlatılıyor, sonra eşleşme olmadığı görülüp kod gösteriliyordu:
    // kullanıcı ekranda bir kod ve aynı anda kimsenin dinlemediği bir
    // makineye akan bir yayın görüyordu.
    if remote_capable && paired_key.is_none() {
        let mut out = HeadsetStartResult::default();
        out.needs_pairing = true;
        out.pairing_code = ensure_code(&state);
        out.remote_error_code = Some("needs_pairing".into());
        out.remote_error = Some("not paired yet".into());
        log::info!("eşleşme yok — eşleştirme kodu gösteriliyor, yayın başlatılmadı");
        return Ok(out);
    }

    let peer_addrs: Vec<std::net::IpAddr> = peer
        .as_ref()
        .map(|p| p.addresses.iter().filter_map(|a| a.parse().ok()).collect())
        .unwrap_or_default();
    let plan = start_headset_locally(&state, role, &audio_target, &peer_addrs)
        .map_err(|(_, e)| e)?;
    // Kendi düğmemize bastık; uzaktan başlatılmış sayılmayız.
    *state.started_by.lock().unwrap() = None;

    let mut out = HeadsetStartResult {
        remote_paired_mic: plan.paired_mic,
        ..Default::default()
    };

    // Eş uzaktan başlatmayı desteklemiyorsa (eski sürüm, ayar kapalı ya da
    // listede görünmeyen elle yazılmış adres) kendi yarımız çalışıyor ve
    // kullanıcıya karşı tarafı elle başlatması söyleniyor.
    let Some(key) = paired_key else {
        let code = if peer.is_some() { "no_control" } else { "manual_target" };
        out.remote_error_code = Some(code.into());
        out.remote_error = Some("the other machine was not started remotely".into());
        return Ok(out);
    };

    let opposite = role.opposite();
    let Some((nonce, ts, mac)) = net::sign_request(&key, opposite.as_str(), my_port) else {
        out.remote_error_code = Some("pair_no_random".into());
        out.remote_error = Some("could not sign the request".into());
        return Ok(out);
    };
    let req = ControlRequest::StartHeadset {
        role: opposite.as_str().to_string(),
        audio_port: my_port,
        name: net::device_name(),
        auth: Some(ControlAuth { id: own_id(&state), nonce, ts, mac }),
    };

    // `parse::<SocketAddr>` yalnızca sayısal adresi kabul ediyor; kullanıcı
    // makine adı yazdıysa ses bağlanıyor ama uzaktan başlatma ham bir
    // ayrıştırma hatasıyla düşüyordu.
    let resolved = {
        use std::net::ToSocketAddrs;
        control_addr
            .to_socket_addrs()
            .map_err(|e| format!("{control_addr}: {e}"))
            .and_then(|mut it| it.next().ok_or_else(|| format!("{control_addr}: not found")))
    };
    match resolved {
        Err(e) => out.remote_error = Some(e),
        Ok(addr) => match net::control_send(addr, &req, CONNECT_TIMEOUT, REPLY_TIMEOUT) {
            Ok(reply) if reply.ok => {
                out.remote_started = true;
                out.remote_name = sanitize_peer_name(&reply.name);
                // Uzak rolü karşı taraf üstlendiyse mikrofon adını o biliyor.
                // Ağdan gelen metin: ekrana girmeden önce daraltılıyor.
                let their_mic = reply
                    .paired_mic
                    .as_deref()
                    .map(|m| net::sanitize_display(m, 96))
                    .filter(|m| !m.is_empty());
                if role == HeadsetRole::Local {
                    out.remote_paired_mic = their_mic.clone();
                }
                *state.started_peer.lock().unwrap() = Some(StartedPeer {
                    name: sanitize_peer_name(&reply.name),
                    control: addr,
                    paired_mic: if role == HeadsetRole::Local { their_mic } else { None },
                    addresses: {
                        let mut a = peer_addrs.clone();
                        if !a.contains(&addr.ip()) {
                            a.push(addr.ip());
                        }
                        a
                    },
                });
                log::info!("karşı taraf uzaktan başlatıldı: {control_addr}");
            }
            Ok(reply) => {
                // Cevabın her alanı karşı taraftan geliyor; ekrana ve loga
                // ham girmemeli.
                out.remote_name = sanitize_peer_name(&reply.name);
                out.remote_error_code = reply.code.filter(|c| {
                    c.len() <= 32 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                });
                out.remote_error = Some(
                    reply
                        .error
                        .map(|e| e.chars().filter(|c| !c.is_control()).take(200).collect())
                        .unwrap_or_else(|| "refused".into()),
                );
                // Karşı taraf bizi tanımıyorsa (o tarafta eşleşme silinmiş ya da
                // iki taraf aynı anda farklı anahtarlarla eşleşmiş) bizdeki
                // anahtar bayattır. Silmezsek iki makine de "eşleşmiş"
                // görünüyor ama hiçbir başlatma çalışmıyor ve arayüz yeniden
                // eşleşme yolu göstermiyor.
                if matches!(out.remote_error_code.as_deref(), Some("auth_bad" | "needs_pairing")) {
                    if let Some(id) = peer.as_ref().and_then(peer_key_id) {
                        let mut cfg = state.config.lock().unwrap();
                        if cfg.paired.remove(&id).is_some() {
                            log::warn!("karşı taraf eşleşmeyi tanımadı — bayat anahtar silindi: {id}");
                            cfg.save();
                        }
                    } // ayar kilidi burada bırakıldı — aşağıdaki durdurma onu da alıyor
                    // Bayat anahtarla açılan yarım oturumu bırakmıyoruz: eşleşme
                    // yokken yayın açık kalırsa arayüz eşleştirme kartını
                    // göstermiyor (oturum sürüyor sayılıyor) ve kullanıcı önce
                    // Durdur'a basması gerektiğini bilemiyordu.
                    stop_headset_locally(&state);
                    out.needs_pairing = true;
                }
            }
            Err(e) => {
                out.remote_error_code = Some(control_error_code(&e).into());
                out.remote_error = Some(e.to_string());
            }
        },
    }
    if let Some(e) = &out.remote_error {
        log::warn!("karşı taraf başlatılamadı ({control_addr}): {e}");
    }
    Ok(out)
}

/// Kontrol kanalı hatasını arayüzün çevirebileceği koda indirger.
///
/// Ham metin ("os error 10060") kullanıcıya bir şey söylemiyordu; "ulaşılamadı"
/// güvenlik duvarına, "cevap vermedi" karşı taraftaki bir soruna işaret ediyor.
fn control_error_code(e: &CoreError) -> &'static str {
    match e {
        CoreError::Unreachable(_) => "unreachable",
        CoreError::NoReply(_) => "no_reply",
        _ => "pair_bad_reply",
    }
}

/// Karşı makinenin ekranındaki kodu girip eşleşir.///
/// İstek kodu **yazdığımız** makineden, kodu **gösteren** makineye gidiyor.
/// Dönen anahtar iki tarafta da diske yazılıyor; bir daha sorulmuyor.
#[tauri::command(async)]
fn pair_with_peer(
    state: tauri::State<'_, AppState>,
    peer_id: String,
    code: String,
) -> Result<String, String> {
    let peer = find_peer(&state, &peer_id).ok_or_else(|| "peer_gone".to_string())?;
    pair_with(&state, &peer, &code)
}

/// Keşfedilen eşi örnek adıyla bulur. Keşif kilidi kendi ifadesinde bırakılıyor.
fn find_peer(state: &AppState, peer_id: &str) -> Option<net::Peer> {
    state
        .discovery
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|d| d.peers().into_iter().find(|p| p.id == peer_id))
}

/// Eşleştirmenin kendisi — keşiften bağımsız, iki taraflı testte doğrudan
/// çağrılabilsin diye ayrı.
fn pair_with(state: &AppState, peer: &net::Peer, code: &str) -> Result<String, String> {
    if peer.control_port == 0 {
        return Err("no_control".into());
    }
    let addr = std::net::SocketAddr::new(
        peer.address.parse().map_err(|e| format!("{e}"))?,
        peer.control_port,
    );
    let req = ControlRequest::Pair {
        code: code.trim().to_string(),
        id: own_id(state),
        name: net::device_name(),
    };
    let reply = net::control_send(addr, &req, CONNECT_TIMEOUT, PAIR_TIMEOUT).map_err(|e| {
        log::warn!("eşleştirme isteği gönderilemedi: {e}");
        control_error_code(&e).to_string()
    })?;
    if !reply.ok {
        return Err(reply.code.unwrap_or_else(|| "pair_wrong".into()));
    }
    let (Some(key), Some(their_id)) = (reply.key, reply.id) else {
        return Err("pair_bad_reply".into());
    };
    let their_id = sanitize_peer_id(&their_id);
    // Cevaptaki kimlik, bağlandığımız eşin mDNS'te ilan ettiğiyle aynı
    // olmalı. Anahtar bu kimlik altında saklanıyor ve sonraki her arama
    // ilan edilen kimlikle yapılıyor; farklıysa eşleşme hiç bulunamayan bir
    // anahtar altında kalırdı. (Farklı olması aynı zamanda araya giren
    // birinin işareti.)
    if their_id == "?" || their_id.is_empty() || Some(&their_id) != peer_key_id(peer).as_ref() {
        log::warn!("eşleştirme cevabındaki kimlik ilan edilenle uyuşmuyor: {their_id}");
        return Err("pair_bad_reply".into());
    }
    let name = sanitize_peer_name(&reply.name);
    log::info!("eşleştirildi: {name} [{their_id}]");
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.paired.insert(their_id, PairedPeer { key, name: name.clone() });
        cfg.save();
    }
    // Karşının kodunu kullandık; bizim ekranımızdaki kod artık gereksiz.
    // Canlı bırakılırsa 3 dakika boyunca başka biri onunla eşleşmeyi
    // deneyebilirdi.
    state.pairing.clear_code();
    Ok(name)
}

/// Bu eşle eşleşmemiz karşı tarafta da geçerli mi? Değilse bizdeki bayat
/// anahtarı siler.
///
/// Dönen değer: `"ok"` (iki taraf da tanıyor), `"dropped"` (karşı taraf bizi
/// tanımıyordu, anahtar silindi — arayüz eşleştirme kartını gösterecek),
/// `"unpaired"` (bizde zaten anahtar yok), `"unknown"` (karşıya ulaşılamadı
/// ya da eski bir sürüm; anahtara dokunulmadı).
///
/// Ulaşılamamak anahtarı silmek için sebep değil: karşı makine kapalı olabilir,
/// açıldığında eşleşme çalışmaya devam etmeli.
#[tauri::command(async)]
fn verify_pairing(state: tauri::State<'_, AppState>, peer_id: String) -> &'static str {
    match find_peer(&state, &peer_id) {
        Some(peer) => verify_with(&state, &peer),
        None => "unknown",
    }
}

/// Doğrulamanın kendisi — keşiften bağımsız, testte doğrudan çağrılabilsin.
fn verify_with(state: &AppState, peer: &net::Peer) -> &'static str {
    let Some(id) = peer_key_id(peer) else { return "unknown" };
    if peer.control_port == 0 {
        return "unknown";
    }
    let key = state.config.lock().unwrap().paired.get(&id).map(|p| p.key.clone());
    let Some(key) = key else { return "unpaired" };

    let Ok(ip) = peer.address.parse::<std::net::IpAddr>() else { return "unknown" };
    let addr = std::net::SocketAddr::new(ip, peer.control_port);
    let Some((nonce, ts, mac)) = net::sign_request(&key, VERIFY_ROLE, 0) else { return "unknown" };
    let req = ControlRequest::Verify {
        auth: ControlAuth { id: own_id(state), nonce, ts, mac },
    };
    match net::control_send(addr, &req, CONNECT_TIMEOUT, PAIR_TIMEOUT) {
        Ok(r) if r.ok => "ok",
        Ok(r) if matches!(r.code.as_deref(), Some("needs_pairing" | "auth_bad")) => {
            let mut cfg = state.config.lock().unwrap();
            if cfg.paired.remove(&id).is_some() {
                log::warn!("eşleşme karşı tarafta yok — bayat anahtar silindi: {} [{id}]", peer.name);
                cfg.save();
            }
            "dropped"
        }
        // Eski sürüm `Verify`'ı tanımıyor ("malformed"), saat sapması vb.:
        // eşleşmenin bozuk olduğuna dair kanıt değil.
        Ok(_) => "unknown",
        Err(_) => "unknown",
    }
}

/// Ekranda gösterilecek kod. Varsa mevcut olanı döndürüyor, yoksa üretiyor.
///
/// Kodun **her iki makinede de** görünür olması gerekiyor: kullanıcı hangi
/// makinenin başında olursa olsun oradaki kodu okuyup diğerine yazabilsin.
/// Eskiden kod yalnızca "başlat"a basılan makinede üretiliyordu, dolayısıyla
/// kullanıcı kodu girmek istediği makinede hiçbir kod göremiyordu.
fn ensure_code(state: &AppState) -> Option<String> {
    state
        .pairing
        .visible_code()
        .map(|(c, _)| c)
        .or_else(|| state.pairing.new_code())
}

/// Bu makinenin eşleştirme kodu — karşı tarafa yazılacak olan.
#[tauri::command(async)]
fn pairing_code(state: tauri::State<'_, AppState>) -> Option<String> {
    ensure_code(&state)
}

/// Ekranda duran kodu iptal eder.
#[tauri::command(async)]
fn cancel_pairing(state: tauri::State<'_, AppState>) {
    state.pairing.clear_code();
}

/// Eşleştirmeyi kaldırır. Argüman eşin **kurulum kimliği**.
#[tauri::command(async)]
fn unpair(state: tauri::State<'_, AppState>, device_id: String) {
    let id = sanitize_peer_id(&device_id);
    let mut cfg = state.config.lock().unwrap();
    if cfg.paired.remove(&id).is_some() {
        log::info!("eşleştirme kaldırıldı: {id}");
        cfg.save();
    }
}

/// Tek düğme: bu makineyi durdur, başlattıysak karşı tarafı da durdur.
/// `async` gerekçesi [`start_headset`] ile aynı — karşı taraf kapanmışsa
/// durdurma isteği zaman aşımına kadar bekliyor.
#[tauri::command(async)]
fn stop_headset(state: tauri::State<'_, AppState>) {
    // Önce haber ver, sonra dur: `notify_peer_stopped` bağlantıyı
    // `started_by`'dan da okuyabiliyor ve `stop_headset_locally` onu siliyor.
    notify_peer_stopped(&state);
    stop_headset_locally(&state);
}

#[tauri::command(async)]
fn start_server(state: tauri::State<'_, AppState>, args: StartServerArgs) -> Result<(), String> {
    let kind = match args.source.as_str() {
        "input" => DeviceKind::Input,
        _ => DeviceKind::Monitor,
    };
    let target = if args.target.contains(':') {
        args.target.clone()
    } else {
        format!("{}:{}", args.target, DEFAULT_PORT)
    };
    state.server.start(&args.device_id, kind, &target)
}

#[tauri::command(async)]
fn stop_server(state: tauri::State<'_, AppState>) {
    state.server.stop();
}

#[tauri::command(async)]
fn start_player(state: tauri::State<'_, AppState>, args: StartPlayerArgs) -> Result<(), String> {
    let r = state
        .player
        .start(args.port, &args.device_id, args.buffer_packets, Vec::new());
    if r.is_ok() {
        announce_port(&state, args.port, true);
    }
    r
}

#[tauri::command(async)]
fn stop_player(state: tauri::State<'_, AppState>) {
    state.player.stop();
    announce(&state, false);
}

/// Ağa "ses alabiliyorum / alamıyorum" bilgisini yayar.
pub fn announce(state: &AppState, listening: bool) {
    if let Some(d) = state.discovery.lock().unwrap().as_ref() {
        if let Err(e) = d.announce(listening) {
            log::warn!("could not update mDNS announcement: {e}");
        }
    }
}

/// İlan edilen portu günceller. Oynatıcı başka bir portta başlatıldığında
/// eşlerin doğru yere göndermesi için şart.
fn announce_port(state: &AppState, port: u16, listening: bool) {
    if let Some(d) = state.discovery.lock().unwrap().as_ref() {
        if let Err(e) = d.set_port(port, listening) {
            log::warn!("could not update mDNS port: {e}");
        }
    }
}

/// Ağda bulunan diğer RelAudio örnekleri.
#[tauri::command(async)]
fn peers(state: tauri::State<'_, AppState>) -> Vec<PeerDto> {
    state
        .discovery
        .lock()
        .unwrap()
        .as_ref()
        .map(|d| {
            d.peers()
                .into_iter()
                .map(|p| PeerDto {
                    id: p.id,
                    name: p.name,
                    address: p.address,
                    port: p.port,
                    os: p.os,
                    listening: p.listening,
                    can_remote_start: p.control_port != 0,
                    device_id: p.device_id,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Bu cihazın ağda görünen adı.
#[derive(Serialize, Default)]
pub struct MicHint {
    /// Seçili çıkış bir sanal kablonun hoparlör ucuysa, mikrofon ucunun adı.
    pub paired_input: Option<String>,
    /// Sistemde herhangi bir sanal kablo var mı?
    pub any_virtual: bool,
}

/// "Uzaktaki mikrofonu bu makinede mikrofon olarak kullan" senaryosunda
/// kullanıcıya ne seçeceğini söylemek için.
#[tauri::command(async)]
fn mic_hint(output_id: String) -> MicHint {
    let Ok(devices) = audio::list_devices() else {
        return MicHint::default();
    };
    let name = devices
        .iter()
        .find(|d| d.id == output_id && d.kind == DeviceKind::Output)
        .map(|d| d.name.clone())
        .unwrap_or_default();
    MicHint {
        paired_input: audio::paired_virtual_input(&name, &devices),
        any_virtual: audio::has_virtual_output(&devices),
    }
}

/// Tepsi menüsünün etiketlerini arayüzün diliyle günceller.
///
/// Tepsi Rust tarafında kuruluyor ve çeviriler frontend'de; sabit Türkçe
/// etiketler 10 dilli bir arayüzle çelişiyordu.
#[tauri::command]
fn set_tray_labels(
    app: tauri::AppHandle,
    show: String,
    stop_all: String,
    quit: String,
) -> Result<(), String> {
    tray::update_labels(&app, &show, &stop_all, &quit).map_err(|e| e.to_string())
}

/// Log dosyasının yolu — sorun bildirirken kullanıcıya göstermek için.
#[tauri::command]
fn log_file() -> String {
    log_path().display().to_string()
}

#[tauri::command]
fn device_name() -> String {
    relaudio_core::net::device_name()
}

#[tauri::command(async)]
fn stats(state: tauri::State<'_, AppState>) -> StatsDto {
    let mut s = StatsDto::default();
    state.server.fill(&mut s);
    state.player.fill(&mut s);
    s.feedback_loop = detect_feedback(&s);
    // İki yarı da öldüyse (aygıt çıkarıldı, karşı taraf durdu) kulaklık modu
    // fiilen bitmiştir; bayrak açık kalırsa arayüz boşuna "Durdur" gösterir.
    if !s.server_running && !s.player_running {
        state.headset_active.store(false, Ordering::Relaxed);
    }
    s.headset_active = state.headset_active.load(Ordering::Relaxed);
    s.started_by = state.started_by.lock().unwrap().clone();
    if let Some(p) = state.started_peer.lock().unwrap().as_ref() {
        s.linked_peer = Some(p.name.clone());
        s.linked_paired_mic = p.paired_mic.clone();
    }
    if let Some((code, left)) = state.pairing.visible_code() {
        s.pairing_code = Some(code);
        s.pairing_seconds = left;
    }
    s.paired_ids = state.config.lock().unwrap().paired.keys().cloned().collect();
    s
}

/// Sunucunun yakaladığı kaynak, oynatıcının yazdığı çıkışla aynı aygıtsa
/// ses kendi kuyruğunu yiyor: çalınan şey yeniden yakalanıp gönderiliyor.
///
/// Linux'ta sistem sesi kaynağı `<sink>.monitor` biçiminde, yani çıkış
/// aygıtının kimliği kaynağın kimliğinin ön eki oluyor. Windows'ta loopback
/// yakalama render aygıtının kimliğini birebir kullanıyor.
fn detect_feedback(s: &StatsDto) -> bool {
    if !(s.server_running && s.player_running) {
        return false;
    }
    let (out, src) = (s.player_device_id.trim(), s.server_device_id.trim());
    if out.is_empty() || src.is_empty() {
        return false;
    }
    src == out || src == format!("{out}.monitor")
}

/// Bu makinenin yerel adresi — karşı tarafa yazılacak olan.
#[tauri::command(async)]
fn local_address() -> Option<String> {
    relaudio_core::net::local_address()
}

#[tauri::command(async)]
fn set_minimize_to_tray(state: tauri::State<'_, AppState>, enabled: bool) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
    let mut c = state.config.lock().unwrap();
    c.minimize_to_tray = enabled;
    c.save();
}

/// Kayıtlı ayarları döndürür — **eşleştirme anahtarları olmadan**.
///
/// Arayüzün anahtara ihtiyacı yok (yalnızca adları listeliyor). Anahtarı
/// webview'e vermek, orada çalışan herhangi bir şeye eşleşmeyi devralma
/// imkânı verirdi.
#[tauri::command(async)]
fn get_config(state: tauri::State<'_, AppState>) -> Config {
    let mut c = state.config.lock().unwrap().clone();
    for p in c.paired.values_mut() {
        p.key.clear();
    }
    c
}

/// Ayarları kaydeder. Arayüz her anlamlı değişiklikte çağırıyor;
/// kullanıcı aynı seçimleri her açılışta tekrar yapmasın.
///
/// **Kimlik ve eşleşmeler arayüzden kabul edilmiyor.** Bunlar yalnızca Rust
/// tarafında yazılıyor (eşleştirme kontrol kanalında, karşı taraf kodu
/// girdiğinde gerçekleşiyor). Eskiden arayüz bütün config'i geri yazıyordu:
/// kodu gösteren makinede anahtar Rust'ta kaydediliyor, ardından arayüzün
/// eski kopyası bir sonraki ayar değişikliğinde onu **siliyordu** —
/// eşleştirme bir an çalışıp kayboluyordu.
#[tauri::command(async)]
fn set_config(app: tauri::AppHandle, state: tauri::State<'_, AppState>, mut config: Config) {
    state.minimize_to_tray.store(config.minimize_to_tray, Ordering::Relaxed);
    let enabled = config.remote_control;
    // Yazma kilit altında: iki `set_config` çakıştığında dosya yarı yazılmış
    // hâlde kalabiliyordu ve sonraki açılışta ayarlar sıfırlanıyordu.
    let changed = {
        let mut guard = state.config.lock().unwrap();
        let changed = guard.remote_control != config.remote_control;
        config.device_id = guard.device_id.clone();
        config.paired = guard.paired.clone();
        *guard = config;
        guard.save();
        changed
    };
    if changed {
        apply_remote_control(&app, enabled);
    }
}

/// Ayar ve log dosyalarının bulunduğu klasör — kullanıcıya göstermek için.
#[tauri::command]
fn config_path() -> String {
    relaudio_core::config::config_path().display().to_string()
}

/// Log dosyasının yeri. Pencereli uygulamada `stderr` kaybolduğu için
/// çökme sonrası elimizde kalan tek kanıt bu dosya.
pub fn log_path() -> std::path::PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")))
    }
    .unwrap_or_else(std::env::temp_dir);
    base.join("RelAudio").join("relaudio.log")
}

fn init_logging() {
    let path = log_path();
    // Boyut yazma anında denetleniyor: uygulama tepside günlerce açık
    // kalabiliyor ve yalnızca açılışta bakmak diski sınırsız büyümeye açık
    // bırakıyordu. Tavan aşılınca dosya silinmiyor, bir önceki kuşak olarak
    // saklanıyor — çökmeden önceki kayıtlar kaybolmasın.
    let file = relaudio_core::logging::CappedLog::open(
        &path,
        relaudio_core::logging::DEFAULT_CAP_BYTES,
    )
    .ok();

    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder.format_timestamp_secs();
    if let Some(f) = file {
        builder.target(env_logger::Target::Pipe(Box::new(f)));
    }
    let _ = builder.try_init();

    // Panic'i logla. Aksi hâlde pencereli uygulamada iz bırakmadan kapanıyor.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIC: {info}");
        default(info);
    }));

    log::info!("RelAudio {} başladı — log: {}", env!("CARGO_PKG_VERSION"), log_path().display());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // İkinci örnek: yeni pencere açma, mevcut olanı öne getir (docs/06).
            show_main(app);
        }))
        .manage(AppState::default())
        .setup(|app| {
            log::info!("setup başladı");
            let state = app.state::<AppState>();

            let cfg = Config::load();
            log::info!("ayarlar yüklendi: dil='{}', mod='{}'", cfg.language, cfg.player_mode);
            state
                .minimize_to_tray
                .store(cfg.minimize_to_tray, Ordering::Relaxed);
            let (auto_listen, port, device, buffer, remote_control, device_id) = (
                cfg.auto_listen,
                cfg.player_port,
                cfg.player_device.clone(),
                cfg.player_buffer,
                cfg.remote_control,
                cfg.device_id.clone(),
            );
            // Kimlik bundan sonra değişmiyor; kilitsiz okunabilsin.
            let _ = state.device_id.set(device_id.clone());
            *state.config.lock().unwrap() = cfg;

            // Kontrol kanalı keşiften ÖNCE açılıyor: fiilen bağlanılan port
            // mDNS'te ilan edilecek ve 59100 doluysa farklı olabiliyor.
            // Ayar kapalıysa hiç bağlanmıyor — bkz. `Config::remote_control`.
            let control_port = open_control_server(app.handle(), remote_control);

            // Keşfi hemen başlat: kullanıcı IP yazmak zorunda kalmasın.
            match Discovery::start(port, control_port, device_id.clone()) {
                Ok(d) => *state.discovery.lock().unwrap() = Some(d),
                Err(e) => log::warn!("ağ keşfi başlatılamadı: {e} — IP elle girilebilir"),
            }

            spawn_orphan_watchdog(app.handle().clone());

            // Oynatıcıyı otomatik başlat: uygulama açık olduğu sürece ses
            // alabilir durumda olsun. Aygıt yoksa sessizce geç — kullanıcı
            // Oynatıcı sekmesinden elle başlatabilir.
            if auto_listen {
                match state.player.start(port, &device, buffer, Vec::new()) {
                    Ok(()) => {
                        log::info!("oynatıcı otomatik başladı — port {port}");
                        announce(&state, true);
                    }
                    Err(e) => log::warn!("oynatıcı otomatik başlatılamadı: {e}"),
                }
            }

            let tray_ok = tray::build(app.handle()).is_ok();
            if !tray_ok {
                // Tepsi yoksa "kapatınca gizle" davranışı uygulamayı erişilemez
                // yapar (docs/06, GNOME senaryosu). Otomatik kapat.
                log::warn!("tepsi simgesi yaratılamadı — kapatma davranışı gerçek kapatmaya çevrildi");
                app.state::<AppState>()
                    .minimize_to_tray
                    .store(false, Ordering::Relaxed);
            }
            log::info!("setup bitti");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if !state.is_quitting.load(Ordering::Relaxed)
                    && state.minimize_to_tray.load(Ordering::Relaxed)
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_devices,
            peers,
            device_name,
            log_file,
            mic_hint,
            headset_plan,
            start_headset,
            stop_headset,
            pair_with_peer,
            pairing_code,
            verify_pairing,
            cancel_pairing,
            unpair,
            get_config,
            set_config,
            set_tray_labels,
            config_path,
            start_server,
            stop_server,
            start_player,
            stop_player,
            stats,
            local_address,
            set_minimize_to_tray,
        ])
        .run(tauri::generate_context!())
        .expect("uygulama başlatılamadı");
    log::info!("uygulama normal şekilde sonlandı");
}

/// Kontrol sunucusunu açar (ya da kapalıysa hiç açmaz) ve bağlandığı portu
/// döndürür. 0 = dinlemiyoruz.
///
/// Ayar kapalıyken "bağlan ama reddet" yapmıyoruz: bağlanan bir port
/// güvenlik duvarı sorusu çıkarır, `ss -ltn` listesinde görünür ve `cport`
/// ilan edilirse eşler bu makineyi uzaktan başlatılabilir sanır.
fn open_control_server(app: &tauri::AppHandle, enabled: bool) -> u16 {
    let state = app.state::<AppState>();
    let mut guard = state.control.lock().unwrap();
    if !enabled {
        if guard.take().is_some() {
            log::info!("kontrol kanalı kapatıldı — uzaktan başlatma devre dışı");
        }
        return 0;
    }
    if let Some(c) = guard.as_ref() {
        return c.port();
    }
    let handle = app.clone();
    match net::control_serve(net::CONTROL_PORT, move |req, from| {
        handle_control(&handle.state::<AppState>(), req, from)
    }) {
        Ok(c) => {
            let p = c.port();
            *guard = Some(c);
            p
        }
        Err(e) => {
            // Tek düğmeli başlatma çalışmaz ama uygulamanın geri kalanı
            // çalışır; iki makinede elle başlatmak hâlâ mümkün.
            log::warn!("kontrol kanalı açılamadı: {e} — uzaktan başlatma kapalı");
            0
        }
    }
}

/// Ayar değiştiğinde sunucuyu aç/kapat ve mDNS'teki `cport`'u güncelle.
/// İlan güncellenmezse eşler bu makineyi hâlâ desteklyor sanıp zaman aşımına
/// düşer — hata mesajı da anlaşılmaz olur.
fn apply_remote_control(app: &tauri::AppHandle, enabled: bool) {
    let port = open_control_server(app, enabled);
    let state = app.state::<AppState>();
    // Kapatmak, yalnızca yeni istekleri değil o iznin açtığı oturumu da
    // bitirmeli; aksi hâlde kullanıcı kutuyu kaldırıyor ama mikrofon yayında
    // kalıyordu.
    if !enabled && state.started_by.lock().unwrap().is_some() {
        log::info!("uzaktan başlatma kapatıldı — o izinle açılan oturum durduruluyor");
        stop_headset_locally(&state);
    }
    let listening = state.player.packets().is_some();
    let guard = state.discovery.lock().unwrap();
    if let Some(d) = guard.as_ref() {
        if let Err(e) = d.set_control_port(port, listening) {
            log::warn!("kontrol portu ilan edilemedi: {e}");
        }
    }
}

/// Kulaklık oturumunda paket akışını izler ve karşı taraf gidince kapatır.
///
/// İki yönü de kapsıyor:
/// - **Bizi başlattılar** (`started_by`): başlatan makine çökerse bize "dur"
///   diyen kimse kalmıyor, mikrofon açık kalıyordu.
/// - **Biz başlattık** (`started_peer`): karşı taraf durduğunda bize haber
///   vermeye çalışıyor ama ulaşamayabiliyor — varsayılan ayarlarla karşı
///   tarafın bizde çalacak bir kapısı yok, çünkü uzaktan başlatma kapalıyken
///   hiç port bağlanmıyor. O yüzden sessizliği kendimiz fark etmeliyiz.
///
/// Gönderici sessizlikte bile paket üretiyor (bkz. `Flags::silence`), yani
/// paket sayacının durması gerçekten karşı tarafın gittiği anlamına geliyor.
fn spawn_orphan_watchdog(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("relaudio-watchdog".into())
        .spawn(move || {
            let tick = std::time::Duration::from_secs(2);
            let mut last_seen: Option<(u64, std::time::Instant)> = None;
            loop {
                std::thread::sleep(tick);
                let state = app.state::<AppState>();

                let linked = {
                    let by = state.started_by.lock().unwrap().clone();
                    let peer = state.started_peer.lock().unwrap().clone();
                    by.map(|o| o.name).or(peer.map(|p| p.name))
                };
                let Some(who) = linked else {
                    last_seen = None;
                    continue;
                };

                // Oynatıcı ölmüşse (aygıt çıkarıldı, hata) bekleyecek paket de
                // yok; ama gönderici hâlâ yayında olabilir. Eskiden burada
                // `continue` ediliyordu ve uzaktan açılmış mikrofon sonsuza
                // kadar yayında kalıyordu.
                let packets = state.player.packets();
                let stalled = match (packets, last_seen) {
                    (None, _) => state.server.is_running(),
                    (Some(n), Some((prev, since))) if prev == n => since.elapsed() >= ORPHAN_TIMEOUT,
                    (Some(n), _) => {
                        last_seen = Some((n, std::time::Instant::now()));
                        false
                    }
                };
                if stalled {
                    log::warn!(
                        "kulaklık oturumunun karşı ucu ({who}) {} sn sessiz — akış kapatıldı",
                        ORPHAN_TIMEOUT.as_secs()
                    );
                    *state.started_peer.lock().unwrap() = None;
                    stop_headset_locally(&state);
                    last_seen = None;
                }
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| log::warn!("gözcü thread'i başlatılamadı: {e}"));
}

pub fn show_main<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    //! Kontrol işleyicisinin gerçek kodu üzerinden testler.
    //!
    //! `crates/relaudio-core/tests/pairing_over_the_wire.rs` protokolü kendi
    //! sahte işleyicisiyle sınıyor; uygulamanın asıl `handle_control`'ünü
    //! hiç çalıştırmıyordu. İşleyicide aynı kilidin iki kez alındığı bir
    //! kendi kendini kilitleme (deadlock) o yüzden gözden kaçtı ve iki makinede
    //! de arayüz donuyordu. Bu testler işleyiciyi ayrı bir iş parçacığında
    //! zaman aşımıyla çalıştırıyor: kilitlenme "test asılı kaldı" değil,
    //! "test başarısız" olarak görünsün.
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    /// Testler ayar dosyasını geçici bir dizine yazsın; geliştiricinin kendi
    /// ayarına dokunulmasın.
    fn isolated_config() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let dir = std::env::temp_dir().join(format!("relaudio-app-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var(relaudio_core::config::CONFIG_DIR_ENV, &dir);
        });
    }

    fn state_with_id(id: &str) -> Arc<AppState> {
        isolated_config();
        let state = AppState::default();
        state.config.lock().unwrap().device_id = id.to_string();
        let _ = state.device_id.set(id.to_string());
        Arc::new(state)
    }

    /// İşleyiciyi zaman aşımıyla çalıştırır; takılırsa test başarısız olur.
    fn run(state: &Arc<AppState>, req: ControlRequest) -> ControlReply {
        let (tx, rx) = mpsc::channel();
        let st = state.clone();
        std::thread::spawn(move || {
            let reply = handle_control(&st, &req, "192.168.1.50".parse().unwrap());
            let _ = tx.send(reply);
        });
        rx.recv_timeout(Duration::from_secs(3))
            .expect("kontrol işleyicisi 3 sn içinde cevap vermedi — kilitlenme var")
    }

    const PEER: &str = "0123456789abcdef0123456789abcdef";
    const ME: &str = "fedcba9876543210fedcba9876543210";

    /// Kodu gösteren taraf doğru kodla gelen isteğe cevap vermeli. Eskiden
    /// ayar kilidi elde tutulurken `own_id` aynı kilidi yeniden alıyor ve
    /// iş parçacığı sonsuza kadar kilitleniyordu.
    #[test]
    fn pairing_with_the_right_code_replies_instead_of_deadlocking() {
        let state = state_with_id(ME);
        let code = state.pairing.new_code().unwrap();
        let reply = run(
            &state,
            ControlRequest::Pair { code, id: PEER.into(), name: "karşı".into() },
        );
        assert!(reply.ok, "eşleşme başarısız: {:?}", reply.error);
        assert_eq!(reply.id.as_deref(), Some(ME), "kendi kimliğimizi döndürmeliyiz");
        let key = reply.key.expect("anahtar dönmeli");
        assert_eq!(state.config.lock().unwrap().paired[PEER].key, key);

        // İşleyici kilidi bırakmış olmalı: arayüzün her 500 ms'de çağırdığı
        // istatistik de aynı kilidi alıyor. Bırakmazsa pencere donuyor.
        let (tx, rx) = mpsc::channel();
        let st = state.clone();
        std::thread::spawn(move || {
            let _ = tx.send(st.config.lock().unwrap().paired.len());
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).expect("ayar kilidi bırakılmadı"), 1);
    }

    #[test]
    fn a_wrong_code_is_refused_promptly() {
        let state = state_with_id(ME);
        let code = state.pairing.new_code().unwrap();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        let reply = run(
            &state,
            ControlRequest::Pair { code: wrong.into(), id: PEER.into(), name: "x".into() },
        );
        assert!(!reply.ok);
        assert_eq!(reply.code.as_deref(), Some("pair_wrong"));
    }

    // — İki taraf birlikte —
    //
    // Önceki testler hep tek bir işleyiciyi sınıyordu; asıl kaçan şey iki
    // tarafın birlikte davranışıydı. Burada iki ayrı uygulama durumu kendi
    // kontrol sunucularını açıp gerçek TCP üzerinden konuşuyor. mDNS yerine
    // birbirlerini elle kurulmuş bir `Peer` ile görüyorlar.

    const A_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn side(id: &str) -> (Arc<AppState>, net::ControlServer) {
        let state = state_with_id(id);
        let st = state.clone();
        let server = net::control_serve(0, move |req, from| handle_control(&st, req, from)).unwrap();
        (state, server)
    }

    fn view(id: &str, name: &str, control_port: u16) -> net::Peer {
        net::Peer {
            id: format!("{name}-test"),
            device_id: id.into(),
            name: name.into(),
            address: "127.0.0.1".into(),
            addresses: vec!["127.0.0.1".into()],
            port: 59101,
            control_port,
            os: "windows".into(),
            listening: true,
        }
    }

    fn key_of(state: &AppState, id: &str) -> Option<String> {
        state.config.lock().unwrap().paired.get(id).map(|p| p.key.clone())
    }

    /// Kullanıcının izlediği yol: A kod gösteriyor, kod B'de yazılıyor.
    /// İki taraf da aynı anahtarla bitmeli ve birbirini doğrulayabilmeli.
    #[test]
    fn two_sides_pair_and_both_confirm() {
        let (a, sa) = side(A_ID);
        let (b, sb) = side(B_ID);
        let (a_seen_by_b, b_seen_by_a) = (view(A_ID, "A", sa.port()), view(B_ID, "B", sb.port()));

        let code = a.pairing.new_code().unwrap();
        let name = pair_with(&b, &a_seen_by_b, &code).expect("eşleşme başarısız");
        assert_eq!(name, net::device_name());

        let (ka, kb) = (key_of(&a, B_ID), key_of(&b, A_ID));
        assert!(ka.is_some(), "kodu gösteren taraf anahtarı kaydetmeli");
        assert_eq!(ka, kb, "iki taraf aynı anahtarla bitmeli");
        assert_eq!(verify_with(&a, &b_seen_by_a), "ok");
        assert_eq!(verify_with(&b, &a_seen_by_b), "ok");
    }

    /// Kullanıcının gerçekten düştüğü durum: eşleşme bir tarafta kaldı.
    /// Eşleşmiş sanan taraf kod göstermiyor, öbürü kod bekliyordu. Doğrulama
    /// bayat anahtarı silmeli; sonra yeniden eşleşme iki yönden de çalışmalı.
    #[test]
    fn a_pairing_left_on_one_side_heals_and_can_be_redone() {
        let (a, sa) = side(A_ID);
        let (b, sb) = side(B_ID);
        let (a_seen_by_b, b_seen_by_a) = (view(A_ID, "A", sa.port()), view(B_ID, "B", sb.port()));
        let code = a.pairing.new_code().unwrap();
        pair_with(&b, &a_seen_by_b, &code).unwrap();

        // Cevap yolda kayboldu / B tarafı kaldırdı: yalnızca A'da kaldı.
        b.config.lock().unwrap().paired.remove(A_ID);

        assert_eq!(verify_with(&a, &b_seen_by_a), "dropped");
        assert!(key_of(&a, B_ID).is_none(), "bayat anahtar silinmeli");

        // Bu sefer ters yönden: B kod gösteriyor, A yazıyor.
        let code = b.pairing.new_code().unwrap();
        pair_with(&a, &b_seen_by_a, &code).unwrap();
        assert_eq!(verify_with(&b, &a_seen_by_b), "ok");
        assert_eq!(verify_with(&a, &b_seen_by_a), "ok");
    }

    /// İki makinede aynı anda "Eşleş"e basılırsa iki taraf farklı anahtar
    /// yazabiliyor. Doğrulama sonunda tutarlı bir duruma varmalı: ya ikisi de
    /// aynı anahtarla eşleşmiş ya da ikisi de eşleşmemiş (yeniden eşleşilir).
    #[test]
    fn pressing_pair_on_both_sides_at_once_converges() {
        let (a, sa) = side(A_ID);
        let (b, sb) = side(B_ID);
        let (a_seen_by_b, b_seen_by_a) = (view(A_ID, "A", sa.port()), view(B_ID, "B", sb.port()));
        let (code_a, code_b) = (a.pairing.new_code().unwrap(), b.pairing.new_code().unwrap());

        let (a2, bva) = (a.clone(), b_seen_by_a.clone());
        let (b2, avb) = (b.clone(), a_seen_by_b.clone());
        let t1 = std::thread::spawn(move || pair_with(&a2, &bva, &code_b));
        let t2 = std::thread::spawn(move || pair_with(&b2, &avb, &code_a));
        let _ = (t1.join().unwrap(), t2.join().unwrap());

        for _ in 0..3 {
            verify_with(&a, &b_seen_by_a);
            verify_with(&b, &a_seen_by_b);
        }
        let (ka, kb) = (key_of(&a, B_ID), key_of(&b, A_ID));
        assert_eq!(ka.is_some(), kb.is_some(), "biri eşleşmiş öbürü değil: çıkmaz");
        assert_eq!(ka, kb, "iki taraf eşleşmişse anahtarlar aynı olmalı");
    }

    /// Eşleşmiş bir tarafın imzalı doğrulaması kabul edilmeli; tanınmayan
    /// tarafınki "eşleşme yok" almalı ki o taraf bayat anahtarını silsin.
    #[test]
    fn verify_confirms_a_real_pairing_and_flags_a_missing_one() {
        let state = state_with_id(ME);
        let key = "ab".repeat(32);
        state.config.lock().unwrap().paired.insert(
            PEER.into(),
            PairedPeer { key: key.clone(), name: "karşı".into() },
        );
        let verify = |id: &str, k: &str| {
            let (nonce, ts, mac) = net::sign_request(k, VERIFY_ROLE, 0).unwrap();
            ControlRequest::Verify { auth: ControlAuth { id: id.into(), nonce, ts, mac } }
        };

        assert!(run(&state, verify(PEER, &key)).ok, "geçerli eşleşme doğrulanmalı");

        let r = run(&state, verify("11111111111111111111111111111111", &key));
        assert_eq!(r.code.as_deref(), Some("needs_pairing"), "tanınmayan kimlik");

        let r = run(&state, verify(PEER, &"cd".repeat(32)));
        assert_eq!(r.code.as_deref(), Some("auth_bad"), "farklı anahtar");
    }

    /// Ağda yakalanan imzalı bir doğrulama, imzalı bir "başlat" isteği olarak
    /// yeniden kullanılamamalı: imza rol alanını kapsıyor ve doğrulama
    /// gerçek bir rol değil.
    #[test]
    fn a_captured_verify_signature_cannot_start_a_session() {
        let state = state_with_id(ME);
        let key = "ab".repeat(32);
        state.config.lock().unwrap().paired.insert(
            PEER.into(),
            PairedPeer { key: key.clone(), name: "karşı".into() },
        );
        let (nonce, ts, mac) = net::sign_request(&key, VERIFY_ROLE, 0).unwrap();
        let reply = run(
            &state,
            ControlRequest::StartHeadset {
                role: "remote".into(),
                audio_port: 0,
                name: "saldırgan".into(),
                auth: Some(ControlAuth { id: PEER.into(), nonce, ts, mac }),
            },
        );
        assert!(!reply.ok, "doğrulama imzasıyla oturum başlamamalı");
        assert_eq!(reply.code.as_deref(), Some("auth_bad"));
        assert!(!state.headset_active.load(Ordering::Relaxed));
    }

    /// Eşleşmemiş bir cihazın başlatma isteği reddedilmeli — ses aygıtı
    /// açılmadan, hemen.
    #[test]
    fn an_unpaired_start_request_is_refused_without_touching_audio() {
        let state = state_with_id(ME);
        let reply = run(
            &state,
            ControlRequest::StartHeadset {
                role: "remote".into(),
                audio_port: 59101,
                name: "yabancı".into(),
                auth: None,
            },
        );
        assert!(!reply.ok);
        assert_eq!(reply.code.as_deref(), Some("needs_pairing"));
        assert!(!state.headset_active.load(Ordering::Relaxed));
    }
}

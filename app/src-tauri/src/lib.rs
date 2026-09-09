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
use relaudio_core::{Config, PairedPeer};
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

    /// Bu makinede kulaklık modu uzaktan mı başlatıldı, kim tarafından?
    pub started_by: Option<RemoteOrigin>,
    /// Ekranda gösterilmesi gereken eşleştirme kodu ve kalan saniye.
    /// Karşı taraf kodu kullanınca kayboluyor — arayüz bunu görüp
    /// başlatmayı kendiliğinden tekrar deniyor.
    pub pairing_code: Option<String>,
    pub pairing_seconds: u64,

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

#[tauri::command]
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
    let saved_capture = match role {
        HeadsetRole::Local => &cfg.server_device_input,
        HeadsetRole::Remote => &cfg.server_device_monitor,
    };
    audio::headset_plan(role, &devices, &cfg.player_device, saved_capture)
}

/// Arayüz için plan. Kayıtlı seçimler argüman olarak geliyor, diskten
/// okunmuyor: kullanıcı bir aygıt seçtiğinde `set_config`'in tamamlanmasını
/// beklemeden doğru planı görmeli.
#[tauri::command]
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
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(48)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() { "?".into() } else { trimmed.to_string() }
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

/// Bu makinenin kalıcı kimliği — mDNS örnek adıyla aynı üretim kuralı,
/// eşleştirmenin iki tarafında da aynı dizeyi kullanmak için.
pub fn own_id() -> String {
    sanitize_peer_id(&format!(
        "{}-{}",
        net::device_name(),
        if cfg!(target_os = "windows") { "windows" } else { "linux" }
    ))
}

/// Bir rolü bu makinede fiilen başlatır. Hem yerel düğme hem uzaktan gelen
/// istek buradan geçiyor — iki yol arasında davranış farkı olmasın diye.
/// Hata, `(kod, metin)` çifti olarak dönüyor: kod karşı tarafta çevrilebilsin,
/// metin bilinmeyen bir kodda yedek kalsın.
fn start_headset_locally(
    state: &AppState,
    role: HeadsetRole,
    target: &str,
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

    state
        .player
        .start(cfg.player_port, &play.id, cfg.player_buffer)
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
    // istekte) aynı seçim tekrarlansın.
    {
        let mut c = state.config.lock().unwrap();
        c.headset_role = role.as_str().to_string();
        c.player_device = play.id.clone();
        c.player_mode = if role == HeadsetRole::Remote { "mic".into() } else { "listen".into() };
        match plan.capture_kind {
            DeviceKind::Input => {
                c.server_source = "input".into();
                c.server_device_input = capture.id.clone();
            }
            _ => {
                c.server_source = "monitor".into();
                c.server_device_monitor = capture.id.clone();
            }
        }
        c.save();
    }
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
    state.server.stop();
    state.player.stop();
    *state.started_by.lock().unwrap() = None;

    let cfg = state.config.lock().unwrap().clone();
    if cfg.auto_listen {
        match state.player.start(cfg.player_port, &cfg.player_device, cfg.player_buffer) {
            Ok(()) => {
                announce_port(state, cfg.player_port, true);
                return;
            }
            Err(e) => log::warn!("dinlemeye geri dönülemedi: {e}"),
        }
    }
    announce(state, false);
}

/// Bağlı olduğumuz tarafa "ben durdum" der.
///
/// Durdurma iki yönde de yayılmalı: kullanıcı hangi makinede Durdur'a basarsa
/// bassın diğeri de dursun. Aksi hâlde uzaktan başlatılan makinede durdurunca
/// karşı taraf mikrofonu göndermeye devam ediyordu ve bunu ancak gözcü 12 sn
/// sonra fark ediyordu.
///
/// Kısa zaman aşımı: karşı taraf kapanmış olabilir, kullanıcıyı bekletmeyelim.
/// [`notify_peer_stopped`]'un arka planda çalışan hâli.
///
/// Tepsi menüsü olay döngüsü thread'inde çalışıyor; orada 700 ms'lik bir TCP
/// denemesi menüyü ve pencereyi dondurur.
pub fn notify_peer_stopped_detached<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let target = take_link_target(&app.state::<AppState>());
    let Some((name, addr)) = target else { return };
    let _ = std::thread::Builder::new()
        .name("relaudio-notify-stop".into())
        .spawn(move || send_stop(&name, addr));
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

fn notify_peer_stopped(state: &AppState) {
    if let Some((name, addr)) = take_link_target(state) {
        send_stop(&name, addr);
    }
}

/// Uzaktan gelen kontrol isteğini işler.
///
/// Sesin gideceği adres **istekten değil**, TCP bağlantısının kaynak IP'sinden
/// alınıyor: bu uç üçüncü bir makineye ses yansıtmak için kullanılamasın.
fn handle_control(state: &AppState, req: &ControlRequest, from: std::net::IpAddr) -> ControlReply {
    let me = net::device_name();
    match req {
        ControlRequest::Ping => ControlReply::ok(me, None),

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
                // Karşı taraf zaten *bize* yayın yapıyorsa meşgul değil:
                // iki makinede aynı anda düğmeye basılmış demektir.
                None if state.player.packets().is_some()
                    && state.server.target().is_some_and(|t| {
                        t.rsplit_once(':').map(|(h, _)| h) == Some(&from.to_string()[..])
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
            match start_headset_locally(state, role, &target) {
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
            match state.pairing.redeem(code) {
                Ok(key) => {
                    let who = sanitize_peer_name(name);
                    let peer_id = sanitize_peer_id(id);
                    log::info!("eşleştirildi: {who} [{peer_id}] ({from})");
                    let mut cfg = state.config.lock().unwrap();
                    cfg.paired.insert(
                        peer_id,
                        PairedPeer { key: key.clone(), name: who },
                    );
                    cfg.save();
                    ControlReply::paired(me, own_id(), key)
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
            let we_started_them = linked.as_ref().is_some_and(|p| p.control.ip().to_string() == from_s);

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
    let peer = if args.peer_id.is_empty() {
        None
    } else {
        state
            .discovery
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|d| d.peers().into_iter().find(|p| p.id == args.peer_id))
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

    let plan = start_headset_locally(&state, role, &audio_target).map_err(|(_, e)| e)?;
    // Kendi düğmemize bastık; uzaktan başlatılmış sayılmayız.
    *state.started_by.lock().unwrap() = None;

    let mut out = HeadsetStartResult {
        remote_paired_mic: plan.paired_mic,
        ..Default::default()
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

    let (my_port, paired_key) = {
        let cfg = state.config.lock().unwrap();
        let key = peer
            .as_ref()
            .and_then(|p| cfg.paired.get(&sanitize_peer_id(&p.id)))
            .map(|p| p.key.clone());
        (cfg.player_port, key)
    };

    // Eşleşmemişsek istek göndermenin anlamı yok: karşı taraf reddedecek.
    // Bunun yerine kodu ekranda gösteriyoruz; kullanıcı karşı makinede
    // yazınca eşleşme kuruluyor ve düğmeye bir daha basılıyor.
    let Some(key) = paired_key else {
        out.needs_pairing = true;
        out.pairing_code = state.pairing.new_code();
        out.remote_error_code = Some("needs_pairing".into());
        out.remote_error = Some("not paired yet".into());
        log::info!("eşleşme yok — eşleştirme kodu gösteriliyor");
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
        auth: Some(ControlAuth { id: own_id(), nonce, ts, mac }),
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
                if role == HeadsetRole::Local {
                    out.remote_paired_mic = reply.paired_mic;
                }
                *state.started_peer.lock().unwrap() = Some(StartedPeer {
                    name: sanitize_peer_name(&reply.name),
                    control: addr,
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
            }
            Err(e) => out.remote_error = Some(e.to_string()),
        },
    }
    if let Some(e) = &out.remote_error {
        log::warn!("karşı taraf başlatılamadı ({control_addr}): {e}");
    }
    Ok(out)
}

/// Karşı makinenin ekranındaki kodu girip eşleşir.
///
/// İstek kodu **yazdığımız** makineden, kodu **gösteren** makineye gidiyor.
/// Dönen anahtar iki tarafta da diske yazılıyor; bir daha sorulmuyor.
#[tauri::command(async)]
fn pair_with_peer(
    state: tauri::State<'_, AppState>,
    peer_id: String,
    code: String,
) -> Result<String, String> {
    let peer = state
        .discovery
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|d| d.peers().into_iter().find(|p| p.id == peer_id))
        .ok_or_else(|| "peer_gone".to_string())?;
    if peer.control_port == 0 {
        return Err("no_control".into());
    }
    let addr = std::net::SocketAddr::new(
        peer.address.parse().map_err(|e| format!("{e}"))?,
        peer.control_port,
    );
    let req = ControlRequest::Pair {
        code: code.trim().to_string(),
        id: own_id(),
        name: net::device_name(),
    };
    let reply = net::control_send(addr, &req, CONNECT_TIMEOUT, REPLY_TIMEOUT)
        .map_err(|e| format!("unreachable: {e}"))?;
    if !reply.ok {
        return Err(reply.code.unwrap_or_else(|| "pair_wrong".into()));
    }
    let (Some(key), Some(their_id)) = (reply.key, reply.id) else {
        return Err("pair_bad_reply".into());
    };
    let their_id = sanitize_peer_id(&their_id);
    let name = sanitize_peer_name(&reply.name);
    log::info!("eşleştirildi: {name} [{their_id}]");
    let mut cfg = state.config.lock().unwrap();
    cfg.paired.insert(their_id, PairedPeer { key, name: name.clone() });
    cfg.save();
    Ok(name)
}

/// Ekranda duran kodu iptal eder.
#[tauri::command]
fn cancel_pairing(state: tauri::State<'_, AppState>) {
    state.pairing.clear_code();
}

/// Seçili eşle eşleşilmiş mi? Arayüz düğmeye basmadan önce bunu gösteriyor.
#[tauri::command]
fn is_paired(state: tauri::State<'_, AppState>, peer_id: String) -> bool {
    let id = sanitize_peer_id(&peer_id);
    state.config.lock().unwrap().paired.contains_key(&id)
}

/// Eşleştirmeyi kaldırır.
#[tauri::command]
fn unpair(state: tauri::State<'_, AppState>, peer_id: String) {
    let id = sanitize_peer_id(&peer_id);
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
        .start(args.port, &args.device_id, args.buffer_packets);
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
#[tauri::command]
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
#[tauri::command]
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

#[tauri::command]
fn stats(state: tauri::State<'_, AppState>) -> StatsDto {
    let mut s = StatsDto::default();
    state.server.fill(&mut s);
    state.player.fill(&mut s);
    s.feedback_loop = detect_feedback(&s);
    s.started_by = state.started_by.lock().unwrap().clone();
    if let Some((code, left)) = state.pairing.visible_code() {
        s.pairing_code = Some(code);
        s.pairing_seconds = left;
    }
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
#[tauri::command]
fn local_address() -> Option<String> {
    relaudio_core::net::local_address()
}

#[tauri::command]
fn set_minimize_to_tray(state: tauri::State<'_, AppState>, enabled: bool) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
    let mut c = state.config.lock().unwrap();
    c.minimize_to_tray = enabled;
    c.save();
}

/// Kayıtlı ayarları döndürür.
#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

/// Ayarları kaydeder. Arayüz her anlamlı değişiklikte çağırıyor;
/// kullanıcı aynı seçimleri her açılışta tekrar yapmasın.
#[tauri::command]
fn set_config(app: tauri::AppHandle, state: tauri::State<'_, AppState>, config: Config) {
    state.minimize_to_tray.store(config.minimize_to_tray, Ordering::Relaxed);
    let enabled = config.remote_control;
    // Yazma kilit altında: iki `set_config` çakıştığında dosya yarı yazılmış
    // hâlde kalabiliyordu ve sonraki açılışta ayarlar sıfırlanıyordu.
    let changed = {
        let mut guard = state.config.lock().unwrap();
        let changed = guard.remote_control != config.remote_control;
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
            let (auto_listen, port, device, buffer, remote_control) = (
                cfg.auto_listen,
                cfg.player_port,
                cfg.player_device.clone(),
                cfg.player_buffer,
                cfg.remote_control,
            );
            *state.config.lock().unwrap() = cfg;

            // Kontrol kanalı keşiften ÖNCE açılıyor: fiilen bağlanılan port
            // mDNS'te ilan edilecek ve 59100 doluysa farklı olabiliyor.
            // Ayar kapalıysa hiç bağlanmıyor — bkz. `Config::remote_control`.
            let control_port = open_control_server(app.handle(), remote_control);

            // Keşfi hemen başlat: kullanıcı IP yazmak zorunda kalmasın.
            match Discovery::start(port, control_port) {
                Ok(d) => *state.discovery.lock().unwrap() = Some(d),
                Err(e) => log::warn!("ağ keşfi başlatılamadı: {e} — IP elle girilebilir"),
            }

            spawn_orphan_watchdog(app.handle().clone());

            // Oynatıcıyı otomatik başlat: uygulama açık olduğu sürece ses
            // alabilir durumda olsun. Aygıt yoksa sessizce geç — kullanıcı
            // Oynatıcı sekmesinden elle başlatabilir.
            if auto_listen {
                match state.player.start(port, &device, buffer) {
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
            cancel_pairing,
            is_paired,
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

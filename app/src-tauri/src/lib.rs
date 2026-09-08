//! RelAudio masaüstü uygulaması — Tauri katmanı.
//!
//! Bu katman **ince** tutulur (docs/02): pencere, tepsi, ayarlar ve çekirdeğe
//! köprü. Ses koduna dokunmaz; onun tamamı `relaudio-core` içinde.

mod session;
mod tray;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, WindowEvent};

use relaudio_core::audio::{self, DeviceKind};
use relaudio_core::net::Discovery;
use session::{PlayerSession, ServerSession};
use std::sync::Mutex;

pub const DEFAULT_PORT: u16 = 59101;

pub struct AppState {
    pub server: ServerSession,
    pub player: PlayerSession,
    /// mDNS ilanı ve taraması. Uygulama açılışında başlar, kapanana kadar yaşar.
    pub discovery: Mutex<Option<Discovery>>,
    /// Tepsi menüsündeki "Çıkış" bunu set eder; yalnızca o zaman gerçekten kapanırız.
    pub is_quitting: AtomicBool,
    /// Kapatma düğmesi tepsiye küçültsün mü? Tepsi yaratılamazsa kapatılır.
    pub minimize_to_tray: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            server: ServerSession::default(),
            player: PlayerSession::default(),
            discovery: Mutex::new(None),
            is_quitting: AtomicBool::new(false),
            minimize_to_tray: AtomicBool::new(true),
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
}

#[derive(Serialize)]
pub struct DeviceDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub is_default: bool,
}

#[derive(Serialize, Default)]
pub struct StatsDto {
    pub server_running: bool,
    pub server_target: String,
    pub server_packets: u64,
    pub server_kbps: f64,
    pub server_silent_ratio: f64,

    pub player_running: bool,
    pub player_port: u16,
    pub player_packets: u64,
    pub player_kbps: f64,
    pub player_lost: u64,
    pub player_late: u64,
    pub player_underruns: u64,
    pub player_dropped: u64,
    pub player_buffer_ms: u64,

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
        .map(|v| {
            v.into_iter()
                .map(|d| DeviceDto {
                    id: d.id,
                    name: d.name,
                    kind: d.kind.as_str().to_string(),
                    is_default: d.is_default,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
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

#[tauri::command]
fn stop_server(state: tauri::State<'_, AppState>) {
    state.server.stop();
}

#[tauri::command]
fn start_player(state: tauri::State<'_, AppState>, args: StartPlayerArgs) -> Result<(), String> {
    let r = state
        .player
        .start(args.port, &args.device_id, args.buffer_packets);
    if r.is_ok() {
        announce(&state, true);
    }
    r
}

#[tauri::command]
fn stop_player(state: tauri::State<'_, AppState>) {
    state.player.stop();
    announce(&state, false);
}

/// Ağa "ses alabiliyorum / alamıyorum" bilgisini yayar.
fn announce(state: &AppState, listening: bool) {
    if let Some(d) = state.discovery.lock().unwrap().as_ref() {
        if let Err(e) = d.announce(listening) {
            log::warn!("mDNS ilanı güncellenemedi: {e}");
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
    s
}

/// Bu makinenin yerel adresi — karşı tarafa yazılacak olan.
#[tauri::command]
fn local_address() -> Option<String> {
    relaudio_core::net::local_address()
}

#[tauri::command]
fn set_minimize_to_tray(state: tauri::State<'_, AppState>, enabled: bool) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
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
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    // Sona ekle — sıfırlarsak önceki çökmenin izi kaybolur. Dosya çok
    // büyürse baştan başla.
    if std::fs::metadata(&path).map(|m| m.len() > 2_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
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
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // İkinci örnek: yeni pencere açma, mevcut olanı öne getir (docs/06).
            show_main(app);
        }))
        .manage(AppState::default())
        .setup(|app| {
            log::info!("setup başladı");
            let state = app.state::<AppState>();

            // Keşfi hemen başlat: kullanıcı IP yazmak zorunda kalmasın.
            match Discovery::start(DEFAULT_PORT) {
                Ok(d) => *state.discovery.lock().unwrap() = Some(d),
                Err(e) => log::warn!("ağ keşfi başlatılamadı: {e} — IP elle girilebilir"),
            }

            // Oynatıcıyı otomatik başlat: uygulama açık olduğu sürece ses
            // alabilir durumda olsun. Aygıt yoksa sessizce geç — kullanıcı
            // Oynatıcı sekmesinden elle başlatabilir.
            match state.player.start(DEFAULT_PORT, "", 8) {
                Ok(()) => {
                    log::info!("oynatıcı otomatik başladı — port {DEFAULT_PORT}");
                    announce(&state, true);
                }
                Err(e) => log::warn!("oynatıcı otomatik başlatılamadı: {e}"),
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

pub fn show_main<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

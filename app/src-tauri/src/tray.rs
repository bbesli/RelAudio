//! Sistem tepsisi. Davranış kuralları docs/06-arkaplan-ve-tepsi.md.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::{announce, show_main, AppState};
use std::sync::atomic::Ordering;

/// Menüyü verilen etiketlerle kurar. İlk kurulumda İngilizce kullanılıyor;
/// arayüz yüklenince [`update_labels`] ile kullanıcının diline çevriliyor.
fn menu<R: Runtime>(
    app: &AppHandle<R>,
    show_l: &str,
    stop_l: &str,
    quit_l: &str,
) -> tauri::Result<Menu<R>> {
    let show = MenuItem::with_id(app, "show", show_l, true, None::<&str>)?;
    let stop_all = MenuItem::with_id(app, "stop_all", stop_l, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_l, true, Some("CmdOrCtrl+Q"))?;
    Menu::with_items(
        app,
        &[&show, &stop_all, &PredefinedMenuItem::separator(app)?, &quit],
    )
}

/// Tepsi etiketlerini değiştirir (dil seçimi sonrası).
pub fn update_labels<R: Runtime>(
    app: &AppHandle<R>,
    show: &str,
    stop_all: &str,
    quit: &str,
) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu(app, show, stop_all, quit)?))?;
    }
    Ok(())
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = menu(app, "Show", "Stop all streams", "Quit")?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(true) // macOS menü çubuğu için (ertelendi ama zararsız)
        .tooltip("RelAudio")
        .menu(&menu)
        // Sol tık menüyü açmasın; pencereyi açsın (Windows davranışı).
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "stop_all" => {
                let state = app.state::<AppState>();
                state.server.stop();
                state.player.stop();
                // Karşı taraf da dursun — yalnızca bizim durmamız onu yayında
                // bırakıyordu — ve uzaktan başlatma izi silinsin, yoksa
                // arayüzdeki "kulaklık modunu X başlattı" bandı artık var
                // olmayan bir oturumu göstermeye devam ediyor.
                crate::notify_peer_stopped_detached(app);
                *state.started_by.lock().unwrap() = None;
                // Eşler bu makineyi hâlâ "dinliyor" görüyordu; ilanı düzelt.
                announce(&state, false);
            }
            "quit" => {
                let state = app.state::<AppState>();
                state.is_quitting.store(true, Ordering::Relaxed);
                // Bağlı olduğumuz taraf bizsiz yayında kalmasın.
                crate::notify_peer_stopped(&state);
                state.server.stop();
                state.player.stop();
                *state.started_by.lock().unwrap() = None;
                // Çıkmadan önce "artık dinlemiyorum" de ve mDNS kaydını kaldır;
                // aksi hâlde eşler dakikalarca ölü bir hedefi listeliyor.
                announce(&state, false);
                if let Some(d) = state.discovery.lock().unwrap().as_ref() {
                    d.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // DİKKAT: Linux'ta (StatusNotifierItem) bu olay gelmez —
            // "Göster" menü öğesi bu yüzden zorunlu (docs/06).
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

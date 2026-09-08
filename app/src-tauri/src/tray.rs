//! Sistem tepsisi. Davranış kuralları docs/06-arkaplan-ve-tepsi.md.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::{show_main, AppState};
use std::sync::atomic::Ordering;

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Göster", true, None::<&str>)?;
    let stop_all = MenuItem::with_id(app, "stop_all", "Tüm yayınları durdur", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Çıkış", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(
        app,
        &[&show, &stop_all, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

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
            }
            "quit" => {
                let state = app.state::<AppState>();
                state.is_quitting.store(true, Ordering::Relaxed);
                state.server.stop();
                state.player.stop();
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

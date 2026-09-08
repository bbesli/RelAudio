# 06 — Arka Plan Çalışma ve Sistem Tepsisi

Gereksinim: **Pencerenin çarpı düğmesine basıldığında uygulama kapanmaz;
sistem tepsisinde/menü çubuğunda kalır ve simgeye tıklanınca geri açılır.**
(Ekran görüntüsündeki Discord davranışı.)

Bu, üç platformda üç farklı sistemdir ve her birinin kendi tuzağı vardır.

---

## Ortak davranış modeli

Uygulamanın üç durumu var:

| Durum | Pencere | Tepsi simgesi | Çekirdek süreç |
|---|---|---|---|
| Görünür | Açık | Var | Çalışıyor |
| Gizli (arka plan) | Gizli, yok edilmemiş | Var | Çalışıyor |
| Kapalı | Yok | Yok | Durmuş |

Kurallar:

1. **Çarpı → gizle.** Pencere `hide()` edilir, `destroy()` edilmez. Yeniden
   göstermek anlıktır (yeniden yükleme yok, durum korunur).
2. **Çıkış yalnızca açık niyetle.** Tepsi menüsündeki "Çıkış", uygulama menüsü
   ve `Ctrl/Cmd+Q` ile. Bu bayrak (`is_quitting`) set edilir, `close` olayı
   artık engellenmez.
3. **Tek örnek (single instance).** İkinci kez çalıştırılırsa yeni süreç
   ölür, var olan pencereyi öne getirir. Bu olmadan tepside iki simge oluşur.
4. **İlk çarpıda bilgilendir.** İlk kez gizlenirken bir bildirim gösterilir:
   "RelAudio arka planda çalışmaya devam ediyor." Bir kez gösterilir, ayarda kapatılabilir.
5. **Ayarla değiştirilebilir.** "Kapatınca tepsiye küçült" seçeneği kapatılırsa
   çarpı gerçekten kapatır. Bazı kullanıcılar bunu ister.
6. **Yayın sürerken çıkışta onay.** Aktif oturum varken "Çıkış" seçilirse onay istenir.

---

## Tepsi menüsü içeriği

```
┌──────────────────────────────┐
│ RelAudio                     │  ← başlık, tıklanamaz
│ ● BURAKBESLI'ye gönderiliyor │  ← canlı durum satırı
├──────────────────────────────┤
│ Göster                       │
│ Yayını duraklat              │
├──────────────────────────────┤
│ Sunucu                     ▸ │  ← hızlı mod değişimi
│ Oynatıcı                   ▸ │
├──────────────────────────────┤
│ Ayarlar                      │
│ Çıkış                Ctrl+Q  │
└──────────────────────────────┘
```

Simge durumu yansıtır: boşta (gri), yayında (renkli), hata (uyarı işareti).
macOS'ta simge **template image** olmalıdır (tek renk + alfa), yoksa koyu/açık
menü çubuğunda kötü görünür.

---

## Tauri v2 ile uygulama

Gerekli eklentiler:

```toml
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-single-instance = "2"
tauri-plugin-autostart = "2"
tauri-plugin-window-state = "2"
tauri-plugin-notification = "2"
```

### Pencere kapanışını yakalamak

```rust
use tauri::{Manager, WindowEvent};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // İkinci örnek: var olan pencereyi öne getir
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if !state.is_quitting.load(Ordering::SeqCst)
                    && state.minimize_to_tray.load(Ordering::SeqCst)
                {
                    api.prevent_close();       // ← kapanmayı iptal et
                    let _ = window.hide();     // ← sadece gizle

                    #[cfg(target_os = "macos")]
                    let _ = window.app_handle()
                        .set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running");
}
```

`tauri.conf.json` içinde pencere ilk açılışta gizli başlatılabilir
(oturum açılışında sessiz başlatma için):

```json
{ "app": { "windows": [{ "label": "main", "visible": false }] } }
```

### Tepsi simgesi

```rust
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

let show  = MenuItem::with_id(app, "show", "Göster", true, None::<&str>)?;
let pause = MenuItem::with_id(app, "pause", "Yayını duraklat", true, None::<&str>)?;
let quit  = MenuItem::with_id(app, "quit", "Çıkış", true, Some("CmdOrCtrl+Q"))?;
let menu  = Menu::with_items(app, &[
    &show, &pause, &PredefinedMenuItem::separator(app)?, &quit,
])?;

TrayIconBuilder::with_id("main-tray")
    .icon(app.default_window_icon().unwrap().clone())
    .icon_as_template(true)          // macOS menü çubuğu için
    .tooltip("RelAudio")
    .menu(&menu)
    .show_menu_on_left_click(false)  // sol tık = pencereyi aç (Win/macOS)
    .on_menu_event(|app, event| match event.id.as_ref() {
        "show" => show_main(app),
        "quit" => {
            app.state::<AppState>().is_quitting.store(true, Ordering::SeqCst);
            app.exit(0);
        }
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        // DİKKAT: Linux'ta (StatusNotifierItem) bu olay gelmez.
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up, ..
        } = event {
            show_main(tray.app_handle());
        }
    })
    .build(app)?;
```

### Otomatik başlatma

```rust
use tauri_plugin_autostart::MacosLauncher;

.plugin(tauri_plugin_autostart::init(
    MacosLauncher::LaunchAgent,
    Some(vec!["--hidden"]),   // ← oturum açılışında pencere açma
))
```

`--hidden` argümanı okunup pencere `show()` edilmez.

---

## Electron ile uygulama

Alternatif yığın seçilirse karşılıkları:

```js
const { app, BrowserWindow, Tray, Menu, nativeImage } = require('electron')

let win, tray
app.isQuitting = false

// 1) Tek örnek
if (!app.requestSingleInstanceLock()) app.quit()
app.on('second-instance', () => { showMain() })

// 2) Tüm pencereler kapansa da çıkma
app.on('window-all-closed', () => { /* bilerek boş */ })

// 3) Çarpı → gizle
function createWindow () {
  win = new BrowserWindow({
    show: false,
    webPreferences: { backgroundThrottling: false }  // arka planda kısılmasın
  })
  win.on('close', (e) => {
    if (!app.isQuitting && settings.minimizeToTray) {
      e.preventDefault()
      win.hide()
      if (process.platform === 'darwin') app.dock.hide()
    }
  })
}

// 4) macOS: Dock simgesine tıklama
app.on('activate', () => showMain())

function showMain () {
  if (!win) createWindow()
  win.show(); win.focus()
  if (process.platform === 'darwin') app.dock.show()
}

// 5) Tepsi
function createTray () {
  tray = new Tray(nativeImage.createFromPath(iconPath))
  tray.setToolTip('RelAudio')
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: 'Göster', click: showMain },
    { type: 'separator' },
    { label: 'Çıkış', accelerator: 'CmdOrCtrl+Q',
      click: () => { app.isQuitting = true; app.quit() } },
  ]))
  tray.on('click', showMain)   // Linux'ta tetiklenmez
}

// 6) Otomatik başlatma (Win/macOS)
app.setLoginItemSettings({ openAtLogin: true, args: ['--hidden'] })
```

Electron'da ek olarak: çekirdek sidecar süreci `app.on('before-quit')` içinde
düzgün sonlandırılmalı, aksi hâlde zombi süreç kalır.

---

## Platform tuzakları

### Windows

- **Simge gizli kutuya düşer.** Windows 11'de yeni uygulamaların tepsi simgesi
  varsayılan olarak taşma menüsünde saklanır. Kullanıcı "RelAudio kapandı"
  sanır. Çözüm: ilk gizlemede balon bildirimi göster ve ayarlarda "Simgeyi
  görev çubuğunda sabitle" talimatını göster. Programatik olarak sabitlemek
  desteklenmiyor.
- **Otomatik başlatma:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
  Task Scheduler alternatifi daha güvenilirdir ama kurulumda yönetici ister.
- **Bildirimler için AppUserModelID** ayarlanmalı, yoksa toast'lar görünmez.
- Explorer çökerse tepsi simgesi kaybolur; `TaskbarCreated` mesajını dinleyip
  yeniden yaratmak gerekir (Tauri/Electron bunu çoğunlukla halleder, test edilmeli).

### macOS *(ertelendi — kapsam dışı)*

- **Menü çubuğu ≠ tepsi.** Simge sağ üstte, menü çubuğunda görünür.
- **Çarpı zaten uygulamayı kapatmaz** — macOS'ta normal davranış budur.
  Kullanıcı beklentisi Windows'takinden farklı; aynı kodu uygulamak yeterli.
- **Dock simgesi:** Pencere gizlendiğinde Dock'ta durmasını istemiyorsak
  `ActivationPolicy::Accessory` (Electron'da `app.dock.hide()`). Ama bu, Cmd+Tab
  ile erişimi de kaldırır. **Öneri: Dock'ta kalsın** (uygulama bir "menü çubuğu
  aracı" değil, tam bir uygulama). Ayarda seçenek sunulabilir.
- **Simge template olmalı** (`icon_as_template(true)`), yoksa koyu menü
  çubuğunda kötü görünür.
- **App Nap:** Arka plandaki uygulama askıya alınabilir. Ses akışı sürerken
  `NSProcessInfo.beginActivity(.userInitiated, reason:)` ile önlenmelidir.
  Çekirdek ayrı süreç olduğu için asıl kritik olan orası.
- **Otomatik başlatma:** `SMAppService` (macOS 13+) ile LaunchAgent kaydı.

### Linux — **en sorunlu platform**

- **Eski XEmbed tepsi protokolü öldü.** Modern masaüstleri
  **StatusNotifierItem (SNI)** kullanır; D-Bus üzerinden çalışır.
  Uygulama tarafında `libayatana-appindicator3` veya `libappindicator3`
  gerekir — Tauri ve Electron ikisi de bunu kullanır. **Paketleme
  bağımlılığı olarak eklenmeli.**
- **GNOME'da varsayılan olarak tepsi YOKTUR.** Kullanıcının
  *AppIndicator and KStatusNotifierItem Support* eklentisini kurması gerekir.
  KDE, XFCE, Cinnamon, MATE, Budgie'de sorunsuz çalışır.
  → **Zorunlu tasarım kararı:** Uygulama tepsi simgesi olmadan da kullanılabilir
    olmalı. Simge yaratılamazsa:
    - Kullanıcıya bir kez açıklayıcı uyarı göster.
    - "Kapatınca tepsiye küçült" davranışını otomatik kapat (aksi hâlde
      uygulama erişilemez hâle gelir — **kritik hata senaryosu**).
    - `.desktop` dosyasına ikinci kez çalıştırıldığında pencereyi açan davranış
      zaten single-instance ile mevcut; kullanıcı uygulama menüsünden geri açar.
- **Tepsi simgesine sol tık olayı gelmez.** SNI'da sol tık davranışı masaüstü
  ortamına bağlıdır; çoğu ortamda yalnızca menü açılır.
  → **"Göster" menü öğesi zorunludur**, sol tıka güvenilemez.
- **Wayland:** tepsi D-Bus üzerinden çalıştığı için Wayland'da da sorun yok;
  ancak pencere konumlandırma ve `set_focus` davranışı X11'den farklı olabilir.
- **Otomatik başlatma:** `~/.config/autostart/relaudio.desktop`:
  ```ini
  [Desktop Entry]
  Type=Application
  Name=RelAudio
  Exec=relaudio --hidden
  X-GNOME-Autostart-enabled=true
  ```
- Flatpak'te autostart portal üzerinden (`org.freedesktop.portal.Background`).

---

## Test listesi

| # | Senaryo | Beklenen |
|---|---|---|
| 1 | Yayın sürerken çarpıya bas | Pencere gizlenir, ses **kesilmez** |
| 2 | Tepsi simgesine tıkla / "Göster" | Pencere aynı durumla geri gelir |
| 3 | Uygulamayı ikinci kez çalıştır | Yeni pencere/simge açılmaz, mevcut öne gelir |
| 4 | Tepsi menüsünden "Çıkış" | Uygulama **ve çekirdek** tamamen sonlanır |
| 5 | Yayın sürerken "Çıkış" | Onay istenir |
| 6 | Oturumu kapat/aç (autostart açık) | Pencere açılmadan tepside başlar, önceki oturum isteğe bağlı sürdürülür |
| 7 | GNOME (eklenti yok) | Uyarı gösterilir, çarpı gerçekten kapatır, uygulama erişilebilir kalır |
| 8 | KDE / XFCE / Cinnamon | Simge ve menü çalışır |
| 9 | Windows 11, simge taşma menüsünde | Bildirim gösterilir |
| 11 | Uzun süre gizli (30+ dk) | Arka plan kısıtlaması yok, ses düzgün |
| 12 | UI süreci öldürülür (`kill`) | Ses **devam eder**, uygulama yeniden açılınca oturuma bağlanır |

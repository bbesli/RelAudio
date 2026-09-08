// Windows'ta konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        // WebKitGTK'nın DMABUF renderer'ı bazı Wayland kompozitörlerinde
        // pencere açılır açılmaz "Gdk-Message: Error 71 (Protocol error)" ile
        // çökmeye yol açıyor. KDE/Wayland'da doğrulandı; devre dışı bırakınca
        // yerel Wayland'da sorunsuz çalışıyor (X11'e düşmeye gerek yok).
        // Kullanıcı açıkça bir değer verdiyse ona dokunmuyoruz.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    relaudio_app_lib::run()
}

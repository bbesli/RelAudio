#!/usr/bin/env bash
# RelAudio'yu kullanıcı dizinine kurar — uygulama menüsünde görünür hâle gelir.
# Yönetici yetkisi gerektirmez; her şey ~/.local altına gider.
set -euo pipefail

ROOT="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
GUI="$ROOT/app/src-tauri/target/release/relaudio-app"
CLI="$ROOT/target/release/relaudio-cli"

if [ ! -x "$GUI" ]; then
  echo "No built application found. First run:"
  echo "  cd $ROOT/app && npx tauri build --no-bundle"
  exit 1
fi

# GUI -> `relaudio`, CLI -> `relaudio-cli`.
# Aynı ada kurulurlarsa README'deki teşhis komutları (relaudio tone, level…)
# arayüzü açıyordu; argüman yok sayıldığı için sessizce yanlış şey oluyordu.
install -Dm755 "$GUI" "$HOME/.local/bin/relaudio"
if [ -x "$CLI" ]; then
  install -Dm755 "$CLI" "$HOME/.local/bin/relaudio-cli"
else
  echo "Note: CLI not built — run  cargo build --release --bin relaudio-cli  to get the diagnostic tool"
fi
install -Dm644 "$ROOT/app/src-tauri/icons/128x128.png" \
  "$HOME/.local/share/icons/hicolor/128x128/apps/relaudio.png"
install -Dm644 "$ROOT/app/src-tauri/icons/icon.png" \
  "$HOME/.local/share/icons/hicolor/512x512/apps/relaudio.png"

mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/relaudio.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=RelAudio
Comment=Yerel ağ üzerinden düşük gecikmeli ses aktarımı
Exec=$HOME/.local/bin/relaudio
Icon=relaudio
Terminal=false
Categories=AudioVideo;
StartupWMClass=relaudio-app
DESKTOP

command -v update-desktop-database >/dev/null && \
  update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && \
  gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "Installed."
echo "  App  : ~/.local/bin/relaudio        (GUI, also in your application menu)"
echo "  CLI  : ~/.local/bin/relaudio-cli    (diagnostics: devices, tone, level)"
echo
echo "If the commands are not found, ~/.local/bin is not on your PATH."

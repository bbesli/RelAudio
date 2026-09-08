#!/usr/bin/env bash
# RelAudio'yu kullanıcı dizinine kurar — uygulama menüsünde görünür hâle gelir.
# Yönetici yetkisi gerektirmez; her şey ~/.local altına gider.
set -euo pipefail

ROOT="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
BIN="$ROOT/app/src-tauri/target/release/relaudio-app"

if [ ! -x "$BIN" ]; then
  echo "Derlenmiş uygulama yok. Önce:"
  echo "  cd $ROOT/app && npx tauri build --no-bundle"
  exit 1
fi

install -Dm755 "$BIN" "$HOME/.local/bin/relaudio"
install -Dm644 "$ROOT/app/src-tauri/icons/128x128.png" \
  "$HOME/.local/share/icons/hicolor/128x128/apps/relaudio.png"
install -Dm644 "$ROOT/app/src-tauri/icons/icon.png" \
  "$HOME/.local/share/icons/hicolor/512x512/apps/relaudio.png"

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

echo "Kuruldu."
echo "  Çalıştırılabilir : ~/.local/bin/relaudio"
echo "  Menü girdisi     : ~/.local/share/applications/relaudio.desktop"
echo
echo "Uygulama menüsünde 'RelAudio' olarak görünmeli."
echo "~/.local/bin PATH'te değilse terminalden tam yolla çalıştır."

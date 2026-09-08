#!/usr/bin/env bash
# install-linux.sh ile kurulanları geri alır.
set -euo pipefail
rm -f "$HOME/.local/bin/relaudio" \
      "$HOME/.local/share/applications/relaudio.desktop" \
      "$HOME/.local/share/icons/hicolor/128x128/apps/relaudio.png" \
      "$HOME/.local/share/icons/hicolor/512x512/apps/relaudio.png"
command -v update-desktop-database >/dev/null && \
  update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
echo "Kaldırıldı."

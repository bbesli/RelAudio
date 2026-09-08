#!/usr/bin/env bash
# Adım 1 — Gönderici (Linux): sistem sesi → UDP
# Kullanım:  ./1a-send.sh <hedef-ip> [sink-adı]
set -uo pipefail

HOST="${1:?kullanım: $0 <hedef-ip> [sink-adi]}"
PORT="${PORT:-59101}"
SINK="${2:-$(pactl get-default-sink)}"
MON="${SINK}.monitor"

echo "kaynak : $MON"
echo "hedef  : $HOST:$PORT"
echo "Ctrl+C ile durdur."
echo

exec parecord --device="$MON" --raw --rate=48000 --channels=2 \
     --format=s16le --latency-msec=10 \
  | socat -u -b 960 - "UDP-DATAGRAM:$HOST:$PORT"

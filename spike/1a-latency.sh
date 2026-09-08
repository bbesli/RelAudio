#!/usr/bin/env bash
# Adım 1a — Gecikme ölçümü (yankı treni, null sink içinde, SESSİZ)
#
# Bulgu: `pw-record --target` / `pw-play --target` PipeWire 1.6.8'de aygıt
# adlarını çözemiyor (sessiz yakalama). `parecord --device` / `paplay --device`
# çalışıyor. Bu betik pulse araçlarını kullanır.
#
# Ölçüm bir null sink içinde döner: fiziksel çıkışa hiç ses gitmez.
set -uo pipefail

D="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
OUT=/tmp/relaudio-spike; mkdir -p "$OUT"; cd "$OUT"
PORT=59101
LATMS="${1:-10}"          # ms
FORCE="${2:-}"
QUANT=$(( 48 * LATMS ))

echo "ölçüm    : null sink içinde (sessiz)"
echo "latency  : ${LATMS} ms istendi"
MOD=""
cleanup() {
  pkill -x socat 2>/dev/null; pkill -x paplay 2>/dev/null; pkill -x parecord 2>/dev/null
  pkill -x tee 2>/dev/null
  [ -n "$MOD" ] && pactl unload-module "$MOD" 2>/dev/null
  [ "$FORCE" = "force" ] && pw-metadata -n settings 0 clock.force-quantum 0 >/dev/null 2>&1
}
trap cleanup EXIT

if [ "$FORCE" = "force" ]; then
  echo "         clock.force-quantum=$QUANT zorlanıyor"
  pw-metadata -n settings 0 clock.force-quantum "$QUANT" >/dev/null 2>&1
fi
echo

MOD=$(pactl load-module module-null-sink sink_name=relaudio_meas \
      sink_properties=device.description=RelAudioMeas)
sleep 0.6

python3 "$D/gen-single.py" single.raw
rm -f echo.raw

# 1) Alıcı: UDP → null sink  (sink'i aktif eder)
timeout 12 socat -u "UDP-RECV:$PORT,reuseaddr" - \
  | timeout 12 paplay --device=relaudio_meas --raw --rate=48000 --channels=2 \
      --format=s16le --latency-msec="$LATMS" &
sleep 1.0

# 2) TEK yakalama akışı: null sink monitor → (dosya + UDP)
timeout 10 parecord --device=relaudio_meas.monitor --raw --rate=48000 --channels=2 \
      --format=s16le --latency-msec="$LATMS" \
  | stdbuf -o0 tee echo.raw \
  | timeout 10 socat -u -b 960 - "UDP-DATAGRAM:127.0.0.1:$PORT" &
sleep 1.0

# 3) Tek patlamayı null sink'e çal
timeout 8 paplay --device=relaudio_meas --raw --rate=48000 --channels=2 \
      --format=s16le single.raw >/dev/null 2>&1
sleep 1.2

pkill -x socat 2>/dev/null; pkill -x paplay 2>/dev/null; pkill -x parecord 2>/dev/null
sleep 0.4

echo "════════════════ SONUÇ ════════════════"
SZ=$(stat -c%s echo.raw 2>/dev/null || echo 0)
echo "  yakalama: $SZ bayt ($(python3 -c "print(f'{$SZ/192000:.2f}')")s)"
python3 "$D/measure.py" echo.raw
echo
echo "Ölçüm anındaki kuantum:"
timeout 8 pw-top -b -n 2 2>/dev/null | grep -E "^R" | awk '$3>0 {printf "  %s -> %.1f ms\n", $NF, $3/$4*1000}' | sort -u | head -4

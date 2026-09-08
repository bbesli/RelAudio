#!/usr/bin/env bash
# Adım 1 — Alıcı: UDP → bu makinenin çıkışı
# Windows spike'ından gelen sesi burada duymak için.
#
# Kullanım:  ./1a-recv.sh [sink-adı]
#
# Not: pw-play/pw-record --target PipeWire 1.6.8'de aygıt adını çözemiyor
# (sessizce başarısız oluyor), bu yüzden pulse araçları kullanılıyor.
set -uo pipefail

PORT="${PORT:-59101}"
SINK="${1:-$(pactl get-default-sink)}"
OUT=/tmp/relaudio-spike; mkdir -p "$OUT"

echo "dinleniyor : 0.0.0.0:$PORT"
echo "çıkış      : $SINK"
echo "kayıt      : $OUT/recv.raw  (analiz için)"
echo
echo "Ctrl+C ile durdur. Durdurunca özet verilir."
echo

cleanup() {
  echo; echo "════════ ÖZET ════════"
  python3 - "$OUT/recv.raw" <<'PY'
import struct, sys
try: d = open(sys.argv[1], 'rb').read()
except FileNotFoundError: print("  kayıt yok"); sys.exit()
n = len(d) // 4
if n == 0: print("  hiç veri gelmedi"); sys.exit()
s = struct.unpack(f'<{n*2}h', d[:n*4])
peak = max((abs(v) for v in s), default=0)
nz = sum(1 for v in s if v != 0)
print(f"  alınan   : {len(d)} bayt = {n/48000:.2f} s ses")
print(f"  tepe     : {peak}   sıfır olmayan örnek: {nz*100//max(len(s),1)}%")
print("  SONUÇ    :", "SES GELDİ ✓" if peak > 500 else "veri geldi ama sessiz")
PY
}
trap cleanup EXIT

exec socat -u "UDP-RECV:$PORT,reuseaddr" - \
  | tee "$OUT/recv.raw" \
  | paplay --device="$SINK" --raw --rate=48000 --channels=2 --format=s16le --latency-msec=30

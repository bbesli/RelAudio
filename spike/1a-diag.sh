#!/usr/bin/env bash
# Adım 1a — Teşhis: hattın her parçasını ayrı ayrı test eder.
# Null sink kullanmaz (onlar suspend oluyordu), gerçek varsayılan aygıtı kullanır.
# ATILACAK SPIKE KODU.
set -uo pipefail

OUT=/tmp/relaudio-spike; mkdir -p "$OUT"; cd "$OUT"
SINK="$(pactl get-default-sink)"
MON="${SINK}.monitor"
PORT=59101
LAT="${1:-256}"

echo "ÖNEMLİ: Çıkış kodları yanıltıcı olabilir — süreç sonlanmasa bile ses"
echo "çalıyor olabilir. ASIL SORU: tik seslerini DUYUYOR MUSUN?"
echo
echo "sink    : $SINK"
echo "monitor : $MON"
echo "latency : $LAT örnek (~$(python3 -c "print(f'{$LAT/48000*1000:.1f}')") ms)"
echo "════════════════════════════════════════════════════════"
echo "TEST 0 — grafiğin gerçek kuantumu (gecikme tavanımız)"
timeout 10 pw-top -b -n 3 2>/dev/null | grep -E "^R" | grep -vE "rate 0|F32P 2 0" | awk '$3>0 {printf "  %s: QUANT=%s RATE=%s -> %.1f ms\n", $NF, $3, $4, $3/$4*1000}' | sort -u | head -5
echo

peak() { python3 -c "
import struct,sys
try: d=open(sys.argv[1],'rb').read()
except FileNotFoundError: print('0 0.00'); sys.exit()
n=len(d)//4
s=struct.unpack(f'<{n*2}h',d[:n*4]) if n else ()
print(max((abs(v) for v in s),default=0), f'{n/48000:.2f}')
" "$1"; }

python3 "$(dirname "$(readlink -f "$0")")/gen-click.py" click.raw >/dev/null 2>&1 \
  || python3 /run/media/bbesli/DEPO/Development_Projects/RelAudio/spike/gen-click.py click.raw

# ── TEST 1: playback (pw-play) ────────────────────────────────
echo
echo "TEST 1 — pw-play ile varsayılan çıkışa çalma"
echo "  3 saniye boyunca yarım saniyede bir 'tik' duymalısın."
timeout 8 pw-play --raw --rate 48000 --channels 2 --format s16 click.raw
RC=$?
[ $RC -eq 0 ] && echo "  → süreç düzgün bitti" || echo "  → süreç asıldı (exit=$RC) — ama ses gelmiş olabilir!"
read -rp "  TİK SESİ DUYDUN MU? (e/h) " A1

# ── TEST 2: playback (paplay) ─────────────────────────────────
echo
echo "TEST 2 — paplay ile varsayılan çıkışa çalma"
timeout 8 paplay --raw --rate=48000 --channels=2 --format=s16le click.raw
RC=$?
[ $RC -eq 0 ] && echo "  → süreç düzgün bitti" || echo "  → süreç asıldı (exit=$RC) — ama ses gelmiş olabilir!"
read -rp "  TİK SESİ DUYDUN MU? (e/h) " A2

# ── TEST 3: capture (monitor'den yakalama) ────────────────────
echo
echo "TEST 3 — monitor'den yakalama (çalarken eş zamanlı kayıt)"
rm -f cap.raw
timeout 8 pw-record --raw --target "$MON" --rate 48000 --channels 2 --format s16 --latency "$LAT" cap.raw &
RP=$!
sleep 0.8
timeout 8 pw-play --raw --rate 48000 --channels 2 --format s16 click.raw >/dev/null 2>&1
sleep 0.4
kill $RP 2>/dev/null; wait $RP 2>/dev/null
read P D <<< "$(peak cap.raw)"
echo "  kayıt: ${D}s  tepe=$P"
[ "$P" -gt 4000 ] && echo "  ✓ yakalama çalışıyor" || echo "  ✗ yakalama sessiz"

# ── TEST 4: UDP taşıma (ses yok, saf veri) ────────────────────
echo
echo "TEST 4 — UDP taşıma (localhost)"
rm -f udp.raw
timeout 5 socat -u "UDP-RECV:$PORT,reuseaddr" - > udp.raw &
SP=$!
sleep 0.5
timeout 5 socat -u -b 960 - "UDP-DATAGRAM:127.0.0.1:$PORT" < click.raw
sleep 0.6; kill $SP 2>/dev/null; wait $SP 2>/dev/null
read P D <<< "$(peak udp.raw)"
echo "  aktarılan: ${D}s  tepe=$P  ($(stat -c%s udp.raw 2>/dev/null || echo 0) / $(stat -c%s click.raw) bayt)"
[ "$P" -gt 4000 ] && echo "  ✓ UDP taşıma çalışıyor" || echo "  ✗ UDP taşıma başarısız"

# ── TEST 5: tam hat (yakala → UDP → çal) ──────────────────────
echo
echo "TEST 5 — tam hat: monitor → UDP → çıkış"
echo "  Not: çıkış tekrar monitor'e düşeceği için hafif yankı normal."
timeout 6 socat -u "UDP-RECV:$PORT,reuseaddr" - \
  | timeout 6 pw-play --raw --rate 48000 --channels 2 --format s16 --latency "$LAT" - &
sleep 0.7
timeout 6 pw-record --raw --target "$MON" --rate 48000 --channels 2 --format s16 --latency "$LAT" - \
  | timeout 6 socat -u -b 960 - "UDP-DATAGRAM:127.0.0.1:$PORT" &
sleep 0.7
timeout 6 pw-play --raw --rate 48000 --channels 2 --format s16 click.raw >/dev/null 2>&1
sleep 1.5
pkill -x socat 2>/dev/null; pkill -x pw-play 2>/dev/null; pkill -x pw-record 2>/dev/null
read -rp "  TİKLER GECİKMELİ OLARAK İKİNCİ KEZ DUYULDU MU? (e/h) " A5

echo
echo "════════════════════════════════════════════════════════"
echo "ÖZET:  pw-play duyuldu=$A1   paplay duyuldu=$A2   tam hat duyuldu=$A5"
echo "Bu çıktıyı olduğu gibi yapıştır."

#!/usr/bin/env python3
"""Gönderilen ve alınan ham PCM'i karşılaştırır: ses geçmiş mi, kaç patlama var."""
import struct, sys

RATE, CH, THRESH = 48000, 2, 4000

def bursts(path):
    data = open(path, 'rb').read()
    n = len(data) // (2 * CH)
    s = struct.unpack(f'<{n*CH}h', data[:n*CH*2])
    left = s[0::CH]
    onsets, armed = [], True
    for i, v in enumerate(left):
        if armed and abs(v) > THRESH:
            onsets.append(i); armed = False
        elif not armed and abs(v) < THRESH // 8:
            # patlama bitti, tekrar tetiklenebilir (en az 50 ms sonra)
            if onsets and i - onsets[-1] > RATE // 20: armed = True
    peak = max((abs(v) for v in left), default=0)
    return n, onsets, peak

try:
    sn, so, sp = bursts(sys.argv[1])
    rn, ro, rp = bursts(sys.argv[2])
except FileNotFoundError as e:
    print(f"  HATA: {e.filename} yok — hat kurulamamış olabilir"); sys.exit(1)

print(f"  gönderilen : {sn/RATE:6.2f} s   {len(so)} patlama   tepe {sp}")
print(f"  alınan     : {rn/RATE:6.2f} s   {len(ro)} patlama   tepe {rp}")
print()
if rn == 0:
    print("  SONUÇ: alıcı tarafta hiç veri yok — hat kurulamadı."); sys.exit(1)
if rp < THRESH:
    print(f"  SONUÇ: veri akıyor ama sinyal yok (tepe {rp}). Yönlendirme yanlış olabilir."); sys.exit(1)
print(f"  SONUÇ: ses uçtan uca geçti. Sinyal bütünlüğü tepe oranı: {rp/sp:.2%}")
if len(ro) < len(so) - 1:
    print(f"  UYARI: patlama sayısı düşük ({len(ro)} < {len(so)}) — kayıp veya kırpılma var.")

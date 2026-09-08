#!/usr/bin/env python3
"""Yankı treninin aralığından hat gecikmesini ölçer.

Tam hat çalışırken monitor kaydı şunu içerir:
  t0        özgün patlama
  t0 + L    hattan dönen ilk yankı
  t0 + 2L   ikinci tur
  ...
Ardışık başlangıçlar arası fark = uçtan uca hat gecikmesi.
"""
import struct, statistics, sys

RATE, CH = 48000, 2

data = open(sys.argv[1], 'rb').read()
n = len(data) // (2 * CH)
if n == 0:
    print("  kayıt boş"); sys.exit(1)
s = struct.unpack(f'<{n*CH}h', data[:n*CH*2])[0::CH]

peak = max((abs(v) for v in s), default=0)
if peak < 500:
    print(f"  kayıtta sinyal yok (tepe={peak})"); sys.exit(1)

thresh = peak * 0.35
gap = int(RATE * 0.008)          # aynı patlamayı iki kez saymamak için 8 ms
onsets, last = [], -gap
for i, v in enumerate(s):
    if abs(v) > thresh and i - last > gap:
        onsets.append(i); last = i

print(f"  kayıt: {n/RATE:.2f}s   tepe={peak}   {len(onsets)} başlangıç bulundu")
if len(onsets) < 2:
    print("  yankı yakalanamadı — hat kurulu değildi veya sinyal çok zayıf"); sys.exit(1)

deltas = [(onsets[i+1]-onsets[i])/RATE*1000 for i in range(len(onsets)-1)]
usable = [d for d in deltas if 3 < d < 500]
print(f"  aralıklar (ms): {', '.join(f'{d:.1f}' for d in deltas[:12])}")
if not usable:
    print("  geçerli aralık yok"); sys.exit(1)
med = statistics.median(usable)
print()
print(f"  ══ UÇTAN UCA HAT GECİKMESİ: {med:.1f} ms ══")
print(f"     (medyan, {len(usable)} ölçüm; min {min(usable):.1f} / max {max(usable):.1f})")
print()
print("  Kalemler: yakalama kuantumu + UDP + çalma kuantumu + socat tamponu")

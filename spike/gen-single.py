#!/usr/bin/env python3
"""Tek bir 3 ms'lik patlama, sonra sessizlik. Yankı treni ölçümü için.
Genlik bilinçli olarak düşük (-14 dBFS) — geri besleme döngüsü rahatsız etmesin."""
import math, struct, sys

RATE, CH = 48000, 2
LEAD_MS, BURST_MS, TAIL_MS, FREQ, AMP = 200, 3, 2800, 2000.0, 6500

lead = int(RATE * LEAD_MS / 1000)
burst = int(RATE * BURST_MS / 1000)
tail = int(RATE * TAIL_MS / 1000)
buf = bytearray()
for i in range(lead):
    buf += struct.pack('<h', 0) * CH
for i in range(burst):
    w = math.sin(math.pi * i / burst)
    buf += struct.pack('<h', int(AMP * w * math.sin(2 * math.pi * FREQ * i / RATE))) * CH
for i in range(tail):
    buf += struct.pack('<h', 0) * CH
open(sys.argv[1], 'wb').write(bytes(buf))

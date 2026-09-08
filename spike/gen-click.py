#!/usr/bin/env python3
"""Test sinyali: her 500 ms'de 5 ms'lik 1 kHz tam ölçek patlama. s16le 48k stereo."""
import math, struct, sys

RATE, CH, DUR = 48000, 2, 3.0
BURST_MS, PERIOD_MS, FREQ = 5, 500, 1000.0

n = int(RATE * DUR)
burst = int(RATE * BURST_MS / 1000)
period = int(RATE * PERIOD_MS / 1000)
buf = bytearray()
for i in range(n):
    pos = i % period
    v = 0
    if pos < burst:
        # kenar tıklaması olmasın diye pencerelenmiş
        w = math.sin(math.pi * pos / burst)
        v = int(28000 * w * math.sin(2 * math.pi * FREQ * pos / RATE))
    buf += struct.pack('<h', v) * CH
open(sys.argv[1], 'wb').write(bytes(buf))
print(f"  test sinyali: {sys.argv[1]}  ({n} kare, {DUR}s, her {PERIOD_MS}ms bir patlama)")

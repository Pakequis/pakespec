"""Sweeps 35 MHz-4.4 GHz with a dipole on RF IN and prints the strongest peaks
(max-hold over several sweeps) to check that real signals show up."""
import sys
sys.path.insert(0, 'src')
from pakespec.ltdz import LTDZ

START, STOP, POINTS, SWEEPS = 35_000_000, 4_400_000_000, 1001, 10
d = LTDZ()
hold = None
for _ in range(SWEEPS):
    step, v = d.sweep(START, STOP, POINTS)
    assert len(v) == POINTS, len(v)
    hold = v if hold is None else [max(a, b) for a, b in zip(hold, v)]
print('min/max raw:', min(hold), max(hold))
peaks = [i for i in range(1, POINTS - 1) if hold[i] >= hold[i-1] and hold[i] >= hold[i+1] and hold[i] > 200]
for i in sorted(peaks, key=lambda i: -hold[i])[:15]:
    print(f'{(START + i * step) / 1e6:8.1f} MHz  raw {hold[i]}')

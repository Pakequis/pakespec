"""Loopback test with RF OUT cabled to RF IN: does the fixed-frequency command
(`0x8F f <freq:9>`) make the board emit a signal that its own analyzer sees?

Run step by step: `baseline`, `emit <MHz>`, `stop`.
"""
import glob
import sys
import time

import serial

PORT = sorted(glob.glob('/dev/ttyUSB*'))[0]
s = serial.Serial(PORT, 57600, timeout=0.05)
s.dtr = s.rts = True
time.sleep(0.3)
s.reset_input_buffer()


def sweep(start_mhz, stop_mhz, points):
    start, stop = int(start_mhz * 1e6), int(stop_mhz * 1e6)
    step = (stop - start) // (points - 1)
    s.reset_input_buffer()
    s.write(b'\x8fx%09d%08d%04d%03d' % (start, step, points, 0))
    s.flush()
    need, got, t0 = points * 4, b'', time.time()
    while len(got) < need and time.time() - t0 < 6:
        got += s.read(4096)
    assert len(got) == need, f'only {len(got)} of {need} bytes'
    return [int.from_bytes(got[i:i + 2], 'little') for i in range(0, need, 4)], step


def report(label, start_mhz, stop_mhz, points, repeats=3):
    runs = [sweep(start_mhz, stop_mhz, points) for _ in range(repeats)]
    avg = [sum(r[0][i] for r in runs) / repeats for i in range(points)]
    step = runs[0][1]
    top = sorted(range(points), key=lambda i: -avg[i])[:3]
    print(f'{label}: min {min(avg):.0f}  mean {sum(avg) / points:.1f}  max {max(avg):.0f}')
    print('   strongest:', [(round(start_mhz + i * step / 1e6, 2), round(avg[i])) for i in top])
    return avg


def fixed(mhz):
    s.write(b'\x8ff%09d' % int(mhz * 1e6))
    s.flush()
    time.sleep(0.3)


if __name__ == '__main__':
    step = sys.argv[1]
    try:
        if step == 'baseline':
            report('35-1000 MHz', 35, 1000, 201)
        elif step == 'emit':
            f = float(sys.argv[2])
            fixed(f)
            report(f'emitting {f} MHz, around it', f - 20, f + 20, 201)
    finally:
        if step != 'baseline':
            fixed(0)
            print('generator stopped (f000000000)')

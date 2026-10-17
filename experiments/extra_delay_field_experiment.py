"""Tests whether a sweep command with an extra trailing 3-digit field (start 9, step 8,
points 4, delay 3 digits) removes the "every other sweep is ignored" behaviour."""
import glob, sys, time
import serial

PORT = sorted(glob.glob('/dev/ttyUSB*'))[0]
s = serial.Serial(PORT, 57600, timeout=0.05)
s.dtr = s.rts = True
time.sleep(0.3)
s.reset_input_buffer()

BASE = b'\x8fx035000000001000000011'  # 11 points, 35 MHz start, 1 MHz step


def attempt(payload, wait=0.6):
    s.reset_input_buffer()
    s.write(payload)
    s.flush()
    time.sleep(wait)
    return len(s.read(4096))


def run(name, payload, n=8):
    got = [attempt(payload) for _ in range(n)]
    print(f'{name:<22} {got}', flush=True)


run('21 digits (baseline)', BASE)
for extra in (b'000', b'001', b'010', b'100'):
    run(f'21 digits + {extra.decode()}', BASE + extra)

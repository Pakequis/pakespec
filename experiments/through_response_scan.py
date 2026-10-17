"""Reads the detector at every MHz from 35 MHz to 4.45 GHz with the fixed-frequency
and measure commands and writes `through_response.csv` (frequency_mhz,raw).

Setup: tracking source on (S1 button), a cable from RF OUT to RF IN. All
frequencies are real; the commands carry them divided by 10. Takes about 70 s.
"""
import glob
import time

import serial

s = serial.Serial(sorted(glob.glob('/dev/ttyUSB*'))[0], 57600, timeout=0.02)
s.dtr = s.rts = True
time.sleep(0.4)
s.reset_input_buffer()


def send(body):
    s.write(b'\x8f' + body)
    s.flush()


def set_frequency(mhz):
    # After the output was stopped the first frequency command is ignored: send it twice.
    for _ in range(2):
        send(b'f%09d' % (mhz * 1_000_000 // 10))


def measure():
    s.reset_input_buffer()
    send(b'm')
    data, t0 = b'', time.time()
    while len(data) < 4 and time.time() - t0 < 0.5:
        data += s.read(4 - len(data))
    return int.from_bytes(data[:2], 'little') if len(data) == 4 else None


try:
    set_frequency(35)
    time.sleep(0.1)
    with open('through_response.csv', 'w') as out:
        out.write('frequency_mhz,raw\n')
        for mhz in range(35, 4451):
            set_frequency(mhz)
            time.sleep(0.012)
            out.write(f'{mhz},{measure()}\n')
finally:
    send(b'f000000000')

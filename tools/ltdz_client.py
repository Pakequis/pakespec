"""Standalone Python client for the LTDZ 35-4400M board (research-phase tool; the application is written in Rust)."""
import glob
import struct
import time

import serial


class LTDZ:
    def __init__(self, port=None):
        port = port or sorted(glob.glob('/dev/ttyUSB*'))[0]
        self.s = serial.Serial(port, 57600, timeout=0.05)
        self.s.dtr = True
        self.s.rts = True
        time.sleep(0.3)
        self.s.reset_input_buffer()

    def _read(self, need, timeout):
        buf = b''
        t0 = time.time()
        while len(buf) < need and time.time() - t0 < timeout:
            buf += self.s.read(need - len(buf))
        return buf

    def sweep(self, start, stop, points, delay=0):
        """Sweep from start to stop (Hz) and return (step_hz, channel_1_raw_values).

        The command must carry the trailing 3-digit delay field. Without it the
        firmware ignores every other sweep command.
        """
        step = (stop - start) // (points - 1)
        cmd = b'\x8fx%09d%08d%04d%03d' % (start, step, points, delay)
        need = points * 4
        self.s.reset_input_buffer()
        self.s.write(cmd)
        self.s.flush()
        buf = self._read(need, points / 800 + 2.0)
        n = len(buf) // 4
        ch1 = [struct.unpack_from('<H', buf, i * 4)[0] for i in range(n)]
        return step, ch1

#!/usr/bin/env python3
"""Amplitude ladder: FY6900 CH1 -> LTDZ RF IN, raw detector value per level.

Setup: FY6900 CH1 connected to RF IN (BNC to SMA), tracking source (S1) off,
nothing on RF OUT. The FY6900 amplitude is in Vpp; into 50 ohm,
dBm = 10*log10((Vpp/2/sqrt(2))**2 / 50 * 1000).

Safety: amplitude is capped (MAX_VPP), starts at the lowest step, and both
FY6900 outputs are switched off at the end, also on errors.
"""
import argparse
import math
import statistics
import sys
import time

import serial

BOARD = "/dev/serial/by-id/usb-1a86_USB2.0-Serial-if00-port0"
FY = "/dev/serial/by-id/usb-1a86_USB_Serial-if00-port0"
MAX_VPP = 0.500  # about -2 dBm into 50 ohm, 7 dB below the +5 dBm input limit
STEPS_VPP = [0.001, 0.002, 0.005, 0.010, 0.015, 0.020, 0.030, 0.050, 0.070,
             0.100, 0.150, 0.200, 0.300, 0.400, 0.500]


def dbm(vpp):
    vrms = vpp / 2 / math.sqrt(2)
    return 10 * math.log10(vrms**2 / 50 * 1000)


class Fy:
    def __init__(self, port):
        self.s = serial.Serial(port, 115200, timeout=1)

    def q(self, cmd):
        self.s.reset_input_buffer()
        self.s.write(cmd.encode() + b"\n")
        time.sleep(0.15)
        return self.s.read(100).decode().strip()

    def wake(self):
        # The first command after a pause is ignored.
        self.q("UMO")
        assert self.q("UMO").startswith("FY6900"), "FY6900 not answering"

    def output(self, on):
        self.q(f"WMN{int(on)}")
        self.q(f"WFN0")


class Board:
    def __init__(self, port):
        self.s = serial.Serial(port, 57600, timeout=1)
        self.s.dtr = self.s.rts = True

    def tune(self, real_hz):
        # Commands carry one tenth of the real frequency; send twice.
        cmd = b"\x8f" + f"f{int(real_hz / 10):09d}".encode()
        for _ in range(2):
            self.s.write(cmd)
            time.sleep(0.05)
        time.sleep(0.05)

    def read(self, n=40):
        vals = []
        for _ in range(n):
            self.s.reset_input_buffer()
            self.s.write(b"\x8fm")
            d = self.s.read(4)
            if len(d) == 4:
                vals.append(int.from_bytes(d[:2], "little"))
        return vals


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--freq", type=float, default=50e6, help="real Hz")
    ap.add_argument("--readings", type=int, default=40)
    a = ap.parse_args()

    fy = Fy(FY)
    fy.wake()
    fy.output(False)
    assert fy.q("RMN") == "0", "FY6900 output did not turn off"
    board = Board(BOARD)
    try:
        board.tune(a.freq)
        floor = board.read(a.readings)
        print(f"floor (FY off): mean {statistics.mean(floor):.2f} sd {statistics.pstdev(floor):.2f}")
        fy.q(f"WMF{int(a.freq * 1e6):014d}")
        print("FY freq:", fy.q("RMF"))
        print("vpp_mV  dBm_nominal  raw_mean  raw_sd")
        for vpp in STEPS_VPP:
            assert vpp <= MAX_VPP
            fy.q(f"WMA{vpp:.3f}")
            if abs(int(fy.q("RMA")) / 10000 - vpp) > 1e-6:
                sys.exit("FY6900 amplitude readback mismatch")
            fy.output(True)
            time.sleep(0.3)
            v = board.read(a.readings)
            print(f"{vpp*1000:6.1f}  {dbm(vpp):10.1f}  {statistics.mean(v):8.2f}  {statistics.pstdev(v):6.2f}", flush=True)
    finally:
        fy.output(False)
        print("FY outputs:", fy.q("RMN"), fy.q("RFN"))
        board.tune(0)


if __name__ == "__main__":
    main()

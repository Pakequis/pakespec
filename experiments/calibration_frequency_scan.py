#!/usr/bin/env python3
"""Flatness of the detector between 35 and 60 MHz at fixed FY6900 levels.

Same setup and safety rules as calibration_amplitude_ladder.py (FY6900 CH1 on
RF IN, S1 off, outputs switched off at the end).
"""
import statistics
import time

from calibration_amplitude_ladder import BOARD, FY, MAX_VPP, Board, Fy, dbm

LEVELS_VPP = [0.050, 0.200]
FREQS_MHZ = list(range(35, 61))


def main():
    fy = Fy(FY)
    fy.wake()
    fy.output(False)
    board = Board(BOARD)
    try:
        for vpp in LEVELS_VPP:
            assert vpp <= MAX_VPP
            fy.q(f"WMA{vpp:.3f}")
            fy.output(True)
            print(f"level {vpp*1000:.0f} mVpp ({dbm(vpp):.1f} dBm nominal)")
            print("MHz  raw_mean  raw_sd")
            for f in FREQS_MHZ:
                fy.q(f"WMF{int(f * 1e12):014d}")
                board.tune(f * 1e6)
                time.sleep(0.1)
                v = board.read(30)
                print(f"{f:3d}  {statistics.mean(v):8.2f}  {statistics.pstdev(v):6.2f}", flush=True)
            fy.output(False)
    finally:
        fy.output(False)
        print("FY outputs:", fy.q("RMN"), fy.q("RFN"))
        board.tune(0)


if __name__ == "__main__":
    main()

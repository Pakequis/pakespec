#!/usr/bin/env python3
"""Through-response scan with a fixed attenuation between RF OUT and RF IN.

Setup: tracking source (S1) ON, RF OUT -> [attenuators] -> RF IN with SMA
attenuators of known value and a short SMA cable. Run once per attenuation
(for example 10, 16, 20, 26, 30, 36 dB); `attenuator_fit.py` then compares them.

Each run takes REPEATS full-band sweeps through `pakespec-cli --json` and stores
the median raw value per frequency in `<out>/att_<dB>.csv`. Use `--simulate` for
a dry run without a board.

Usage: attenuator_scan.py DB [--repeats N] [--out DIR] [--cli PATH] [--port PATH]
"""
import argparse
import csv
import json
import os
import statistics
import subprocess
import sys

BOARD = "/dev/serial/by-id/usb-1a86_USB2.0-Serial-if00-port0"
START, STOP, POINTS = 35_000_000, 4_400_000_000, 440  # 438 points is the minimum


def sweep(cli, port, simulate):
    cmd = [cli, "--json", "--start", str(START), "--stop", str(STOP), "--points", str(POINTS)]
    cmd += ["--simulate"] if simulate else ["--port", port]
    out = subprocess.run(cmd, capture_output=True, text=True)
    reply = json.loads(out.stdout)
    if not reply["ok"]:
        sys.exit(f"sweep failed: {reply['error']['message']}")
    return reply["freq_hz"], reply["raw"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("db", help="nominal attenuation in dB, used in the file name")
    ap.add_argument("--repeats", type=int, default=8)
    ap.add_argument("--out", default="experiments/attenuator-data")
    ap.add_argument("--cli", default="target/release/pakespec-cli")
    ap.add_argument("--port", default=BOARD)
    ap.add_argument("--simulate", action="store_true")
    a = ap.parse_args()

    runs, freqs = [], None
    for _ in range(a.repeats):
        freqs, raw = sweep(a.cli, a.port, a.simulate)
        runs.append(raw)
    median = [statistics.median(col) for col in zip(*runs)]
    os.makedirs(a.out, exist_ok=True)
    path = os.path.join(a.out, f"att_{a.db}.csv")
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["freq_hz", "raw_median"])
        w.writerows(zip(freqs, median))
    spread = max(max(c) - min(c) for c in zip(*runs))
    print(f"{path}: {len(freqs)} points, median raw {statistics.median(median):.0f}, "
          f"largest spread between repeats {spread}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Compares the scans made with different attenuations (see attenuator_scan.py).

For each frequency band it fits raw = slope * (-attenuation_dB) + level over the
runs whose raw value is inside the detector's straight part (LOW..HIGH), and
prints the slope in counts per dB (about 8.55 at 50 MHz) and the extrapolated
raw value at 0 dB of attenuation (the tracking output through the cable). The
change of that level with frequency, divided by the slope, is the flatness of
the whole chain in dB: an approximate correction for dBm values above 60 MHz.

Usage: attenuator_fit.py [DIR] [--bands N]
"""
import argparse
import csv
import glob
import os
import re
import statistics

LOW, HIGH = 165, 425  # raw range where the detector is a straight line in dB
REFERENCE_SLOPE = 8.55  # counts per dB measured at 50 MHz


def load(directory):
    runs = {}
    for path in glob.glob(os.path.join(directory, "att_*.csv")):
        db = float(re.search(r"att_([\d.]+)\.csv", path).group(1))
        with open(path) as f:
            rows = list(csv.reader(f))[1:]
        runs[db] = [(int(r[0]), float(r[1])) for r in rows]
    return runs


def fit(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    return slope, my - slope * mx


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir", nargs="?", default="experiments/attenuator-data")
    ap.add_argument("--bands", type=int, default=16)
    a = ap.parse_args()
    runs = load(a.dir)
    if len(runs) < 3:
        raise SystemExit(f"need at least 3 attenuations in {a.dir}, found {sorted(runs)}")
    dbs = sorted(runs)
    n = len(runs[dbs[0]])
    size = max(1, n // a.bands)
    print(f"attenuations: {dbs}")
    print("band start MHz | runs used | counts/dB (ref 8.55) | raw at 0 dB | level vs first band (dB)")
    first_level = None
    for i in range(0, n, size):
        sl = slice(i, min(i + size, n))
        freq = runs[dbs[0]][i][0] / 1e6
        xs, ys = [], []
        for db in dbs:
            raw = statistics.median(v for _, v in runs[db][sl])
            if LOW <= raw <= HIGH:
                xs.append(-db)
                ys.append(raw)
        if len(xs) < 3:
            print(f"{freq:14.0f} | {len(xs):9d} | not enough points in the straight part")
            continue
        slope, level = fit(xs, ys)
        first_level = level if first_level is None else first_level
        print(f"{freq:14.0f} | {len(xs):9d} | {slope:20.2f} | {level:11.0f} | "
              f"{(level - first_level) / REFERENCE_SLOPE:+.1f}")


if __name__ == "__main__":
    main()

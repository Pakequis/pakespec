# dBm calibration with the FY6900 function generator

Status: in progress, first ladder measured on 2026-10-17.

## Setup

FeelElec FY6900-60M CH1 -> BNC cable -> SMA to BNC adapter -> RF IN of the LTDZ board. Tracking source (S1) off, nothing on RF OUT. The generator is used directly, without an attenuator, at very low amplitudes only (cap in the scripts: 100 mVpp, about -16 dBm into 50 ohm; the board input limit is +5 dBm).

## FY6900 serial protocol (confirmed on the unit)

- `/dev/serial/by-id/usb-1a86_USB_Serial-if00-port0`, 115200 baud, commands end with `\n`. The board and the generator are both CH340 (`1a86:7523`); only the by-id name tells them apart.
- The first command after a pause may be ignored; send `UMO` twice to wake it.
- `UMO` model, `UID` id. Read: `RMF` frequency (Hz), `RMA` amplitude (units of 0.1 mV, so `100` is 10 mVpp), `RMW` waveform, `RMO` offset, `RMN` and `RFN` output state of CH1 and CH2 (`255` on, `0` off).
- Write: `WMF` + 14 digits in microhertz (50 MHz is `WMF50000000000000`), `WMA0.010` amplitude in Vpp, `WMN1`/`WMN0` CH1 output, `WFN1`/`WFN0` CH2 output.

## Amplitude ladder (50 MHz, 40 readings per level, 2026-10-17)

Nominal dBm assumes the FY6900 amplitude is accurate into 50 ohm (not verified without an oscilloscope). The scripts cap the amplitude at 500 mVpp (about -2 dBm, 7 dB below the board input limit).

| FY6900 | nominal dBm | raw mean |
|---|---|---|
| off | | 151.0 (sd 0.7) |
| 1 mV | -56.0 | 150.4 |
| 2 mV | -50.0 | 150.8 |
| 5 mV | -42.0 | 152.6 |
| 10 mV | -36.0 | 153.9 |
| 15 mV | -32.5 | 168.0 |
| 20 mV | -30.0 | 184.9 |
| 30 mV | -26.5 | 214.6 |
| 50 mV | -22.0 | 254.1 |
| 70 mV | -19.1 | 281.4 |
| 100 mV | -16.0 | 305.6 |
| 150 mV | -12.5 | 337.1 |
| 200 mV | -10.0 | 359.4 |
| 300 mV | -6.5 | 388.7 |
| 400 mV | -4.0 | 408.7 |
| 500 mV | -2.0 | 424.7 |

- Below about -36 dBm the detector does not leave the noise floor (about 150 raw).
- From 15 mV (-32.5 dBm) to 500 mV (-2 dBm) the response is a straight line in dB: raw = 8.55 x dBm + 443.3, residuals within +-2.6 counts (about +-0.3 dB). So the usable dynamic range is about -33 to at least -2 dBm, 31 dB, with no saturation seen yet.
- Conversion: dBm = (raw - 443.3) / 8.55, valid for raw 165 to 425 at 50 MHz, if the generator amplitude is accurate.

## Flatness 35 to 60 MHz

Every 1 MHz, 30 readings, at 50 mVpp and 200 mVpp: the readings stay within 253.3 to 255.3 and 358.3 to 359.0 raw (about +-0.1 dB). The detector is flat over the generator's whole range. Above 60 MHz the generator cannot go, so the response there (the through response in `serial-protocol.md` falls by about 2 dB per GHz on the cable) stays uncalibrated.

## In the application

If `calibration.toml` exists next to `settings.toml` (Linux: `~/.config/pakespec/`), the screen shows dBm: the reference level and scale per division are in dBm and dB, and the marker readout is in dBm. The line measured at 50 MHz is applied to every frequency (decision of 2026-10-17): when the trace goes outside 35 to 60 MHz the header adds `approximate dBm`, and a marker value outside the measured raw range (for example the noise floor) or frequencies gets a `~`. A manual scale saved in raw units is not converted when a calibration appears; switch to auto scale once. Create the file with `pakespec-cli --fit-calibration`.

## Matching the original software's scale (2026-10-17)

Reference: two screenshots of the manufacturer's Windows program on the same board, with RF OUT cabled to RF IN (tracking on, then off). The through trace runs from about +6 dB at 35 MHz to about -21 dB near 3.45 GHz, and the floor with the tracking off is about -58.8 dB.

The red trace was digitised from the screenshot and paired, by frequency, with a 1000-point sweep taken with this application in the same state. One straight line fits the whole range, including the noise floor: `dB = (raw - 455.7) / 5.30` (rms residual 0.7 dB). The floor reading of about 148 raw gives -58 dB, as in the screenshot.

This scale (5.30 counts/dB) differs from the FY6900 ladder above (8.55 counts/dB between about -33 and -2 dBm), so one of the two references has an error in level, probably the generator amplitude, which was never checked with an oscilloscope. For a display that looks like the original, use:

```toml
counts_per_db = 5.30
raw_at_0_dbm = 455.67
min_raw = 148.0
max_raw = 500.0
min_freq_hz = 35000000
max_freq_hz = 4400000000
```

These dB values follow the original program's scale; they are not an absolute level.

This line is built into the application (`Calibration::standard()` in `src/calibration.rs`) and is used whenever no valid `calibration.toml` exists, by the application, `pakespec-cli` and `pakespec-mcp`. A file next to `settings.toml` takes priority.

## Optional: levels above 60 MHz

The FY6900 stops at 60 MHz, so above it the same line is applied and marked `approximate dBm`. A way to measure the response there without another generator: SMA attenuators of known value (for example 10, 16, 20, 26, 30 and 36 dB, built from fixed parts) between RF OUT and RF IN with S1 on, one scan per value with `experiments/attenuator_scan.py DB`, then `experiments/attenuator_fit.py`. It prints the slope in counts per dB and the level change with frequency per band. This gives the flatness of the whole chain (cable and adapters included), not an absolute level, which stays tied to the FY6900 at 35 to 60 MHz. Check that two 10 dB parts in series agree with one 20 dB part to spot a bad attenuator. Not done yet: no attenuators were available.

## Open

- Absolute accuracy: needs a check of the FY6900 amplitude with an oscilloscope on 50 ohm.
- Levels above -2 dBm and the shape of the response above 60 MHz.
- Store the fit in a per-board calibration file (feature C4).

Scripts: `experiments/calibration_amplitude_ladder.py`, `experiments/calibration_frequency_scan.py`.

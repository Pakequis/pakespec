# Serial protocol of the LTDZ 35-4400M

Everything here was observed on the board; the tests that repeat it are in `tests/hardware.rs` (run with the board connected, see that file) and the scripts in `experiments/`. Frequencies in this document are **real** frequencies unless it says "command units". Last updated 2026-10-17.

## Serial layer

- Device: CH340 (`1a86:7523`), `/dev/ttyUSB0` on Linux.
- 57600 baud, 8 data bits, no parity, 1 stop bit. DTR and RTS asserted. No flow control. No line terminator.
- If the board answers nothing: unplug and replug the USB cable.

## Frequency units

**Every frequency in a command is one tenth of the real frequency**: the board multiplies it by 10. The manufacturer's setup instructions also say to set a frequency multiplier of 10 in the vendor software. To sweep 35 MHz to 4.4 GHz the command carries 3.5 MHz to 440 MHz; to set 100 MHz it carries `f010000000` (10,000,000).

Independent proof on the board: with the tracking source on and a cable from RF OUT to RF IN, the signal exists exactly between a commanded 3.5 MHz and 444.9 MHz, that is **35 MHz to 4.449 GHz real**. At a commanded 3.4 MHz the reading is half-way (358), below 3.3 MHz the output is off (148), and above a commanded 445 MHz it is off. (A commanded value of 1000 MHz has 10 digits, overflows the field and is read as 100 MHz.)

The Rust client (`protocol.rs`) works in real frequencies and divides by 10 when it builds a command; all start and step values are multiples of 10 Hz.

## Commands

Every command starts with the byte `0x8F`, followed by ASCII text.

| Command | Format | Answer |
|---|---|---|
| Sweep | `x` + start (9 digits) + step (8 digits) + points (4 digits) + delay (3 digits); start and step in command units | `points × 4` bytes |
| Set frequency | `f` + 9 digits in command units, e.g. `f010000000` for 100 MHz; `f000000000` stops the output | none |
| Measure | `m` | 4 bytes: two little-endian 16-bit detector values, taken at the frequency of the last `f` or of the end of the last sweep; about 50 per second |
| Version | `v` | 1 byte, `0x77` (119), read as firmware 1.19 |
| Status | `s` | 4 bytes `0a 00 00 00` (little-endian 10), read as hardware version 10 |

**The S1 state cannot be read.** Checked on 2026-10-17: `s` returns `0a 00 00 00` and `v` returns `77` identically with S1 on and off, before any `f`, with a frequency set and after `f000000000`. The only way to tell is to infer it from a sweep with RF OUT cabled to RF IN (about 489 with the tracking source on, about 150 with it off); this cannot tell S1 off from a loose cable.

**Do not send unknown commands.** `0x8F` followed by `a` and, on a second try, by `b` each left the board silent: no answer to `s`, `v` or `m`, even when resent, until the USB cable was unplugged and plugged in again. Nothing else was tried after that, so no other letter is known to be safe or useful.

## Sweep

- **Response:** `points × 4` bytes, streamed while the board measures. Per point, channel 1 as a 16-bit little-endian value and channel 2 as a 16-bit little-endian value; on this single-channel board they are equal.
- **Speed:** about 800 points per second (101 points 0.13 s, 500 points 0.62 s, 2001 points 2.47 s, 5000 points 6.2 s, 9999 points 12.3 s). Points per sweep: 0 gives no answer, 1 gives one reading, 2 to 9999 work.
- **Delay field:** the command has 24 characters after `0x8F` `x`; the last 3 digits (delay) are mandatory, because without them the board waits for more bytes and only fires on the next command, which looks like "every other sweep is ignored". The value does not change the sweep time or the readings (0, 1, 2, 5, 10, 50, 100, 500, 999 tested); `000` works.
- **Step limit:** the board takes the commanded step modulo 1,000,000 command units, that is **modulo 10 MHz real**. Up to 9.9 MHz the step is exact; a step of 10 MHz does not advance the frequency at all (the trace is flat at the level of the start frequency); 11 MHz advances 1 MHz, 15 MHz advances 5 MHz, 20 MHz and 30 MHz do not advance. A sweep therefore needs `floor((stop - start) / 10 MHz) + 2` points at least: **438 points** for the full 35 MHz to 4.4 GHz range.
- **Zero span:** a step of 0 gives a time series at one frequency (200 points in 0.25 s, noise of about ±1).
- **No commands during a sweep:** while the board streams a sweep it ignores new commands (a new command sent after reading 100 of 1001 points was neither answered nor queued; the rest of the old sweep arrived, 3604 bytes). A sweep cannot be aborted. To change or stop a sweep quickly, sweep in short segments (the client uses 100 points, 0.13 s) and look at the new settings between segments.
- **First point:** the first point of the first sweep after the output was stopped (`f000000000`) reads wrong (356 instead of 484 at 35 MHz). After a sweep it is right, even if that sweep ended at 4.4 GHz. Fix: send `f` at the start frequency (twice, see below) and wait 50 ms before the sweep; the client does this when it has not tuned the board yet.

## Generator: the tracking source

The board has no fixed-frequency generator separate from the tracking source. RF OUT carries a signal only while the tracking source is on, and the **S1 button** on the board turns it on. With S1 on:

- `f` sets the frequency of the signal and `m` reads it back through a cable to RF IN; a sweep makes the signal follow the sweep.
- `f000000000` turns the output off (readings fall to the noise floor).
- **After `f000000000` the first `f` with a frequency is ignored**; sending it twice back to back (or 50 ms apart) brings the tone back (3 of 3 times), while one `f`, an `m` before it, an extra digit or a second `f000000000` do not. Changing from one frequency to another needs only one `f`. The client sends a frequency twice.
- The output is ready within 5 ms of an `f`.

**Correction of 2026-10-17:** with S1 off (LED off) a fixed frequency (`f`) still puts a signal on RF OUT: `m` read 486 to 489 at 50 MHz to 1 GHz with RF OUT cabled to RF IN, and 148 to 150 after `f000000000` or with no `f` sent. Sweeps, on the other hand, read only the noise floor (150) with S1 off. So S1 gates the tracking signal during sweeps, not the fixed-frequency output. (Earlier text said nothing arrives with S1 off; that was not true for `f` and `m`.)

Old statement, kept for the record: with S1 off nothing arrives at RF IN, whatever is sent. Whether S1 is on cannot be read from the board; the application can only notice a reading at the noise floor.

**Through response** with a 25 cm BNC cable and SMA to BNC adapters, raw `m` readings: 35 MHz to 500 MHz about 485, 1 GHz 478, 2 GHz 464, 3 GHz 452, 4 GHz 425, 4.3 GHz 320, 4.4 GHz 273, 4.45 GHz 252, then off (148) from 4.45 GHz. Nothing was damaged by connecting the output straight to the input; the highest value read was 492 of a 12-bit range.

## Detector readings

- Output off: 147.8 with a standard deviation of 0.4 (200 readings). A tone at 100 MHz through the cable: about 486 (±1), at 4 GHz about 426 (±1.3).
- Through response, `m` at every MHz from 35 MHz to 4.449 GHz (`experiments/through_response_scan.py`, no reading missing, no dead frequency): median raw value 486 from 35 to 500 MHz (range 484 to 489), 484 from 500 MHz to 1 GHz (476 to 487), 476 from 1 to 2 GHz (464 to 486), 471 from 2 to 3 GHz (406 to 489), 409 from 3 to 4 GHz (367 to 473), 409 from 4 to 4.3 GHz (316 to 425) and 305 from 4.3 to 4.449 GHz (250 to 320). It includes the cable and adapters. An earlier reading of 150 at exactly 4250 MHz was the ignored first `f` after `f000000000`, not a dead frequency.
- Ten identical full-range sweeps differ by about ±1 per point (maximum 5.5).
- Forty full-range sweeps of 438 points in a row all completed.
- The raw values are not calibrated; the conversion to dBm is not known (see below).

## Reception test with an antenna (2026-10-17)

Setup: a dipole of about 10 cm on RF IN, nothing on RF OUT, tracking source off (S1). Receive-only sweeps with the Rust client, real frequencies, 12 to 50 sweeps with max-hold. Floor 146 to 153 raw (one isolated 182 in 1 to 2.5 GHz).

- **FM broadcast (88 to 108 MHz), 35 to 1000 MHz, 2.5 to 4.4 GHz and the 433 MHz ISM band:** nothing above the floor. A 10 cm dipole is about 3 % of a wavelength at 100 MHz, so broadcast signals arrive far below the board's floor.
- **2.4 GHz band (2.38 to 2.5 GHz):** bursts reaching raw 222 (about 74 above the floor) between **2.418 and 2.425 GHz**, seen in 27 frequency points over 50 sweeps. This matches Wi-Fi activity around channel 3 (centered at 2.422 GHz), and independently confirms the frequency scale: with the commanded frequencies taken as real the burst would sit at 242 MHz.
- **1.93 GHz:** bursts of 180 to 188 raw at **1.930 to 1.934 GHz** (3 to 5 times in 50 sweeps); cellular downlink is the likely source (not identified).
- **About 807 MHz:** two points at 160 to 164 raw, seen once each.

Script: `experiments/ambient_antenna_sweep.py` (old, command units; use `pakespec-cli` for new checks).

## dBm calibration (not implemented)

Planned approach: a line defined by two measurements (a known level and the same level attenuated by at least 20 dB): slope = dB difference / raw-value difference. See `next-steps.md`. Open point: the through response above includes the cable and adapters, so the shape of the raw values alone is not the board's response.

## Experiments written before the frequency units were known

The scripts in `experiments/` written before 2026-10-17 afternoon pass **command units** (real divided by 10) to the board but label them as real frequencies; read their frequencies multiplied by 10.

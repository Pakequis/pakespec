# LTDZ 35-4400M board (spectrum analyzer and tracking generator): research record

Hand-over document from the research phase, written when the Pakespec project was created on 2026-10-17. It gathers what was researched, the tools used, what was done and tested on the board, what was solved and what is still open. Anyone opening the project should be able to continue from this document and the scripts in `experiments/`. The Rust code is in `src/`, and the Python client used during the research is `tools/ltdz_client.py`.

## 1. Summary

- The board is an **LTDZ 35M-4400M** (silkscreen, dated 2019-03-08), a heterodyne spectrum analyzer with a tracking generator, shipped by Banggood in 2019 (post: https://www.pakequis.com.br/2019/08/analisador-de-espector-de-35-mhz-44-ghz.html).
- Hardware: STM32F103C8T6, CH340G (USB serial), two ADF4351 (local oscillator and tracking), IAM-81008 mixer, AD8307 detector.
- The software that came with the board (WinNWT4) is Windows-only. Its author (DL4JAL) no longer lists WinNWT4 on his page; the newer NWT4000 v1.11 and NWT2 v1.12 (uploaded in 2021) have Windows, Linux and source releases, but the page says they are not suitable for LTDZ boards. There is no open, cross-platform client for the LTDZ.
- Communication is serial at **57600 8N1** with commands `0x8F` + ASCII.
- The **sweep protocol was confirmed** on the board, from Linux, in Python (section 4).
- The apparent "every other sweep is ignored" behavior was caused by a **missing 3-digit delay field** in the command and is solved (section 5).
- If the board does not answer, **unplugging and replugging the USB cable** fixes it (happened once).
- Still missing: an absolute dBm calibration and a measurement of the tracking output level. The application (Rust, egui) exists; see `next-steps.md`.
- Short-term goal tied to another project: use the 50 MHz tracking output as a reference to calibrate Anritsu power sensors (section 8).

## 2. Environment and tools used

Machine: Ubuntu 26.04 (Linux 7.0), 12 cores, ~15 GB RAM.

| Tool | Purpose |
|---|---|
| System Python 3 + pyserial 3.5 | Talk to the board (`/dev/ttyUSB0`); run scripts with `python3 -I` |
| pdftotext, pdftoppm, tesseract | Read the manufacturer's documents and the power sensor manual |
| udevadm, lsusb, journalctl -k | Identify the device and see when it (re)enumerated |
| Web search and fetch | Research on the LTDZ, ADF4351, alternative software (sources in section 9) |
| Image reading | Photo of the board and manufacturer schematic |

## 3. Timeline (2026-10-17)

1. Hardware research through web search (Silicon Chip, Practical Electronics, EEVblog) and reading of the 2019 post.
2. The manufacturer's package was found (photos, schematic and instructions).
3. The photo of the board and the manufacturer's schematic confirmed the model and the chips. Important correction: the 4-pin connector with the "USART" silkscreen is actually the STM32 **SWD** port (schematic, connector P1).
4. First tests with the board on Linux (Python): **no answer** to any command, at 11 baud rates, 4 DTR/RTS combinations and 8 sweep formats. The board LEDs were off; after they lit up, it was still mute.
5. The user **unplugged and replugged the USB cable** of the board. A wait script fired the tests as soon as the device reappeared. The board **answered**.
6. Sequence experiments showed that every other sweep command seemed to be ignored. A minimal client was written and tested.
7. Later the same day, the problem was traced to the missing 3-digit delay field (section 5), and a reception test with a dipole antenna was done (`serial-protocol.md`).
8. The application was built in Rust (protocol, board access, simulator, worker, command-line tool and an egui screen with four styles, three themes, English and Portuguese, and a front panel with keys and a knob).
9. Tests with the board connected to itself by a cable showed that the board **multiplies every command frequency by 10**, that the tracking source (turned on by the S1 button) is the only generator, that the first frequency command after a stop is ignored, that sweep steps wrap at 10 MHz, and that the board ignores commands during a sweep. The client and the screens were corrected (`serial-protocol.md`).
10. Reception was tested with a dipole: Wi-Fi bursts at 2.42 GHz and cellular bursts near 1.93 GHz appeared, which also confirmed the frequency scale. Hardware tests that repeat all this were added (`testing.md`).

## 4. Protocol confirmed on the board

Serial: **57600 baud, 8 bits, no parity, 1 stop bit**, DTR and RTS asserted, no flow control, no line terminator. Open `/dev/ttyUSB0` (CH340, `1a86:7523`).

**Sweep command:**

```
0x8F  'x'  <start: 9 digits>  <step: 8 digits>  <points: 4 digits>  <delay: 3 digits>
```

Step = (stop − start) / (points − 1). Digits are zero-padded. Start and step are in command units, which are real frequency divided by 10 (the board multiplies them by 10; see `serial-protocol.md`). The board works from 35 MHz up to about 4.45 GHz.

**Response:** `points × 4` bytes. Each point: channel 1 as 16-bit little-endian (bytes 0 and 1), channel 2 as 16-bit little-endian (bytes 2 and 3). On the single-channel board, channel 2 is equal or nearly equal to channel 1. Raw values measured with nothing connected: **144 to 182** (12-bit STM32 ADC reading the AD8307 detector). Speed: 1001 points from 35 MHz to 4.4 GHz in **1.24 s**; 101 points in 0.13 s.

**Fixed frequency:** `0x8F` + `f` + 9 digits (Hz), for example `f010000000`. No response. `f000000000` stops it. The fixed frequency is also in command units. The board does emit it on RF OUT, also with S1 off (read back through a cable to RF IN); its absolute level was not measured with another instrument (see `serial-protocol.md`).

**Firmware version:** `0x8F` + `v`. Right after reconnecting it returned 1 byte `0x77` (119), consistent with a firmware version. Later reads returned pending ADC data, so `v` is not reliable as a standalone query.

## 5. The "alternation" of the `x` command (solved)

Observed repeatedly in the first experiments (scripts in `experiments/`):

- Each `x` sweep command seemed to fire only if the previous one had been "ignored": one worked, the next did not, and so on. It held for any terminator (`\n`, space, `0x8F`, `\r`, `0x00`).
- The `f000000000` command in the middle of the sequence also seemed to change that state. In a battery of 22 sweeps with `f000000000` after each one, all failed.
- A workaround that worked partially: send `x`; if nothing arrives within `points/800 + 0.5` s, send it again (up to 3 tries). It was not reliable.

**Cause:** the sweep command has a 3-digit delay field after the point count. The first commands were sent with 21 digits, so the firmware kept waiting for the missing bytes and used the first bytes of the next command to complete the previous one. With the 3 extra digits (`000`, `001`, `010` and `100` tested) 32 of 32 sweeps of 11 points answered, plus 12 of 12 of 1001 and 101 points. The delay value did not change the timing without a signal source. Script: `experiments/extra_delay_field_experiment.py`.

## 6. Hardware (summary; details in `hardware.md`)

- Manufacturer specifications: RBW 5 to 120 kHz, more than 800 points per second, logarithmic dynamic range above 50 dB, input up to 10 dBm (the board recommends up to +5 dBm), tracking 120 kHz away from the local oscillator, button S1 turns the tracking on.
- **P1, 4 pins at the top** (silkscreen "USART") is the STM32 **SWD** port (SWC and SWD). With an ST-Link it would be possible to read the firmware, if there is no readout protection. Check the exact pinout on the board before connecting.
- External research: noise floor around −76 dBm, no input amplifier, fixed wide RBW, spurious peaks at ±25 and ±75 MHz from a 2.5 GHz signal.
- RF connectors: the setup changes between tests; check what is connected before every test (see the README cautions).

## 7. How to talk to the board

1. Plug the board into USB. Check `ls /dev/ttyUSB*` and `lsusb | grep 1a86` (CH340 `1a86:7523`). Do not touch any other serial device (for example `1a86:55d3` on `/dev/ttyACM0`, an unrelated instrument).
2. If it does not answer, unplug and replug the USB cable.
3. Use `cargo run --release --bin pakespec-cli` (see the README), or the Python client `tools/ltdz_client.py`. Example scripts are in `experiments/`.

## 8. Linked goal: reference for the Anritsu power sensors

The MA4701A and MA4702A sensors (RF power sensors found at a scrapyard) are calibrated on the original meter with 0 dBm at 50 MHz (and a 30 dB attenuator on the MA4702A). The tracking output of the LTDZ (about 0 dBm, constant, according to an external source) would be the reference. **The real level on the test unit has not been measured yet.** Measure it with an attenuator at the input of another instrument (the LTDZ's own input accepts at most +5 dBm).

## 9. Sources consulted

- Silicon Chip, "El Cheapo Modules: LTDZ Spectrum Analyser" (Jan 2022): https://siliconchip.com.au/Issue/2022/January/El+Cheapo+Modules%3A+LTDZ+Spectrum+Analyser
- Practical Electronics, "Geekcreit's LTDZ V5.0 Spectrum Analyser" (Feb 2023): https://siliconchip.com.au/Issue/PE/2023/February/Geekcreit%E2%80%99s+LTDZ+V5.0+Spectrum+Analyser
- EEVblog, Bicurico's software (VMA SSA) for the 138 MHz to 4.4 GHz SMA analyzers: https://www.eevblog.com/forum/testgear/presenting-my-new-software-for-the-sma-spectrum-analyzer-138mhz-4-4ghz/75/
- EEVblog, protocol of another ADF4351 generator (does not apply to the LTDZ): https://www.eevblog.com/forum/testgear/aliexpress-$30-35mhz-4-4ghz-adf4351-touchscreen-signal-generator/msg3228100/
- Python library for ADF4350/4351 (another board): https://github.com/Ho-Ro/adf435x

## 10. Existing alternatives

- **VMA SSA** (Vitor Martins Augusto): free, but with an activation code (about 3 months of trial, permanent for a donation of about US$ 10). Windows only.
- The vendor's own program (WinNWT4 4.11.09): only a Windows version is known here, no published source. The author's newer NWT4 releases (GPL v2 source, Linux builds, checked on dl4jal.de on 2026-10-17) are documented as not suitable for LTDZ boards.
- No open, cross-platform client for the LTDZ is known. That is the gap this project fills.

## 11. Open items and next steps

See `next-steps.md` for the maintained, ordered list.

## 12. Cautions

- RF input recommended up to +5 dBm; use an attenuator with any strong source.
- Do not touch unrelated serial devices (`/dev/ttyACM0`, `/dev/ttyS0`) during LTDZ tests.
- In this document, "confirmed" means observed directly on the board.

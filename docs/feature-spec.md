# Pakespec feature specification

Draft for discussion, written on 2026-10-17. It lists what the application should do, what already exists, what blocks each item and what needs a decision. Items are grouped by area; each has an id so they can be discussed and tracked.

**Product goal:** free, cross-platform (Windows and Linux) software that makes the LTDZ 35-4400M board behave like a bench spectrum analyzer and tracking generator, with a screen and controls that feel like a real instrument.

**Priorities:** **P0** needed for the first public release, **P1** next, **P2** later or optional.
**Status:** **Done**, **Partial**, **Planned**, **Blocked** (needs a hardware measurement or a decision first).

## 1. What the board allows and does not allow

These limits shape every feature. All were observed on the board (see `serial-protocol.md`).

| Fact | Consequence |
|---|---|
| 35 MHz to 4.4 GHz | Frequency range of every screen. |
| About 780 points per second | 1001 points take 1.28 s; 201 points 0.26 s. Update rate is a trade-off with resolution. |
| Step between points at most 99.999.999 Hz | Wide ranges need a minimum number of points (45 for the full range). |
| 2 to 9999 points per sweep | Point count limits. |
| Raw output is a 12-bit ADC value (144 to 182 with nothing connected) | Only about 35 levels at the noise floor; the trace looks stepped until averaged and calibrated. |
| RBW fixed (5 to 120 kHz according to the manufacturer) | No RBW, VBW, attenuator, preamplifier or detector controls. |
| RF input at most +5 dBm | The application must warn about input level; there is no protection. |
| The board does not report its state | No way to read its version reliably or detect a stuck state other than a timeout. |
| Tracking output level not measured; emission by the `f` command not verified | Generator features are blocked until measured. |

## 2. Feature list

### A. Connection and device

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| A1 | List serial ports with USB identity and suggest the board (CH340) when exactly one matches | P0 | Done (Linux) | |
| A2 | Same on Windows (registry) | P0 | Planned | A Windows machine to test |
| A3 | Choose the port by hand; remember the last choice | P0 | Done | |
| A4 | Never write to a port the user did not select | P0 | Done | |
| A5 | Clear error when the board is mute, with the unplug and replug advice | P0 | Done | |
| A6 | Automatic reconnect after the cable is replugged | P1 | Planned | |
| A7 | Linux help for serial permissions (group `dialout`) and Windows help for the CH340 driver | P1 | Planned | |
| A8 | Built-in simulator for use without a board | P0 | Done | |

### B. Sweep control

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| B1 | Start, stop, center and span entry; full range; zoom in and out | P0 | Done | |
| B2 | Center is kept exactly as typed; span shrinks if it does not fit | P0 | Done | |
| B3 | Point count with the minimum imposed by the board | P0 | Done | |
| B4 | Continuous sweep with the trace drawn while points arrive | P0 | Done | |
| B5 | Changes of range apply within about 0.13 s | P0 | Done: sweeps run in segments of 100 points because the board cannot abort a sweep | |
| B6 | Single sweep | P1 | Planned | |
| B7 | Span steps in a 1-2-5 sequence; settable center step | P1 | Planned | |
| B8 | Measured sweep time shown | P0 | Done | |
| B9 | Last span (undo the previous range) | P2 | Planned | |
| B10 | Zero span: show the level at one frequency over time (the board supports a step of 0, about 800 readings per second) | P2 | Planned | |

### C. Amplitude and scale

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| C1 | Fixed-step vertical scale that follows the data (raw units) | P0 | Done | |
| C2 | Manual reference level and scale per division | P0 | Done | |
| C3 | Calibration to dBm from known levels | P0 | Partial: fit, conversion and calibration file in `src/calibration.rs`, dBm column in `pakespec-cli --calibration`; measured 35 to 60 MHz with the FY6900 (`calibration.md`); the screen shows dBm when a calibration file exists (`calibration.toml` next to `settings.toml`), with `approximate dBm` in the header when the trace goes outside the calibrated frequencies and `~` before a marker value that is outside the measured range | Oscilloscope check of the generator amplitude |
| C4 | Calibration procedure inside the application, stored in a file per board | P1 | Planned | C3 |
| C9 | **Automated calibration rig.** A FeelElec FY6900 function generator (DDS, up to 60 MHz) is controlled over its USB serial port by the test scripts, so the calibration runs without anyone present: steps the amplitude in known increments at 35, 50 and 60 MHz, reads the board at each step, and fits the slope (raw units per dB) and the absolute level near 50 MHz. It covers only 35 to 60 MHz. The same rig can supply the 0 dBm at 50 MHz reference for the Anritsu power sensors. Needs: the FY6900 connected by USB, its serial protocol read from its manual, a check of how its displayed amplitude relates to a 50 ohm load (oscilloscope with a 50 ohm termination at a low frequency), and strict limits in the script (start low, never above +5 dBm at the board input, output off at the end). | P1 | Done for 35 to 60 MHz (`calibration.md`); the amplitude check with an oscilloscope is open | The FY6900 on USB; C3 |
| C5 | Display units dBm, dBµV, W | P2 | Planned | C3 |
| C6 | Reference offset for an external attenuator or amplifier | P1 | Planned | C3 |
| C7 | Correction table (for example antenna factor) | P2 | Planned | C3 |
| C8 | Warning when the signal is near the input limit | P1 | Planned | C3 |

### D. Traces

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| D1 | Live trace with averaging of 1, 4 or 16 sweeps | P0 | Done | |
| D2 | Max hold and min hold, clear hold | P0 | Done | |
| D3 | Average counter shown (`Avg 4/10`) | P1 | Planned | |
| D4 | View (freeze) and blank | P1 | Planned | |
| D5 | Up to three traces, each with its own mode | P2 | Planned | |
| D6 | Averaging length chosen freely | P2 | Planned | |

### E. Markers

Decided: no separate marker table and no waterfall. Readouts stay inside the plot or in the header.

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| E1 | One marker with peak search, next peak and marker to center | P0 | Done | |
| E2 | Marker moves with the knob and can be typed | P0 | Planned | |
| E3 | Several markers (M1 to M4) | P1 | Planned | |
| E4 | Delta marker (difference in frequency and level between two markers) | P1 | Planned | E3 |
| E5 | Marker to reference level | P1 | Planned | |
| E6 | Peak threshold and excursion setting | P2 | Planned | |
| E7 | Band power and noise markers | P2 | Planned | C3 |

### F. Screen and look

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| F1 | Four styles (Bench, Classic, Instrument, Modern) and presets | P0 | Done | |
| F2 | Three themes (dark, light, phosphor); any style with any theme | P0 | Done | |
| F3 | Header with reference level, scale, averaging and sweep time; footer with center, points and span | P0 | Done | |
| F4 | Front panel: hard keys, soft keys, keypad with units, knob (also with the mouse wheel) | P0 | Done | |
| F5 | Compact mode with plain controls | P1 | Done | |
| F6 | Zoom and pan of the plot with the mouse | P2 | Planned | Was available before the plots were redrawn |
| F7 | Limit lines (pass or fail) | P2 | Planned | C3 |
| F8 | Realistic physical look of the panel (bezel, relief, LEDs) | P2 | Planned | |
| F9 | Full-screen mode | P1 | Planned | |
| F10 | More styles and themes added later without touching the core | P1 | Done (architecture) | |

### G. Signal generator and tracking

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| G1 | Measure the real tracking output level and what the S1 button and the `f` command do | P0 | Partial: behavior understood (see `serial-protocol.md`); the absolute level is not measured | Another instrument with an attenuator for the level |
| G2 | Generator mode: set the tracking source frequency with `f` and read it with `m` (S1 on the board turns the output on) | P0 | Done: GEN key with frequency, output on and off and a live reading; checked on the board | |
| G3 | Tracking mode: sweep with the tracking source on | P1 | Works with the S1 button on (a sweep shows the through response); the application cannot turn S1 on or read its state, so it shows a hint when the reading is at the noise floor | |
| G4 | Filter response (S21): store a reference trace and show the difference | P1 | Blocked | G1, C3 |
| G5 | Reference source use: 50 MHz level to calibrate the Anritsu power sensors | P2 | Blocked | G1 |

### K. Automation and integration (future)

Both are thin front ends over the same core library that the graphical application uses (`src/board.rs`, `protocol.rs`, `worker.rs`), so the core must not depend on the screen. Safety rules that apply to both: validate every frequency and point count (already done in `protocol.rs`), never expose a firmware update, never turn anything on by itself (the output only exists while S1 is on), and report the same errors as the application.

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| K1 | **Use from the command line, so that other programs can control it.** Commands for listing ports, one sweep, continuous sweeps, setting and stopping the generator, reading the detector, and showing the status. Machine-readable output (JSON, and CSV for traces), stable exit codes, no screen needed, and the same settings file or explicit options. A long-running mode that reads commands from standard input and answers on standard output would let a program keep the board open between requests. | P2 | Done (2026-10-17): JSON output, `--measure`, `--generator`, `--status`, `--serve`, exit codes; see `automation.md`. Tried on the real board on 2026-10-17 (`--measure`, `--json`, `--serve`, `--generator-off`) | C3 |
| K2 | **MCP server so that an LLM can control it.** Tools such as list ports, sweep, generator on and off, read level, and get the last trace; results as structured data. Needs a permissive-license MCP library (check the license first) and a clear statement of what the model may do (a generator in particular). | P2 | Done (2026-10-17): `pakespec-mcp`, own JSON-RPC code so no library; `--no-generator`; see `automation.md`. Tried on 2026-10-17 with Claude Code as the client (`status` and `sweep` on the real board, `--no-generator`) | K1 |

### H. Data

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| H1 | Export the trace to CSV (frequency, level, hold traces) | P0 | Planned | |
| H2 | Save a screenshot as PNG | P0 | Planned | |
| H3 | Save and load the trace in the application's own format | P1 | Planned | |
| H4 | Copy the screenshot or the data to the clipboard | P2 | Planned | |
| H5 | Save and recall instrument states (range, scale, style, theme) | P1 | Planned | |
| H6 | Settings file with style, theme, language, port and last range | P0 | Done | |

### I. Usability

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| I1 | Interface in English and Portuguese, language picked from the system | P0 | Done | |
| I2 | More languages by adding a catalog file | P1 | Done (architecture) | |
| I3 | Keyboard shortcuts for the hard keys and the Run key | P1 | Planned | |
| I4 | Window layout that works on small screens | P1 | Partial | |
| I5 | Built-in help and a user guide | P1 | Planned | |

### J. Distribution and quality

| Id | Feature | Pri | Status | Depends on |
|---|---|---|---|---|
| J1 | Build and tests on Linux and Windows in GitHub Actions | P0 | Partial (written, never run) | The repository on GitHub |
| J2 | Release binaries attached to version tags | P0 | Written (`.github/workflows/release.yml`, never run; the packaging steps were checked locally for Linux) | J1 |
| J3 | License check of every dependency in the pipeline | P0 | Done (passes locally, pipeline never run) | J1 |
| J4 | Third-party license texts shipped with the binaries | P0 | Done in the release archives (`THIRD-PARTY-NOTICES.md`, `LICENSE`, `README.md`); never run on GitHub | J2 |
| J5 | Verified on a real Windows machine with the board | P0 | Planned | A Windows machine |
| J6 | Installer or portable package for Windows | P1 | Planned | J2 |
| J7 | Procedure to test on real hardware, kept in the repository | P1 | Done: `tests/hardware.rs` (ignored by default) | |

## 3. Acceptance criteria for the P0 features still open

- **A2:** on Windows the board is listed with its USB ids and suggested when it is the only CH340.
- **B5:** after changing center, span or points, the first points of the new sweep appear within 300 ms, on the real board, with no stuck or wrong data on the next sweeps.
- **C3 (with C4):** after calibrating, a known signal reads within 1 dB of its true level over the stated range, and the calibration file is kept between runs.
- **E2:** the marker can be moved with the knob and by typing a frequency; the readout stays in sync.
- **H1:** the CSV opens in a spreadsheet, has one row per point, and the header names the units.
- **H2:** the PNG shows exactly the current screen in the current style and theme.
- **J2, J5:** a tagged release produces a Windows binary and a Linux binary that open and sweep with the board.

## 4. Milestones (suggestion)

| Version | Contents | Goal |
|---|---|---|
| 0.1 | Scope (2026-10-17): show the spectrum, the basic controls of an analyzer and a working generator. Everything P0 except the dBm calibration: A2, E2, H1, H2, J1, J2, J4, J5 (B5 and G2 are done) | A usable viewer and generator, safe to publish, with raw levels |
| 0.2 | C3, C4, C6, C8, E3, E4, E5, D3, D4, B6, B7 | Real measurements in dBm and a fuller instrument feel |
| 0.3 | G3 and G4, the absolute output level (G1) | Filter response and a known generator level |
| 0.4 | K1 | The application controlled from the command line and from other programs |
| 0.5 | K2 | The application controlled by an LLM through MCP |
| Later | All P2 | Polish |

## 5. Decisions needed

1. ~~Is publishing a 0.1 with raw levels acceptable?~~ **Answered 2026-10-17:** yes, the first release only has to show the spectrum, have the basic controls and a working generator.
2. **Reference for the calibration and for G1:** which instrument is available (another analyzer, a power meter, a signal generator, a known attenuator)?
3. **Span behavior at the board's limits:** should the span, like the center, keep its value and shrink the other, or keep shifting the range?
4. **Zoom and pan with the mouse (F6):** needed, given the front panel?
5. **Windows:** who tests, and on which versions (10, 11)?
6. **Exports:** only CSV and PNG, or also formats of other tools (for example Touchstone for S21)?
7. **Distribution:** a plain archive, an installer, or both?

## 6. Out of scope

- Controls for RBW, VBW, attenuator, preamplifier and detector, because the board does not have them.
- Real-time (FFT) spectrum, waterfall and spectrogram displays.
- Reading or writing the board's firmware.
- Any other board family, until the LTDZ is finished.

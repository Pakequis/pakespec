# Testing

Three layers. Only the last needs the board.

## 1. Unit and integration tests (no hardware)

`cargo test` runs them. They use the board simulator (`src/sim.rs`), which reproduces the protocol, including the board's quirks: the mandatory delay field, the frequencies multiplied by 10, the first frequency command after a stop being ignored, and responses streamed while a sweep runs. What they cover: command encoding and decoding, point and step limits, sweeps in segments (equal to a plain sweep, stoppable between segments, no lone last point), the generator and the measure command, the background worker (progress events, generator readings, a new command interrupting a sweep), port detection with a fake sysfs, settings, themes, translations (every catalog has the same strings and placeholders) and the front panel logic (ranges, number entry, knob steps).

## 2. Screen tests (no hardware)

`tests/ui.rs` runs the application inside egui's test harness (`egui_kittest`), with the simulator as the port and no settings file: it clicks the keys by their labels, types numbers on the keypad, turns the generator on and off, starts a sweep and switches between the compact bar and the front panel. Text drawn directly on the plot is not part of the accessibility tree, so the plots themselves are not read by these tests; they are checked by looking at the application.

## 3. Hardware tests (ignored by default)

`tests/hardware.rs` and two tests in `tests/ui.rs` talk to the real board. Setup: the board on USB, a cable from RF OUT to RF IN, and **the tracking source turned on with the S1 button** (nothing in software can do that). Run:

```
cargo test -- --ignored --test-threads=1
```

The port comes from `PAKESPEC_PORT` (default `/dev/ttyUSB0`). They check, on the real board: the generator turns on, off and on again (the first frequency command after a stop is ignored, so the client sends it twice); the output stops at the edges of the band; sweeps in segments agree with plain sweeps; the first point of a sweep is right after the output was stopped; a full-range sweep needs 438 points; 40 full-range sweeps in a row never fail; the noise floor with the output off; and the application reading the generator and sweeping the board. Each test leaves the output off.

## Pipeline

`.github/workflows/ci.yml` runs formatting, clippy and the first two layers on Ubuntu and Windows, plus a license check of every dependency (`deny.toml`). It has not run yet because the repository has not been published.

## Checks done by hand

Reception with an antenna (see `serial-protocol.md`) and looking at the screens in the application. There is no screenshot test.

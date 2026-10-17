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

`src/automation.rs` and `src/mcp.rs` have unit tests against the simulator: request validation and error codes, generator permission, dBm filling, the line protocol, and the MCP handshake, tool list and calls. A manual check with a real MCP client: `claude -p --strict-mcp-config --mcp-config FILE` with a config that starts `pakespec-mcp --no-generator`, then ask it to call `status` and `sweep`.

## Pipeline

`.github/workflows/ci.yml` runs formatting, clippy and the first two layers on Ubuntu and Windows, plus `cargo deny check licenses bans advisories` (`deny.toml`). The pipeline has not run yet because the repository has not been published. The same deny check was run locally on 2026-10-17 and passes (licenses, bans, advisories; only duplicate-version warnings). Regenerate the notice file with `python3 tools/gen_third_party_notices.py` whenever dependencies change.

## Releases

`.github/workflows/release.yml` runs when a tag such as `v0.1.0` is pushed: it builds `pakespec`, `pakespec-cli` and `pakespec-mcp` on Ubuntu and Windows, packs each platform into an archive with `LICENSE`, `THIRD-PARTY-NOTICES.md` and `README.md`, and creates a GitHub release with both archives. Never run yet; the Linux packaging steps were checked by hand.

## Windows check (2026-10-17)

Windows 10, Rust 1.95, build output outside the project folder (`CARGO_TARGET_DIR`). Board on `COM5` (CH340), a cable from RF OUT to RF IN, S1 on, nothing else on the SMA connectors.

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass; the hardware tests are skipped by default. Clippy 1.95 flagged one `collapsible_match` in `src/worker.rs`; the arm now carries an `allow` with the reason.
- `cargo build --release --bins` builds `pakespec.exe`, `pakespec-cli.exe` and `pakespec-mcp.exe`.
- `pakespec-mcp --simulate` answers `tools/list` with the six tools.
- Port detection: the non-Linux code used to return ports without a USB identity, so no board was ever suggested on Windows. It now reads `HKLM\SYSTEM\CurrentControlSet\Enum\USB` (`VID_xxxx&PID_xxxx` keys, `Device Parameters\PortName`) and matches it against the ports that exist now. Result: `COM5` has `board_candidate` and `suggested`; the other CH34x device (`COM6`, `1a86:55d3`) is listed but not suggested. The key-name parsing has unit tests that run on every platform; the registry read itself was checked on this machine only.
- Real board through `pakespec-cli`: a 101-point sweep from 35 MHz to 1 GHz read 476 to 492 raw, and a measurement at 100 MHz read 487.
- `cargo test -- --ignored --test-threads=1` with `PAKESPEC_PORT=COM5`: all 9 hardware tests pass (7 in `tests/hardware.rs`, 2 in `tests/ui.rs`). The output was turned off afterwards.
- Application, checked by hand with the board: `pakespec.exe` opens, picks `COM5` on its own and shows the front panel. Release builds used to open a console window beside it; `src/bin/pakespec.rs` now uses the Windows GUI subsystem. The first Sweep was refused because the default of 401 points is too few for the full range (438 needed); the default is now 438 and loaded settings are raised to the minimum for their range. After that a full-range sweep draws the through response (about 490 at low frequencies, about 300 at 4.4 GHz), the generator screen reads 486 at 100 MHz, and the Portuguese language and the light theme apply without cut text. The status bar keeps the language it was written in until the next event. Settings are written to `%APPDATA%\pakespec\settings.toml`, created on the first change. At the time, the application showed raw values because no calibration existed; it now ships a built-in one.
- `cargo deny check licenses bans advisories` (cargo-deny 0.20.2) passes. `winreg` (new, Windows only) is MIT; its dependencies are `cfg-if` and `windows-sys`. New RustSec notices for `zvariant`, `zbus_names` and `zvariant_derive` (unmaintained, no vulnerability, no safe upgrade, pulled in only on Linux through eframe's accessibility support) are ignored in `deny.toml` with a comment; review them when eframe updates.
- Generator and closing the application, checked on the board: with the generator on, killing `pakespec.exe` leaves the output on (the board kept the tone, detector 486, until `--generator-off`); software cannot prevent that. Closing the window normally used to leave it on as well, because the worker returned without stopping the generator. The worker now turns the output off when it ends (`leave_output_off` in `src/worker.rs`, with unit tests); after closing the window with the generator on the detector read 148 to 150.

## Checks done by hand

Reception with an antenna (see `serial-protocol.md`) and looking at the screens in the application. There is no screenshot test.

# Pakespec

Free, cross-platform (Linux and Windows) software to control the **LTDZ 35M-4400M** family of spectrum analyzer and tracking generator boards (35 MHz to 4.4 GHz, STM32F103 + two ADF4351 + AD8307).

Why: the vendor software for this board is Windows-only and unmaintained, and there is no open, cross-platform client for it.

## Status (2026-10-16)

Early stage, working on the real board. What exists:

- **Protocol confirmed** on the board from Linux (see `docs/serial-protocol.md`): sweep, measure and generator commands, the board multiplies every frequency by 10, steps wrap at 10 MHz.
- A Rust core (protocol, board access, simulator, command-line sweep tool) with tests and a CI workflow, verified on the real board.
- A first graphical application (egui): four selectable screen styles (Bench, Classic, Instrument, Modern) with ready-made presets, dark, light and phosphor themes, max and min hold traces, English and Portuguese, serial port selection with board detection. Levels are raw ADC values until the dBm calibration exists.
- A generator screen (the tracking source, turned on by the S1 button on the board) and reception tested with an antenna (Wi-Fi and cellular bursts seen).
- Hardware research and documented experiments on the board.

Nothing has been published. The repository is local only.

## Where to read first

1. `docs/full-record.md`: everything done so far, tools, timeline, open items.
2. `docs/serial-protocol.md`: the board protocol on one page.
3. `docs/hardware.md`: chips, connectors and specifications.
4. `docs/next-steps.md`: what to do, in order.
5. `docs/decisions.md`: choices already made (name, license, platforms) and what is still open.
6. `docs/feature-spec.md`: the feature list, what is done, what is blocked, and the decisions needed.
7. `docs/testing.md`: how the tests are organized and how to run them.

## Folders

- `src/`: Rust code. `protocol.rs` (wire format), `board.rs` (talking to a board), `sim.rs` (board simulator), `ports.rs` (port detection), `worker.rs` (background sweeps), `theme.rs`, `i18n.rs` and `settings.rs`, `ui/` (egui screens), `bin/` (the application and the sweep-to-CSV tool).
- `locales/`: interface translations.
- `tests/`: integration tests against the simulator.
- `tools/`: standalone Python client used during the research.
- `experiments/`: test scripts used during the research. Not production code.
- `docs/`: documentation.

## Quick usage

Graphical application (pick the port, press Sweep; the simulator entry works without a board):

```
cargo run --release --bin pakespec
```

Command line:

With the board on USB (CH340, `/dev/ttyUSB0` on Linux), a sweep printed as CSV (`freq_hz,ch1,ch2`, raw values):

```
cargo run --release --bin pakespec-cli -- --port /dev/ttyUSB0 --start 35000000 --stop 1000000000 --points 101
```

Without `--port`, the only USB serial converter that matches the board is used; `--list-ports` shows what was found. Add `--simulate` to run without a board. The generator needs the S1 button on the board turned on.

Tests: `cargo test` needs no hardware. With the board connected, RF OUT cabled to RF IN and S1 on: `cargo test -- --ignored --test-threads=1`. Run the tests with `cargo test` (no hardware needed). If the board does not answer, unplug and replug the USB cable.

## License

MIT - see [`LICENSE`](LICENSE).

## Cautions

- Recommended RF input up to +5 dBm; use an attenuator with strong sources.
- Do not open other serial devices on the machine while testing; only the board's CH340 (`1a86:7523`).

# Next steps

Suggested order. Update this file when something is finished or changes.

1. ~~Understand the `x` command alternation.~~ **Solved on 2026-10-16**: the 3-digit delay field was missing (see `serial-protocol.md`). Still to find out: what the delay value does, if anything (test with the tracking source on).
2. **Generator and units.** Done on 2026-10-16: the board multiplies command frequencies by 10 (the client works in real frequencies), the step wraps at 10 MHz (the earlier level "anomaly" was this), `m` measures, the generator is the tracking source gated by the S1 button, and the application has a GEN screen. Still open: redoing the antenna test with real frequencies, and the absolute level of the output. Original item: **Measure the tracking output** (loopback of 2026-10-16: tracking on gives a flat trace; the `f` command gave no visible signal with tracking off; see `serial-protocol.md`)  (S1 button and the `f` command) at 50 MHz, with an attenuator, using another instrument. This decides whether the board can serve as a reference to calibrate the Anritsu MA4701A and MA4702A power sensors (see `full-record.md`, section 10).
3. **dBm calibration.** First fit done on 2026-10-16 with the FY6900 (see `calibration.md`): raw = 8.55 x dBm + 443.3 from -33 to -2 dBm at 50 MHz, flat from 35 to 60 MHz. The core part is implemented (`src/calibration.rs`, `pakespec-cli --fit-calibration` and `--calibration`). Still to do: show dBm on the screen, check the generator amplitude with an oscilloscope. Original plan: build our own calibration from two measurements with a known level and a known attenuator (at least 20 dB apart): slope = dB difference / raw-value difference.
4. **Tracking mode.** How to turn the tracking source on from software and how to combine the `f` command with the sweep.
5. ~~Application skeleton~~ **Done on 2026-10-16**: Rust crate with protocol, board access, simulator, command-line tool, tests, CI, themes, translations (English and Portuguese), saved settings, port detection, a background sweep thread and an egui screen with four selectable styles (Bench, Classic, Instrument, Modern), three themes, presets, max and min hold traces and an automatic fixed-step vertical scale. First tried on the real board on 2026-10-16. The board delivers about 780 points per second (1001 points take 1.28 s, 201 points 0.26 s), so the trace is drawn while the points arrive; a frequency range or point count change applies at the next sweep; the trace can be averaged over 4 or 16 sweeps (default 4). The raw ADC value has few levels (about 35 across the whole range with nothing connected), so the trace stays stepped until the dBm calibration and a fixed vertical scale exist. The screen now has a front panel (hard keys, soft keys, keypad, knob; see `decisions.md`). The egui font license exception is recorded in `decisions.md`. Still to do: Windows port detection through the registry; the dBm calibration; more styles and themes.
6. **Minimum application features (egui):** continuous sweep, dBm plot, markers, CSV and image export, filter response with the tracking source.
7. **Before publishing:** check the licenses of any third-party material, remove any third-party file from the repository, and get the owner's approval to publish.

## State at the end of 2026-10-17

Done on 2026-10-17: license check (`cargo deny`) and third-party notices (`tools/gen_third_party_notices.py`); command-line JSON output, `--serve` mode and the MCP server (`automation.md`), tried on the real board and with Claude Code as the client; findings about S1 and unknown commands in `serial-protocol.md`. All 9 hardware tests pass (S1 must be on for them).

Waiting for: SMA attenuators ordered by the owner (about 6, 10, 10, 20 dB, maybe 3 dB, plus a 15 cm SMA cable; arrival in two to three weeks). No oscilloscope, NanoVNA, RF generator or power sensor available.

Next, in this order:

1. **Calibration above 60 MHz with the attenuators** (approximate, enough for a coherent spectrum). Ask the owner to switch S1 on and to build each attenuation between RF OUT and RF IN (all through-signal levels stay at or below the tracking output, which was connected straight to the input before without damage: raw 486 is about +5 dBm, so use 10 dB or more). For each of 10, 16 (10+6), 20, 26 (20+6), 30 (20+10), 36 (20+10+6) dB run `python3 experiments/attenuator_scan.py DB` (add `--simulate` for a dry run), then `python3 experiments/attenuator_fit.py`. It prints the slope in counts per dB per band (the reference is 8.55 at 50 MHz) and the level change with frequency. Check that 10+10 agrees with 20 dB to spot a bad attenuator.
2. **Use the result in the application.** If the slope is about the same everywhere, add a per-frequency level correction (flatness table) to the calibration file and apply it in `calibration.rs`, with the `approximate dBm` badge kept where the correction is not measured. The flatness found this way includes the cable and adapters.
3. **Interface text.** The generator screen still says the output needs S1; fixed-frequency output works with S1 off (see `serial-protocol.md`). Change the text in `src/ui/generator.rs` and `locales/*.toml`, after the owner confirms.
4. **Pipeline and Windows.** Run the pipeline on GitHub (needs the repository published, owner decision), verify on Windows, attach `THIRD-PARTY-NOTICES.md` to release archives.
5. **Check the FY6900 amplitude** with an oscilloscope on 50 ohm if one becomes available; the absolute dBm values depend on it.

Hardware reminders: the FY6900 is `/dev/serial/by-id/usb-1a86_USB_Serial-if00-port0`, both outputs off (it may be powered on with 5 Vpp and outputs on: read `RMN`, `RFN` and `RMA`, switch the outputs off and set the amplitude before connecting anything). Never send unknown commands to the board (it goes silent until the USB cable is replugged).

## State at the end of 2026-10-16 (earlier)

Done today: FY6900 control (`calibration.md`), first dBm calibration (35 to 60 MHz), dBm on the screen with an `approximate dBm` badge outside the calibrated range, all hardware tests passing. Next, in this order:

1. Check the FY6900 amplitude with an oscilloscope on 50 ohm; the dBm values depend on it.
2. Calibrate above 60 MHz (needs another reference source, for example the tracking output with a known attenuator).
3. ~~License check and notice file~~ **Done on 2026-10-17**: `cargo deny check licenses bans advisories` passes; `THIRD-PARTY-NOTICES.md` is generated by `tools/gen_third_party_notices.py`. Still open: run the pipeline on GitHub (needs the repository published) and verify on Windows; ship the notice file with release archives.
4. ~~Command-line control and the MCP server~~ **Done on 2026-10-17** (`automation.md`; unit tests against the simulator). Tried the same day on the real board and with Claude Code as an MCP client (`status`, `sweep`); the generator tools have only been tried through the command line.

Open question (2026-10-17): the S1 state cannot be read (see `serial-protocol.md`), and probing unknown command letters freezes the board, so the application can only infer S1 from a sweep with the output cabled to the input. The generator screen text still says the output needs S1; it should be changed after the owner confirms that fixed-frequency output without S1 is acceptable behavior to document.

## Behaving like a real spectrum analyzer

Requirement from the owner (2026-10-16): the application has to work like a real bench spectrum analyzer. Gaps found so far, in the suggested order (the first four are software only):

1. ~~Immediate reaction to changes.~~ **Done on 2026-10-16.** The board ignores commands while it sends a sweep, so a sweep cannot be aborted; the application sweeps in segments of 100 points (about 0.13 s) and looks at the new settings between segments.
2. **Several markers.** A real analyzer has several markers, delta mode (difference between two points) and a peak table. Here there is one marker (M1) with peak search, next peak and marker to center.
3. **Trace modes.** Clear and write, average with a counter (`Avg 4/10`), max hold, min hold, view (freeze) and blank. Here: average of 1, 4 or 16, max hold, min hold and clear hold.
4. **Save and export.** Screenshot and CSV of the trace. Not implemented.
5. **Level in dBm.** The axis shows raw values, and the automatic scale does not exist on real analyzers, which start from a fixed reference level. Needs the dBm calibration (item 3 above) with a known reference signal, such as the tracking output through an attenuator or another signal generator.
6. **Knob and step behavior.** Real analyzers step the span in a 1-2-5 sequence and let the user set the center step. Here the knob steps are proportional to the span.
7. **Single sweep.** One sweep on demand, besides continuous sweeping.
8. **Span and center interplay.** Setting the center keeps the center exactly and shrinks the span if it does not fit inside the board's limits (35 MHz to 4.4 GHz); setting the span keeps the center when possible but shifts the range at the limits. Decide whether the span should also keep the center and shrink.

Not possible on this board, so no keys for them: RBW and VBW control, input attenuator, preamplifier, detector choice.

## Future (requested by the owner on 2026-10-16)

0. **Automatic calibration with the function generator** (idea, see `feature-spec.md`, C9). The owner wants the tests run automatically by the assistant, so the FY6900 has to be controlled over USB serial by the scripts. First step: connect it, identify the port, read its protocol, and check with output off before sending any level.

1. **Command-line control for other programs.** Extend `pakespec-cli` into a scriptable interface: JSON and CSV output, a command to read the detector, to set and stop the generator, to list ports and show the status, stable exit codes, and a mode that keeps the board open and reads commands from standard input. Details in `feature-spec.md`, K1.
2. **MCP server for LLM control**, built on the command-line layer, with a license check on any library before using it. Details in `feature-spec.md`, K2.
3. **Zero span** (step 0) as a time view of the level at one frequency; see B10.
4. **Antenna.** The 10 cm dipole sees Wi-Fi and cellular bursts but not FM broadcast; a longer wire or telescopic antenna would show FM.
5. **Windows:** verify the application and the port detection (registry) on a real machine.

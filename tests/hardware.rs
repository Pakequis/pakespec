//! Tests against a real board. They are ignored by default.
//!
//! Setup: the board on USB, a cable from RF OUT to RF IN, and the tracking
//! source turned on with the S1 button. Run with
//! `cargo test --test hardware -- --ignored --test-threads=1`.
//! The port is taken from `PAKESPEC_PORT` (default `/dev/ttyUSB0`).

use pakespec::board::{Board, open_serial};
use pakespec::protocol::SweepParams;
use pakespec::worker::Transport;

fn board() -> Board<Box<dyn Transport>> {
    let port = std::env::var("PAKESPEC_PORT").unwrap_or_else(|_| "/dev/ttyUSB0".into());
    let b = open_serial(&port).unwrap_or_else(|e| panic!("cannot open {port}: {e}"));
    Board::new(Box::new(b.into_inner()) as Box<dyn Transport>)
}

#[test]
#[ignore = "needs the board with the tracking source on"]
fn generator_turns_on_off_and_on_again() {
    let mut b = board();
    b.set_fixed_frequency(100_000_000).unwrap();
    let on = b.measure().unwrap();
    assert!(on > 400, "tone at 100 MHz read {on}");
    b.set_fixed_frequency(0).unwrap();
    let off = b.measure().unwrap();
    assert!(off < 200, "output off read {off}");
    // The board ignores the first frequency command after a stop; the client
    // sends it twice, so the tone must be back.
    b.set_fixed_frequency(100_000_000).unwrap();
    let again = b.measure().unwrap();
    assert!(again > 400, "tone after a stop read {again}");
    b.set_fixed_frequency(0).unwrap();
}

#[test]
#[ignore = "needs the board with the tracking source on"]
fn the_tone_stops_at_the_edges_of_the_band() {
    // Real frequencies: the board multiplies the commanded value by 10.
    let mut b = board();
    b.set_fixed_frequency(4_300_000_000).unwrap();
    assert!(
        b.measure().unwrap() > 250,
        "4.3 GHz should still carry a signal"
    );
    b.set_fixed_frequency(4_460_000_000).unwrap();
    assert!(b.measure().unwrap() < 200, "4.46 GHz is above the band");
    b.set_fixed_frequency(0).unwrap();
}

#[test]
#[ignore = "needs the board with the tracking source on"]
fn segmented_and_plain_sweeps_agree() {
    let mut b = board();
    let params = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
    let plain = b.sweep(&params).unwrap();
    let segmented = b
        .sweep_segmented(&params, 30, |_, _| {}, || true)
        .unwrap()
        .unwrap();
    assert_eq!(plain.samples.len(), segmented.samples.len());
    // Same frequencies, so levels agree up to the detector noise.
    for (i, (p, s)) in plain.samples.iter().zip(&segmented.samples).enumerate() {
        assert!(
            p.ch1.abs_diff(s.ch1) < 30,
            "point {i}: {} vs {}",
            p.ch1,
            s.ch1
        );
    }
    // The through response with the cable is high at 100 MHz.
    assert!(plain.samples[7].ch1 > 400);
}

#[test]
#[ignore = "needs the board with the tracking source on"]
fn a_full_range_sweep_needs_438_points_and_covers_the_band() {
    assert!(SweepParams::new(35_000_000, 4_400_000_000, 437).is_err());
    let mut b = board();
    let params = SweepParams::new(35_000_000, 4_400_000_000, 438).unwrap();
    let sweep = b.sweep(&params).unwrap();
    assert_eq!(sweep.samples.len(), 438);
    let at = |mhz: u64| {
        let i = ((mhz * 1_000_000 - 35_000_000) / params.step_hz()) as usize;
        sweep.samples[i].ch1
    };
    assert!(at(100) > 400);
    // The response falls toward the top of the band, as the cable does.
    assert!(at(4300) < at(100));
}

#[test]
#[ignore = "needs the board"]
fn the_board_reports_firmware_noise_floor_when_the_output_is_off() {
    let mut b = board();
    b.set_fixed_frequency(0).unwrap();
    let v = b.measure().unwrap();
    assert!((140..=160).contains(&v), "noise floor read {v}");
}

#[test]
#[ignore = "needs the board with the tracking source on"]
fn the_first_sweep_point_is_right_after_the_output_was_stopped() {
    let mut b = board();
    let params = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
    for _ in 0..3 {
        b.set_fixed_frequency(0).unwrap();
        let sweep = b.sweep(&params).unwrap();
        // Without parking the synthesizer the first point reads about 356.
        assert!(
            sweep.samples[0].ch1 > 450,
            "first point {}",
            sweep.samples[0].ch1
        );
    }
}

#[test]
#[ignore = "needs the board with the tracking source on; takes about 30 seconds"]
fn many_sweeps_in_a_row_never_fail() {
    let mut b = board();
    let params = SweepParams::new(35_000_000, 4_400_000_000, 438).unwrap();
    for i in 0..40 {
        let sweep = b
            .sweep_segmented(&params, 100, |_, _| {}, || true)
            .unwrap_or_else(|e| panic!("sweep {i} failed: {e}"))
            .unwrap();
        assert_eq!(sweep.samples.len(), 438);
        assert!(
            sweep.samples[10].ch1 > 400,
            "sweep {i}: {}",
            sweep.samples[10].ch1
        );
    }
}

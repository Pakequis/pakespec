use std::io::{Read, Write};

use pakespec::board::{Board, BoardError};
use pakespec::protocol::{self, SweepParams};
use pakespec::sim::SimBoard;

#[test]
fn sweep_returns_all_points_with_expected_levels() {
    let params = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
    let sweep = Board::new(SimBoard::new()).sweep(&params).unwrap();
    assert_eq!(sweep.samples.len(), 101);
    for (i, s) in sweep.samples.iter().enumerate() {
        assert_eq!(s.ch1, SimBoard::expected_level(&params, i));
        assert_eq!(s.ch1, s.ch2);
    }
}

#[test]
fn repeated_sweeps_never_skip() {
    let params = SweepParams::new(35_000_000, 45_000_000, 11).unwrap();
    let mut board = Board::new(SimBoard::new());
    for _ in 0..20 {
        assert_eq!(board.sweep(&params).unwrap().samples.len(), 11);
    }
}

#[test]
fn largest_sweep_works() {
    let params = SweepParams::new(35_000_000, 4_400_000_000, 1001).unwrap();
    let sweep = Board::new(SimBoard::new()).sweep(&params).unwrap();
    assert_eq!(sweep.samples.len(), 1001);
    // The synthetic 940 MHz signal must stand out from the floor.
    let max = sweep.samples.iter().map(|s| s.ch1).max().unwrap();
    assert!(max > 250, "peak {max}");
}

#[test]
fn command_without_delay_field_gets_no_answer_until_completed() {
    // Reproduces the board's behaviour: 21 digits leave the command incomplete.
    let params = SweepParams::new(35_000_000, 45_000_000, 11).unwrap();
    let mut cmd = protocol::encode_sweep(&params, 0);
    cmd.truncate(cmd.len() - 3);
    let mut sim = SimBoard::new();
    sim.write_all(&cmd).unwrap();
    let mut buf = [0u8; 4];
    assert_eq!(
        sim.read(&mut buf).unwrap_err().kind(),
        std::io::ErrorKind::TimedOut
    );
}

#[test]
fn mute_board_reports_timeout() {
    struct Mute;
    impl Read for Mute {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "mute"))
        }
    }
    impl Write for Mute {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let params = SweepParams::new(35_000_000, 45_000_000, 11).unwrap();
    match Board::new(Mute).sweep(&params) {
        Err(BoardError::Timeout {
            expected: 44,
            received: 0,
        }) => {}
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn fixed_frequency_and_measure_use_real_frequencies() {
    // The command carries the real frequency divided by 10; the simulator
    // multiplies it back, so the reading matches the real frequency.
    let mut board = Board::new(SimBoard::new());
    board.set_fixed_frequency(940_000_000).unwrap();
    let at_940 = board.measure().unwrap();
    assert_eq!(at_940, SimBoard::level_at(940_000_000));
    board.set_fixed_frequency(0).unwrap();
    assert!(board.measure().unwrap() < at_940);
}

#[test]
fn generator_comes_back_after_being_stopped() {
    let mut board = Board::new(SimBoard::new());
    board.set_fixed_frequency(940_000_000).unwrap();
    let on = board.measure().unwrap();
    board.set_fixed_frequency(0).unwrap();
    // The board ignores the first frequency command after a stop; the client
    // sends it twice, so the tone is back.
    board.set_fixed_frequency(940_000_000).unwrap();
    assert_eq!(board.measure().unwrap(), on);
}

#[test]
fn a_single_frequency_command_after_a_stop_is_ignored() {
    let mut sim = SimBoard::new();
    sim.write_all(&protocol::encode_fixed_frequency(0)).unwrap();
    sim.write_all(&protocol::encode_fixed_frequency(940_000_000))
        .unwrap();
    let mut board = Board::new(sim);
    assert_eq!(board.measure().unwrap(), SimBoard::level_at(0));
}

#[test]
fn segmented_sweep_equals_a_plain_sweep() {
    let params = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
    let plain = Board::new(SimBoard::new()).sweep(&params).unwrap();
    let mut seen = Vec::new();
    let segmented = Board::new(SimBoard::new())
        .sweep_segmented(
            &params,
            30,
            |first, pts| {
                assert_eq!(first, seen.len());
                seen.extend_from_slice(pts);
            },
            || true,
        )
        .unwrap()
        .unwrap();
    assert_eq!(segmented.samples, plain.samples);
    assert_eq!(seen, plain.samples);
}

#[test]
fn segmented_sweep_stops_between_segments() {
    let params = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
    let mut seen = 0;
    let result = Board::new(SimBoard::new())
        .sweep_segmented(&params, 30, |_, pts| seen += pts.len(), || false)
        .unwrap();
    assert!(result.is_none());
    assert_eq!(seen, 30);
}

#[test]
fn a_lone_last_point_joins_the_previous_segment() {
    // 61 points in segments of 30 would leave one point alone.
    let params = SweepParams::new(35_000_000, 400_000_000, 61).unwrap();
    let sweep = Board::new(SimBoard::new())
        .sweep_segmented(&params, 30, |_, _| {}, || true)
        .unwrap()
        .unwrap();
    assert_eq!(sweep.samples.len(), 61);
}

//! Wire format of the LTDZ 35-4400M serial protocol.
//!
//! Every command starts with the byte `0x8F` followed by ASCII text. See
//! `docs/serial-protocol.md` for how each item was verified on the board.

use std::fmt;

/// Serial line speed of the board (8 data bits, no parity, 1 stop bit).
pub const BAUD_RATE: u32 = 57_600;
/// First byte of every command.
pub const COMMAND_PREFIX: u8 = 0x8F;
/// Lowest frequency the board can sweep, in Hz.
pub const MIN_FREQ_HZ: u64 = 35_000_000;
/// Highest frequency the board can sweep, in Hz.
pub const MAX_FREQ_HZ: u64 = 4_400_000_000;
/// Number of bytes the board returns per sweep point.
pub const BYTES_PER_POINT: usize = 4;
/// The board multiplies every frequency of a command by this number: the
/// start, step and fixed-frequency fields hold the real frequency divided by 10.
pub const FREQ_UNIT_HZ: u64 = 10;
/// The board takes the commanded step modulo 1,000,000 units, that is modulo
/// 10 MHz real: a step of 10 MHz does not advance the frequency at all and a step
/// of 11 MHz advances 1 MHz. Steps must therefore stay below this value.
pub const STEP_WRAP_HZ: u64 = 1_000_000 * FREQ_UNIT_HZ;
/// Largest real step that behaves correctly.
pub const MAX_STEP_HZ: u64 = STEP_WRAP_HZ - FREQ_UNIT_HZ;
/// Length of the ASCII part of a sweep command after the `x` marker:
/// start (9) + step (8) + points (4) + delay (3).
pub const SWEEP_FIELDS_LEN: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Start is not below stop, or the range is outside what the board supports.
    InvalidRange { start_hz: u64, stop_hz: u64 },
    /// The point count must be between 2 and 9999 (four decimal digits).
    InvalidPoints(u16),
    /// The step between points would reach 10 MHz, which the board cannot do.
    TooFewPoints { points: u16, min_points: u16 },
    /// The response has a different length than `points * 4` bytes.
    BadResponseLength { expected: usize, actual: usize },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { start_hz, stop_hz } => write!(
                f,
                "invalid frequency range {start_hz} Hz to {stop_hz} Hz (supported: \
                 {MIN_FREQ_HZ} Hz to {MAX_FREQ_HZ} Hz, start below stop)"
            ),
            Self::InvalidPoints(n) => write!(f, "invalid point count {n} (supported: 2 to 9999)"),
            Self::TooFewPoints { points, min_points } => write!(
                f,
                "{points} points are too few for this range: the step between points must be \
                 below 10 MHz, so at least {min_points} points are needed"
            ),
            Self::BadResponseLength { expected, actual } => {
                write!(f, "response has {actual} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Parameters of one sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepParams {
    pub start_hz: u64,
    pub stop_hz: u64,
    pub points: u16,
}

impl SweepParams {
    pub fn new(start_hz: u64, stop_hz: u64, points: u16) -> Result<Self, ProtocolError> {
        if start_hz < MIN_FREQ_HZ || stop_hz > MAX_FREQ_HZ || start_hz >= stop_hz {
            return Err(ProtocolError::InvalidRange { start_hz, stop_hz });
        }
        if !(2..=9999).contains(&points) {
            return Err(ProtocolError::InvalidPoints(points));
        }
        let min_points = Self::min_points(start_hz, stop_hz);
        if points < min_points {
            return Err(ProtocolError::TooFewPoints { points, min_points });
        }
        Ok(Self {
            // The command carries tenths, so the start is a multiple of 10 Hz.
            start_hz: start_hz / FREQ_UNIT_HZ * FREQ_UNIT_HZ,
            stop_hz,
            points,
        })
    }

    /// Fewest points that keep the step between points within `MAX_STEP_HZ`.
    pub fn min_points(start_hz: u64, stop_hz: u64) -> u16 {
        let span = stop_hz.saturating_sub(start_hz);
        u16::try_from(span / STEP_WRAP_HZ + 2).unwrap_or(u16::MAX)
    }

    /// Distance between two consecutive points, in Hz (integer division).
    pub fn step_hz(&self) -> u64 {
        (self.stop_hz - self.start_hz) / u64::from(self.points - 1) / FREQ_UNIT_HZ * FREQ_UNIT_HZ
    }

    /// Frequency of point `index`, in Hz.
    pub fn freq_at(&self, index: usize) -> u64 {
        self.start_hz + self.step_hz() * index as u64
    }

    /// A part of this sweep: `count` points starting at point `first`, with the
    /// same step, so that its points are exactly points `first..first + count` of
    /// the whole sweep.
    pub fn segment(&self, first: usize, count: usize) -> SweepParams {
        let step = self.step_hz();
        let start_hz = self.start_hz + step * first as u64;
        SweepParams {
            start_hz,
            stop_hz: start_hz + step * (count as u64).saturating_sub(1),
            points: count as u16,
        }
    }

    /// Number of response bytes the board sends for this sweep.
    pub fn response_len(&self) -> usize {
        usize::from(self.points) * BYTES_PER_POINT
    }
}

/// Builds a sweep command: `0x8F x <start:9> <step:8> <points:4> <delay:3>`.
///
/// The trailing delay field is mandatory: without it the board keeps waiting
/// for the missing bytes and answers only after the next command arrives.
pub fn encode_sweep(params: &SweepParams, delay: u16) -> Vec<u8> {
    let text = format!(
        "x{:09}{:08}{:04}{:03}",
        params.start_hz / FREQ_UNIT_HZ,
        params.step_hz() / FREQ_UNIT_HZ,
        params.points,
        delay.min(999)
    );
    let mut out = Vec::with_capacity(1 + text.len());
    out.push(COMMAND_PREFIX);
    out.extend_from_slice(text.as_bytes());
    out
}

/// Builds a fixed-frequency command: `0x8F f <freq:9>`. Frequency 0 stops it.
pub fn encode_fixed_frequency(freq_hz: u64) -> Vec<u8> {
    let mut out = vec![COMMAND_PREFIX];
    out.extend_from_slice(format!("f{:09}", freq_hz / FREQ_UNIT_HZ).as_bytes());
    out
}

/// Builds the measure command `0x8F m`: the board answers with 4 bytes (two
/// little-endian 16-bit detector values) taken at the frequency set by the last
/// fixed-frequency command.
pub fn encode_measure() -> Vec<u8> {
    vec![COMMAND_PREFIX, b'm']
}

/// One point of a sweep response, as raw 16-bit ADC-scale values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub ch1: u16,
    pub ch2: u16,
}

/// Decodes `points * 4` response bytes: two little-endian u16 values per point.
pub fn decode_sweep(bytes: &[u8], params: &SweepParams) -> Result<Vec<Sample>, ProtocolError> {
    let expected = params.response_len();
    if bytes.len() != expected {
        return Err(ProtocolError::BadResponseLength {
            expected,
            actual: bytes.len(),
        });
    }
    Ok(decode_samples(bytes))
}

/// Decodes every complete 4-byte point in `bytes`; a trailing partial point is ignored.
pub fn decode_samples(bytes: &[u8]) -> Vec<Sample> {
    bytes
        .as_chunks::<BYTES_PER_POINT>()
        .0
        .iter()
        .map(|c| Sample {
            ch1: u16::from_le_bytes([c[0], c[1]]),
            ch2: u16::from_le_bytes([c[2], c[3]]),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_command_has_all_four_fields() {
        let p = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
        assert_eq!(p.step_hz(), 9_650_000);
        let cmd = encode_sweep(&p, 0);
        assert_eq!(cmd[0], 0x8F);
        assert_eq!(&cmd[1..], b"x003500000009650000101000");
        assert_eq!(cmd.len() - 2, SWEEP_FIELDS_LEN);
    }

    #[test]
    fn fixed_frequency_command() {
        assert_eq!(encode_fixed_frequency(100_000_000), b"\x8ff010000000");
        assert_eq!(encode_fixed_frequency(10_000_000), b"\x8ff001000000");
        assert_eq!(encode_measure(), b"\x8fm");
        assert_eq!(encode_fixed_frequency(0), b"\x8ff000000000");
    }

    #[test]
    fn segments_cover_the_sweep_exactly() {
        let p = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
        let (a, b) = (p.segment(0, 40), p.segment(40, 61));
        assert_eq!(a.step_hz(), p.step_hz());
        assert_eq!(b.step_hz(), p.step_hz());
        assert_eq!(a.freq_at(39), p.freq_at(39));
        assert_eq!(b.freq_at(0), p.freq_at(40));
        assert_eq!(b.freq_at(60), p.freq_at(100));
        assert_eq!(a.points + b.points, p.points);
    }

    #[test]
    fn rejects_bad_parameters() {
        assert!(SweepParams::new(1_000_000, 100_000_000, 11).is_err());
        assert!(SweepParams::new(35_000_000, 5_000_000_000, 11).is_err());
        assert!(SweepParams::new(100_000_000, 100_000_000, 11).is_err());
        assert!(SweepParams::new(35_000_000, 100_000_000, 1).is_err());
        assert!(SweepParams::new(35_000_000, 100_000_000, 10_000).is_err());
    }

    #[test]
    fn step_must_fit_the_eight_digit_field() {
        // The board wraps the step at 10 MHz, so the widest sweep needs 438 points.
        assert_eq!(SweepParams::min_points(MIN_FREQ_HZ, MAX_FREQ_HZ), 438);
        assert_eq!(
            SweepParams::new(MIN_FREQ_HZ, MAX_FREQ_HZ, 437),
            Err(ProtocolError::TooFewPoints {
                points: 437,
                min_points: 438
            })
        );
        let ok = SweepParams::new(MIN_FREQ_HZ, MAX_FREQ_HZ, 438).unwrap();
        assert!(ok.step_hz() <= MAX_STEP_HZ);
        assert_eq!(encode_sweep(&ok, 0).len() - 2, SWEEP_FIELDS_LEN);
        assert_eq!(SweepParams::min_points(100_000_000, 500_000_000), 42);
        // Two points are enough for spans below 10 MHz.
        assert_eq!(SweepParams::min_points(100_000_000, 105_000_000), 2);
        assert_eq!(SweepParams::min_points(100_000_000, 110_000_000), 3);
    }

    #[test]
    fn decodes_little_endian_pairs() {
        let p = SweepParams::new(35_000_000, 40_000_000, 2).unwrap();
        let samples = decode_sweep(&[0x96, 0x00, 0x97, 0x01, 0xB6, 0x00, 0xB6, 0x00], &p).unwrap();
        assert_eq!(samples[0], Sample { ch1: 150, ch2: 407 });
        assert_eq!(samples[1], Sample { ch1: 182, ch2: 182 });
    }

    #[test]
    fn rejects_truncated_response() {
        let p = SweepParams::new(35_000_000, 45_000_000, 3).unwrap();
        assert_eq!(
            decode_sweep(&[0; 8], &p),
            Err(ProtocolError::BadResponseLength {
                expected: 12,
                actual: 8
            })
        );
    }
}

//! Talking to a board over any byte transport (a real serial port or the simulator).

use std::io::{self, Read, Write};
use std::time::Duration;

use crate::protocol::{self, Sample, SweepParams};

#[derive(Debug)]
pub enum BoardError {
    Io(io::Error),
    Protocol(protocol::ProtocolError),
    /// The board did not send the full response in time.
    Timeout {
        expected: usize,
        received: usize,
    },
}

impl std::fmt::Display for BoardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "serial I/O error: {e}"),
            Self::Protocol(e) => write!(f, "{e}"),
            Self::Timeout { expected, received } => write!(
                f,
                "timeout: received {received} of {expected} bytes (if the board stays mute, unplug and replug the USB cable)"
            ),
        }
    }
}

impl std::error::Error for BoardError {}

impl From<io::Error> for BoardError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<protocol::ProtocolError> for BoardError {
    fn from(e: protocol::ProtocolError) -> Self {
        Self::Protocol(e)
    }
}

/// Result of one sweep.
#[derive(Debug, Clone)]
pub struct Sweep {
    pub params: SweepParams,
    pub samples: Vec<Sample>,
}

/// A board reachable through a transport.
pub struct Board<T> {
    transport: T,
    /// The synthesizer is tuned: after the output was stopped (or before the
    /// first command) the first point of the next sweep reads wrong.
    tuned: bool,
}

/// Time the synthesizer gets to settle at the start frequency before a sweep.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(50);

impl<T: Read + Write> Board<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            tuned: false,
        }
    }

    /// Gives back the transport.
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Runs one sweep and waits for the complete response.
    pub fn sweep(&mut self, params: &SweepParams) -> Result<Sweep, BoardError> {
        self.sweep_with_progress(params, |_, _| {})
    }

    /// Runs one sweep. The board sends points while it measures, so
    /// `on_points(first_index, points)` is called with each group of newly
    /// arrived points, in order, before the sweep is complete.
    pub fn sweep_with_progress(
        &mut self,
        params: &SweepParams,
        mut on_points: impl FnMut(usize, &[Sample]),
    ) -> Result<Sweep, BoardError> {
        if !self.tuned {
            // Park the synthesizer at the start frequency first, as the first
            // sweep point is otherwise a transient.
            self.set_fixed_frequency(params.start_hz)?;
            std::thread::sleep(SETTLE);
        }
        self.transport
            .write_all(&protocol::encode_sweep(params, 0))?;
        self.transport.flush()?;

        let expected = params.response_len();
        let mut buf = vec![0u8; expected];
        let mut received = 0;
        let mut reported = 0; // complete points already passed to `on_points`
        while received < expected {
            match self.transport.read(&mut buf[received..]) {
                Ok(0) => break,
                Ok(n) => received += n,
                Err(e) if e.kind() == io::ErrorKind::TimedOut => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
            let complete = received / protocol::BYTES_PER_POINT;
            if complete > reported {
                let from = reported * protocol::BYTES_PER_POINT;
                let to = complete * protocol::BYTES_PER_POINT;
                on_points(reported, &protocol::decode_samples(&buf[from..to]));
                reported = complete;
            }
        }
        if received < expected {
            return Err(BoardError::Timeout { expected, received });
        }
        let samples = protocol::decode_sweep(&buf, params)?;
        Ok(Sweep {
            params: *params,
            samples,
        })
    }

    /// Reads the detector once at the frequency set by `set_fixed_frequency`:
    /// returns the first channel's raw value.
    pub fn measure(&mut self) -> Result<u16, BoardError> {
        self.transport.write_all(&protocol::encode_measure())?;
        self.transport.flush()?;
        let mut buf = [0u8; protocol::BYTES_PER_POINT];
        let mut received = 0;
        while received < buf.len() {
            match self.transport.read(&mut buf[received..]) {
                Ok(0) => break,
                Ok(n) => received += n,
                Err(e) if e.kind() == io::ErrorKind::TimedOut => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
        if received < buf.len() {
            return Err(BoardError::Timeout {
                expected: buf.len(),
                received,
            });
        }
        Ok(protocol::decode_samples(&buf)[0].ch1)
    }

    /// Sets the board's frequency (0 Hz stops the output). The board sends no
    /// reply. The output carries a signal at this frequency (also with the S1
    /// button off; S1 only gates the tracking signal during sweeps), and
    /// `measure` reads it through a cable from RF OUT to RF IN.
    ///
    /// After the output was stopped the board ignores the first command that
    /// sets a frequency, so a frequency is sent twice.
    pub fn set_fixed_frequency(&mut self, freq_hz: u64) -> Result<(), BoardError> {
        let command = protocol::encode_fixed_frequency(freq_hz);
        let repeats = if freq_hz == 0 { 1 } else { 2 };
        for _ in 0..repeats {
            self.transport.write_all(&command)?;
        }
        self.transport.flush()?;
        self.tuned = freq_hz != 0;
        Ok(())
    }

    /// Runs a sweep as a series of short sweeps of `segment_points` points. The
    /// board ignores commands while it is sending a sweep, so short segments are
    /// what lets the caller change or stop the sweep quickly: `keep_going` is
    /// asked between segments and the sweep is dropped when it says `false`
    /// (`Ok(None)`). `on_points` gets the points of the whole sweep in order.
    pub fn sweep_segmented(
        &mut self,
        params: &SweepParams,
        segment_points: usize,
        mut on_points: impl FnMut(usize, &[Sample]),
        mut keep_going: impl FnMut() -> bool,
    ) -> Result<Option<Sweep>, BoardError> {
        let total = usize::from(params.points);
        let segment_points = segment_points.max(2);
        let mut samples = Vec::with_capacity(total);
        let mut first = 0;
        while first < total {
            let mut count = segment_points.min(total - first);
            // A segment needs at least two points: join a lone last point to this one.
            if total - first - count == 1 {
                count += 1;
            }
            let part = params.segment(first, count);
            let swept = self.sweep_with_progress(&part, |offset, points| {
                on_points(first + offset, points);
            })?;
            samples.extend(swept.samples);
            first += count;
            if first < total && !keep_going() {
                return Ok(None);
            }
        }
        Ok(Some(Sweep {
            params: *params,
            samples,
        }))
    }
}

/// Opens a real serial port with the board's line settings.
pub fn open_serial(path: &str) -> io::Result<Board<serial2::SerialPort>> {
    let mut port = serial2::SerialPort::open(path, protocol::BAUD_RATE)?;
    port.set_read_timeout(Duration::from_millis(500))?;
    port.set_write_timeout(Duration::from_millis(500))?;
    port.set_dtr(true)?;
    port.set_rts(true)?;
    std::thread::sleep(Duration::from_millis(300));
    port.discard_input_buffer()?;
    Ok(Board::new(port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::SimBoard;

    /// Hands out at most 7 bytes per read, to split points across reads.
    struct Trickle(SimBoard);
    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(7);
            self.0.read(&mut buf[..n])
        }
    }
    impl Write for Trickle {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    #[test]
    fn progress_reports_every_point_once_in_order() {
        let params = SweepParams::new(35_000_000, 1_000_000_000, 101).unwrap();
        let mut seen = Vec::new();
        let sweep = Board::new(Trickle(SimBoard::new()))
            .sweep_with_progress(&params, |first, points| {
                assert_eq!(first, seen.len());
                seen.extend_from_slice(points);
            })
            .unwrap();
        assert_eq!(seen, sweep.samples);
        assert_eq!(seen.len(), 101);
    }
}

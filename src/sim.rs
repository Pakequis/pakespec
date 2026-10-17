//! Software simulation of the board, used by tests and by `--simulate`.
//!
//! It implements `Read` and `Write` like a serial port. Commands are parsed
//! from the bytes written to it, and the sweep response is produced from a
//! synthetic spectrum. A sweep command without the trailing delay field stays
//! incomplete, which reproduces the behaviour seen on the real board.

use std::collections::VecDeque;
use std::io::{self, Read, Write};

use crate::protocol::{COMMAND_PREFIX, FREQ_UNIT_HZ, SWEEP_FIELDS_LEN, SweepParams};

#[derive(Debug, Default)]
pub struct SimBoard {
    incoming: Vec<u8>,
    outgoing: VecDeque<u8>,
    /// Real frequency set by the last fixed-frequency command, in Hz.
    tuned_hz: u64,
    /// After `f000000000` the board ignores the first following `f` with a frequency.
    ignore_next_f: bool,
}

impl SimBoard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Raw ADC-scale value of the synthetic spectrum at `freq_hz`.
    fn level(freq_hz: u64) -> u16 {
        let f = freq_hz as f64 / 1e6;
        let mut v = 150.0 + ((freq_hz / 7919) % 5) as f64;
        for (centre, width, height) in [
            (433.9, 3.0, 20.0),
            (940.0, 8.0, 160.0),
            (2437.0, 12.0, 120.0),
        ] {
            let d = (f - centre) / width;
            v = v.max(150.0 + height * (-d * d).exp());
        }
        v as u16
    }

    fn handle(&mut self) {
        loop {
            // Drop noise before the next command prefix.
            match self.incoming.iter().position(|&b| b == COMMAND_PREFIX) {
                Some(0) => {}
                Some(n) => {
                    self.incoming.drain(..n);
                }
                None => {
                    self.incoming.clear();
                    return;
                }
            }
            let Some(&kind) = self.incoming.get(1) else {
                return;
            };
            match kind {
                b'x' => {
                    if self.incoming.len() < 2 + SWEEP_FIELDS_LEN {
                        return;
                    }
                    let fields: Vec<u8> = self
                        .incoming
                        .drain(..2 + SWEEP_FIELDS_LEN)
                        .skip(2)
                        .collect();
                    self.sweep(&fields);
                }
                b'f' => {
                    if self.incoming.len() < 11 {
                        return;
                    }
                    let digits: Vec<u8> = self.incoming.drain(..11).skip(2).collect();
                    let units: u64 = std::str::from_utf8(&digits)
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    if units == 0 {
                        self.tuned_hz = 0;
                        self.ignore_next_f = true;
                    } else if self.ignore_next_f {
                        self.ignore_next_f = false;
                    } else {
                        self.tuned_hz = units * FREQ_UNIT_HZ;
                    }
                }
                b'm' => {
                    self.incoming.drain(..2);
                    let v = Self::level(self.tuned_hz).to_le_bytes();
                    self.outgoing.extend([v[0], v[1], v[0], v[1]]);
                }
                b'v' => {
                    self.incoming.drain(..2);
                    self.outgoing.push_back(0x77);
                }
                _ => {
                    self.incoming.drain(..2);
                }
            }
        }
    }

    fn sweep(&mut self, fields: &[u8]) {
        let num = |range: std::ops::Range<usize>| -> Option<u64> {
            std::str::from_utf8(&fields[range]).ok()?.parse().ok()
        };
        let (Some(start), Some(step), Some(points)) = (num(0..9), num(9..17), num(17..21)) else {
            return;
        };
        for i in 0..points {
            let v = Self::level((start + step * i) * FREQ_UNIT_HZ).to_le_bytes();
            self.outgoing.extend([v[0], v[1], v[0], v[1]]);
        }
    }

    /// Level the simulator reports at `freq_hz`.
    pub fn level_at(freq_hz: u64) -> u16 {
        Self::level(freq_hz)
    }

    /// Frequency-to-level helper for tests that want to predict the response.
    pub fn expected_level(params: &SweepParams, index: usize) -> u16 {
        Self::level(params.freq_at(index))
    }
}

impl Write for SimBoard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.incoming.extend_from_slice(buf);
        self.handle();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for SimBoard {
    /// Returns the pending bytes, or a timeout error when there are none,
    /// like a serial port with a read timeout.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.outgoing.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "no data from board",
            ));
        }
        let n = buf.len().min(self.outgoing.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.outgoing.pop_front().unwrap_or(0);
        }
        Ok(n)
    }
}

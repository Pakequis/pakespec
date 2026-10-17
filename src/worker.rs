//! Background thread that owns the board and sweeps continuously, so the
//! interface never waits for the serial port.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::board::{Board, Sweep, open_serial};
use crate::protocol::{Sample, SweepParams};
use crate::sim::SimBoard;

/// Any byte transport that can move to the worker thread.
pub trait Transport: Read + Write + Send {}
impl<T: Read + Write + Send> Transport for T {}

/// Where the sweeps come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Serial(String),
    Simulator,
}

#[derive(Debug)]
enum Command {
    Start(SweepParams),
    /// Turns the generator on at this real frequency, or off with `None`.
    Generator(Option<u64>),
    Stop,
    Quit,
}

#[derive(Debug)]
pub enum Event {
    /// Points of a sweep in progress, in order: `samples` start at point `offset`.
    Progress {
        params: SweepParams,
        offset: usize,
        samples: Vec<Sample>,
    },
    /// A sweep finished (all of its points were already reported as `Progress`).
    Sweep(Sweep),
    /// A detector reading taken at the generator frequency (raw value of channel 1).
    Reading(u16),
    /// The worker stopped because of a problem; the text is for the user.
    Failed(String),
    /// Sweeping or the generator stopped because the user asked for it.
    Stopped,
}

pub struct Worker {
    commands: Sender<Command>,
    pub events: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts the worker. `wake` is called after every event (use it to
    /// request a redraw of the interface).
    pub fn spawn(source: Source, wake: impl Fn() + Send + 'static) -> Worker {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let handle = thread::spawn(move || run(source, command_rx, event_tx, wake));
        Worker {
            commands,
            events,
            handle: Some(handle),
        }
    }

    /// Sweeps again and again with these parameters until `stop`.
    pub fn start(&self, params: SweepParams) {
        let _ = self.commands.send(Command::Start(params));
    }

    /// Turns the generator on at `freq_hz` (or changes its frequency) and reports
    /// detector readings at that frequency until `stop` or a sweep starts. The
    /// output is on whatever the S1 button says.
    pub fn generator_on(&self, freq_hz: u64) {
        let _ = self.commands.send(Command::Generator(Some(freq_hz)));
    }

    pub fn generator_off(&self) {
        let _ = self.commands.send(Command::Generator(None));
    }

    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Longest wait before newly arrived points are handed to the interface.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(30);
/// Points per segment of a sweep: about 0.13 s on the board, which is how long a
/// change of settings can wait.
const SEGMENT_POINTS: usize = 100;
/// Time between two generator readings.
const READING_INTERVAL: Duration = Duration::from_millis(50);

enum Mode {
    Idle,
    Sweeping(SweepParams),
    /// Generator running (its frequency lives in the board).
    Generating,
}

fn run(source: Source, commands: Receiver<Command>, events: Sender<Event>, wake: impl Fn()) {
    let send = |event: Event| {
        let _ = events.send(event);
        wake();
    };
    let mut board: Option<Board<Box<dyn Transport>>> = None;
    let mut mode = Mode::Idle;
    // A command that arrived while a sweep was running.
    let mut stash: Option<Command> = None;
    let pause = if source == Source::Simulator {
        Duration::from_millis(150)
    } else {
        Duration::ZERO
    };

    loop {
        let command = match stash.take() {
            Some(c) => Some(c),
            None if matches!(mode, Mode::Idle) => match commands.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            },
            None => match commands.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            },
        };

        match command {
            Some(Command::Quit) => return,
            Some(Command::Stop) => {
                if let (Mode::Generating, Some(b)) = (&mode, board.as_mut()) {
                    let _ = b.set_fixed_frequency(0);
                }
                mode = Mode::Idle;
                send(Event::Stopped);
            }
            Some(Command::Generator(None)) => {
                if let Some(b) = board.as_mut() {
                    let _ = b.set_fixed_frequency(0);
                }
                mode = Mode::Idle;
                send(Event::Stopped);
            }
            // Opening the board has side effects, so it stays out of a match guard.
            #[allow(clippy::collapsible_match)]
            Some(Command::Start(params)) => {
                if ensure_open(&mut board, &source, &send) {
                    mode = Mode::Sweeping(params);
                }
            }
            Some(Command::Generator(Some(freq_hz))) => {
                let opened = ensure_open(&mut board, &source, &send);
                if opened {
                    match board.as_mut().map(|b| b.set_fixed_frequency(freq_hz)) {
                        Some(Err(e)) => {
                            board = None;
                            mode = Mode::Idle;
                            send(Event::Failed(e.to_string()));
                        }
                        _ => mode = Mode::Generating,
                    }
                }
            }
            None => {}
        }

        match (&mode, board.as_mut()) {
            (Mode::Sweeping(params), Some(b)) => {
                let params = *params;
                let mut pending: Vec<Sample> = Vec::new();
                let mut pending_offset = 0;
                let mut last_sent = Instant::now();
                let result = b.sweep_segmented(
                    &params,
                    SEGMENT_POINTS,
                    |offset, points| {
                        if pending.is_empty() {
                            pending_offset = offset;
                        }
                        pending.extend_from_slice(points);
                        if last_sent.elapsed() >= PROGRESS_INTERVAL {
                            send(Event::Progress {
                                params,
                                offset: pending_offset,
                                samples: std::mem::take(&mut pending),
                            });
                            last_sent = Instant::now();
                        }
                    },
                    || match commands.try_recv() {
                        Ok(c) => {
                            stash = Some(c);
                            false
                        }
                        Err(TryRecvError::Empty) => true,
                        Err(TryRecvError::Disconnected) => false,
                    },
                );
                if !pending.is_empty() {
                    send(Event::Progress {
                        params,
                        offset: pending_offset,
                        samples: pending,
                    });
                }
                match result {
                    Ok(Some(sweep)) => send(Event::Sweep(sweep)),
                    Ok(None) => {}
                    Err(e) => {
                        mode = Mode::Idle;
                        board = None;
                        send(Event::Failed(e.to_string()));
                    }
                }
                thread::sleep(pause);
            }
            (Mode::Generating, Some(b)) => {
                match b.measure() {
                    Ok(v) => send(Event::Reading(v)),
                    Err(e) => {
                        mode = Mode::Idle;
                        board = None;
                        send(Event::Failed(e.to_string()));
                    }
                }
                thread::sleep(READING_INTERVAL);
            }
            _ => {}
        }
    }
}

/// Opens the board if needed; reports a failure and returns `false` when it cannot.
fn ensure_open(
    board: &mut Option<Board<Box<dyn Transport>>>,
    source: &Source,
    send: &impl Fn(Event),
) -> bool {
    if board.is_some() {
        return true;
    }
    match open(source) {
        Ok(b) => {
            *board = Some(b);
            true
        }
        Err(message) => {
            send(Event::Failed(message));
            false
        }
    }
}

fn open(source: &Source) -> Result<Board<Box<dyn Transport>>, String> {
    match source {
        Source::Simulator => Ok(Board::new(Box::new(SimBoard::new()) as Box<dyn Transport>)),
        Source::Serial(path) => open_serial(path)
            .map(|b| Board::new(Box::new(b.into_inner()) as Box<dyn Transport>))
            .map_err(|e| format!("{path}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(worker: &Worker) -> Event {
        worker
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("event in time")
    }

    #[test]
    fn sweeps_until_stopped() {
        let worker = Worker::spawn(Source::Simulator, || {});
        worker.start(SweepParams::new(35_000_000, 45_000_000, 11).unwrap());
        let mut sweeps = 0;
        let mut points = 0;
        while sweeps < 3 {
            match next(&worker) {
                Event::Progress {
                    offset, samples, ..
                } => {
                    assert_eq!(offset, points % 11);
                    points += samples.len();
                }
                Event::Sweep(s) => {
                    assert_eq!(s.samples.len(), 11);
                    sweeps += 1;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(points, 33);
        worker.stop();
        // Sweeps already queued may arrive before the stop notice.
        while matches!(next(&worker), Event::Progress { .. } | Event::Sweep(_)) {}
    }

    #[test]
    fn generator_reports_readings_until_stopped() {
        let worker = Worker::spawn(Source::Simulator, || {});
        worker.generator_on(940_000_000);
        let mut readings = Vec::new();
        while readings.len() < 3 {
            match next(&worker) {
                Event::Reading(v) => readings.push(v),
                other => panic!("unexpected {other:?}"),
            }
        }
        // The simulator has a strong signal at 940 MHz.
        assert!(readings.iter().all(|&v| v > 250), "{readings:?}");
        worker.generator_off();
        while !matches!(next(&worker), Event::Stopped) {}
    }

    #[test]
    fn a_new_command_interrupts_a_sweep_within_a_segment() {
        let worker = Worker::spawn(Source::Simulator, || {});
        worker.start(SweepParams::new(35_000_000, 4_400_000_000, 1001).unwrap());
        // Wait for the first points, then change to a small sweep.
        while !matches!(next(&worker), Event::Progress { .. }) {}
        let small = SweepParams::new(35_000_000, 45_000_000, 11).unwrap();
        worker.start(small);
        loop {
            if let Event::Progress { params, .. } = next(&worker)
                && params == small
            {
                break;
            }
        }
    }

    #[test]
    fn missing_port_is_reported() {
        let worker = Worker::spawn(Source::Serial("/nonexistent/port".into()), || {});
        worker.start(SweepParams::new(35_000_000, 45_000_000, 11).unwrap());
        assert!(matches!(next(&worker), Event::Failed(_)));
    }
}

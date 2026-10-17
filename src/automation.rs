//! Scriptable access to the board for other programs.
//!
//! A `Session` turns JSON requests (`{"cmd": "sweep", "start": ...}`) into board
//! operations and JSON replies (`{"ok": true, ...}` or `{"ok": false, "error":
//! {"code": ..., "message": ...}}`). The command-line tool (`--serve`, one JSON
//! document per line) and the MCP server both sit on top of it. The board is
//! opened on the first command that needs it and reopened after a board error.

use std::io::{self, BufRead, Read, Write};
use std::time::Duration;

use serde_json::{Value, json};

use crate::board::{Board, BoardError, open_serial};
use crate::calibration::Calibration;
use crate::ports::{list_ports, suggest};
use crate::protocol::{MAX_FREQ_HZ, MIN_FREQ_HZ, SweepParams};
use crate::sim::SimBoard;

/// Time the synthesizer gets to settle before a single detector reading.
const MEASURE_SETTLE: Duration = Duration::from_millis(50);

/// Any byte transport a board can sit behind.
pub trait Transport: Read + Write + Send {}
impl<T: Read + Write + Send> Transport for T {}

/// Where the board comes from.
#[derive(Debug, Clone)]
pub enum Source {
    Simulator,
    /// A serial port path, or the single board candidate found when `None`.
    Port(Option<String>),
}

/// A failed command: a stable machine-readable code and a human message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmdError {
    pub code: &'static str,
    pub message: String,
}

impl CmdError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Process exit code for this error: 2 for a bad request, 3 when the board
    /// cannot be found or opened, 4 when it fails while in use.
    pub fn exit_code(&self) -> u8 {
        match self.code {
            "invalid_request" | "invalid_parameter" | "not_allowed" | "unknown_command" => 2,
            "no_board" | "open_failed" => 3,
            _ => 4,
        }
    }
}

pub struct Session {
    source: Source,
    board: Option<Board<Box<dyn Transport>>>,
    calibration: Option<Calibration>,
    allow_generator: bool,
}

impl Session {
    pub fn new(source: Source, calibration: Option<Calibration>, allow_generator: bool) -> Self {
        Self {
            source,
            board: None,
            calibration,
            allow_generator,
        }
    }

    pub fn generator_allowed(&self) -> bool {
        self.allow_generator
    }

    /// Runs one request and returns the reply document. An `id` in the request
    /// is copied to the reply.
    pub fn execute(&mut self, request: &Value) -> Value {
        self.execute_with_code(request).0
    }

    /// Like `execute`, but also returns the process exit code (0 on success).
    pub fn execute_with_code(&mut self, request: &Value) -> (Value, u8) {
        let (mut reply, code) = match self.dispatch(request) {
            Ok(mut v) => {
                v["ok"] = json!(true);
                (v, 0)
            }
            Err(e) => (
                json!({"ok": false, "error": {"code": e.code, "message": e.message}}),
                e.exit_code(),
            ),
        };
        if let Some(id) = request.get("id") {
            reply["id"] = id.clone();
        }
        (reply, code)
    }

    fn dispatch(&mut self, request: &Value) -> Result<Value, CmdError> {
        let cmd = request
            .get("cmd")
            .and_then(Value::as_str)
            .ok_or_else(|| CmdError::new("invalid_request", "missing string field \"cmd\""))?;
        match cmd {
            "list_ports" => Ok(list_ports_json()),
            "status" => Ok(self.status()),
            "sweep" => self.sweep(request),
            "measure" => self.measure(request),
            "generator" => self.generator(request),
            "generator_off" => self.generator_off(),
            other => Err(CmdError::new(
                "unknown_command",
                format!("unknown command {other:?}"),
            )),
        }
    }

    fn status(&self) -> Value {
        json!({
            "source": match &self.source {
                Source::Simulator => "simulator".to_string(),
                Source::Port(Some(p)) => p.clone(),
                Source::Port(None) => "auto".to_string(),
            },
            "board_open": self.board.is_some(),
            "generator_allowed": self.allow_generator,
            "min_freq_hz": MIN_FREQ_HZ,
            "max_freq_hz": MAX_FREQ_HZ,
            "calibration": self.calibration.as_ref().map(|c| json!({
                "counts_per_db": c.counts_per_db,
                "raw_at_0_dbm": c.raw_at_0_dbm,
                "min_freq_hz": c.min_freq_hz,
                "max_freq_hz": c.max_freq_hz,
            })),
        })
    }

    fn sweep(&mut self, request: &Value) -> Result<Value, CmdError> {
        let start = field_u64(request, "start")?.unwrap_or(MIN_FREQ_HZ);
        let stop = field_u64(request, "stop")?.unwrap_or(1_000_000_000);
        let points = match field_u64(request, "points")? {
            Some(p) => u16::try_from(p)
                .map_err(|_| CmdError::new("invalid_parameter", "points is out of range"))?,
            None => 101,
        };
        let params = SweepParams::new(start, stop, points)
            .map_err(|e| CmdError::new("invalid_parameter", e.to_string()))?;
        let result = self.board()?.sweep(&params);
        let sweep = result.map_err(|e| self.board_error(e))?;

        let freqs: Vec<u64> = (0..sweep.samples.len())
            .map(|i| params.freq_at(i))
            .collect();
        let raw: Vec<u16> = sweep.samples.iter().map(|s| s.ch1).collect();
        let dbm: Vec<Value> = freqs
            .iter()
            .zip(&raw)
            .map(|(&f, &r)| self.dbm(f64::from(r), f))
            .collect();
        let peak = (0..raw.len())
            .max_by_key(|&i| raw[i])
            .map(|i| json!({"freq_hz": freqs[i], "raw": raw[i], "dbm": dbm[i]}));
        Ok(json!({
            "start_hz": params.start_hz,
            "stop_hz": params.freq_at(raw.len() - 1),
            "points": raw.len(),
            "step_hz": params.step_hz(),
            "calibrated": self.calibration.is_some(),
            "peak": peak,
            "freq_hz": freqs,
            "raw": raw,
            "dbm": dbm,
        }))
    }

    /// One detector reading at a fixed frequency; the output is stopped afterwards.
    /// Needs the generator permission because the output is on during the reading.
    fn measure(&mut self, request: &Value) -> Result<Value, CmdError> {
        // A reading puts a tone on RF OUT, so it needs the same permission as the generator.
        self.require_generator()?;
        let freq = required_freq(request)?;
        let board = self.board()?;
        let result = (|| {
            board.set_fixed_frequency(freq)?;
            std::thread::sleep(MEASURE_SETTLE);
            let raw = board.measure();
            board.set_fixed_frequency(0)?;
            raw
        })();
        let raw = result.map_err(|e| self.board_error(e))?;
        Ok(json!({"freq_hz": freq, "raw": raw, "dbm": self.dbm(f64::from(raw), freq)}))
    }

    /// Sets the tracking-source frequency. Measured on 2026-10-17: the output is
    /// on after this command even with the S1 button off (S1 only gates the
    /// tracking signal during sweeps).
    fn generator(&mut self, request: &Value) -> Result<Value, CmdError> {
        self.require_generator()?;
        let freq = required_freq(request)?;
        let result = self.board()?.set_fixed_frequency(freq);
        result.map_err(|e| self.board_error(e))?;
        Ok(json!({
            "freq_hz": freq,
            "note": "the fixed-frequency output can be on whatever the S1 button says; send generator_off to stop it",
        }))
    }

    fn generator_off(&mut self) -> Result<Value, CmdError> {
        self.require_generator()?;
        let result = self.board()?.set_fixed_frequency(0);
        result.map_err(|e| self.board_error(e))?;
        Ok(json!({"freq_hz": 0}))
    }

    fn require_generator(&self) -> Result<(), CmdError> {
        if self.allow_generator {
            Ok(())
        } else {
            Err(CmdError::new(
                "not_allowed",
                "generator control is disabled for this session",
            ))
        }
    }

    /// dBm for a reading, or null outside what the calibration was measured over.
    fn dbm(&self, raw: f64, freq_hz: u64) -> Value {
        match &self.calibration {
            Some(c) if c.is_trusted(raw, freq_hz) => {
                json!((c.raw_to_dbm(raw) * 10.0).round() / 10.0)
            }
            _ => Value::Null,
        }
    }

    /// Opens the board if needed.
    fn board(&mut self) -> Result<&mut Board<Box<dyn Transport>>, CmdError> {
        if self.board.is_none() {
            let transport: Box<dyn Transport> = match &self.source {
                Source::Simulator => Box::new(SimBoard::new()),
                Source::Port(port) => {
                    let path = match port {
                        Some(p) => p.clone(),
                        None => {
                            let ports = list_ports();
                            suggest(&ports).map(|p| p.path.clone()).ok_or_else(|| {
                                CmdError::new(
                                    "no_board",
                                    "no single board candidate found; list the ports and choose one",
                                )
                            })?
                        }
                    };
                    let board = open_serial(&path).map_err(|e| {
                        CmdError::new("open_failed", format!("cannot open {path}: {e}"))
                    })?;
                    Box::new(board.into_inner())
                }
            };
            self.board = Some(Board::new(transport));
        }
        Ok(self.board.as_mut().expect("board was just opened"))
    }

    /// Drops the connection (it is reopened by the next command) and maps the error.
    fn board_error(&mut self, e: BoardError) -> CmdError {
        self.board = None;
        let code = match e {
            BoardError::Timeout { .. } => "board_timeout",
            BoardError::Io(_) => "board_io",
            BoardError::Protocol(_) => "board_protocol",
        };
        CmdError::new(code, e.to_string())
    }
}

fn field_u64(request: &Value, name: &str) -> Result<Option<u64>, CmdError> {
    match request.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| {
            CmdError::new(
                "invalid_request",
                format!("{name} must be a non-negative integer"),
            )
        }),
    }
}

/// The `freq_hz` field, or 0 (stop) is not accepted here: it must be inside the band.
fn required_freq(request: &Value) -> Result<u64, CmdError> {
    let freq = field_u64(request, "freq_hz")?
        .ok_or_else(|| CmdError::new("invalid_request", "missing field \"freq_hz\""))?;
    if (MIN_FREQ_HZ..=MAX_FREQ_HZ).contains(&freq) {
        Ok(freq)
    } else {
        Err(CmdError::new(
            "invalid_parameter",
            format!("freq_hz must be between {MIN_FREQ_HZ} and {MAX_FREQ_HZ}"),
        ))
    }
}

/// The serial ports of the machine; `suggested` marks the one chosen when none is given.
pub fn list_ports_json() -> Value {
    let ports = list_ports();
    let pick = suggest(&ports).map(|p| p.path.clone());
    let list: Vec<Value> = ports
        .iter()
        .map(|p| {
            json!({
                "path": p.path,
                "vid": p.vid.map(|v| format!("{v:04x}")),
                "pid": p.pid.map(|v| format!("{v:04x}")),
                "product": p.product,
                "board_candidate": p.is_board_candidate(),
                "suggested": pick.as_deref() == Some(p.path.as_str()),
            })
        })
        .collect();
    json!({"ports": list})
}

/// Reads one JSON request per line and writes one JSON reply per line, until
/// the input ends or a `{"cmd": "quit"}` request arrives.
pub fn serve_lines(
    session: &mut Session,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(request) if request.get("cmd").and_then(Value::as_str) == Some("quit") => {
                return Ok(());
            }
            Ok(request) => session.execute(&request),
            Err(e) => {
                json!({"ok": false, "error": {"code": "invalid_request", "message": e.to_string()}})
            }
        };
        writeln!(output, "{reply}")?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim(allow_generator: bool) -> Session {
        Session::new(Source::Simulator, None, allow_generator)
    }

    #[test]
    fn sweep_reply_has_matching_arrays_and_a_peak() {
        let r = sim(true).execute(&json!({"cmd": "sweep", "start": 35_000_000u64, "stop": 1_000_000_000u64, "points": 101, "id": 7}));
        assert_eq!(r["ok"], true);
        assert_eq!(r["id"], 7);
        assert_eq!(r["raw"].as_array().unwrap().len(), 101);
        assert_eq!(r["freq_hz"].as_array().unwrap().len(), 101);
        let peak = r["peak"]["freq_hz"].as_u64().unwrap();
        assert!(
            (900_000_000..=980_000_000).contains(&peak),
            "peak at {peak}"
        );
        assert!(r["dbm"][0].is_null());
    }

    #[test]
    fn measure_reads_the_level_at_the_frequency() {
        let r = sim(true).execute(&json!({"cmd": "measure", "freq_hz": 940_000_000u64}));
        assert_eq!(r["ok"], true);
        assert_eq!(
            r["raw"].as_u64().unwrap(),
            u64::from(SimBoard::level_at(940_000_000))
        );
    }

    #[test]
    fn bad_requests_get_stable_error_codes() {
        let mut s = sim(true);
        for (request, code, exit) in [
            (json!({}), "invalid_request", 2),
            (json!({"cmd": "dance"}), "unknown_command", 2),
            (json!({"cmd": "sweep", "start": "x"}), "invalid_request", 2),
            (
                json!({"cmd": "sweep", "start": 1u64}),
                "invalid_parameter",
                2,
            ),
            (json!({"cmd": "measure"}), "invalid_request", 2),
            (
                json!({"cmd": "generator", "freq_hz": 5u64}),
                "invalid_parameter",
                2,
            ),
        ] {
            let (r, c) = s.execute_with_code(&request);
            assert_eq!(r["error"]["code"], code, "{request}");
            assert_eq!(c, exit);
        }
    }

    #[test]
    fn measure_needs_the_generator_permission() {
        let r = sim(false).execute(&json!({"cmd": "measure", "freq_hz": 940_000_000u64}));
        assert_eq!(r["error"]["code"], "not_allowed");
    }

    #[test]
    fn generator_commands_obey_the_permission() {
        let off = sim(false).execute(&json!({"cmd": "generator", "freq_hz": 100_000_000u64}));
        assert_eq!(off["error"]["code"], "not_allowed");
        let on = sim(true).execute(&json!({"cmd": "generator", "freq_hz": 100_000_000u64}));
        assert_eq!(on["ok"], true);
        assert_eq!(
            sim(true).execute(&json!({"cmd": "generator_off"}))["ok"],
            true
        );
    }

    #[test]
    fn dbm_is_filled_inside_the_calibrated_range_only() {
        let cal = Calibration {
            counts_per_db: 8.55,
            raw_at_0_dbm: 443.3,
            min_raw: 165.0,
            max_raw: 425.0,
            min_freq_hz: 35_000_000,
            max_freq_hz: 940_000_000,
        };
        let mut s = Session::new(Source::Simulator, Some(cal), true);
        let r = s.execute(&json!({"cmd": "measure", "freq_hz": 940_000_000u64}));
        assert!(r["dbm"].is_number(), "{r}");
        let r = s.execute(&json!({"cmd": "measure", "freq_hz": 2_000_000_000u64}));
        assert!(r["dbm"].is_null());
    }

    #[test]
    fn serve_answers_one_line_per_request_and_stops_on_quit() {
        let input =
            b"{\"cmd\":\"status\",\"id\":1}\nnot json\n{\"cmd\":\"quit\"}\n{\"cmd\":\"status\"}\n";
        let mut out = Vec::new();
        serve_lines(&mut sim(false), &input[..], &mut out).unwrap();
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[1]["error"]["code"], "invalid_request");
    }
}

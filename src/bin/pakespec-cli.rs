//! Command-line tool: runs one sweep and prints CSV (`freq_hz,ch1,ch2`), or
//! JSON for other programs.
//!
//! Usage: pakespec-cli [--list-ports | --port PATH | --simulate] [--start HZ] [--stop HZ] [--points N]
//!                     [--calibration FILE] [--json]
//!        pakespec-cli --measure HZ | --generator HZ | --generator-off | --status
//!        pakespec-cli --serve
//!        pakespec-cli --fit-calibration LADDER.csv --min-freq HZ --max-freq HZ --output FILE
//!
//! Without `--port`, the only USB serial converter that matches the board is used.
//! A `dbm` column is added, from `--calibration` or the calibration file next to the
//! settings file, otherwise the standard calibration; it is empty where the reading is
//! outside what the calibration was measured over.
//! `--fit-calibration` reads `dbm,raw` lines and writes a calibration file.
//!
//! `--json`, `--measure`, `--generator`, `--generator-off`, `--status` and
//! `--list-ports --json` print one JSON document (`{"ok": true, ...}` or
//! `{"ok": false, "error": {"code", "message"}}`). `--serve` keeps the board open
//! and answers one JSON request per input line (see `src/automation.rs`).
//!
//! Exit codes: 0 success, 1 other failure, 2 bad command line or request,
//! 3 board not found or cannot be opened, 4 board failed while in use.

use std::process::ExitCode;

use pakespec::automation::{Session, Source, list_ports_json, serve_lines};
use pakespec::board::{Board, open_serial};
use pakespec::calibration::{Calibration, parse_ladder};
use pakespec::ports::{list_ports, suggest};
use pakespec::protocol::SweepParams;
use pakespec::sim::SimBoard;

/// A mistake in the command line, reported with exit code 2.
#[derive(Debug)]
struct Usage(String);

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Usage {}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(if e.is::<Usage>() { 2 } else { 1 })
        }
    }
}

fn number<T: std::str::FromStr>(name: &str, text: String) -> Result<T, Usage> {
    text.parse()
        .map_err(|_| Usage(format!("{name} needs a number, got {text:?}")))
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    let mut port: Option<String> = None;
    let mut list_only = false;
    let (mut start, mut stop, mut points) = (35_000_000u64, 1_000_000_000u64, 101u16);
    let mut simulate = false;
    let mut calibration: Option<String> = None;
    let mut fit: Option<String> = None;
    let mut output: Option<String> = None;
    let (mut min_freq, mut max_freq) = (None::<u64>, None::<u64>);
    let mut json = false;
    let mut serve = false;
    // A request for the session layer (--measure, --generator, ...).
    let mut request: Option<serde_json::Value> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| Usage(format!("{name} needs a value")))
        };
        match arg.as_str() {
            "--port" => port = Some(value("--port")?),
            "--list-ports" => list_only = true,
            "--start" => start = number("--start", value("--start")?)?,
            "--stop" => stop = number("--stop", value("--stop")?)?,
            "--points" => points = number("--points", value("--points")?)?,
            "--simulate" => simulate = true,
            "--calibration" => calibration = Some(value("--calibration")?),
            "--fit-calibration" => fit = Some(value("--fit-calibration")?),
            "--output" => output = Some(value("--output")?),
            "--min-freq" => min_freq = Some(number("--min-freq", value("--min-freq")?)?),
            "--max-freq" => max_freq = Some(number("--max-freq", value("--max-freq")?)?),
            "--json" => json = true,
            "--serve" => serve = true,
            "--status" => request = Some(serde_json::json!({"cmd": "status"})),
            "--generator-off" => request = Some(serde_json::json!({"cmd": "generator_off"})),
            "--measure" => {
                let freq_hz: u64 = number("--measure", value("--measure")?)?;
                request = Some(serde_json::json!({"cmd": "measure", "freq_hz": freq_hz}));
            }
            "--generator" => {
                let freq_hz: u64 = number("--generator", value("--generator")?)?;
                request = Some(serde_json::json!({"cmd": "generator", "freq_hz": freq_hz}));
            }
            other => return Err(Usage(format!("unknown argument {other}")).into()),
        }
    }

    if let Some(ladder) = fit {
        let out = output.ok_or_else(|| Usage("--fit-calibration needs --output".into()))?;
        let (lo, hi) = min_freq
            .zip(max_freq)
            .ok_or_else(|| Usage("--fit-calibration needs --min-freq and --max-freq".into()))?;
        let points = parse_ladder(&std::fs::read_to_string(&ladder)?);
        let cal = Calibration::fit(&points, lo, hi)?;
        cal.save(std::path::Path::new(&out))?;
        println!(
            "{:.3} counts/dB, {:.1} raw at 0 dBm, raw {:.1} to {:.1}, {} to {} Hz",
            cal.counts_per_db, cal.raw_at_0_dbm, cal.min_raw, cal.max_raw, lo, hi
        );
        return Ok(0);
    }

    let calibration = match calibration {
        Some(path) => Some(
            Calibration::load(std::path::Path::new(&path))
                .ok_or("cannot read the calibration file")?,
        ),
        None => Some(Calibration::load_or_standard(
            pakespec::calibration::default_path().as_deref(),
        )),
    };

    if serve || request.is_some() || (json && !list_only) {
        let source = if simulate {
            Source::Simulator
        } else {
            Source::Port(port)
        };
        let mut session = Session::new(source, calibration, true);
        if serve {
            serve_lines(
                &mut session,
                std::io::stdin().lock(),
                std::io::stdout().lock(),
            )?;
            return Ok(0);
        }
        let request = request.unwrap_or_else(
            || serde_json::json!({"cmd": "sweep", "start": start, "stop": stop, "points": points}),
        );
        let (reply, code) = session.execute_with_code(&request);
        println!("{reply}");
        return Ok(code);
    }

    if list_only && json {
        println!("{}", list_ports_json());
        return Ok(0);
    }

    if list_only {
        let ports = list_ports();
        let pick = suggest(&ports).map(|p| p.path.clone());
        for p in &ports {
            let id = match (p.vid, p.pid) {
                (Some(v), Some(d)) => format!("{v:04x}:{d:04x}"),
                _ => "----:----".to_string(),
            };
            let mark = if pick.as_deref() == Some(p.path.as_str()) {
                "  <- board"
            } else {
                ""
            };
            println!(
                "{}  {id}  {}{mark}",
                p.path,
                p.product.as_deref().unwrap_or("")
            );
        }
        return Ok(0);
    }

    let params = SweepParams::new(start, stop, points).map_err(|e| Usage(e.to_string()))?;
    let sweep = if simulate {
        Board::new(SimBoard::new()).sweep(&params)?
    } else {
        let port = match port {
            Some(p) => p,
            None => {
                let ports = list_ports();
                suggest(&ports)
                    .map(|p| p.path.clone())
                    .ok_or("no single board candidate found; use --list-ports and --port")?
            }
        };
        open_serial(&port)?.sweep(&params)?
    };

    if calibration.is_some() {
        println!("freq_hz,ch1,ch2,dbm");
    } else {
        println!("freq_hz,ch1,ch2");
    }
    for (i, s) in sweep.samples.iter().enumerate() {
        let freq = params.freq_at(i);
        match &calibration {
            Some(cal) => {
                let raw = f64::from(s.ch1);
                let dbm = if cal.is_trusted(raw, freq) {
                    format!("{:.1}", cal.raw_to_dbm(raw))
                } else {
                    String::new()
                };
                println!("{freq},{},{},{dbm}", s.ch1, s.ch2);
            }
            None => println!("{freq},{},{}", s.ch1, s.ch2),
        }
    }
    Ok(0)
}

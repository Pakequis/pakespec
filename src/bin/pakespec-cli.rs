//! Command-line tool: runs one sweep and prints CSV (`freq_hz,ch1,ch2`).
//!
//! Usage: pakespec-cli [--list-ports | --port PATH | --simulate] [--start HZ] [--stop HZ] [--points N]
//!                     [--calibration FILE]
//!        pakespec-cli --fit-calibration LADDER.csv --min-freq HZ --max-freq HZ --output FILE
//!
//! Without `--port`, the only USB serial converter that matches the board is used.
//! With `--calibration`, a `dbm` column is added; it is empty where the reading is
//! outside what the calibration was measured over.
//! `--fit-calibration` reads `dbm,raw` lines and writes a calibration file.

use std::process::ExitCode;

use pakespec::board::{Board, open_serial};
use pakespec::calibration::{Calibration, parse_ladder};
use pakespec::ports::{list_ports, suggest};
use pakespec::protocol::SweepParams;
use pakespec::sim::SimBoard;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut port: Option<String> = None;
    let mut list_only = false;
    let (mut start, mut stop, mut points) = (35_000_000u64, 1_000_000_000u64, 101u16);
    let mut simulate = false;
    let mut calibration: Option<String> = None;
    let mut fit: Option<String> = None;
    let mut output: Option<String> = None;
    let (mut min_freq, mut max_freq) = (None::<u64>, None::<u64>);

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--port" => port = Some(value("--port")?),
            "--list-ports" => list_only = true,
            "--start" => start = value("--start")?.parse()?,
            "--stop" => stop = value("--stop")?.parse()?,
            "--points" => points = value("--points")?.parse()?,
            "--simulate" => simulate = true,
            "--calibration" => calibration = Some(value("--calibration")?),
            "--fit-calibration" => fit = Some(value("--fit-calibration")?),
            "--output" => output = Some(value("--output")?),
            "--min-freq" => min_freq = Some(value("--min-freq")?.parse()?),
            "--max-freq" => max_freq = Some(value("--max-freq")?.parse()?),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }

    if let Some(ladder) = fit {
        let out = output.ok_or("--fit-calibration needs --output")?;
        let (lo, hi) = min_freq
            .zip(max_freq)
            .ok_or("--fit-calibration needs --min-freq and --max-freq")?;
        let points = parse_ladder(&std::fs::read_to_string(&ladder)?);
        let cal = Calibration::fit(&points, lo, hi)?;
        cal.save(std::path::Path::new(&out))?;
        println!(
            "{:.3} counts/dB, {:.1} raw at 0 dBm, raw {:.1} to {:.1}, {} to {} Hz",
            cal.counts_per_db, cal.raw_at_0_dbm, cal.min_raw, cal.max_raw, lo, hi
        );
        return Ok(());
    }

    let calibration = match calibration {
        Some(path) => Some(
            Calibration::load(std::path::Path::new(&path))
                .ok_or("cannot read the calibration file")?,
        ),
        None => None,
    };

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
        return Ok(());
    }

    let params = SweepParams::new(start, stop, points)?;
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
    Ok(())
}

//! Command-line tool: runs one sweep and prints CSV (`freq_hz,ch1,ch2`).
//!
//! Usage: pakespec-cli [--list-ports | --port PATH | --simulate] [--start HZ] [--stop HZ] [--points N]
//!
//! Without `--port`, the only USB serial converter that matches the board is used.

use std::process::ExitCode;

use pakespec::board::{Board, open_serial};
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
            other => return Err(format!("unknown argument {other}").into()),
        }
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

    println!("freq_hz,ch1,ch2");
    for (i, s) in sweep.samples.iter().enumerate() {
        println!("{},{},{}", params.freq_at(i), s.ch1, s.ch2);
    }
    Ok(())
}

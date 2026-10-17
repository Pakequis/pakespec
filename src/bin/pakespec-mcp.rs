//! MCP server: lets a language model drive the board through tools.
//!
//! Usage: pakespec-mcp [--port PATH | --simulate] [--calibration FILE] [--no-generator]
//!
//! Talks JSON-RPC on standard input and output. Without `--calibration`, the
//! calibration file next to the settings file is used when it exists.
//! `--no-generator` hides the tools that set the tracking-source frequency.

use std::io::{stdin, stdout};
use std::process::ExitCode;

use pakespec::automation::{Session, Source};
use pakespec::calibration::{Calibration, default_path};

fn main() -> ExitCode {
    let mut source = Source::Port(None);
    let mut calibration_path = default_path();
    let mut allow_generator = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => match args.next() {
                Some(p) => source = Source::Port(Some(p)),
                None => return usage("--port needs a value"),
            },
            "--simulate" => source = Source::Simulator,
            "--calibration" => match args.next() {
                Some(p) => calibration_path = Some(p.into()),
                None => return usage("--calibration needs a value"),
            },
            "--no-generator" => allow_generator = false,
            other => return usage(&format!("unknown argument {other}")),
        }
    }
    let calibration = calibration_path.and_then(|p| Calibration::load(&p));
    let mut session = Session::new(source, calibration, allow_generator);
    match pakespec::mcp::serve(&mut session, stdin().lock(), stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::from(2)
}

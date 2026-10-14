mod classify;
mod cli;
mod git;
mod model;
mod output;
mod scan;

use crate::{
    cli::{Cli, Command, Format},
    model::ScanOptions,
};
use clap::Parser;
use std::{
    io::{self, Write},
    process::ExitCode,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let (command, args) = match cli.command {
        Command::Scan(args) => ("scan", args),
        Command::Stats(args) => ("stats", args),
        Command::Tree(args) => ("tree", args),
    };
    let scan = scan::scan_repository(&ScanOptions {
        root: args.path,
        count_lines: !args.no_lines,
        max_line_count_file_bytes: args.max_line_file_size,
    })?;
    let rendered = match args.format {
        Format::Human => output::human(command, &scan),
        Format::Json => output::json(command, &scan)?,
    };
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{rendered}") {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => Ok(result?),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ctx: {error}");
            ExitCode::FAILURE
        }
    }
}

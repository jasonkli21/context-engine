mod classify;
mod cli;
mod git;
mod model;
mod output;
mod scan;

use crate::{
    cli::{Cli, Command, Format, PathArgs, TreeArgs},
    model::ScanOptions,
};
use clap::Parser;
use std::{
    io::{self, Write},
    process::ExitCode,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let (command, options, format) = match cli.command {
        Command::Scan(args) => {
            let format = args.format;
            ("scan", scan_options(args), format)
        }
        Command::Stats(args) => {
            let format = args.format;
            ("stats", scan_options(args), format)
        }
        Command::Tree(args) => {
            let format = args.format;
            ("tree", tree_options(args), format)
        }
    };
    let scan = scan::scan_repository(&options)?;
    let rendered = match format {
        Format::Human => output::human(command, &scan),
        Format::Json => output::json(command, &scan)?,
    };
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{rendered}") {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => Ok(result?),
    }
}

fn scan_options(args: PathArgs) -> ScanOptions {
    ScanOptions {
        root: args.path,
        count_lines: !args.no_lines,
        max_line_count_file_bytes: args.max_line_file_size,
        probe_git: true,
    }
}

fn tree_options(args: TreeArgs) -> ScanOptions {
    ScanOptions {
        root: args.path,
        count_lines: false,
        max_line_count_file_bytes: 0,
        probe_git: false,
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ctx: {}", output::escape_controls(&error.to_string()));
            ExitCode::FAILURE
        }
    }
}

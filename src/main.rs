mod classify;
mod cli;
mod git;
mod index;
mod language;
mod model;
mod output;
mod scan;
mod source;

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
    let rendered = match cli.command {
        Command::Index(args) => analyze("index", args, None)?,
        Command::Symbols(args) => analyze("symbols", args, None)?,
        Command::Symbol(args) => analyze("symbol", args.analysis, Some(&args.name))?,
        command => scan_command(command)?,
    };
    write_output(&rendered)
}

fn scan_command(command: Command) -> Result<String, Box<dyn std::error::Error>> {
    let (command, options, format) = match command {
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
        _ => unreachable!("analysis commands are handled before scan dispatch"),
    };
    let scan = scan::scan_repository(&options)?;
    Ok(match format {
        Format::Human => output::human(command, &scan),
        Format::Json => output::json(command, &scan)?,
    })
}

fn analyze(
    command: &str,
    args: cli::AnalysisArgs,
    query: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let index = index::index_repository(&model::IndexOptions {
        root: args.path,
        max_parse_file_bytes: args.max_parse_file_size,
        probe_git: true,
    })?;
    Ok(match args.format {
        Format::Human => output::index_human(command, &index, query),
        Format::Json => output::index_json(command, &index, query)?,
    })
}

fn write_output(rendered: &str) -> Result<(), Box<dyn std::error::Error>> {
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

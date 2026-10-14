use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::model::DEFAULT_MAX_LINE_BYTES;

#[derive(Debug, Parser)]
#[command(
    name = "ctx",
    version,
    about = "Local-first codebase and context intelligence engine"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan a repository and show a summary.
    Scan(PathArgs),
    /// Show repository statistics.
    Stats(PathArgs),
    /// Show the repository tree.
    Tree(TreeArgs),
}

#[derive(Debug, Args)]
pub struct PathArgs {
    /// Repository or directory to inspect.
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Output format (JSON schema version 1).
    #[arg(long, value_enum, default_value = "human")]
    pub format: Format,
    /// Skip physical text-line counting.
    #[arg(long)]
    pub no_lines: bool,
    /// Maximum bytes per file eligible for line counting.
    #[arg(long, default_value_t = DEFAULT_MAX_LINE_BYTES)]
    pub max_line_file_size: u64,
}

#[derive(Debug, Args)]
pub struct TreeArgs {
    /// Repository or directory to inspect.
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Output format (JSON schema version 1).
    #[arg(long, value_enum, default_value = "human")]
    pub format: Format,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Format {
    Human,
    Json,
}

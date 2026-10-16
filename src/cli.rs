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
    /// Search file paths and symbol names (case-sensitive metadata search).
    Search(SearchArgs),
    /// Explore local syntax evidence without crossing the repository containment hub.
    Neighborhood(NeighborhoodArgs),
    /// Rank structural activity and central symbols in the current graph.
    Hotspots(HotspotArgs),
    /// Build a fresh in-memory repository relationship graph (syntax evidence).
    Graph(AnalysisArgs),
    /// Traverse best-effort outgoing call-site references and unresolved facts.
    Deps(GraphQueryArgs),
    /// Find best-effort syntactic callers through resolved call-site references.
    Callers(GraphQueryArgs),
    /// Traverse lexical containment and reference neighbors in both directions.
    Related(GraphQueryArgs),
    /// Build a fresh in-memory syntax/symbol index (no persistence).
    Index(AnalysisArgs),
    /// List Rust and Python source symbols.
    Symbols(AnalysisArgs),
    /// Look up an exact symbol name.
    Symbol(SymbolArgs),
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

#[derive(Debug, Args)]
pub struct AnalysisArgs {
    /// Repository or directory to analyze.
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Output format (JSON schema version 1).
    #[arg(long, value_enum, default_value = "human")]
    pub format: Format,
    /// Maximum bytes per source file eligible for parsing.
    #[arg(long, default_value_t = crate::model::DEFAULT_MAX_PARSE_BYTES)]
    pub max_parse_file_size: u64,
}
#[derive(Debug, Args)]
pub struct SymbolArgs {
    /// Exact qualified name, falling back to exact local name.
    pub name: String,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

#[derive(Debug, Args)]
pub struct GraphQueryArgs {
    /// Exact qualified name, falling back to exact local name.
    pub name: String,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
    /// Traversal depth (0 through 8).
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=crate::graph::MAX_DEPTH as i64))]
    pub depth: u8,
    /// Narrow name matches to this scan-root-relative file path.
    #[arg(long)]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
    #[arg(long, value_enum, default_value = "all")]
    pub kind: SearchKindArg,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=200))]
    pub limit: u16,
}
#[derive(Debug, Args)]
pub struct HotspotArgs {
    #[command(flatten)]
    pub analysis: AnalysisArgs,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=200))]
    pub limit: u16,
}
#[derive(Debug, Args)]
pub struct NeighborhoodArgs {
    #[arg(long, required_unless_present = "file", conflicts_with = "file")]
    pub symbol: Option<String>,
    #[arg(long, required_unless_present = "symbol")]
    pub file: Option<PathBuf>,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=8))]
    pub depth: u8,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SearchKindArg {
    All,
    Symbol,
    File,
}
impl From<SearchKindArg> for crate::query::SearchKind {
    fn from(kind: SearchKindArg) -> Self {
        match kind {
            SearchKindArg::All => Self::All,
            SearchKindArg::Symbol => Self::Symbol,
            SearchKindArg::File => Self::File,
        }
    }
}

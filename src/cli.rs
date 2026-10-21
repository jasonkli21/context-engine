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
    /// Serve authenticated read-only HTTP on a loopback IP.
    Serve(ServeArgs),
    /// Serve read-only MCP 2026-07-28 over stdio (protocol stdout only).
    Mcp(McpArgs),
    /// Trace a committed HEAD symbol through bounded first-parent history.
    History(HistoryArgs),
    /// Summarize a historical path over bounded first-parent history.
    Evolution(EvolutionArgs),
    /// Assemble explainable full-file context under a UTF-8 content-byte budget.
    Context(ContextArgs),
    /// Compare structure in two committed revisions.
    Diff(ChangeArgs),
    /// Traverse prior/current structural neighbors of committed changes.
    Impact(ImpactArgs),
    /// Search file paths and symbol names (case-sensitive metadata search).
    Search(SearchArgs),
    /// Explore local syntax evidence without crossing the repository containment hub.
    Neighborhood(NeighborhoodArgs),
    /// Rank structural activity and central symbols in the current graph.
    Hotspots(HotspotArgs),
    /// Inspect the current repository relationship graph (validated analysis cache).
    Graph(AnalysisArgs),
    /// Traverse best-effort outgoing call-site references and unresolved facts.
    Deps(GraphQueryArgs),
    /// Find best-effort syntactic callers through resolved call-site references.
    Callers(GraphQueryArgs),
    /// Traverse lexical containment and reference neighbors in both directions.
    Related(GraphQueryArgs),
    /// Force fresh analysis and commit the external persistent index.
    Index(AnalysisArgs),
    /// Keep the external index current in the foreground (stderr status only).
    Watch(WatchArgs),
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

#[derive(Debug, Args)]
pub struct ChangeArgs {
    /// Explicit BASE..HEAD commit range (no triple-dot or working-tree mode).
    pub range: String,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}
#[derive(Debug, Args)]
pub struct ImpactArgs {
    #[command(flatten)]
    pub change: ChangeArgs,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=crate::graph::MAX_DEPTH as i64))]
    pub depth: u8,
}

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[arg(long)]
    pub task: String,
    /// Maximum raw UTF-8 content bytes (1 through 16777216).
    #[arg(long)]
    pub budget: u64,
    /// Optional committed range; requires a clean current HEAD matching its head.
    #[arg(long)]
    pub change: Option<String>,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

#[derive(Debug, Args)]
pub struct WatchArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Maximum bytes per source file eligible for parsing.
    #[arg(long, default_value_t = crate::model::DEFAULT_MAX_PARSE_BYTES)]
    pub max_parse_file_size: u64,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    pub name: String,
    #[command(flatten)]
    pub analysis: AnalysisArgs,
    #[arg(long)]
    pub file: Option<PathBuf>,
    #[arg(long, default_value_t = 128, value_parser = clap::value_parser!(u16).range(1..=512))]
    pub max_commits: u16,
}
#[derive(Debug, Args)]
pub struct EvolutionArgs {
    /// Historical scan-root-relative file/directory selector; need not exist.
    pub target: PathBuf,
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    #[arg(long, default_value_t = 128, value_parser = clap::value_parser!(u16).range(1..=512))]
    pub max_commits: u16,
    #[arg(long, default_value_t = crate::model::DEFAULT_MAX_PARSE_BYTES)]
    pub max_parse_file_size: u64,
    #[arg(long, value_enum, default_value = "human")]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long, default_value = "127.0.0.1:0")]
    pub listen: std::net::SocketAddr,
}
#[derive(Debug, Args)]
pub struct McpArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

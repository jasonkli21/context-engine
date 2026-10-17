mod change;
mod classify;
mod cli;
mod context;
mod git;
mod graph;
mod history;
mod index;
mod language;
mod model;
mod output;
mod paths;
mod persistence;
mod query;
mod relations;
mod scan;
mod source;
mod store;
mod watch;

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
        Command::History(args) => {
            let result = history::history(
                &args.analysis.path,
                &args.name,
                args.file.as_deref(),
                args.max_commits as usize,
                args.analysis.max_parse_file_size,
            )?;
            match args.analysis.format {
                Format::Human => output::history::human("history", &result)?,
                Format::Json => output::history::json("history", &result)?,
            }
        }
        Command::Evolution(args) => {
            let result = history::evolution(
                &args.repo,
                &args.target,
                args.max_commits as usize,
                args.max_parse_file_size,
            )?;
            match args.format {
                Format::Human => output::history::human("evolution", &result)?,
                Format::Json => output::history::json("evolution", &result)?,
            }
        }
        Command::Watch(args) => return watch_command(args),
        Command::Context(args) => context_command(args)?,
        Command::Diff(args) => change_command("diff", args, None)?,
        Command::Impact(args) => change_command("impact", args.change, Some(args.depth))?,
        Command::Search(args) => search_command(args)?,
        Command::Neighborhood(args) => neighborhood_command(args)?,
        Command::Hotspots(args) => hotspots_command(args)?,
        Command::Graph(args) => graph_command("graph", args, None)?,
        Command::Deps(args) => graph_query("deps", args)?,
        Command::Callers(args) => graph_query("callers", args)?,
        Command::Related(args) => graph_query("related", args)?,
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

fn current_analysis(
    options: &model::IndexOptions,
    force: bool,
) -> Result<persistence::AnalysisArtifact, persistence::CacheError> {
    let result = persistence::reconcile(options, force)?;
    for warning in result.warnings {
        eprintln!("ctx: warning: {}", output::escape_controls(&warning));
    }
    Ok(result.artifact)
}
fn current_graph(
    options: &model::IndexOptions,
) -> Result<graph::RepositoryGraph, persistence::CacheError> {
    let artifact = current_analysis(options, false)?;
    Ok(graph::from_analysis(
        artifact.index,
        artifact.relation_files,
    )?)
}

fn analyze(
    command: &str,
    args: cli::AnalysisArgs,
    query: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let index = current_analysis(
        &model::IndexOptions {
            root: args.path,
            max_parse_file_bytes: args.max_parse_file_size,
            probe_git: true,
        },
        command == "index",
    )?
    .index;
    Ok(match args.format {
        Format::Human => output::index_human(command, &index, query),
        Format::Json => output::index_json(command, &index, query)?,
    })
}

fn graph_query(
    command: &str,
    args: cli::GraphQueryArgs,
) -> Result<String, Box<dyn std::error::Error>> {
    graph_command(
        command,
        args.analysis,
        Some((&args.name, args.file.as_deref(), args.depth)),
    )
}
fn graph_command(
    command: &str,
    args: cli::AnalysisArgs,
    query: Option<(&str, Option<&std::path::Path>, u8)>,
) -> Result<String, Box<dyn std::error::Error>> {
    let graph = current_graph(&model::IndexOptions {
        root: args.path,
        max_parse_file_bytes: args.max_parse_file_size,
        probe_git: true,
    })?;
    let result = query.map(|(name, file, depth)| {
        graph.query(
            match command {
                "deps" => graph::QueryKind::Deps,
                "callers" => graph::QueryKind::Callers,
                _ => graph::QueryKind::Related,
            },
            name,
            file,
            depth,
        )
    });
    Ok(match args.format {
        Format::Human => output::graph_human(command, &graph, query.map(|q| q.0), result.as_ref()),
        Format::Json => output::graph_json(command, &graph, query.map(|q| q.0), result.as_ref())?,
    })
}

fn options(args: &cli::AnalysisArgs) -> model::IndexOptions {
    model::IndexOptions {
        root: args.path.clone(),
        max_parse_file_bytes: args.max_parse_file_size,
        probe_git: false,
    }
}
fn search_command(args: cli::SearchArgs) -> Result<String, Box<dyn std::error::Error>> {
    query::validate_query(&args.query)?;
    let index = current_analysis(&options(&args.analysis), false)?.index;
    let result = query::search(&index, &args.query, args.kind.into(), args.limit as usize)?;
    Ok(match args.analysis.format {
        Format::Human => {
            output::query::search_human(&index, &args.query, args.kind.into(), &result)
        }
        Format::Json => output::query::search_json(&index, &args.query, args.kind.into(), &result)?,
    })
}
fn neighborhood_command(args: cli::NeighborhoodArgs) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(file) = &args.file
        && (file.as_os_str().is_empty()
            || file
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_))))
    {
        return Err(
            "file selector must be an exact scan-root-relative path without . or .. components"
                .into(),
        );
    }
    let selector = args
        .symbol
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| args.file.as_ref().unwrap().to_string_lossy().into_owned());
    let g = current_graph(&options(&args.analysis))?;
    let result = g.query(
        graph::QueryKind::Neighborhood,
        &selector,
        args.file.as_deref(),
        args.depth,
    );
    Ok(match args.analysis.format {
        Format::Human => output::query::neighborhood_human(&g, &selector, &result),
        Format::Json => {
            output::query::neighborhood_json(&g, &selector, args.file.is_some(), &result)?
        }
    })
}
fn hotspots_command(args: cli::HotspotArgs) -> Result<String, Box<dyn std::error::Error>> {
    let g = current_graph(&options(&args.analysis))?;
    let result = query::hotspots(&g, args.limit as usize)?;
    Ok(match args.analysis.format {
        Format::Human => output::query::hotspots_human(&g, &result),
        Format::Json => output::query::hotspots_json(&g, &result)?,
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

fn change_command(
    command: &str,
    args: cli::ChangeArgs,
    depth: Option<u8>,
) -> Result<String, Box<dyn std::error::Error>> {
    let set = change::analyze(
        &args.analysis.path,
        &args.range,
        args.analysis.max_parse_file_size,
        depth,
    )?;
    Ok(match args.analysis.format {
        Format::Human => output::change::human(command, &set)?,
        Format::Json => output::change::json(command, &set)?,
    })
}

fn context_command(args: cli::ContextArgs) -> Result<String, Box<dyn std::error::Error>> {
    let task = context::Task::parse(&args.task)?;
    context::validate_budget(args.budget)?;
    let mut opts = options(&args.analysis);
    opts.probe_git = true;
    let g = current_graph(&opts)?;
    let changes = args
        .change
        .as_deref()
        .map(|range| context::compatible_change(&g, range, args.analysis.max_parse_file_size))
        .transpose()?;
    let ranking = context::rank(&g, &task, changes.as_ref());
    let bundle = context::select(&g, ranking, args.budget);
    if let Some(changes) = &changes {
        context::recheck_snapshot(&g, &changes.head)?;
    }
    Ok(match args.analysis.format {
        Format::Human => output::context::human(&g, &task, &bundle, changes.as_ref())?,
        Format::Json => output::context::json(&g, &task, &bundle, changes.as_ref())?,
    })
}

fn watch_command(args: cli::WatchArgs) -> Result<(), Box<dyn std::error::Error>> {
    let root = args.path.canonicalize()?;
    let options = model::IndexOptions {
        root: root.clone(),
        max_parse_file_bytes: args.max_parse_file_size,
        probe_git: false,
    };
    // Register before the initial reconciliation so edits during startup remain
    // queued. One foreground owner performs every scan/parse/store operation.
    let watcher = watch::Watch::new(&root)?;
    eprintln!(
        "ctx: watching {}",
        output::escape_controls(&root.to_string_lossy())
    );
    let update = || match persistence::reconcile(&options, false) {
        Ok(result) => {
            for warning in result.warnings {
                eprintln!("ctx: warning: {}", output::escape_controls(&warning));
            }
            if result.committed {
                eprintln!("ctx: index current");
            } else {
                eprintln!("ctx: warning: cache unchanged; update was not committed");
            }
        }
        Err(error) => eprintln!(
            "ctx: warning: {}",
            output::escape_controls(&error.to_string())
        ),
    };
    update();
    loop {
        if let Some(activity) = watcher.next() {
            if activity.watcher_error {
                eprintln!("ctx: warning: watcher error; rescanning repository");
            } else if activity.resync {
                eprintln!("ctx: warning: event queue overflow; rescanning repository");
            }
            update();
        }
    }
}

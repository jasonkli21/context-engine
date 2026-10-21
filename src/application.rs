//! Synchronous use cases shared by every adapter. No printing or transport types.
use crate::{change, context, graph, model, persistence, query};
use std::path::{Component, Path};

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
pub struct Application {
    pub options: model::IndexOptions,
}
pub struct Outcome<T> {
    pub data: T,
    pub warnings: Vec<String>,
}
pub struct Symbol {
    pub index: model::RepositoryIndex,
    pub query: String,
    pub match_mode: &'static str,
    pub matches: Vec<model::Symbol>,
}
pub struct Search {
    pub index: model::RepositoryIndex,
    pub result: query::SearchResult,
}
pub struct Neighborhood {
    pub graph: graph::RepositoryGraph,
    pub selector: String,
    pub result: graph::QueryResult,
}
pub struct Context {
    pub graph: graph::RepositoryGraph,
    pub task: context::Task,
    pub bundle: context::Bundle,
    pub changes: Option<context::ChangeSignal>,
}
impl Application {
    pub fn pinned(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err("repository root must be a directory".into());
        }
        Ok(Self {
            options: model::IndexOptions {
                root,
                max_parse_file_bytes: model::DEFAULT_MAX_PARSE_BYTES,
                probe_git: true,
            },
        })
    }
    pub fn analysis(&self, force: bool) -> Result<Outcome<persistence::AnalysisArtifact>> {
        let r = persistence::reconcile(&self.options, force)?;
        Ok(Outcome {
            data: r.artifact,
            warnings: r.warnings,
        })
    }
    pub fn symbol(&self, name: &str) -> Result<Outcome<Symbol>> {
        let r = self.analysis(false)?;
        let (match_mode, matches) = r.data.index.lookup(name);
        let matches = matches.into_iter().cloned().collect();
        Ok(Outcome {
            data: Symbol {
                index: r.data.index,
                query: name.to_owned(),
                match_mode,
                matches,
            },
            warnings: r.warnings,
        })
    }
    fn graph(&self) -> Result<Outcome<graph::RepositoryGraph>> {
        let r = self.analysis(false)?;
        Ok(Outcome {
            data: graph::from_analysis(r.data.index, r.data.relation_files)?,
            warnings: r.warnings,
        })
    }
    pub fn search(
        &self,
        text: &str,
        kind: query::SearchKind,
        limit: usize,
    ) -> Result<Outcome<Search>> {
        query::validate_query(text)?;
        query::validate_limit(limit)?;
        let r = self.analysis(false)?;
        let result = query::search(&r.data.index, text, kind, limit)?;
        Ok(Outcome {
            data: Search {
                index: r.data.index,
                result,
            },
            warnings: r.warnings,
        })
    }
    pub fn neighborhood(
        &self,
        symbol: Option<&str>,
        file: Option<&Path>,
        depth: u8,
    ) -> Result<Outcome<Neighborhood>> {
        if depth > graph::MAX_DEPTH {
            return Err("depth must be between 0 and 8".into());
        }
        if symbol.is_some() == file.is_some() {
            return Err("provide exactly one symbol or file".into());
        }
        if let Some(file) = file {
            validate_file(file)?;
        }
        let selector = symbol
            .map(str::to_owned)
            .unwrap_or_else(|| file.unwrap().to_string_lossy().into_owned());
        let r = self.graph()?;
        let result = r
            .data
            .query(graph::QueryKind::Neighborhood, &selector, file, depth);
        Ok(Outcome {
            data: Neighborhood {
                graph: r.data,
                selector,
                result,
            },
            warnings: r.warnings,
        })
    }
    pub fn impact(&self, range: &str, depth: u8) -> Result<change::ChangeSet> {
        if depth > graph::MAX_DEPTH {
            return Err("depth must be between 0 and 8".into());
        }
        change::analyze(
            &self.options.root,
            range,
            self.options.max_parse_file_bytes,
            Some(depth),
        )
    }
    pub fn context(
        &self,
        task: &str,
        budget: u64,
        change: Option<&str>,
    ) -> Result<Outcome<Context>> {
        let task = context::Task::parse(task)?;
        context::validate_budget(budget)?;
        let r = self.graph()?;
        let changes = change
            .map(|range| {
                context::compatible_change(&r.data, range, self.options.max_parse_file_bytes)
            })
            .transpose()?;
        let ranking = context::rank(&r.data, &task, changes.as_ref());
        let bundle = context::select(&r.data, ranking, budget);
        if let Some(changes) = &changes {
            context::recheck_snapshot(&r.data, &changes.head)?;
        }
        Ok(Outcome {
            data: Context {
                graph: r.data,
                task,
                bundle,
                changes,
            },
            warnings: r.warnings,
        })
    }
}
pub fn validate_file(file: &Path) -> Result<()> {
    if file.as_os_str().is_empty()
        || file
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(
            "file selector must be an exact scan-root-relative path without . or .. components"
                .into(),
        );
    }
    Ok(())
}
// Trusted launch configuration only. Remote request types never contain this.
impl From<model::IndexOptions> for Application {
    fn from(options: model::IndexOptions) -> Self {
        Self { options }
    }
}

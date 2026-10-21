//! Versioned adapter inputs/output, and completion-owned blocking admission.
use crate::{
    application::{self, Application},
    output, query,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io, path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

pub const MAX_REQUEST: usize = 16 * 1024;
pub const MAX_RESPONSE: usize = 8 * 1024 * 1024;
pub const MAX_CONTEXT: u64 = 1024 * 1024;
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    All,
    File,
    Symbol,
}
impl From<Kind> for query::SearchKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::All => Self::All,
            Kind::File => Self::File,
            Kind::Symbol => Self::Symbol,
        }
    }
}
fn kind() -> Kind {
    Kind::All
}
fn limit() -> usize {
    20
}
fn depth() -> u8 {
    1
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Search {
    #[schemars(length(min = 1, max = 4096))]
    pub query: String,
    #[serde(default = "kind")]
    pub kind: Kind,
    #[serde(default = "limit")]
    #[schemars(range(min = 1, max = 200))]
    pub limit: usize,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    #[schemars(length(min = 1, max = 4096))]
    pub name: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Neighborhood {
    pub symbol: Option<String>,
    pub file: Option<PathBuf>,
    #[serde(default = "depth")]
    #[schemars(range(min = 0, max = 8))]
    pub depth: u8,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Impact {
    #[schemars(length(min = 1, max = 4096))]
    pub range: String,
    #[serde(default = "depth")]
    #[schemars(range(min = 0, max = 8))]
    pub depth: u8,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Context {
    #[schemars(length(min = 1, max = 4096))]
    pub task: String,
    #[schemars(range(min = 1, max = 1048576))]
    pub budget: u64,
    pub change: Option<String>,
}
pub enum Request {
    Search(Search),
    Symbol(Symbol),
    Neighborhood(Neighborhood),
    Impact(Impact),
    Context(Context),
}
#[derive(Debug, Clone, Copy, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    #[error("invalid arguments")]
    InvalidArguments,
    #[error("repository query unavailable")]
    QueryFailed,
    #[error("committed Git data unavailable; requires a Git repository and resolvable commits")]
    GitUnavailable,
    #[error("response safety limit exceeded")]
    ResponseLimit,
    #[error("repository worker busy")]
    Busy,
    #[error("repository worker failed")]
    Internal,
}
impl Request {
    pub fn parse(name: &str, bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_REQUEST {
            return Err(Error::InvalidArguments);
        }
        fn parse<T: serde::de::DeserializeOwned>(b: &[u8]) -> Result<T, Error> {
            serde_json::from_slice(b).map_err(|_| Error::InvalidArguments)
        }
        let r = match name {
            "search_codebase" => Self::Search(parse(bytes)?),
            "find_symbol" => Self::Symbol(parse(bytes)?),
            "get_dependency_neighborhood" => Self::Neighborhood(parse(bytes)?),
            "get_change_impact" => Self::Impact(parse(bytes)?),
            "build_task_context" => Self::Context(parse(bytes)?),
            _ => return Err(Error::InvalidArguments),
        };
        r.validate().map_err(|_| Error::InvalidArguments)?;
        Ok(r)
    }
    fn validate(&self) -> application::Result<()> {
        match self {
            Self::Search(r) => {
                query::validate_query(&r.query)?;
                query::validate_limit(r.limit)?;
            }
            Self::Symbol(r) => query::validate_query(&r.name)?,
            Self::Neighborhood(r) => {
                if r.depth > 8 || r.symbol.is_some() == r.file.is_some() {
                    return Err("invalid selector".into());
                }
                if let Some(s) = &r.symbol {
                    query::validate_query(s)?;
                }
                if let Some(f) = &r.file {
                    application::validate_file(f)?;
                    let text = f.to_str().ok_or("file selector must be UTF-8")?;
                    if text.len() > 4096
                        || text
                            .split('/')
                            .any(|c| c.is_empty() || c == "." || c == "..")
                    {
                        return Err("invalid exact relative path".into());
                    }
                }
            }
            Self::Impact(r) => {
                if r.depth > 8 {
                    return Err("invalid depth".into());
                }
                crate::git::revision::range(&r.range, 4096)?;
            }
            Self::Context(r) => {
                crate::context::Task::parse(&r.task)?;
                if r.budget > MAX_CONTEXT {
                    return Err("adapter budget exceeds 1 MiB".into());
                }
                crate::context::validate_budget(r.budget)?;
                if let Some(c) = &r.change {
                    crate::git::revision::range(c, 4096)?;
                }
            }
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Reply {
    pub api_version: u32,
    pub state: &'static str,
    pub warnings: Vec<String>,
    pub result: Value,
}
struct Counter {
    bytes: usize,
    max: usize,
}
impl io::Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.saturating_add(bytes.len());
        if self.bytes > self.max {
            return Err(io::Error::other("response limit"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub fn check_size(value: &impl Serialize, max: usize) -> Result<(), Error> {
    serde_json::to_writer(Counter { bytes: 0, max }, value).map_err(|_| Error::ResponseLimit)
}
fn value(dto: impl Serialize) -> Result<Value, Error> {
    check_size(&dto, MAX_RESPONSE)?;
    serde_json::to_value(dto).map_err(|_| Error::Internal)
}
fn execute(app: &Application, request: Request) -> Result<Reply, Error> {
    let fail = |e: application::Error| {
        if e.downcast_ref::<crate::git::revision::Error>().is_some() {
            Error::GitUnavailable
        } else {
            Error::QueryFailed
        }
    };
    let (result, warnings, state) = match request {
        Request::Search(r) => {
            let o = app.search(&r.query, r.kind.into(), r.limit).map_err(fail)?;
            (
                value(output::query::search_dto(
                    &o.data.index,
                    &r.query,
                    r.kind.into(),
                    &o.data.result,
                ))?,
                o.warnings,
                "current_working_tree_best_effort",
            )
        }
        Request::Symbol(r) => {
            let o = app.symbol(&r.name).map_err(fail)?;
            (
                value(output::index::symbol_dto(&o.data))?,
                o.warnings,
                "current_working_tree_best_effort",
            )
        }
        Request::Neighborhood(r) => {
            let o = app
                .neighborhood(r.symbol.as_deref(), r.file.as_deref(), r.depth)
                .map_err(fail)?;
            let d = o.data;
            (
                value(output::query::neighborhood_dto(
                    &d.graph,
                    &d.selector,
                    r.file.is_some(),
                    &d.result,
                ))?,
                o.warnings,
                "current_working_tree_best_effort",
            )
        }
        Request::Impact(r) => {
            let d = app.impact(&r.range, r.depth).map_err(fail)?;
            (
                value(output::change::dto("impact", &d).map_err(|_| Error::ResponseLimit)?)?,
                vec![],
                "committed_revisions",
            )
        }
        Request::Context(r) => {
            let o = app
                .context(&r.task, r.budget, r.change.as_deref())
                .map_err(fail)?;
            let d = o.data;
            (
                value(
                    output::context::dto(&d.graph, &d.task, &d.bundle, d.changes.as_ref())
                        .map_err(|_| Error::ResponseLimit)?,
                )?,
                o.warnings,
                "current_working_tree_best_effort",
            )
        }
    };
    let reply = Reply {
        api_version: 1,
        state,
        warnings,
        result,
    };
    check_size(&reply, MAX_RESPONSE)?;
    Ok(reply)
}
#[derive(Clone)]
pub struct Worker {
    app: Arc<Application>,
    admission: Arc<Semaphore>,
}
impl Worker {
    pub fn new(app: Application) -> Self {
        Self {
            app: Arc::new(app),
            admission: Arc::new(Semaphore::new(1)),
        }
    }
    pub async fn run<T: Send + 'static>(
        &self,
        request: Request,
        render: impl FnOnce(Reply) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let app = self.app.clone();
        self.submit(move || render(execute(&app, request)?)).await
    }
    async fn submit<T: Send + 'static>(
        &self,
        job: impl FnOnce() -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        // Owned by the job: aborting its caller cannot admit another computation.
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            job()
        })
        .await
        .map_err(|_| Error::Internal)?
    }
    pub async fn drain(&self) {
        let _ = self.admission.acquire().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_keeps_admission_until_job_finishes() {
        let dir = tempfile::tempdir().unwrap();
        let worker = Worker::new(Application::pinned(dir.path()).unwrap());
        let (started, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let w = worker.clone();
        let caller = tokio::spawn(async move {
            w.submit(move || {
                started.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
            .await
        });
        started_rx.await.unwrap();
        caller.abort();
        assert!(matches!(worker.submit(|| Ok(())).await, Err(Error::Busy)));
        release.send(()).unwrap();
        worker.drain().await;
        worker.submit(|| Ok(())).await.unwrap();
    }
    #[test]
    fn size_preflight_handles_escaping_without_materializing_response() {
        assert!(check_size(&"\n".repeat(100), 150).is_err());
        check_size(&"a".repeat(100), 102).unwrap();
    }
}

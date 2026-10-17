//! Bounded, on-demand first-parent evidence. No current-state cache access.
use crate::{
    change::{self, Snapshot},
    git::revision::{self, Commit, Entry, FileChange, Revisions},
    model::Symbol,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub metadata: usize,
    pub diff: [usize; 3], // unscoped records, path bytes, raw bytes
    pub source: u64,
    pub files: usize,
    pub structural_records: usize,
    pub comparisons: usize,
    pub events: usize,
    pub event_bytes: usize,
    pub cochanges: usize,
    pub rendered_cochanges: usize,
}
pub(crate) const LIMITS: Limits = Limits {
    metadata: 1024 * 1024,
    diff: [50_000, 16 * 1024 * 1024, 64 * 1024 * 1024],
    source: 256 * 1024 * 1024,
    files: 20_000,
    structural_records: 200_000,
    comparisons: 4096,
    events: 8192,
    event_bytes: 2 * 1024 * 1024,
    cochanges: 4096,
    rendered_cochanges: 100,
};
fn limit() -> Box<dyn std::error::Error> {
    "history-wide resource safety limit exceeded".into()
}
fn spend(n: &mut usize, used: usize) -> Result<()> {
    *n = n.checked_sub(used).ok_or_else(limit)?;
    Ok(())
}
pub fn selector(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("target must be a relative path without parent traversal".into());
    }
    Ok(path
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect())
}
#[derive(Clone)]
pub struct Descriptor {
    pub path: PathBuf,
    pub language: String,
    pub kind: &'static str,
    pub qualified_name: String,
}
impl Descriptor {
    fn from(s: &Symbol) -> Self {
        Self {
            path: s.relative_path.clone(),
            language: s.language.name().into(),
            kind: s.kind.name(),
            qualified_name: s.qualified_name.clone(),
        }
    }
    fn matches(&self, s: &Symbol) -> bool {
        s.relative_path == self.path
            && s.language.name() == self.language
            && s.kind.name() == self.kind
            && s.qualified_name == self.qualified_name
    }
}
#[derive(Default)]
pub struct Structural {
    pub available: bool,
    pub relations_available: bool,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub relations_added: usize,
    pub relations_removed: usize,
    pub ambiguous_keys: usize,
}
pub struct Event {
    pub commit: Commit,
    pub kind: &'static str,
    pub git: FileChange,
    pub structural: Structural,
    pub gap: Option<String>,
    pub lineage_symbol_modified: Option<bool>,
}
#[derive(Default)]
pub struct Coverage {
    pub commits_inspected: usize,
    pub target_change_commits: usize,
    pub structural_comparisons: usize,
    pub structural_available: usize,
    pub relation_comparisons_available: usize,
    pub gaps: usize,
    pub history_truncated: bool,
    pub lineage_complete_to_introduction: bool,
    pub cochanges_omitted: usize,
    pub head_analysis_gaps: usize,
}
pub struct HistoryResult {
    pub root: PathBuf,
    pub scope: PathBuf,
    pub start_revision: String,
    pub target: PathBuf,
    pub symbol: Option<Descriptor>,
    pub events: Vec<Event>,
    pub cochanges: Vec<(PathBuf, usize)>,
    pub coverage: Coverage,
}
struct Work {
    remaining: Limits,
    max_parse: u64,
}
impl Work {
    fn snapshot(
        &mut self,
        r: &Revisions,
        oid: &str,
        mut entries: Vec<Entry>,
        texts: bool,
    ) -> Result<Snapshot> {
        spend(&mut self.remaining.files, entries.len())?;
        for e in &mut entries {
            if e.regular() {
                e.size = r.size(e)?;
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let wanted = if texts {
            entries.iter().map(|e| e.path.clone()).collect()
        } else {
            BTreeSet::new()
        };
        let s = change::snapshot(
            r,
            oid,
            entries,
            &wanted,
            self.max_parse,
            revision::Limits {
                source_bytes: self.remaining.source,
                ..revision::LIMITS
            },
        )?;
        self.remaining.source = self
            .remaining
            .source
            .checked_sub(s.source_bytes)
            .ok_or_else(limit)?;
        spend(
            &mut self.remaining.structural_records,
            s.graph
                .index
                .symbols
                .len()
                .saturating_add(s.graph.relations.len()),
        )?;
        Ok(s)
    }
    fn changes(&mut self, r: &Revisions) -> Result<Vec<FileChange>> {
        let (changes, used) = r.changes_with_usage(revision::Limits {
            changes: self.remaining.diff[0].min(revision::LIMITS.changes),
            change_paths: self.remaining.diff[1].min(revision::LIMITS.change_paths),
            diff_bytes: self.remaining.diff[2].min(revision::LIMITS.diff_bytes),
            ..revision::LIMITS
        })?;
        for (remaining, used) in self.remaining.diff.iter_mut().zip(used) {
            spend(remaining, used)?;
        }
        Ok(changes)
    }
}
fn matching(s: &Snapshot, d: &Descriptor) -> Vec<usize> {
    s.graph
        .index
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| d.matches(s))
        .map(|(i, _)| i)
        .collect()
}
fn touches(c: &FileChange, target: &Path, directory: bool) -> bool {
    [&c.before, &c.after]
        .into_iter()
        .flatten()
        .any(|e| e.path == target || (directory && e.path.starts_with(target)))
}
fn gap(s: &Snapshot, e: Option<&Entry>) -> String {
    match e {
        None => "absent".into(),
        Some(e) if e.path.to_str().is_none() => "non_utf8_path".into(),
        Some(e) if !e.regular() => "non_regular_git_mode".into(),
        Some(_) => change::file_analysis(s, e)
            .map(|a| {
                if a.syntax_error_count > 0 {
                    "syntax_errors".into()
                } else {
                    a.status.name().into()
                }
            })
            .unwrap_or_else(|| "missing_analysis".into()),
    }
}
pub fn history(
    path: &Path,
    name: &str,
    file: Option<&Path>,
    max_commits: usize,
    max_parse: u64,
) -> Result<HistoryResult> {
    crate::query::validate_query(name)?;
    run(path, Some(name), file, max_commits, max_parse, LIMITS)
}
pub fn evolution(
    path: &Path,
    target: &Path,
    max_commits: usize,
    max_parse: u64,
) -> Result<HistoryResult> {
    run(path, None, Some(target), max_commits, max_parse, LIMITS)
}
fn run(
    path: &Path,
    name: Option<&str>,
    target: Option<&Path>,
    max_commits: usize,
    max_parse: u64,
    limits: Limits,
) -> Result<HistoryResult> {
    if !(1..=512).contains(&max_commits) {
        return Err("max-commits must be between 1 and 512".into());
    }
    let target = target.map(selector).transpose()?;
    if name.is_some() && target.as_ref().is_some_and(|p| p.as_os_str().is_empty()) {
        return Err("--file requires an exact relative file path".into());
    }
    let mut r = Revisions::open(path, "HEAD..HEAD", revision::LIMITS)?;
    let commits = r.commits(max_commits, limits.metadata)?;
    let mut result = HistoryResult {
        root: r.root.clone(),
        scope: r.scope.clone(),
        start_revision: r.head.clone(),
        target: target.clone().unwrap_or_default(),
        symbol: None,
        events: vec![],
        cochanges: vec![],
        coverage: Coverage {
            history_truncated: commits.len() > max_commits,
            ..Coverage::default()
        },
    };
    let mut work = Work {
        remaining: limits,
        max_parse,
    };
    let mut lineage = if let Some(name) = name {
        let entries = r.inventory(&r.head, revision::LIMITS)?;
        let snapshot = work.snapshot(&r, &r.head, entries, false)?;
        result.coverage.head_analysis_gaps = snapshot
            .graph
            .index
            .files
            .iter()
            .filter(|f| {
                f.status != crate::model::AnalysisStatus::Analyzed || f.syntax_error_count > 0
            })
            .count();
        let syms = &snapshot.graph.index.symbols;
        let mut found: Vec<_> = syms.iter().filter(|s| s.qualified_name == name).collect();
        if found.is_empty() {
            found = syms.iter().filter(|s| s.name == name).collect();
        }
        if let Some(target) = &target {
            found.retain(|s| s.relative_path == *target);
        }
        if found.len() != 1 {
            return Err(format!("committed HEAD symbol {name:?}: {} matches; use an exact qualified name and --file (analysis gaps: {})", found.len(), result.coverage.head_analysis_gaps).into());
        }
        let d = Descriptor::from(found[0]);
        // A recovered symbol in a damaged file cannot seed confident lineage.
        let entry = Entry {
            path: d.path.clone(),
            mode: "100644".into(),
            oid: String::new(),
            size: 0,
        };
        if !change::usable(&snapshot, Some(&entry)) {
            return Err("committed HEAD symbol analysis is incomplete".into());
        }
        result.symbol = Some(d.clone());
        result.target = d.path.clone();
        Some(d)
    } else {
        None
    };
    let mut cochanges = BTreeMap::<PathBuf, usize>::new();
    for commit in commits.into_iter().take(max_commits) {
        r.head = commit.oid.clone();
        r.base = commit.first_parent.clone().unwrap_or_default();
        let changes = work.changes(&r)?;
        result.coverage.commits_inspected += 1;
        let tracked = lineage
            .as_ref()
            .map(|d| d.path.as_path())
            .unwrap_or(&result.target);
        let relevant: Vec<_> = changes
            .iter()
            .filter(|c| {
                if name.is_some() {
                    c.after.as_ref().is_some_and(|e| e.path == tracked)
                } else {
                    touches(c, tracked, true)
                }
            })
            .collect();
        if relevant.is_empty() {
            continue;
        }
        let event_start = result.events.len();
        let mut stop = false;
        for &git in &relevant {
            spend(&mut work.remaining.comparisons, 1)?;
            result.coverage.structural_comparisons += 1;
            let old = work.snapshot(&r, &r.base, git.before.iter().cloned().collect(), true)?;
            let new = work.snapshot(&r, &r.head, git.after.iter().cloned().collect(), true)?;
            let compared = change::compare_file(git.clone(), &old, &new);
            let mut structural = Structural {
                available: compared.comparison_available,
                relations_available: compared.relation_comparison_available,
                ambiguous_keys: compared.ambiguous_keys,
                ..Structural::default()
            };
            for s in &compared.symbols {
                match s.kind {
                    "added" => structural.added += 1,
                    "removed" => structural.removed += 1,
                    "modified" => structural.modified += 1,
                    _ => {}
                }
            }
            for rel in &compared.relations {
                structural.relations_added += rel.after.count.saturating_sub(rel.before.count);
                structural.relations_removed += rel.before.count.saturating_sub(rel.after.count);
            }
            result.coverage.structural_available += usize::from(structural.available);
            result.coverage.relation_comparisons_available +=
                usize::from(structural.relations_available);
            let mut unavailable = (!structural.available).then(|| {
                format!(
                    "before: {}; after: {}",
                    gap(&old, git.before.as_ref()),
                    gap(&new, git.after.as_ref())
                )
            });
            if !structural.relations_available && unavailable.is_none() {
                unavailable = Some("relation comparison incomplete or ambiguous".into());
            }
            let mut lineage_symbol_modified = None;
            let kind = if let Some(d) = &mut lineage {
                if git.status == "entered_scope" || !structural.available {
                    stop = true;
                    unavailable.get_or_insert("lineage crosses scan scope".into());
                    "comparison_unavailable"
                } else {
                    let after_matches = matching(&new, d);
                    let mut before_d = d.clone();
                    if let Some(e) = &git.before {
                        before_d.path = e.path.clone();
                    }
                    let before_matches = matching(&old, &before_d);
                    if after_matches.len() != 1 || before_matches.len() > 1 {
                        stop = true;
                        unavailable =
                            Some("exact descriptor is not unique at historical boundary".into());
                        "ambiguous"
                    } else if before_matches.is_empty() {
                        stop = true;
                        result.coverage.lineage_complete_to_introduction = true;
                        "introduced"
                    } else {
                        let modified = compared.symbols.iter().any(|s| {
                            s.kind == "modified" && s.after == after_matches.first().copied()
                        });
                        lineage_symbol_modified = Some(modified);
                        d.path = before_d.path;
                        if git.status == "renamed" {
                            "file_renamed"
                        } else if modified {
                            "modified"
                        } else if unavailable.is_some() {
                            "comparison_unavailable"
                        } else {
                            continue;
                        }
                    }
                }
            } else {
                "file_change"
            };
            if unavailable.is_some() {
                result.coverage.gaps += 1;
            }
            spend(&mut work.remaining.events, 1)?;
            let event_bytes = commit
                .subject
                .len()
                .saturating_add(unavailable.as_deref().map_or(0, str::len))
                .saturating_add(git.before.as_ref().map_or(0, |e| e.path.as_os_str().len()))
                .saturating_add(git.after.as_ref().map_or(0, |e| e.path.as_os_str().len()));
            spend(&mut work.remaining.event_bytes, event_bytes)?;
            result.events.push(Event {
                commit: commit.clone(),
                kind,
                git: git.clone(),
                structural,
                gap: unavailable,
                lineage_symbol_modified,
            });
        }
        if name.is_none() || result.events.len() > event_start {
            result.coverage.target_change_commits += 1;
            let excluded: BTreeSet<_> = relevant
                .iter()
                .flat_map(|c| [&c.before, &c.after])
                .flatten()
                .map(|e| e.path.clone())
                .collect();
            let other: BTreeSet<_> = changes
                .iter()
                .flat_map(|c| [&c.before, &c.after])
                .flatten()
                .map(|e| e.path.clone())
                .filter(|p| {
                    !excluded.contains(p) && !(name.is_none() && p.starts_with(&result.target))
                })
                .collect();
            for p in other {
                if !cochanges.contains_key(&p) {
                    spend(&mut work.remaining.cochanges, 1)?;
                }
                *cochanges.entry(p).or_default() += 1;
            }
        }
        if stop {
            break;
        }
    }
    result.coverage.history_truncated &= result.coverage.commits_inspected == max_commits;
    let mut cochanges: Vec<_> = cochanges.into_iter().collect();
    cochanges.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    result.coverage.cochanges_omitted = cochanges.len().saturating_sub(limits.rendered_cochanges);
    cochanges.truncate(limits.rendered_cochanges);
    result.cochanges = cochanges;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};
    fn git(r: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(r)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_DATE", "2026-10-15T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-10-15T00:00:00Z")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        git(r, &["init", "-q"]);
        git(r, &["config", "user.name", "Fixture"]);
        git(r, &["config", "user.email", "test@example.test"]);
        for i in 0..2 {
            fs::write(r.join("a.py"), format!("def f(): return {i}\n")).unwrap();
            fs::write(r.join("b.py"), format!("def g(): return {i}\n")).unwrap();
            git(r, &["add", "-A"]);
            git(r, &["commit", "-qm", "fixture"]);
        }
        d
    }
    #[test]
    fn history_wide_budgets_fail_closed_and_exact_retention_is_visible() {
        let d = fixture();
        let r = d.path();
        let check = |limits| run(r, None, Some(Path::new("a.py")), 128, 1024, limits);
        assert!(check(LIMITS).is_ok());
        for limits in [
            Limits {
                metadata: 1,
                ..LIMITS
            },
            Limits {
                diff: [1, LIMITS.diff[1], LIMITS.diff[2]],
                ..LIMITS
            },
            Limits {
                diff: [LIMITS.diff[0], 1, LIMITS.diff[2]],
                ..LIMITS
            },
            Limits {
                diff: [LIMITS.diff[0], LIMITS.diff[1], 1],
                ..LIMITS
            },
            Limits {
                source: 1,
                ..LIMITS
            },
            Limits { files: 1, ..LIMITS },
            Limits {
                structural_records: 1,
                ..LIMITS
            },
            Limits {
                comparisons: 1,
                ..LIMITS
            },
            Limits {
                events: 1,
                ..LIMITS
            },
            Limits {
                event_bytes: 1,
                ..LIMITS
            },
            Limits {
                cochanges: 0,
                ..LIMITS
            },
        ] {
            assert!(check(limits).is_err());
        }
        let result = check(Limits {
            rendered_cochanges: 0,
            ..LIMITS
        })
        .unwrap();
        assert_eq!(result.coverage.cochanges_omitted, 1);
        assert!(result.cochanges.is_empty());
        // One diff fits; the second exceeds the command-wide limit.
        let r = Revisions::open(r, "HEAD..HEAD", revision::LIMITS).unwrap();
        let n = r.inventory(&r.head, revision::LIMITS).unwrap()[0].size;
        assert!(
            check(Limits {
                source: n * 2,
                ..LIMITS
            })
            .is_err()
        );
    }
    #[test]
    fn lexical_targets_and_non_utf8_evidence() {
        assert_eq!(
            selector(Path::new("./src/./x.py")).unwrap(),
            Path::new("src/x.py")
        );
        assert_eq!(selector(Path::new(".")).unwrap(), Path::new(""));
        for p in ["", "../x", "src/../x", "/x"] {
            assert!(selector(Path::new(p)).is_err());
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let d = fixture();
            let r = Revisions::open(d.path(), "HEAD..HEAD", revision::LIMITS).unwrap();
            let mut entry = r.inventory(&r.head, revision::LIMITS).unwrap().remove(0);
            entry.path = PathBuf::from(std::ffi::OsString::from_vec(b"a\xff.py".to_vec()));
            let mut work = Work {
                remaining: LIMITS,
                max_parse: 1024,
            };
            let snap = work
                .snapshot(&r, &r.head, vec![entry.clone()], true)
                .unwrap();
            assert!(!change::usable(&snap, Some(&entry)));
            assert_eq!(gap(&snap, Some(&entry)), "non_utf8_path");
        }
    }
    #[test]
    fn incomplete_ancestry_is_rejected() {
        let d = fixture();
        let r = Revisions::open(d.path(), "HEAD..HEAD", revision::LIMITS).unwrap();
        // Git's shallow boundary hides real parents; it must not become introduction.
        fs::write(d.path().join(".git/shallow"), format!("{}\n", r.head)).unwrap();
        assert!(evolution(d.path(), Path::new("a.py"), 128, 1024).is_err());
        fs::remove_file(d.path().join(".git/shallow")).unwrap();
        fs::write(d.path().join(".git/info/grafts"), format!("{}\n", r.head)).unwrap();
        assert!(evolution(d.path(), Path::new("a.py"), 128, 1024).is_err());
    }
}

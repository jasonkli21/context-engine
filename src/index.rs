use crate::{
    classify::{FileRole, Language},
    language::{AnalysisError, AnalyzerRegistry},
    model::*,
    scan::ScanError,
    source,
};
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Scan(#[from] ScanError),
    #[error("analyzer initialization failed: {0}")]
    Analyzer(#[from] AnalysisError),
    #[error("global graph safety limit exceeded")]
    GraphLimit,
    #[error("repository analysis safety limit exceeded")]
    AnalysisLimit,
}
#[cfg(test)]
pub fn graph_analysis(
    options: &IndexOptions,
) -> Result<(RepositoryIndex, Vec<crate::relations::FileRelations>), IndexError> {
    let scan = crate::scan::scan_repository(&ScanOptions {
        root: options.root.clone(),
        count_lines: false,
        max_line_count_file_bytes: 0,
        probe_git: options.probe_git,
    })?;
    analyze_scan(scan, options.max_parse_file_bytes, true)
}
#[cfg(test)]
fn index_scan(
    scan: RepositoryScan,
    max_parse_file_bytes: u64,
) -> Result<RepositoryIndex, IndexError> {
    Ok(analyze_scan(scan, max_parse_file_bytes, false)?.0)
}

#[derive(Clone, Copy)]
struct RepositoryLimits {
    symbols: usize,
    name_bytes: usize,
    detail_bytes: usize,
    diagnostics: usize,
    relation_bytes: usize,
}
const REPOSITORY_LIMITS: RepositoryLimits = RepositoryLimits {
    symbols: 50_000,
    name_bytes: 8 * 1024 * 1024,
    detail_bytes: 16 * 1024 * 1024,
    diagnostics: 10_000,
    relation_bytes: crate::graph::MAX_RELATION_BYTES,
};
pub(crate) fn analyze_scan(
    scan: RepositoryScan,
    max_parse_file_bytes: u64,
    graph: bool,
) -> Result<(RepositoryIndex, Vec<crate::relations::FileRelations>), IndexError> {
    analyze_scan_with_limits(scan, max_parse_file_bytes, graph, REPOSITORY_LIMITS)
}
fn analyze_scan_with_limits(
    scan: RepositoryScan,
    max_parse_file_bytes: u64,
    graph: bool,
    limits: RepositoryLimits,
) -> Result<(RepositoryIndex, Vec<crate::relations::FileRelations>), IndexError> {
    analyze_with_loader(
        scan,
        max_parse_file_bytes,
        graph,
        limits,
        |root, file, max| source::load(root, &file.relative_path, max),
    )
}
pub(crate) fn analyze_loaded<F>(
    scan: RepositoryScan,
    max: u64,
    loader: F,
) -> Result<(RepositoryIndex, Vec<crate::relations::FileRelations>), IndexError>
where
    F: FnMut(&std::path::Path, &FileRecord, u64) -> Result<String, AnalysisStatus>,
{
    analyze_with_loader(scan, max, true, REPOSITORY_LIMITS, loader)
}
fn analyze_with_loader<F>(
    scan: RepositoryScan,
    max_parse_file_bytes: u64,
    graph: bool,
    limits: RepositoryLimits,
    mut loader: F,
) -> Result<(RepositoryIndex, Vec<crate::relations::FileRelations>), IndexError>
where
    F: FnMut(&std::path::Path, &FileRecord, u64) -> Result<String, AnalysisStatus>,
{
    let mut totals = AnalysisTotals {
        files: scan.files.len(),
        relation_bytes: scan
            .files
            .iter()
            .map(|f| f.relative_path.as_os_str().len())
            .sum(),
        ..AnalysisTotals::default()
    };
    totals.check(graph, limits)?;
    let mut registry = AnalyzerRegistry::new()?;
    let mut index = RepositoryIndex {
        root: scan.root,
        git: scan.git,
        scan_warnings: scan.warnings,
        files: vec![],
        symbols: vec![],
        diagnostics: vec![],
    };
    let mut relation_files = vec![];
    for file in scan.files {
        let unit = analyze_file(
            &index.root,
            &file,
            max_parse_file_bytes,
            graph,
            &mut registry,
            &mut loader,
        )?;
        totals.add(&unit.symbols, &unit.diagnostics, unit.relations.as_slice());
        totals.check(graph, limits)?;
        index.files.push(unit.analysis);
        index.symbols.extend(unit.symbols);
        index.diagnostics.extend(unit.diagnostics);
        relation_files.extend(unit.relations);
    }
    qualify_paths(&mut index);
    index.sort();
    Ok((index, relation_files))
}

pub(crate) struct FileAnalysisUnit {
    pub analysis: FileAnalysis,
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub relations: Option<crate::relations::FileRelations>,
}
fn analyze_file<F>(
    root: &std::path::Path,
    file: &FileRecord,
    max_parse_file_bytes: u64,
    graph: bool,
    registry: &mut AnalyzerRegistry,
    loader: &mut F,
) -> Result<FileAnalysisUnit, IndexError>
where
    F: FnMut(&std::path::Path, &FileRecord, u64) -> Result<String, AnalysisStatus>,
{
    let mut symbols = vec![];
    let mut diagnostics = vec![];
    let mut relation_files = vec![];
    let mut analysis = FileAnalysis {
        relative_path: file.relative_path.clone(),
        language: file.language.clone(),
        role: file.role,
        status: AnalysisStatus::UnsupportedLanguage,
        symbol_count: 0,
        syntax_error_count: 0,
    };
    if let Some(analyzer) = registry.analyzer_for(&file.language) {
        if matches!(file.role, FileRole::Generated | FileRole::Vendor) {
            analysis.status = AnalysisStatus::SkippedRole;
        } else if file.size_bytes > max_parse_file_bytes {
            analysis.status = AnalysisStatus::TooLarge;
        } else {
            match loader(root, file, max_parse_file_bytes) {
                Ok(text) => match if graph {
                    analyzer.analyze_graph(&file.relative_path, &text)
                } else {
                    analyzer.analyze(&file.relative_path, &text)
                } {
                    Ok(mut result) => {
                        if graph && let Some(extraction) = result.relations.take() {
                            relation_files.push(crate::relations::FileRelations {
                                relative_path: file.relative_path.clone(),
                                extraction,
                            });
                        }
                        analysis.status = AnalysisStatus::Analyzed;
                        analysis.symbol_count = result.symbols.len();
                        analysis.syntax_error_count = result.syntax_error_count;
                        symbols.extend(result.symbols);
                        diagnostics.extend(result.diagnostics);
                    }
                    Err(error) => {
                        let limit_exceeded = matches!(&error, AnalysisError::LimitExceeded);
                        analysis.status = if limit_exceeded {
                            AnalysisStatus::AnalysisLimitExceeded
                        } else {
                            AnalysisStatus::ParseFailed
                        };
                        diagnostics.push(AnalysisDiagnostic {
                            relative_path: file.relative_path.clone(),
                            kind: if limit_exceeded {
                                DiagnosticKind::AnalysisLimitExceeded
                            } else {
                                DiagnosticKind::ParseFailure
                            },
                            message: error.to_string(),
                            range: None,
                        });
                    }
                },
                Err(status) => {
                    analysis.status = status;
                    if status != AnalysisStatus::TooLarge {
                        let message = if status == AnalysisStatus::InvalidUtf8 {
                            "source is not valid UTF-8"
                        } else {
                            "source unavailable or no longer a safe regular file"
                        };
                        diagnostics.push(AnalysisDiagnostic {
                            relative_path: file.relative_path.clone(),
                            kind: if status == AnalysisStatus::InvalidUtf8 {
                                DiagnosticKind::InvalidUtf8
                            } else {
                                DiagnosticKind::ReadError
                            },
                            message: message.into(),
                            range: None,
                        });
                    }
                }
            }
        }
    }

    Ok(FileAnalysisUnit {
        analysis,
        symbols,
        diagnostics,
        relations: relation_files.pop(),
    })
}

pub(crate) fn validate_analysis(
    index: &RepositoryIndex,
    relations: &[crate::relations::FileRelations],
) -> Result<(), IndexError> {
    validate_with_limits(index, relations, true, REPOSITORY_LIMITS)
}
fn validate_with_limits(
    index: &RepositoryIndex,
    relations: &[crate::relations::FileRelations],
    graph: bool,
    limits: RepositoryLimits,
) -> Result<(), IndexError> {
    let mut totals = AnalysisTotals {
        files: index.files.len(),
        relation_bytes: index
            .files
            .iter()
            .map(|f| f.relative_path.as_os_str().len())
            .sum(),
        ..AnalysisTotals::default()
    };
    totals.add(&index.symbols, &index.diagnostics, relations);
    totals.check(graph, limits)
}
#[derive(Default)]
struct AnalysisTotals {
    files: usize,
    symbols: usize,
    names: usize,
    details: usize,
    diagnostics: usize,
    candidates: usize,
    relation_bytes: usize,
}
impl AnalysisTotals {
    fn add(
        &mut self,
        symbols: &[Symbol],
        diagnostics: &[AnalysisDiagnostic],
        relations: &[crate::relations::FileRelations],
    ) {
        self.symbols += symbols.len();
        self.diagnostics += diagnostics.len();
        for symbol in symbols {
            self.names += symbol.name.len() + symbol.qualified_name.len();
            self.details += symbol.relative_path.as_os_str().len();
            self.relation_bytes += symbol.relative_path.as_os_str().len();
        }
        for diagnostic in diagnostics {
            self.details += diagnostic.relative_path.as_os_str().len() + diagnostic.message.len();
        }
        for file in relations {
            self.candidates += file.extraction.candidates.len();
            for candidate in &file.extraction.candidates {
                self.relation_bytes +=
                    crate::relations::candidate_bytes(candidate, &file.relative_path);
            }
        }
    }
    fn check(&self, graph: bool, limits: RepositoryLimits) -> Result<(), IndexError> {
        if self.symbols > limits.symbols
            || self.names > limits.name_bytes
            || self.details > limits.detail_bytes
            || self.diagnostics > limits.diagnostics
        {
            return Err(IndexError::AnalysisLimit);
        }
        if graph
            && (self.files + self.symbols + 1 > crate::graph::MAX_ENTITIES
                || self.files + self.symbols + self.candidates > crate::graph::MAX_RELATIONS
                || self.relation_bytes > limits.relation_bytes)
        {
            return Err(IndexError::GraphLimit);
        }
        Ok(())
    }
}
pub(crate) fn qualify_paths(index: &mut RepositoryIndex) {
    // Preserve Phase 0's explicit qualification of lossy native paths.
    if index.root.to_str().is_none()
        || index
            .files
            .iter()
            .any(|f| f.relative_path.to_str().is_none())
        || index
            .git
            .as_ref()
            .is_some_and(|g| g.repository_root.to_str().is_none())
        || index
            .scan_warnings
            .iter()
            .any(|w| w.path.as_deref().is_some_and(|p| p.to_str().is_none()))
    {
        index.scan_warnings.push(ScanWarning { path: None, message: "some paths contain non-UTF-8 bytes; displayed paths use replacement characters and may not be unique".into() });
    }
}
pub fn is_supported(language: &Language) -> bool {
    matches!(language, Language::Rust | Language::Python)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_limits_apply_across_files_and_analysis_modes() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.py", "b.py"] {
            std::fs::write(
                dir.path().join(name),
                "def f(): pass\nimport outside as alias\n",
            )
            .unwrap();
        }
        let run = |limits, graph| {
            let scan = crate::scan::scan_repository(&ScanOptions {
                root: dir.path().into(),
                count_lines: false,
                max_line_count_file_bytes: 0,
                probe_git: false,
            })
            .unwrap();
            analyze_scan_with_limits(scan, 1000, graph, limits)
        };
        let exact = RepositoryLimits {
            symbols: 2,
            name_bytes: 4,
            detail_bytes: 8,
            ..REPOSITORY_LIMITS
        };
        for graph in [false, true] {
            assert!(run(exact, graph).is_ok());
            for limits in [
                RepositoryLimits {
                    symbols: 1,
                    ..exact
                },
                RepositoryLimits {
                    name_bytes: 3,
                    ..exact
                },
                RepositoryLimits {
                    detail_bytes: 7,
                    ..exact
                },
            ] {
                assert!(matches!(run(limits, graph), Err(IndexError::AnalysisLimit)));
            }
        }
        // Both files independently fit; aggregate targets, aliases and paths exceed the cap.
        let bytes = 2 * ("outside".len() + "alias".len() + "a.py".len() * 3);
        assert!(
            run(
                RepositoryLimits {
                    relation_bytes: bytes,
                    ..exact
                },
                true
            )
            .is_ok()
        );
        assert!(matches!(
            run(
                RepositoryLimits {
                    relation_bytes: bytes - 1,
                    ..exact
                },
                true
            ),
            Err(IndexError::GraphLimit)
        ));
        for name in ["a.py", "b.py"] {
            std::fs::write(dir.path().join(name), "def broken(\n").unwrap();
        }
        let (idx, _) = run(REPOSITORY_LIMITS, false).unwrap();
        assert!(!idx.diagnostics.is_empty());
        let count = idx.diagnostics.len();
        assert!(
            run(
                RepositoryLimits {
                    diagnostics: count,
                    ..REPOSITORY_LIMITS
                },
                false
            )
            .is_ok()
        );
        assert!(matches!(
            run(
                RepositoryLimits {
                    diagnostics: count - 1,
                    ..REPOSITORY_LIMITS
                },
                false
            ),
            Err(IndexError::AnalysisLimit)
        ));
    }
    #[test]
    fn merged_retained_and_changed_units_obey_all_aggregate_limits() {
        let root = tempfile::tempdir().unwrap();
        for name in ["a.py", "b.py"] {
            std::fs::write(
                root.path().join(name),
                "import outside as alias\ndef f(): pass\ndef broken(\n",
            )
            .unwrap();
        }
        let scan = || {
            crate::scan::scan_repository(&ScanOptions {
                root: root.path().into(),
                count_lines: false,
                max_line_count_file_bytes: 0,
                probe_git: false,
            })
            .unwrap()
        };
        let mut a = scan();
        a.files
            .retain(|f| f.relative_path == std::path::Path::new("a.py"));
        let mut b = scan();
        b.files
            .retain(|f| f.relative_path == std::path::Path::new("b.py"));
        let (mut retained, mut relations) = analyze_scan(a, 1024, true).unwrap();
        let (changed, changed_relations) = analyze_scan(b, 1024, true).unwrap();
        let mut totals = AnalysisTotals {
            files: retained.files.len(),
            relation_bytes: 4,
            ..AnalysisTotals::default()
        };
        totals.add(&retained.symbols, &retained.diagnostics, &relations);
        let limits = RepositoryLimits {
            symbols: totals.symbols,
            diagnostics: totals.diagnostics,
            name_bytes: totals.names,
            detail_bytes: totals.details,
            relation_bytes: totals.relation_bytes,
        };
        assert!(validate_with_limits(&retained, &relations, true, limits).is_ok());
        assert!(validate_with_limits(&changed, &changed_relations, true, limits).is_ok());
        retained.files.extend(changed.files);
        retained.symbols.extend(changed.symbols);
        retained.diagnostics.extend(changed.diagnostics);
        relations.extend(changed_relations);
        for limit in [
            RepositoryLimits {
                symbols: limits.symbols,
                ..REPOSITORY_LIMITS
            },
            RepositoryLimits {
                diagnostics: limits.diagnostics,
                ..REPOSITORY_LIMITS
            },
            RepositoryLimits {
                name_bytes: limits.name_bytes,
                ..REPOSITORY_LIMITS
            },
            RepositoryLimits {
                detail_bytes: limits.detail_bytes,
                ..REPOSITORY_LIMITS
            },
            RepositoryLimits {
                relation_bytes: limits.relation_bytes,
                ..REPOSITORY_LIMITS
            },
        ] {
            assert!(validate_with_limits(&retained, &relations, true, limit).is_err());
        }
    }
    #[test]
    fn disappearing_source_is_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("gone.rs"), "fn gone() {}").unwrap();
        std::fs::write(dir.path().join("ok.py"), "def ok(): pass").unwrap();
        let scan = crate::scan::scan_repository(&ScanOptions {
            root: dir.path().into(),
            count_lines: false,
            max_line_count_file_bytes: 0,
            probe_git: false,
        })
        .unwrap();
        std::fs::remove_file(dir.path().join("gone.rs")).unwrap();
        let index = index_scan(scan, 100).unwrap();
        assert_eq!(index.files[0].status, AnalysisStatus::ReadFailed);
        assert_eq!(index.files[1].status, AnalysisStatus::Analyzed);
        assert_eq!(index.symbols.len(), 1);
        assert_eq!(
            index.diagnostics[0].relative_path,
            std::path::PathBuf::from("gone.rs")
        );
    }
}

#[cfg(all(test, unix))]
mod mutation_tests {
    use super::*;
    #[test]
    fn unsafe_and_growing_files_after_discovery_are_recoverable() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        for name in ["grown.rs", "link.rs", "ok.rs"] {
            std::fs::write(dir.path().join(name), "fn safe() {}").unwrap();
        }
        let scan = crate::scan::scan_repository(&ScanOptions {
            root: dir.path().into(),
            count_lines: false,
            max_line_count_file_bytes: 0,
            probe_git: false,
        })
        .unwrap();
        std::fs::write(dir.path().join("grown.rs"), vec![b' '; 101]).unwrap();
        std::fs::remove_file(dir.path().join("link.rs")).unwrap();
        symlink(dir.path().join("ok.rs"), dir.path().join("link.rs")).unwrap();
        let index = index_scan(scan, 100).unwrap();
        assert_eq!(
            index.files.iter().map(|f| f.status).collect::<Vec<_>>(),
            vec![
                AnalysisStatus::TooLarge,
                AnalysisStatus::ReadFailed,
                AnalysisStatus::Analyzed
            ]
        );
        assert_eq!(index.symbols.len(), 1);
    }
}

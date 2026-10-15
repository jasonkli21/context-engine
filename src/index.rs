use crate::{
    classify::{FileRole, Language},
    language::{AnalysisError, AnalyzerRegistry},
    model::*,
    scan::{self, ScanError},
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
pub fn index_repository(options: &IndexOptions) -> Result<RepositoryIndex, IndexError> {
    let scan = scan::scan_repository(&ScanOptions {
        root: options.root.clone(),
        count_lines: false,
        max_line_count_file_bytes: 0,
        probe_git: options.probe_git,
    })?;
    Ok(analyze_scan(scan, options.max_parse_file_bytes, false)?.0)
}

pub fn graph_analysis(
    options: &IndexOptions,
) -> Result<(RepositoryIndex, Vec<crate::relations::FileRelations>), IndexError> {
    let scan = scan::scan_repository(&ScanOptions {
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
fn analyze_scan(
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
    let (mut name_bytes, mut detail_bytes) = (0usize, 0usize);
    let mut relation_bytes = scan
        .files
        .iter()
        .map(|f| f.relative_path.as_os_str().len())
        .sum::<usize>();
    if graph && relation_bytes > limits.relation_bytes {
        return Err(IndexError::GraphLimit);
    }
    if graph && scan.files.len().saturating_add(1) > crate::graph::MAX_ENTITIES {
        return Err(IndexError::GraphLimit);
    }
    let mut relation_files = vec![];
    let mut relation_count = scan.files.len();
    let file_count = scan.files.len();
    let mut registry = AnalyzerRegistry::new()?;
    let mut index = RepositoryIndex {
        root: scan.root,
        git: scan.git,
        scan_warnings: scan.warnings,
        files: vec![],
        symbols: vec![],
        diagnostics: vec![],
    };
    for file in scan.files {
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
                match source::load(&index.root, &file.relative_path, max_parse_file_bytes) {
                    Ok(text) => match if graph {
                        analyzer.analyze_graph(&file.relative_path, &text)
                    } else {
                        analyzer.analyze(&file.relative_path, &text)
                    } {
                        Ok(mut result) => {
                            let names = result
                                .symbols
                                .iter()
                                .map(|s| s.name.len().saturating_add(s.qualified_name.len()))
                                .sum::<usize>();
                            let details = result
                                .symbols
                                .iter()
                                .map(|s| s.relative_path.as_os_str().len())
                                .sum::<usize>()
                                + result
                                    .diagnostics
                                    .iter()
                                    .map(|d| d.relative_path.as_os_str().len() + d.message.len())
                                    .sum::<usize>();
                            name_bytes = name_bytes.saturating_add(names);
                            detail_bytes = detail_bytes.saturating_add(details);
                            if index.symbols.len().saturating_add(result.symbols.len())
                                > limits.symbols
                                || name_bytes > limits.name_bytes
                                || detail_bytes > limits.detail_bytes
                                || index
                                    .diagnostics
                                    .len()
                                    .saturating_add(result.diagnostics.len())
                                    > limits.diagnostics
                            {
                                return Err(IndexError::AnalysisLimit);
                            }
                            if graph {
                                if file_count
                                    .saturating_add(index.symbols.len())
                                    .saturating_add(result.symbols.len())
                                    .saturating_add(1)
                                    > crate::graph::MAX_ENTITIES
                                {
                                    return Err(IndexError::GraphLimit);
                                }
                                if let Some(extraction) = result.relations.take() {
                                    relation_bytes =
                                        relation_bytes
                                            .saturating_add(
                                                extraction
                                                    .candidates
                                                    .iter()
                                                    .map(|c| {
                                                        crate::relations::candidate_bytes(
                                                            c,
                                                            &file.relative_path,
                                                        )
                                                    })
                                                    .sum::<usize>(),
                                            )
                                            .saturating_add(result.symbols.len().saturating_mul(
                                                file.relative_path.as_os_str().len(),
                                            ));
                                    if relation_bytes > limits.relation_bytes {
                                        return Err(IndexError::GraphLimit);
                                    }
                                    relation_count = relation_count
                                        .saturating_add(result.symbols.len())
                                        .saturating_add(extraction.candidates.len());
                                    if relation_count > crate::graph::MAX_RELATIONS {
                                        return Err(IndexError::GraphLimit);
                                    }
                                    relation_files.push(crate::relations::FileRelations {
                                        relative_path: file.relative_path.clone(),
                                        extraction,
                                    });
                                }
                            }
                            analysis.status = AnalysisStatus::Analyzed;
                            analysis.symbol_count = result.symbols.len();
                            analysis.syntax_error_count = result.syntax_error_count;
                            index.symbols.extend(result.symbols);
                            index.diagnostics.extend(result.diagnostics);
                        }
                        Err(error) => {
                            let limit_exceeded = matches!(&error, AnalysisError::LimitExceeded);
                            analysis.status = if limit_exceeded {
                                AnalysisStatus::AnalysisLimitExceeded
                            } else {
                                AnalysisStatus::ParseFailed
                            };
                            detail_bytes = detail_bytes.saturating_add(
                                file.relative_path.as_os_str().len() + error.to_string().len(),
                            );
                            if index.diagnostics.len() >= limits.diagnostics
                                || detail_bytes > limits.detail_bytes
                            {
                                return Err(IndexError::AnalysisLimit);
                            }
                            index.diagnostics.push(AnalysisDiagnostic {
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
                            detail_bytes = detail_bytes.saturating_add(
                                file.relative_path.as_os_str().len() + message.len(),
                            );
                            if index.diagnostics.len() >= limits.diagnostics
                                || detail_bytes > limits.detail_bytes
                            {
                                return Err(IndexError::AnalysisLimit);
                            }
                            index.diagnostics.push(AnalysisDiagnostic {
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
        index.files.push(analysis);
    }
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
    index.sort();
    Ok((index, relation_files))
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
            let scan = scan::scan_repository(&ScanOptions {
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
    fn disappearing_source_is_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("gone.rs"), "fn gone() {}").unwrap();
        std::fs::write(dir.path().join("ok.py"), "def ok(): pass").unwrap();
        let scan = scan::scan_repository(&ScanOptions {
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
        let scan = scan::scan_repository(&ScanOptions {
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

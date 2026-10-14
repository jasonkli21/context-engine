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
}
pub fn index_repository(options: &IndexOptions) -> Result<RepositoryIndex, IndexError> {
    let scan = scan::scan_repository(&ScanOptions {
        root: options.root.clone(),
        count_lines: false,
        max_line_count_file_bytes: 0,
        probe_git: options.probe_git,
    })?;
    index_scan(scan, options.max_parse_file_bytes)
}

fn index_scan(
    scan: RepositoryScan,
    max_parse_file_bytes: u64,
) -> Result<RepositoryIndex, IndexError> {
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
                    Ok(text) => match analyzer.analyze(&file.relative_path, &text) {
                        Ok(result) => {
                            analysis.status = AnalysisStatus::Analyzed;
                            analysis.symbol_count = result.symbols.len();
                            analysis.syntax_error_count = result.syntax_error_count;
                            index.symbols.extend(result.symbols);
                            index.diagnostics.extend(result.diagnostics);
                        }
                        Err(error) => {
                            analysis.status = AnalysisStatus::ParseFailed;
                            index.diagnostics.push(AnalysisDiagnostic {
                                relative_path: file.relative_path.clone(),
                                kind: DiagnosticKind::ParseFailure,
                                message: error.to_string(),
                                range: None,
                            });
                        }
                    },
                    Err(status) => {
                        analysis.status = status;
                        if status != AnalysisStatus::TooLarge {
                            index.diagnostics.push(AnalysisDiagnostic {
                                relative_path: file.relative_path.clone(),
                                kind: if status == AnalysisStatus::InvalidUtf8 {
                                    DiagnosticKind::InvalidUtf8
                                } else {
                                    DiagnosticKind::ReadError
                                },
                                message: if status == AnalysisStatus::InvalidUtf8 {
                                    "source is not valid UTF-8"
                                } else {
                                    "source unavailable or no longer a safe regular file"
                                }
                                .into(),
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
    Ok(index)
}
pub fn is_supported(language: &Language) -> bool {
    matches!(language, Language::Rust | Language::Python)
}

#[cfg(test)]
mod tests {
    use super::*;
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

//! Current-state full-analysis reuse. No query or graph IDs cross this boundary.
use crate::{
    classify::FileRole, graph, index, model::*, relations::FileRelations, scan, source, store,
};
use std::path::{Path, PathBuf};

// Bump when scanner/classifier, Rust/Python analysis, relation extraction, or
// persisted enum semantics change. Keep this rule adjacent to the version gate.
pub const INDEX_COMPAT_VERSION: i64 = 1;
pub const FINGERPRINT_VERSION: &str = "freshness-v1";
const MAX_FRESHNESS_BYTES: u64 = 256 * 1024 * 1024;
pub struct AnalysisArtifact {
    pub index: RepositoryIndex,
    pub relation_files: Vec<FileRelations>,
}
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error(transparent)]
    Analysis(#[from] index::IndexError),
    #[error(transparent)]
    Scan(#[from] scan::ScanError),
    #[error("freshness resource limit exceeded (256 MiB); reduce the parse file limit or scope")]
    FreshnessLimit,
    #[error("cannot establish source freshness; check readability and retry")]
    Freshness,
    #[error("repository changed during both analysis attempts; retry when files are stable")]
    Changed,
    #[error("cache I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache configuration: {0}")]
    Configuration(&'static str),
    #[error(transparent)]
    Store(#[from] store::StoreError),
}
fn scan_now(options: &IndexOptions) -> Result<RepositoryScan, CacheError> {
    Ok(scan::scan_repository(&ScanOptions {
        root: options.root.clone(),
        count_lines: false,
        max_line_count_file_bytes: 0,
        probe_git: options.probe_git,
    })?)
}
fn field(hash: &mut blake3::Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn fingerprint(scan: &RepositoryScan, max: u64) -> Result<[u8; 32], CacheError> {
    fingerprint_with_limit(scan, max, MAX_FRESHNESS_BYTES)
}
fn fingerprint_with_limit(
    scan: &RepositoryScan,
    max: u64,
    limit: u64,
) -> Result<[u8; 32], CacheError> {
    let mut hash = blake3::Hasher::new();
    field(&mut hash, FINGERPRINT_VERSION.as_bytes());
    hash.update(&max.to_le_bytes());
    let mut used = 0u64;
    for file in &scan.files {
        field(&mut hash, &store::native_bytes(&file.relative_path));
        field(
            &mut hash,
            &serde_json::to_vec(&file.language).expect("language codec"),
        );
        field(&mut hash, file.role.name().as_bytes());
        hash.update(&file.size_bytes.to_le_bytes());
        if index::is_supported(&file.language)
            && !matches!(file.role, FileRole::Generated | FileRole::Vendor)
            && file.size_bytes <= max
        {
            // Charge the maximum possible read, including a growth probe.
            let allowance = file.size_bytes.saturating_add(1);
            used = used.saturating_add(allowance);
            if used > limit {
                return Err(CacheError::FreshnessLimit);
            }
            let bytes = source::load_raw(&scan.root, &file.relative_path, file.size_bytes)
                .map_err(|_| CacheError::Freshness)?;
            field(&mut hash, &bytes);
        }
    }
    Ok(*hash.finalize().as_bytes())
}
fn location(root: &Path, max: u64) -> Result<PathBuf, CacheError> {
    let base = match std::env::var_os("CONTEXT_ENGINE_CACHE_DIR") {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute()
                || path.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::CurDir
                    )
                })
            {
                return Err(CacheError::Configuration(
                    "CONTEXT_ENGINE_CACHE_DIR must be absolute without . or .. components",
                ));
            }
            path
        }
        None => dirs::cache_dir()
            .ok_or(CacheError::Configuration(
                "no platform cache directory; set CONTEXT_ENGINE_CACHE_DIR",
            ))?
            .join("context-engine"),
    };
    // Resolve the nearest existing ancestor before creating anything. Even an
    // override beneath a repository symlink must not pollute that repository.
    let target = base.join("indexes");
    let mut ancestor = target.as_path();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .ok_or(CacheError::Configuration("invalid cache path"))?
                .to_os_string(),
        );
        ancestor = ancestor
            .parent()
            .ok_or(CacheError::Configuration("invalid cache path"))?;
    }
    let mut base = ancestor.canonicalize()?;
    for part in missing.iter().rev() {
        base.push(part);
    }
    if base.starts_with(root) {
        return Err(CacheError::Configuration(
            "cache directory must be outside the analyzed repository",
        ));
    }
    std::fs::create_dir_all(&base)?;
    let mut hash = blake3::Hasher::new();
    field(&mut hash, store::PATH_CODEC.as_bytes());
    field(&mut hash, &store::native_bytes(root));
    hash.update(&max.to_le_bytes());
    Ok(base.join(format!("{}.sqlite3", hash.finalize().to_hex())))
}
pub fn load_or_build(options: &IndexOptions, force: bool) -> Result<AnalysisArtifact, CacheError> {
    load_with(options, force, None, |scan| {
        let (index, relation_files) =
            index::analyze_scan(scan, options.max_parse_file_bytes, true)?;
        Ok(AnalysisArtifact {
            index,
            relation_files,
        })
    })
}
fn load_with<F>(
    options: &IndexOptions,
    force: bool,
    test_path: Option<&Path>,
    build: F,
) -> Result<AnalysisArtifact, CacheError>
where
    F: FnMut(RepositoryScan) -> Result<AnalysisArtifact, CacheError>,
{
    load_with_fingerprint(options, force, test_path, build, fingerprint)
}
fn warn_unverified() {
    eprintln!(
        "ctx: warning: source freshness could not be established; returning uncached best-effort analysis"
    );
}
fn load_with_fingerprint<F, H>(
    options: &IndexOptions,
    force: bool,
    test_path: Option<&Path>,
    mut build: F,
    mut fingerprint_fn: H,
) -> Result<AnalysisArtifact, CacheError>
where
    F: FnMut(RepositoryScan) -> Result<AnalysisArtifact, CacheError>,
    H: FnMut(&RepositoryScan, u64) -> Result<[u8; 32], CacheError>,
{
    for attempt in 0..2 {
        let before = scan_now(options)?;
        let stamp = match fingerprint_fn(&before, options.max_parse_file_bytes) {
            Ok(stamp) => stamp,
            Err(CacheError::Freshness) if !force => {
                warn_unverified();
                return build(before);
            }
            Err(error) => return Err(error),
        };
        let path = test_path
            .map(|p| Ok(p.to_path_buf()))
            .unwrap_or_else(|| location(&before.root, options.max_parse_file_bytes));
        if !force
            && let Ok(path) = &path
            && let Ok(artifact) = store::load(path, &before, options.max_parse_file_bytes, &stamp)
        {
            return Ok(artifact);
        }
        let mut artifact = build(before)?;
        if artifact
            .index
            .files
            .iter()
            .any(|f| f.status == AnalysisStatus::ReadFailed)
        {
            if force {
                return Err(CacheError::Freshness);
            }
            warn_unverified();
            return Ok(artifact);
        }
        let after = scan_now(options)?;
        let post = match fingerprint_fn(&after, options.max_parse_file_bytes) {
            Ok(stamp) => stamp,
            Err(CacheError::Freshness) if !force => {
                warn_unverified();
                return Ok(artifact);
            }
            Err(error) => return Err(error),
        };
        if post != stamp || after.root != artifact.index.root {
            if attempt == 0 {
                continue;
            }
            return Err(CacheError::Changed);
        }
        artifact.index.git = after.git;
        artifact.index.scan_warnings = after.warnings;
        index::qualify_paths(&mut artifact.index);
        artifact.index.sort();
        let saved = path.and_then(|path| {
            Ok(store::save(
                &path,
                &artifact,
                options.max_parse_file_bytes,
                &stamp,
            )?)
        });
        if let Err(error) = saved {
            if force {
                return Err(error);
            }
            eprintln!(
                "ctx: warning: {}",
                crate::output::escape_controls(&error.to_string())
            );
        }
        return Ok(artifact);
    }
    unreachable!()
}
pub fn graph(options: &IndexOptions) -> Result<graph::RepositoryGraph, CacheError> {
    let artifact = load_or_build(options, false)?;
    Ok(graph::from_analysis(
        artifact.index,
        artifact.relation_files,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options(root: &Path) -> IndexOptions {
        IndexOptions {
            root: root.into(),
            max_parse_file_bytes: 1024,
            probe_git: false,
        }
    }
    fn build(scan: RepositoryScan) -> Result<AnalysisArtifact, CacheError> {
        let (index, relation_files) = index::analyze_scan(scan, 1024, true)?;
        Ok(AnalysisArtifact {
            index,
            relation_files,
        })
    }
    #[test]
    fn warm_hit_bypasses_builder_and_force_rebuilds() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.py"), "def one(): pass\none()\n").unwrap();
        let opts = options(root.path());
        let path = cache.path().join("index.sqlite3");
        let cold = load_with(&opts, false, Some(&path), build).unwrap();
        let warm = load_with(&opts, false, Some(&path), |_| {
            panic!("warm cache invoked parser builder")
        })
        .unwrap();
        assert_eq!(
            crate::output::index_json("symbols", &cold.index, None).unwrap(),
            crate::output::index_json("symbols", &warm.index, None).unwrap()
        );
        let mut builds = 0;
        load_with(&opts, true, Some(&path), |s| {
            builds += 1;
            build(s)
        })
        .unwrap();
        assert_eq!(builds, 1);
        std::fs::write(root.path().join("a.py"), "def two(): pass\ntwo()\n").unwrap();
        let changed = load_with(&opts, false, Some(&path), |s| {
            builds += 1;
            build(s)
        })
        .unwrap();
        assert_eq!(builds, 2);
        assert_eq!(changed.index.symbols[0].name, "two");
    }
    #[test]
    fn inventory_content_options_and_eligibility_freshness() {
        let root = tempfile::tempdir().unwrap();
        let mut opts = options(root.path());
        let stamp = |opts: &IndexOptions| {
            fingerprint(&scan_now(opts).unwrap(), opts.max_parse_file_bytes).unwrap()
        };
        for (file, before, after) in [
            ("a.rs", "fn one() {}", "fn two() {}"),
            ("a.py", "def one(): pass", "def two(): pass"),
        ] {
            std::fs::write(root.path().join(file), before).unwrap();
            let a = stamp(&opts);
            assert_eq!(a, stamp(&opts));
            std::fs::write(root.path().join(file), after).unwrap();
            assert_ne!(a, stamp(&opts));
            let b = stamp(&opts);
            std::fs::rename(root.path().join(file), root.path().join("renamed.rs")).unwrap();
            assert_ne!(b, stamp(&opts));
            std::fs::remove_file(root.path().join("renamed.rs")).unwrap();
            assert_ne!(b, stamp(&opts));
        }
        std::fs::write(root.path().join("invalid.py"), [255]).unwrap();
        let a = stamp(&opts); // raw invalid UTF-8 is hashable
        opts.max_parse_file_bytes = 0;
        assert_ne!(a, stamp(&opts));
        std::fs::create_dir(root.path().join("generated")).unwrap();
        std::fs::write(root.path().join("generated/a.rs"), "fn one() {}").unwrap();
        let a = stamp(&opts);
        std::fs::write(root.path().join("generated/a.rs"), "fn two() {}").unwrap();
        assert_eq!(a, stamp(&opts));
        std::fs::write(root.path().join("invalid.py"), [254]).unwrap();
        assert_eq!(a, stamp(&opts)); // above parse limit
        opts.max_parse_file_bytes = 1024;
        std::fs::create_dir(root.path().join("vendor")).unwrap();
        std::fs::write(root.path().join("vendor/a.rs"), "fn one() {}").unwrap();
        std::fs::write(root.path().join("large.rs"), vec![b'a'; 1025]).unwrap();
        let a = stamp(&opts);
        for file in ["generated/a.rs", "vendor/a.rs"] {
            std::fs::write(root.path().join(file), "fn six() {}").unwrap();
            assert_eq!(a, stamp(&opts));
        }
        std::fs::write(root.path().join("large.rs"), vec![b'b'; 1025]).unwrap();
        assert_eq!(a, stamp(&opts));
        assert!(matches!(
            fingerprint_with_limit(&scan_now(&opts).unwrap(), 1024, 1),
            Err(CacheError::FreshnessLimit)
        ));
    }
    #[test]
    fn repeated_build_mutation_never_replaces_prior_cache() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let source = root.path().join("a.rs");
        std::fs::write(&source, "fn one() {}").unwrap();
        let opts = options(root.path());
        let path = cache.path().join("cache.sqlite3");
        load_with(&opts, true, Some(&path), build).unwrap();
        let prior = std::fs::read(&path).unwrap();
        let mut n = 0;
        let result = load_with(&opts, true, Some(&path), |s| {
            let a = build(s)?;
            n += 1;
            std::fs::write(&source, format!("fn changed{n}() {{}}")).unwrap();
            Ok(a)
        });
        assert!(matches!(result, Err(CacheError::Changed)));
        assert_eq!(n, 2);
        assert_eq!(prior, std::fs::read(&path).unwrap());
        let mut n = 0;
        load_with(&opts, true, Some(&path), |s| {
            let a = build(s)?;
            n += 1;
            if n == 1 {
                std::fs::write(&source, "fn stable() {}").unwrap();
            }
            Ok(a)
        })
        .unwrap();
        assert_eq!(n, 2);
    }
    #[test]
    fn automatic_read_failure_returns_uncached_best_effort_analysis() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("good.py"), "def available(): pass\n").unwrap();
        std::fs::write(root.path().join("unreadable.py"), "def hidden(): pass\n").unwrap();
        let opts = options(root.path());
        let path = cache.path().join("cache.sqlite3");
        let artifact = load_with_fingerprint(
            &opts,
            false,
            Some(&path),
            |scan| {
                let mut artifact = build(scan)?;
                let failed = artifact
                    .index
                    .files
                    .iter_mut()
                    .find(|f| f.relative_path == Path::new("unreadable.py"))
                    .unwrap();
                failed.status = AnalysisStatus::ReadFailed;
                failed.symbol_count = 0;
                artifact
                    .index
                    .symbols
                    .retain(|s| s.relative_path != Path::new("unreadable.py"));
                Ok(artifact)
            },
            |_, _| Err(CacheError::Freshness),
        )
        .unwrap();
        assert!(artifact.index.symbols.iter().any(|s| s.name == "available"));
        assert_eq!(
            artifact
                .index
                .files
                .iter()
                .find(|f| f.relative_path == Path::new("unreadable.py"))
                .unwrap()
                .status,
            AnalysisStatus::ReadFailed
        );
        let json = crate::output::index_json("symbols", &artifact.index, None).unwrap();
        serde_json::from_str::<serde_json::Value>(&json).unwrap();
        assert!(!path.exists());
        assert!(matches!(
            load_with_fingerprint(&opts, true, Some(&path), build, |_, _| Err(
                CacheError::Freshness
            )),
            Err(CacheError::Freshness)
        ));
        assert!(!path.exists());
    }
    #[test]
    fn incompatible_analysis_version_rebuilds_the_derived_cache() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.py"), "def current(): pass\n").unwrap();
        let opts = options(root.path());
        let path = cache.path().join("cache.sqlite3");
        load_with(&opts, false, Some(&path), build).unwrap();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute("UPDATE meta SET compat=compat+1", [])
            .unwrap();
        let mut builds = 0;
        load_with(&opts, false, Some(&path), |scan| {
            builds += 1;
            build(scan)
        })
        .unwrap();
        assert_eq!(builds, 1);
        let compat: i64 = rusqlite::Connection::open(&path)
            .unwrap()
            .query_row("SELECT compat FROM meta", [], |row| row.get(0))
            .unwrap();
        assert_eq!(compat, INDEX_COMPAT_VERSION);
    }
    #[cfg(unix)]
    #[test]
    fn real_non_utf8_source_path_survives_cold_and_warm_persistence() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let native_name = std::ffi::OsString::from_vec(b"native-\xff.py".to_vec());
        let creation = std::fs::write(
            root.path().join(&native_name),
            "def native_symbol(): pass\n",
        );
        if let Err(error) = creation {
            if matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidInput
            ) {
                return;
            }
            panic!("failed to create native path fixture: {error}");
        }
        let opts = options(root.path());
        let path = cache.path().join("cache.sqlite3");
        let cold = load_with(&opts, false, Some(&path), build).unwrap();
        let warm = load_with(&opts, false, Some(&path), |_| {
            panic!("warm load reparsed source")
        })
        .unwrap();
        let cold_path = cold
            .index
            .files
            .iter()
            .find(|f| f.relative_path.as_os_str().as_bytes() == native_name.as_bytes())
            .unwrap();
        let warm_path = warm
            .index
            .files
            .iter()
            .find(|f| f.relative_path.as_os_str().as_bytes() == native_name.as_bytes())
            .unwrap();
        assert_eq!(cold_path.relative_path, warm_path.relative_path);
        assert!(
            warm.index
                .symbols
                .iter()
                .any(
                    |s| s.relative_path.as_os_str().as_bytes() == native_name.as_bytes()
                        && s.name == "native_symbol"
                )
        );
    }
}

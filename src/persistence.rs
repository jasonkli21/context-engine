//! Current-state file-local analysis reconciliation. No query or graph IDs cross this boundary.
use crate::{classify::FileRole, index, model::*, relations::FileRelations, scan, source, store};
use std::path::{Path, PathBuf};

// Bump when scanner/classifier, Rust/Python analysis, relation extraction, or
// persisted enum semantics change. Keep this rule adjacent to the version gate.
pub const INDEX_COMPAT_VERSION: i64 = 1;
pub const FINGERPRINT_VERSION: &str = "freshness-v2";
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
#[cfg(test)]
fn fingerprint(scan: &RepositoryScan, max: u64) -> Result<[u8; 32], CacheError> {
    Ok(file_inputs(scan, max, MAX_FRESHNESS_BYTES)?.0)
}
#[cfg(test)]
fn fingerprint_with_limit(
    scan: &RepositoryScan,
    max: u64,
    limit: u64,
) -> Result<[u8; 32], CacheError> {
    Ok(file_inputs(scan, max, limit)?.0)
}
fn file_inputs(
    scan: &RepositoryScan,
    max: u64,
    limit: u64,
) -> Result<([u8; 32], store::Inputs), CacheError> {
    let mut used = 0u64;
    let mut inputs = store::Inputs::new();
    for file in &scan.files {
        let mut hash = blake3::Hasher::new();
        field(&mut hash, FINGERPRINT_VERSION.as_bytes());
        field(&mut hash, store::PATH_CODEC.as_bytes());
        hash.update(&INDEX_COMPAT_VERSION.to_le_bytes());
        hash.update(&max.to_le_bytes());
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
            used = used.saturating_add(file.size_bytes.saturating_add(1));
            if used > limit {
                return Err(CacheError::FreshnessLimit);
            }
            let bytes = source::load_raw(&scan.root, &file.relative_path, file.size_bytes)
                .map_err(|_| CacheError::Freshness)?;
            field(&mut hash, &bytes);
        }
        inputs.insert(
            file.relative_path.clone(),
            store::FileInput {
                size: file.size_bytes,
                stamp: *hash.finalize().as_bytes(),
            },
        );
    }
    Ok((generation(&inputs, max), inputs))
}
pub(crate) fn generation(inputs: &store::Inputs, max: u64) -> [u8; 32] {
    let mut hash = blake3::Hasher::new();
    field(&mut hash, FINGERPRINT_VERSION.as_bytes());
    field(&mut hash, store::PATH_CODEC.as_bytes());
    hash.update(&INDEX_COMPAT_VERSION.to_le_bytes());
    hash.update(&max.to_le_bytes());
    for (path, input) in inputs {
        field(&mut hash, &store::native_bytes(path));
        field(&mut hash, &input.stamp);
    }
    *hash.finalize().as_bytes()
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
pub struct Reconciled {
    pub artifact: AnalysisArtifact,
    pub warnings: Vec<String>,
    pub committed: bool,
}
pub fn reconcile(options: &IndexOptions, force: bool) -> Result<Reconciled, CacheError> {
    reconcile_with(
        options,
        force,
        None,
        |scan| {
            let (index, relation_files) =
                index::analyze_scan(scan, options.max_parse_file_bytes, true)?;
            Ok(AnalysisArtifact {
                index,
                relation_files,
            })
        },
        |scan, max| file_inputs(scan, max, MAX_FRESHNESS_BYTES),
    )
}
const UNVERIFIED: &str =
    "source freshness could not be established; returning uncached best-effort analysis";
fn reconcile_with<F, H>(
    options: &IndexOptions,
    force: bool,
    test_path: Option<&Path>,
    mut build: F,
    mut inputs_fn: H,
) -> Result<Reconciled, CacheError>
where
    F: FnMut(RepositoryScan) -> Result<AnalysisArtifact, CacheError>,
    H: FnMut(&RepositoryScan, u64) -> Result<([u8; 32], store::Inputs), CacheError>,
{
    for attempt in 0..2 {
        let mut before = scan_now(options)?;
        let (stamp, inputs) = match inputs_fn(&before, options.max_parse_file_bytes) {
            Ok(inputs) => inputs,
            Err(CacheError::Freshness) if !force => {
                return Ok(Reconciled {
                    artifact: build(before)?,
                    warnings: vec![UNVERIFIED.into()],
                    committed: false,
                });
            }
            Err(error) => return Err(error),
        };
        let path = test_path
            .map(|p| Ok(p.to_path_buf()))
            .unwrap_or_else(|| location(&before.root, options.max_parse_file_bytes));
        let snapshot = if force {
            None
        } else {
            path.as_ref().ok().and_then(|p| {
                store::load_snapshot(p, &before.root, options.max_parse_file_bytes).ok()
            })
        };
        let expected = snapshot.as_ref().map(|s| s.generation);
        let mut artifact = if let Some(mut snapshot) = snapshot {
            if snapshot.generation == stamp && snapshot.inputs == inputs {
                live_metadata(&mut snapshot.artifact, before);
                return Ok(Reconciled {
                    artifact: snapshot.artifact,
                    warnings: vec![],
                    committed: true,
                });
            }
            let unchanged: std::collections::BTreeSet<_> = inputs
                .iter()
                .filter(|(p, i)| snapshot.inputs.get(*p) == Some(*i))
                .map(|(p, _)| p.clone())
                .collect();
            before
                .files
                .retain(|f| !unchanged.contains(&f.relative_path));
            let mut changed = build(before)?;
            let old = &mut snapshot.artifact;
            old.index
                .files
                .retain(|f| unchanged.contains(&f.relative_path));
            old.index
                .symbols
                .retain(|f| unchanged.contains(&f.relative_path));
            old.index
                .diagnostics
                .retain(|f| unchanged.contains(&f.relative_path));
            old.relation_files
                .retain(|f| unchanged.contains(&f.relative_path));
            changed.index.files.append(&mut old.index.files);
            changed.index.symbols.append(&mut old.index.symbols);
            changed.index.diagnostics.append(&mut old.index.diagnostics);
            changed.relation_files.append(&mut old.relation_files);
            changed
        } else {
            build(before)?
        };
        index::validate_analysis(&artifact.index, &artifact.relation_files)?;
        if artifact
            .index
            .files
            .iter()
            .any(|f| f.status == AnalysisStatus::ReadFailed)
        {
            if force {
                return Err(CacheError::Freshness);
            }
            // The merged candidate may contain snapshot-retained units. Once a
            // changed-file read fails, discard the whole candidate and rebuild
            // only from a new live scan so no unverified cached evidence escapes.
            let live = scan_now(options)?;
            let mut artifact = build(live)?;
            index::validate_analysis(&artifact.index, &artifact.relation_files)?;
            live_metadata(&mut artifact, scan_now(options)?);
            return Ok(Reconciled {
                artifact,
                warnings: vec![UNVERIFIED.into()],
                committed: false,
            });
        }
        let after = scan_now(options)?;
        let (post, post_inputs) = match inputs_fn(&after, options.max_parse_file_bytes) {
            Ok(inputs) => inputs,
            Err(CacheError::Freshness) if !force => {
                // The failed post-pass may concern a retained file. Re-read live
                // evidence rather than returning now-unverified cached units.
                return Ok(Reconciled {
                    artifact: build(after)?,
                    warnings: vec![UNVERIFIED.into()],
                    committed: false,
                });
            }
            Err(error) => return Err(error),
        };
        if post != stamp || post_inputs != inputs || after.root != artifact.index.root {
            if attempt == 0 {
                continue;
            }
            return Err(CacheError::Changed);
        }
        live_metadata(&mut artifact, after);
        let saved = path.and_then(|path| {
            let _lock = store::lock_writers(&path)?;
            // The candidate was analyzed before acquiring the cross-mode writer
            // lock. Revalidate under it, then keep the lock through commit so a
            // stale full replacement cannot overtake a newer incremental patch.
            let locked_scan = scan_now(options)?;
            let (locked_stamp, locked_inputs) =
                inputs_fn(&locked_scan, options.max_parse_file_bytes)?;
            if locked_stamp != stamp || locked_inputs != inputs {
                return Err(CacheError::Changed);
            }
            if let Some(expected) = expected {
                store::patch_locked(
                    &path,
                    &expected,
                    &artifact,
                    options.max_parse_file_bytes,
                    &stamp,
                    &inputs,
                )?
            } else {
                store::save_snapshot_locked(
                    &path,
                    &artifact,
                    options.max_parse_file_bytes,
                    &stamp,
                    &inputs,
                )?
            }
            Ok(())
        });
        match saved {
            Err(CacheError::Changed) if attempt == 0 => continue,
            Err(error @ CacheError::Changed) => return Err(error),
            Err(CacheError::Store(store::StoreError::Conflict)) if attempt == 0 => continue,
            Err(error @ CacheError::Store(store::StoreError::Conflict)) => return Err(error),
            Err(error) if force => return Err(error),
            Err(CacheError::Freshness) => {
                let live = scan_now(options)?;
                let mut artifact = build(live)?;
                index::validate_analysis(&artifact.index, &artifact.relation_files)?;
                live_metadata(&mut artifact, scan_now(options)?);
                return Ok(Reconciled {
                    artifact,
                    warnings: vec![UNVERIFIED.into()],
                    committed: false,
                });
            }
            Err(error) => {
                return Ok(Reconciled {
                    artifact,
                    warnings: vec![error.to_string()],
                    committed: false,
                });
            }
            Ok(()) => {
                return Ok(Reconciled {
                    artifact,
                    warnings: vec![],
                    committed: true,
                });
            }
        }
    }
    unreachable!()
}
fn live_metadata(artifact: &mut AnalysisArtifact, scan: RepositoryScan) {
    artifact.index.git = scan.git;
    artifact.index.scan_warnings = scan.warnings;
    index::qualify_paths(&mut artifact.index);
    artifact.index.sort();
    artifact
        .relation_files
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
}
#[cfg(test)]
fn load_with<F>(
    options: &IndexOptions,
    force: bool,
    path: Option<&Path>,
    build: F,
) -> Result<AnalysisArtifact, CacheError>
where
    F: FnMut(RepositoryScan) -> Result<AnalysisArtifact, CacheError>,
{
    Ok(reconcile_with(options, force, path, build, |s, m| {
        file_inputs(s, m, MAX_FRESHNESS_BYTES)
    })?
    .artifact)
}
#[cfg(test)]
fn load_with_fingerprint<F, H>(
    options: &IndexOptions,
    force: bool,
    path: Option<&Path>,
    build: F,
    mut fingerprint: H,
) -> Result<AnalysisArtifact, CacheError>
where
    F: FnMut(RepositoryScan) -> Result<AnalysisArtifact, CacheError>,
    H: FnMut(&RepositoryScan, u64) -> Result<[u8; 32], CacheError>,
{
    Ok(reconcile_with(options, force, path, build, |s, m| {
        let stamp = fingerprint(s, m)?;
        Ok((stamp, file_inputs(s, m, MAX_FRESHNESS_BYTES)?.1))
    })?
    .artifact)
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
            ) || (cfg!(target_os = "macos") && error.raw_os_error() == Some(92))
            {
                // APFS rejects invalid UTF-8 with EILSEQ. Injected native-path
                // identity/store tests still run on these filesystems.
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
    fn checked_build(
        scan: RepositoryScan,
        parsed: &mut Vec<PathBuf>,
    ) -> Result<AnalysisArtifact, CacheError> {
        let (index, relation_files) = index::analyze_loaded(scan, 1024, |root, file, max| {
            parsed.push(file.relative_path.clone());
            source::load(root, &file.relative_path, max)
        })?;
        Ok(AnalysisArtifact {
            index,
            relation_files,
        })
    }
    #[test]
    fn file_delta_parser_work_and_full_graph_equivalence() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let path = cache.path().join("cache.sqlite3");
        let opts = options(root.path());
        std::fs::write(
            root.path().join("a.py"),
            "import os\ndef old():\n    old()\n",
        )
        .unwrap();
        std::fs::write(root.path().join("b.rs"), "fn retained() {}\n").unwrap();
        load_with(&opts, false, Some(&path), build).unwrap();
        // Detect any attempt to rewrite an unrelated ownership group.
        rusqlite::Connection::open(&path).unwrap().execute_batch(
            "CREATE TRIGGER untouched BEFORE DELETE ON files WHEN OLD.path=x'622e7273' BEGIN SELECT RAISE(ABORT,'rewrote retained file'); END;"
        ).unwrap();
        for step in 0..8 {
            let expected: &[&str] = match step {
                0 => &[],
                1 => {
                    std::fs::write(root.path().join("a.py"), "def replacement(): pass\n").unwrap();
                    &["a.py"]
                }
                2 => {
                    std::fs::write(root.path().join("c.py"), "def added(): pass\n").unwrap();
                    &["c.py"]
                }
                3 => {
                    std::fs::remove_file(root.path().join("a.py")).unwrap();
                    &[]
                }
                4 => {
                    std::fs::rename(root.path().join("c.py"), root.path().join("d.py")).unwrap();
                    &["d.py"]
                }
                5 => {
                    std::fs::create_dir(root.path().join("generated")).unwrap();
                    std::fs::write(root.path().join("generated/a.py"), "def skipped(): pass\n")
                        .unwrap();
                    std::fs::write(root.path().join("README.md"), "unsupported").unwrap();
                    std::fs::write(root.path().join("large.py"), vec![b' '; 1025]).unwrap();
                    &[]
                }
                6 => {
                    std::fs::write(root.path().join("large.py"), "def now_small(): pass\n")
                        .unwrap();
                    &["large.py"]
                }
                _ => {
                    std::fs::write(root.path().join("large.py"), vec![b' '; 1025]).unwrap();
                    &[]
                }
            };
            let mut parsed = vec![];
            let result = reconcile_with(
                &opts,
                false,
                Some(&path),
                |s| checked_build(s, &mut parsed),
                |s, m| file_inputs(s, m, MAX_FRESHNESS_BYTES),
            )
            .unwrap();
            assert!(result.committed, "{:?}", result.warnings);
            assert_eq!(
                parsed,
                expected.iter().map(PathBuf::from).collect::<Vec<_>>(),
                "step {step}"
            );
            let full = build(scan_now(&opts).unwrap()).unwrap();
            assert_eq!(
                crate::output::index_json("symbols", &result.artifact.index, None).unwrap(),
                crate::output::index_json("symbols", &full.index, None).unwrap()
            );
            let incremental_graph =
                crate::graph::from_analysis(result.artifact.index, result.artifact.relation_files)
                    .unwrap();
            let full_graph = crate::graph::from_analysis(full.index, full.relation_files).unwrap();
            assert_eq!(
                crate::output::graph_json("graph", &incremental_graph, None, None).unwrap(),
                crate::output::graph_json("graph", &full_graph, None, None).unwrap(),
                "step {step}"
            );
            let conn = rusqlite::Connection::open(&path).unwrap();
            let stale: i64 = conn
                .query_row(
                    "SELECT count(*) FROM symbols WHERE instr(CAST(payload AS TEXT),'old')>0",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(stale, if step == 0 { 1 } else { 0 });
            if step > 0 {
                let candidates: i64 = conn
                    .query_row("SELECT count(*) FROM candidates", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(candidates, 0);
            }
        }
    }
    #[test]
    fn incremental_mutation_checks_changed_and_previously_unchanged_files() {
        for mutate in ["a.py", "b.py"] {
            let root = tempfile::tempdir().unwrap();
            let cache = tempfile::tempdir().unwrap();
            let path = cache.path().join("cache.sqlite3");
            let opts = options(root.path());
            for file in ["a.py", "b.py"] {
                std::fs::write(root.path().join(file), "def old(): pass\n").unwrap();
            }
            load_with(&opts, false, Some(&path), build).unwrap();
            let prior = std::fs::read(&path).unwrap();
            std::fs::write(root.path().join("a.py"), "def changed(): pass\n").unwrap();
            let mut attempts = 0;
            let result = load_with(&opts, false, Some(&path), |s| {
                let a = build(s)?;
                attempts += 1;
                std::fs::write(
                    root.path().join(mutate),
                    format!("def mutation{attempts}(): pass\n"),
                )
                .unwrap();
                Ok(a)
            });
            assert!(matches!(result, Err(CacheError::Changed)));
            assert_eq!(attempts, 2);
            assert_eq!(prior, std::fs::read(&path).unwrap());
            let mut attempts = 0;
            let stable = load_with(&opts, false, Some(&path), |s| {
                let a = build(s)?;
                attempts += 1;
                if attempts == 1 {
                    std::fs::write(root.path().join(mutate), "def stable(): pass\n").unwrap();
                }
                Ok(a)
            })
            .unwrap();
            assert_eq!(attempts, 2);
            assert!(stable.index.symbols.iter().any(|s| s.name == "stable"));
        }
    }
    #[test]
    fn conflict_reloads_and_automatic_patch_failure_returns_fresh_result() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let path = cache.path().join("cache.sqlite3");
        let opts = options(root.path());
        std::fs::write(root.path().join("a.py"), "def old(): pass\n").unwrap();
        load_with(&opts, false, Some(&path), build).unwrap();
        let old = store::load_snapshot(&path, &opts.root.canonicalize().unwrap(), 1024).unwrap();
        std::fs::write(root.path().join("a.py"), "def newer(): pass\n").unwrap();
        let mut calls = 0;
        let result = load_with(&opts, false, Some(&path), |s| {
            calls += 1;
            assert_eq!(calls, 1, "retry should warm-load the other writer's result");
            let a = build(s)?;
            let (stamp, inputs) = file_inputs(&scan_now(&opts)?, 1024, MAX_FRESHNESS_BYTES)?;
            store::patch(&path, &old.generation, &a, 1024, &stamp, &inputs)?;
            Ok(a)
        })
        .unwrap();
        assert!(result.index.symbols.iter().any(|s| s.name == "newer"));
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_patch BEFORE INSERT ON symbols BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        drop(conn);
        let prior = std::fs::read(&path).unwrap();
        std::fs::write(root.path().join("a.py"), "def freshest(): pass\n").unwrap();
        let result = reconcile_with(&opts, false, Some(&path), build, |s, m| {
            file_inputs(s, m, MAX_FRESHNESS_BYTES)
        })
        .unwrap();
        assert!(!result.committed);
        assert!(!result.warnings.is_empty());
        assert!(
            result
                .artifact
                .index
                .symbols
                .iter()
                .any(|s| s.name == "freshest")
        );
        assert_eq!(prior, std::fs::read(&path).unwrap());
    }
    #[test]
    fn unknown_freshness_never_reuses_or_replaces_a_trusted_stale_base() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let path = cache.path().join("cache.sqlite3");
        let opts = options(root.path());
        std::fs::write(root.path().join("a.py"), "def old(): pass\n").unwrap();
        load_with(&opts, false, Some(&path), build).unwrap();
        let prior = std::fs::read(&path).unwrap();
        std::fs::write(root.path().join("a.py"), "def live(): pass\n").unwrap();
        let mut calls = 0;
        let result = reconcile_with(
            &opts,
            false,
            Some(&path),
            |s| {
                calls += 1;
                build(s)
            },
            |_, _| Err(CacheError::Freshness),
        )
        .unwrap();
        assert_eq!(calls, 1);
        assert!(!result.committed);
        assert!(
            result
                .artifact
                .index
                .symbols
                .iter()
                .any(|s| s.name == "live")
        );
        assert_eq!(prior, std::fs::read(&path).unwrap());
    }

    #[test]
    fn post_freshness_failure_reloads_live_instead_of_retaining_cached_units() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let path = cache.path().join("cache.sqlite3");
        let opts = options(root.path());
        for file in ["a.py", "b.py"] {
            std::fs::write(root.path().join(file), "def cached(): pass\n").unwrap();
        }
        load_with(&opts, false, Some(&path), build).unwrap();
        let prior = std::fs::read(&path).unwrap();
        std::fs::write(root.path().join("a.py"), "def changed(): pass\n").unwrap();
        let (mut builds, mut passes) = (0, 0);
        let result = reconcile_with(
            &opts,
            false,
            Some(&path),
            |scan| {
                builds += 1;
                let (index, relation_files) =
                    index::analyze_loaded(scan, 1024, |root, file, max| {
                        if builds == 2 && file.relative_path == Path::new("b.py") {
                            return Err(AnalysisStatus::ReadFailed);
                        }
                        source::load(root, &file.relative_path, max)
                    })?;
                Ok(AnalysisArtifact {
                    index,
                    relation_files,
                })
            },
            |scan, max| {
                passes += 1;
                if passes == 2 {
                    return Err(CacheError::Freshness);
                }
                file_inputs(scan, max, MAX_FRESHNESS_BYTES)
            },
        )
        .unwrap();
        assert_eq!(builds, 2);
        assert!(!result.committed);
        assert!(result.artifact.index.files.iter().any(
            |f| f.relative_path == Path::new("b.py") && f.status == AnalysisStatus::ReadFailed
        ));
        assert!(
            !result
                .artifact
                .index
                .symbols
                .iter()
                .any(|s| s.name == "cached")
        );
        assert_eq!(prior, std::fs::read(&path).unwrap());
    }

    #[test]
    fn incremental_analyzer_read_failure_discards_retained_units_and_rebuilds_live() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let path = cache.path().join("cache.sqlite3");
        let opts = options(root.path());
        std::fs::write(root.path().join("a.py"), "def old_a(): pass\n").unwrap();
        std::fs::write(root.path().join("b.py"), "def old_b(): pass\n").unwrap();
        load_with(&opts, false, Some(&path), build).unwrap();
        let prior = std::fs::read(&path).unwrap();
        let before =
            store::load_snapshot(&path, &std::fs::canonicalize(root.path()).unwrap(), 1024)
                .unwrap();
        std::fs::write(root.path().join("a.py"), "def new_a(): pass\n").unwrap();
        let (_, now) = file_inputs(&scan_now(&opts).unwrap(), 1024, MAX_FRESHNESS_BYTES).unwrap();
        assert_ne!(before.inputs, now);
        let mut builds = 0;
        let result = reconcile_with(
            &opts,
            false,
            Some(&path),
            |scan| {
                builds += 1;
                let (mut index, relation_files) = index::analyze_scan(scan, 1024, true)?;
                if builds == 1 {
                    let failed = index
                        .files
                        .iter_mut()
                        .find(|f| f.relative_path == Path::new("a.py"))
                        .unwrap();
                    failed.status = AnalysisStatus::ReadFailed;
                    failed.symbol_count = 0;
                    index
                        .symbols
                        .retain(|s| s.relative_path != Path::new("a.py"));
                    // This file was initially classified unchanged. It must not
                    // leak its old cached evidence through the failure fallback.
                    std::fs::write(root.path().join("b.py"), "def live_b(): pass\n").unwrap();
                }
                Ok(AnalysisArtifact {
                    index,
                    relation_files,
                })
            },
            |scan, max| file_inputs(scan, max, MAX_FRESHNESS_BYTES),
        )
        .unwrap();
        assert_eq!(builds, 2);
        assert!(!result.committed);
        assert!(
            result
                .artifact
                .index
                .symbols
                .iter()
                .any(|s| s.name == "live_b")
        );
        assert!(
            !result
                .artifact
                .index
                .symbols
                .iter()
                .any(|s| s.name == "old_b")
        );
        assert_eq!(prior, std::fs::read(&path).unwrap());
    }
}

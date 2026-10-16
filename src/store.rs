//! SQLite schema-v1. Native paths live in BLOB columns; each structural record
//! has an explicit named JSON codec. No graph IDs, operational metadata or bodies.
use crate::{
    model::*,
    persistence::{AnalysisArtifact, FINGERPRINT_VERSION, INDEX_COMPAT_VERSION},
    relations::*,
};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub const SCHEMA_VERSION: i64 = 1;
const MAX_DB_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PAYLOAD_BYTES: i64 = 32 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 1024 * 1024;
#[cfg(unix)]
pub const PATH_CODEC: &str = "unix_bytes_v1";
#[cfg(windows)]
pub const PATH_CODEC: &str = "windows_wide_le_v1";
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cache missing")]
    Missing,
    #[error("stale cache fingerprint")]
    Stale,
    #[error("incompatible cache schema, analysis version, or path codec")]
    Incompatible,
    #[error("invalid cache records or resource bounds")]
    Invalid,
    #[error("cache database I/O: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("cache filesystem I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid cache record codec: {0}")]
    Codec(#[from] serde_json::Error),
}
type Result<T> = std::result::Result<T, StoreError>;
#[cfg(unix)]
pub fn native_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}
#[cfg(windows)]
pub fn native_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}
fn native_path(bytes: Vec<u8>) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(std::ffi::OsString::from_vec(bytes).into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        if bytes.len() % 2 != 0 {
            return Err(StoreError::Invalid);
        }
        let wide: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        Ok(std::ffi::OsString::from_wide(&wide).into())
    }
}
fn valid_path(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
        // components normalizes interior '.', so additionally require canonical spelling.
        && path.components().collect::<PathBuf>() == path
        && !native_bytes(path).split(|b| *b == b'/').any(|b| b == b".")
}
fn setup(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(2))?;
    conn.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        2 * 1024 * 1024,
    )?;
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-2048;",
    )?;
    Ok(())
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_ROW_BYTES {
        return Err(StoreError::Invalid);
    }
    Ok(bytes)
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_ROW_BYTES {
        return Err(StoreError::Invalid);
    }
    Ok(serde_json::from_slice(bytes)?)
}
const TABLES: [(&str, i64); 5] = [
    ("files", 49_999),
    ("symbols", 50_000),
    ("diagnostics", 10_000),
    ("extractions", 49_999),
    ("candidates", 100_000),
];
fn preflight(conn: &Connection) -> Result<()> {
    preflight_with_limit(conn, MAX_PAYLOAD_BYTES)
}
fn preflight_with_limit(conn: &Connection, max_payload: i64) -> Result<()> {
    let mut total = 0i64;
    for (table, max) in TABLES {
        let sql = format!(
            "SELECT count(*),coalesce(sum(length(payload)),0),coalesce(max(length(payload)),0) FROM {table}"
        );
        let (count, bytes, largest): (i64, i64, i64) =
            conn.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        total = total.checked_add(bytes).ok_or(StoreError::Invalid)?;
        if count > max || bytes < 0 || largest > MAX_ROW_BYTES as i64 || total > max_payload {
            return Err(StoreError::Invalid);
        }
    }
    let paths: i64 =
        conn.query_row("SELECT coalesce(sum(length(path)),0) FROM files", [], |r| {
            r.get(0)
        })?;
    if paths > 4 * 1024 * 1024 {
        return Err(StoreError::Invalid);
    }
    Ok(())
}
pub fn save(path: &Path, artifact: &AnalysisArtifact, max: u64, stamp: &[u8; 32]) -> Result<()> {
    std::fs::create_dir_all(path.parent().ok_or(StoreError::Invalid)?)?;
    // Build a complete replacement beside the destination. Failed writes leave
    // the prior snapshot intact; open readers retain their original snapshot.
    let temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    {
        let mut conn = Connection::open(temp.path())?;
        setup(&conn)?;
        conn.execute_batch("PRAGMA max_page_count=32768; PRAGMA page_size=4096;")?;
        let tx = conn.transaction()?;
        tx.execute_batch("PRAGMA user_version=1;
            CREATE TABLE meta (id INTEGER PRIMARY KEY CHECK(id=1), compat INTEGER NOT NULL, fingerprint_version TEXT NOT NULL, codec TEXT NOT NULL, root BLOB NOT NULL, parse_limit TEXT NOT NULL, stamp BLOB NOT NULL);
            CREATE TABLE files (ordinal INTEGER PRIMARY KEY, path BLOB NOT NULL UNIQUE, payload BLOB NOT NULL);
            CREATE TABLE symbols (ordinal INTEGER PRIMARY KEY, file INTEGER NOT NULL REFERENCES files(ordinal), payload BLOB NOT NULL);
            CREATE TABLE diagnostics (ordinal INTEGER PRIMARY KEY, file INTEGER NOT NULL REFERENCES files(ordinal), payload BLOB NOT NULL);
            CREATE TABLE extractions (ordinal INTEGER PRIMARY KEY REFERENCES files(ordinal), payload BLOB NOT NULL);
            CREATE TABLE candidates (ordinal INTEGER PRIMARY KEY, file INTEGER NOT NULL REFERENCES extractions(ordinal), payload BLOB NOT NULL);")?;
        tx.execute(
            "INSERT INTO meta VALUES(1,?1,?2,?3,?4,?5,?6)",
            params![
                INDEX_COMPAT_VERSION,
                FINGERPRINT_VERSION,
                PATH_CODEC,
                native_bytes(&artifact.index.root),
                max.to_string(),
                stamp.as_slice()
            ],
        )?;
        let mut ids = BTreeMap::new();
        for (ordinal, file) in artifact.index.files.iter().enumerate() {
            let ordinal = i64::try_from(ordinal).map_err(|_| StoreError::Invalid)?;
            ids.insert(file.relative_path.as_path(), ordinal);
            tx.execute(
                "INSERT INTO files VALUES(?1,?2,?3)",
                params![ordinal, native_bytes(&file.relative_path), encode(file)?],
            )?;
        }
        for (ordinal, symbol) in artifact.index.symbols.iter().enumerate() {
            insert(
                &tx,
                "symbols",
                ordinal,
                *ids.get(symbol.relative_path.as_path())
                    .ok_or(StoreError::Invalid)?,
                symbol,
            )?;
        }
        for (ordinal, diagnostic) in artifact.index.diagnostics.iter().enumerate() {
            insert(
                &tx,
                "diagnostics",
                ordinal,
                *ids.get(diagnostic.relative_path.as_path())
                    .ok_or(StoreError::Invalid)?,
                diagnostic,
            )?;
        }
        let mut ordinal = 0;
        for file in &artifact.relation_files {
            let id = *ids
                .get(file.relative_path.as_path())
                .ok_or(StoreError::Invalid)?;
            tx.execute(
                "INSERT INTO extractions VALUES(?1,?2)",
                params![
                    id,
                    encode(&(file.extraction.status, file.extraction.unsupported))?
                ],
            )?;
            for candidate in &file.extraction.candidates {
                insert(&tx, "candidates", ordinal, id, candidate)?;
                ordinal += 1;
            }
        }
        preflight(&tx)?;
        tx.commit()?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| StoreError::Io(e.error))?;
    Ok(())
}
fn insert(
    conn: &Connection,
    table: &str,
    ordinal: usize,
    file: i64,
    value: &impl Serialize,
) -> Result<()> {
    let ordinal = i64::try_from(ordinal).map_err(|_| StoreError::Invalid)?;
    conn.execute(
        &format!("INSERT INTO {table} VALUES(?1,?2,?3)"),
        params![ordinal, file, encode(value)?],
    )?;
    Ok(())
}
pub fn load(
    path: &Path,
    scan: &RepositoryScan,
    max: u64,
    stamp: &[u8; 32],
) -> Result<AnalysisArtifact> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::Missing),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_file() || meta.len() > MAX_DB_BYTES {
        return Err(StoreError::Invalid);
    }
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    setup(&conn)?;
    let tx = conn.transaction()?;
    let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(StoreError::Incompatible);
    }
    let (compat, fingerprint, codec, root, limit, saved): (
        i64,
        String,
        String,
        Vec<u8>,
        String,
        Vec<u8>,
    ) = tx.query_row(
        "SELECT compat,fingerprint_version,codec,root,parse_limit,stamp FROM meta WHERE id=1",
        [],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )?;
    if compat != INDEX_COMPAT_VERSION
        || fingerprint != FINGERPRINT_VERSION
        || codec != PATH_CODEC
        || root != native_bytes(&scan.root)
        || limit != max.to_string()
    {
        return Err(StoreError::Incompatible);
    }
    if saved != stamp {
        return Err(StoreError::Stale);
    }
    preflight(&tx)?;
    let mut check = tx.prepare("PRAGMA foreign_key_check")?;
    if check.query([])?.next()?.is_some() {
        return Err(StoreError::Invalid);
    }
    let mut files = Vec::new();
    let mut statement = tx.prepare("SELECT ordinal,path,payload FROM files ORDER BY ordinal")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let ordinal: usize =
            usize::try_from(row.get::<_, i64>(0)?).map_err(|_| StoreError::Invalid)?;
        if ordinal != files.len() {
            return Err(StoreError::Invalid);
        }
        let path = native_path(row.get(1)?)?;
        let mut file: FileAnalysis =
            decode(row.get_ref(2)?.as_blob().map_err(|_| StoreError::Invalid)?)?;
        let current = scan.files.get(ordinal).ok_or(StoreError::Invalid)?;
        if !valid_path(&path)
            || path != current.relative_path
            || file.language != current.language
            || file.role != current.role
        {
            return Err(StoreError::Invalid);
        }
        file.relative_path = path;
        files.push(file);
    }
    if files.len() != scan.files.len() {
        return Err(StoreError::Invalid);
    }
    let mut index = RepositoryIndex {
        root: scan.root.clone(),
        git: None,
        scan_warnings: vec![],
        files,
        symbols: vec![],
        diagnostics: vec![],
    };
    let mut symbol_counts = vec![0usize; index.files.len()];
    let mut names = 0usize;
    let mut details = 0usize;
    read_records(&tx, "symbols", |id, bytes| {
        let file = index.files.get(id).ok_or(StoreError::Invalid)?;
        let mut s: Symbol = decode(bytes)?;
        if s.language != file.language || file.status != AnalysisStatus::Analyzed {
            return Err(StoreError::Invalid);
        }
        validate_range(s.range, scan.files[id].size_bytes)?;
        validate_range(s.name_range, scan.files[id].size_bytes)?;
        if s.name_range.start.byte < s.range.start.byte || s.name_range.end.byte > s.range.end.byte
        {
            return Err(StoreError::Invalid);
        }
        s.relative_path = file.relative_path.clone();
        names = names
            .saturating_add(s.name.len())
            .saturating_add(s.qualified_name.len());
        details = details.saturating_add(s.relative_path.as_os_str().len());
        if names > 8 * 1024 * 1024 || details > 16 * 1024 * 1024 {
            return Err(StoreError::Invalid);
        }
        symbol_counts[id] += 1;
        index.symbols.push(s);
        Ok(())
    })?;
    if index
        .files
        .iter()
        .zip(symbol_counts)
        .any(|(f, n)| f.symbol_count != n || f.syntax_error_count > 1_000_000)
        || index.files.len() + index.symbols.len() + 1 > crate::graph::MAX_ENTITIES
    {
        return Err(StoreError::Invalid);
    }
    read_records(&tx, "diagnostics", |id, bytes| {
        let mut d: AnalysisDiagnostic = decode(bytes)?;
        d.relative_path = index
            .files
            .get(id)
            .ok_or(StoreError::Invalid)?
            .relative_path
            .clone();
        if let Some(range) = d.range {
            validate_range(range, scan.files[id].size_bytes)?;
        }
        details = details
            .saturating_add(d.relative_path.as_os_str().len())
            .saturating_add(d.message.len());
        if details > 16 * 1024 * 1024 {
            return Err(StoreError::Invalid);
        }
        index.diagnostics.push(d);
        Ok(())
    })?;
    let mut extractions = BTreeMap::new();
    let mut statement = tx.prepare("SELECT ordinal,payload FROM extractions ORDER BY ordinal")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: usize = usize::try_from(row.get::<_, i64>(0)?).map_err(|_| StoreError::Invalid)?;
        if index.files.get(id).ok_or(StoreError::Invalid)?.status != AnalysisStatus::Analyzed {
            return Err(StoreError::Invalid);
        }
        let (status, unsupported): (RelationStatus, usize) =
            decode(row.get_ref(1)?.as_blob().map_err(|_| StoreError::Invalid)?)?;
        if unsupported > 1_000_000 {
            return Err(StoreError::Invalid);
        }
        extractions.insert(
            id,
            Extraction {
                status,
                unsupported,
                candidates: vec![],
            },
        );
    }
    if index
        .files
        .iter()
        .enumerate()
        .any(|(id, f)| (f.status == AnalysisStatus::Analyzed) != extractions.contains_key(&id))
    {
        return Err(StoreError::Invalid);
    }
    let mut relation_bytes = index
        .files
        .iter()
        .map(|f| f.relative_path.as_os_str().len())
        .sum::<usize>()
        + index
            .symbols
            .iter()
            .map(|s| s.relative_path.as_os_str().len())
            .sum::<usize>();
    let mut relation_count = index.files.len() + index.symbols.len();
    read_records(&tx, "candidates", |id, bytes| {
        let c: Candidate = decode(bytes)?;
        validate_range(
            c.range,
            scan.files.get(id).ok_or(StoreError::Invalid)?.size_bytes,
        )?;
        let extraction = extractions.get_mut(&id).ok_or(StoreError::Invalid)?;
        if extraction.status != RelationStatus::Analyzed
            || !matches!(
                (&c.kind, &c.metadata, &c.derivation),
                (
                    RelationKind::Imports,
                    Metadata::Import { .. },
                    Derivation::RustUseSyntax | Derivation::PythonImportSyntax
                ) | (
                    RelationKind::References,
                    Metadata::Reference { .. },
                    Derivation::RustCallSyntax | Derivation::PythonCallSyntax
                )
            )
        {
            return Err(StoreError::Invalid);
        }
        relation_bytes =
            relation_bytes.saturating_add(candidate_bytes(&c, &index.files[id].relative_path));
        relation_count += 1;
        if relation_bytes > crate::graph::MAX_RELATION_BYTES
            || relation_count > crate::graph::MAX_RELATIONS
        {
            return Err(StoreError::Invalid);
        }
        extraction.candidates.push(c);
        Ok(())
    })?;
    if relation_bytes > crate::graph::MAX_RELATION_BYTES {
        return Err(StoreError::Invalid);
    }
    let relation_files = extractions
        .into_iter()
        .map(|(id, extraction)| FileRelations {
            relative_path: index.files[id].relative_path.clone(),
            extraction,
        })
        .collect();
    // Current scan metadata is authoritative even on a warm structural hit.
    index.git = scan.git.as_ref().map(|g| GitMetadata {
        repository_root: g.repository_root.clone(),
        head_commit: g.head_commit.clone(),
        branch: g.branch.clone(),
        is_dirty: g.is_dirty,
    });
    index.scan_warnings = scan
        .warnings
        .iter()
        .map(|w| ScanWarning {
            path: w.path.clone(),
            message: w.message.clone(),
        })
        .collect();
    crate::index::qualify_paths(&mut index);
    index.sort();
    Ok(AnalysisArtifact {
        index,
        relation_files,
    })
}
fn read_records(
    conn: &Connection,
    table: &str,
    mut consume: impl FnMut(usize, &[u8]) -> Result<()>,
) -> Result<()> {
    let mut statement = conn.prepare(&format!(
        "SELECT ordinal,file,payload FROM {table} ORDER BY ordinal"
    ))?;
    let mut rows = statement.query([])?;
    let mut expected = 0usize;
    while let Some(row) = rows.next()? {
        let ordinal: usize =
            usize::try_from(row.get::<_, i64>(0)?).map_err(|_| StoreError::Invalid)?;
        if ordinal != expected {
            return Err(StoreError::Invalid);
        }
        expected += 1;
        consume(
            usize::try_from(row.get::<_, i64>(1)?).map_err(|_| StoreError::Invalid)?,
            row.get_ref(2)?.as_blob().map_err(|_| StoreError::Invalid)?,
        )?;
    }
    Ok(())
}
fn validate_range(range: SourceRange, size: u64) -> Result<()> {
    if range.start.byte > range.end.byte
        || u64::try_from(range.end.byte).map_err(|_| StoreError::Invalid)? > size
        || range.start.row > range.end.row
        || (range.start.row == range.end.row && range.start.column > range.end.column)
        || range.start.row > range.start.byte
        || range.end.row > range.end.byte
        || range.start.column > range.start.byte
        || range.end.column > range.end.byte
    {
        return Err(StoreError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path) -> (RepositoryScan, AnalysisArtifact) {
        std::fs::write(
            root.join("a.py"),
            "import os as alias\nfrom x import *\ndef one():\n    one()\n\ndef broken(\n",
        )
        .unwrap();
        std::fs::write(root.join("Makefile"), "all:\n").unwrap();
        let scan = || {
            crate::scan::scan_repository(&ScanOptions {
                root: root.into(),
                count_lines: false,
                max_line_count_file_bytes: 0,
                probe_git: false,
            })
            .unwrap()
        };
        let (index, relation_files) = crate::index::analyze_scan(scan(), 1024, true).unwrap();
        (
            scan(),
            AnalysisArtifact {
                index,
                relation_files,
            },
        )
    }
    #[test]
    fn round_trip_and_transaction_failure_preserves_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (scan, mut artifact) = fixture(root.path());
        let path = cache.path().join("cache.sqlite3");
        save(&path, &artifact, 1024, &[1; 32]).unwrap();
        let warm = load(&path, &scan, 1024, &[1; 32]).unwrap();
        assert_eq!(
            crate::output::index_json("index", &artifact.index, None).unwrap(),
            crate::output::index_json("index", &warm.index, None).unwrap()
        );
        assert!(matches!(
            warm.index.files[0].language,
            crate::classify::Language::Other(_)
        ));
        assert!(!warm.index.diagnostics.is_empty());
        assert_eq!(
            serde_json::to_vec(&artifact.relation_files[0].extraction).unwrap(),
            serde_json::to_vec(&warm.relation_files[0].extraction).unwrap()
        );
        let prior = std::fs::read(&path).unwrap();
        artifact.index.symbols[0].name = "a".repeat(MAX_ROW_BYTES + 1);
        assert!(save(&path, &artifact, 1024, &[2; 32]).is_err());
        assert_eq!(prior, std::fs::read(&path).unwrap());
        for metadata in [
            Metadata::None,
            Metadata::Import {
                binding: None,
                wildcard: true,
            },
            Metadata::Import {
                binding: Some("alias".into()),
                wildcard: false,
            },
            Metadata::Reference {
                context: ReferenceContext::Call,
            },
        ] {
            assert_eq!(
                metadata,
                decode::<Metadata>(&encode(&metadata).unwrap()).unwrap()
            );
        }
    }
    #[test]
    fn reject_versions_unknown_codecs_ordinals_foreign_keys_ranges_and_limits() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (scan, artifact) = fixture(root.path());
        let path = cache.path().join("cache.sqlite3");
        for sql in [
            "PRAGMA user_version=9",
            "UPDATE meta SET codec='unknown'",
            "UPDATE meta SET compat=99",
            "UPDATE files SET payload=replace(cast(payload as text),'analyzed','future_status')",
            "UPDATE symbols SET ordinal=999 WHERE ordinal=0",
            "UPDATE symbols SET file=999 WHERE ordinal=0",
            "UPDATE files SET path=x'2e2e2f612e7079' WHERE ordinal=1",
            "UPDATE symbols SET payload=replace(cast(payload as text),'function','mystery')",
            "UPDATE symbols SET payload=json_set(cast(payload as text),'$.range.end.byte',999999) WHERE ordinal=0",
            "DELETE FROM extractions",
        ] {
            save(&path, &artifact, 1024, &[1; 32]).unwrap();
            Connection::open(&path)
                .unwrap()
                .execute_batch(&format!("PRAGMA foreign_keys=OFF; {sql}"))
                .unwrap();
            assert!(
                load(&path, &scan, 1024, &[1; 32]).is_err(),
                "accepted {sql}"
            );
        }
        save(&path, &artifact, 1024, &[1; 32]).unwrap();
        let conn = Connection::open(&path).unwrap();
        assert!(preflight_with_limit(&conn, 1).is_err());
        assert!(matches!(
            load(&path, &scan, 1024, &[2; 32]),
            Err(StoreError::Stale)
        ));
        assert!(matches!(
            load(&path, &scan, 100, &[1; 32]),
            Err(StoreError::Incompatible)
        ));
        drop(conn);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_DB_BYTES + 1)
            .unwrap();
        assert!(matches!(
            load(&path, &scan, 1024, &[1; 32]),
            Err(StoreError::Invalid)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn native_non_utf8_path_round_trips_in_database() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (mut scan, mut artifact) = fixture(root.path());
        // Inject native inventory records; filesystem creation is not needed for
        // the store codec and can be restricted by desktop sandbox providers.
        let path: PathBuf = std::ffi::OsString::from_vec(b"native-\xff.py".to_vec()).into();
        scan.files[1].relative_path = path.clone();
        artifact.index.files[1].relative_path = path.clone();
        for s in &mut artifact.index.symbols {
            s.relative_path = path.clone();
        }
        for d in &mut artifact.index.diagnostics {
            d.relative_path = path.clone();
        }
        artifact.relation_files[0].relative_path = path.clone();
        let db = cache.path().join("cache.sqlite3");
        save(&db, &artifact, 1024, &[1; 32]).unwrap();
        let loaded = load(&db, &scan, 1024, &[1; 32]).unwrap();
        assert_eq!(loaded.index.files[1].relative_path, path);
        assert_eq!(loaded.index.symbols[0].relative_path, path);
        assert_eq!(loaded.relation_files[0].relative_path, path);
    }
}

//! SQLite schema-v3. Native paths live in BLOB columns; each structural record
//! has an explicit named JSON codec. No graph IDs, operational metadata or bodies.
use crate::{
    model::*,
    persistence::{AnalysisArtifact, FINGERPRINT_VERSION, INDEX_COMPAT_VERSION},
    relations::*,
};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub const SCHEMA_VERSION: i64 = 3;
const MAX_DB_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PAYLOAD_BYTES: i64 = 32 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 1024 * 1024;
const ARTIFACT_INTEGRITY_VERSION: &str = "blake3-rows-v2";
#[cfg(unix)]
pub const PATH_CODEC: &str = "unix_bytes_v1";
#[cfg(windows)]
pub const PATH_CODEC: &str = "windows_wide_le_v1";
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cache missing")]
    Missing,
    #[error("store generation conflict; reload and reconcile")]
    Conflict,
    #[cfg(test)]
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileInput {
    pub size: u64,
    pub stamp: [u8; 32],
}
pub type Inputs = BTreeMap<PathBuf, FileInput>;
pub struct Snapshot {
    pub artifact: AnalysisArtifact,
    pub inputs: Inputs,
    pub generation: [u8; 32],
}
struct CacheMetadata {
    compat: i64,
    fingerprint: String,
    codec: String,
    root: Vec<u8>,
    limit: String,
    stamp: Vec<u8>,
    integrity_version: String,
    artifact_digest: Vec<u8>,
}
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
    let (input_bytes, largest): (i64, i64) = conn.query_row(
        "SELECT coalesce(sum(length(input)),0),coalesce(max(length(input)),0) FROM files",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if largest > MAX_ROW_BYTES as i64 || total.saturating_add(input_bytes) > max_payload {
        return Err(StoreError::Invalid);
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
fn file_id(path: &Path) -> i64 {
    let digest = blake3::hash(&native_bytes(path));
    i64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap()) & i64::MAX
}
fn digest_field(hash: &mut blake3::Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn artifact_digest(conn: &Connection) -> Result<Vec<u8>> {
    let mut hash = blake3::Hasher::new();
    digest_field(&mut hash, ARTIFACT_INTEGRITY_VERSION.as_bytes());
    for (table, query) in [
        (
            "files",
            "SELECT id,path,input,payload FROM files ORDER BY id",
        ),
        (
            "symbols",
            "SELECT file,ordinal,payload FROM symbols ORDER BY file,ordinal",
        ),
        (
            "diagnostics",
            "SELECT file,ordinal,payload FROM diagnostics ORDER BY file,ordinal",
        ),
        (
            "extractions",
            "SELECT file,payload FROM extractions ORDER BY file",
        ),
        (
            "candidates",
            "SELECT file,ordinal,payload FROM candidates ORDER BY file,ordinal",
        ),
    ] {
        digest_field(&mut hash, table.as_bytes());
        let mut stmt = conn.prepare(query)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let columns = match table {
                "files" => 4,
                "extractions" => 2,
                _ => 3,
            };
            for col in 0..columns {
                if col == columns - 1 {
                    let blob = row
                        .get_ref(col)?
                        .as_blob()
                        .map_err(|_| StoreError::Invalid)?;
                    digest_field(&mut hash, blob);
                } else if table == "files" && (col == 1 || col == 2) {
                    let blob = row
                        .get_ref(col)?
                        .as_blob()
                        .map_err(|_| StoreError::Invalid)?;
                    digest_field(&mut hash, blob);
                } else {
                    let value: i64 = row.get(col)?;
                    digest_field(&mut hash, &value.to_le_bytes());
                }
            }
        }
    }
    Ok(hash.finalize().as_bytes().to_vec())
}
pub fn save_snapshot(
    path: &Path,
    artifact: &AnalysisArtifact,
    max: u64,
    stamp: &[u8; 32],
    inputs: &Inputs,
) -> Result<()> {
    std::fs::create_dir_all(path.parent().ok_or(StoreError::Invalid)?)?;
    // Build a complete replacement beside the destination. Failed writes leave
    // the prior snapshot intact; open readers retain their original snapshot.
    let temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    {
        let mut conn = Connection::open(temp.path())?;
        setup(&conn)?;
        conn.execute_batch("PRAGMA max_page_count=32768; PRAGMA page_size=4096;")?;
        let tx = conn.transaction()?;
        tx.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION};
            CREATE TABLE meta (id INTEGER PRIMARY KEY CHECK(id=1), compat INTEGER NOT NULL, fingerprint_version TEXT NOT NULL, codec TEXT NOT NULL, root BLOB NOT NULL, parse_limit TEXT NOT NULL, stamp BLOB NOT NULL, integrity_version TEXT NOT NULL, artifact_digest BLOB NOT NULL);
            CREATE TABLE files (id INTEGER PRIMARY KEY, path BLOB NOT NULL UNIQUE, input BLOB NOT NULL, payload BLOB NOT NULL);
            CREATE TABLE symbols (file INTEGER NOT NULL REFERENCES files(id), ordinal INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(file,ordinal));
            CREATE TABLE diagnostics (file INTEGER NOT NULL REFERENCES files(id), ordinal INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(file,ordinal));
            CREATE TABLE extractions (file INTEGER PRIMARY KEY REFERENCES files(id), payload BLOB NOT NULL);
            CREATE TABLE candidates (file INTEGER NOT NULL REFERENCES extractions(file), ordinal INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(file,ordinal));"))?;
        tx.execute(
            "INSERT INTO meta VALUES(1,?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                INDEX_COMPAT_VERSION,
                FINGERPRINT_VERSION,
                PATH_CODEC,
                native_bytes(&artifact.index.root),
                max.to_string(),
                stamp.as_slice(),
                ARTIFACT_INTEGRITY_VERSION,
                Vec::<u8>::new()
            ],
        )?;
        write_records(&tx, artifact, inputs, None)?;
        preflight(&tx)?;
        let digest = artifact_digest(&tx)?;
        tx.execute("UPDATE meta SET artifact_digest=?1 WHERE id=1", [digest])?;
        read_snapshot(&tx, &artifact.index.root, max)?;
        tx.commit()?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| StoreError::Io(e.error))?;
    Ok(())
}
fn write_records(
    conn: &Connection,
    artifact: &AnalysisArtifact,
    inputs: &Inputs,
    selected: Option<&BTreeSet<PathBuf>>,
) -> Result<()> {
    let mut ids = BTreeMap::new();
    let mut used_ids = BTreeSet::new();
    for file in &artifact.index.files {
        if selected.is_some_and(|s| !s.contains(&file.relative_path)) {
            continue;
        }
        let id = file_id(&file.relative_path);
        if !used_ids.insert(id) {
            return Err(StoreError::Invalid);
        }
        ids.insert(file.relative_path.as_path(), id);
        conn.execute(
            "INSERT INTO files VALUES(?1,?2,?3,?4)",
            params![
                id,
                native_bytes(&file.relative_path),
                encode(inputs.get(&file.relative_path).ok_or(StoreError::Invalid)?)?,
                encode(file)?
            ],
        )?;
    }
    let mut ordinals = BTreeMap::<i64, i64>::new();
    for symbol in &artifact.index.symbols {
        if selected.is_some_and(|s| !s.contains(&symbol.relative_path)) {
            continue;
        }
        let file = *ids
            .get(symbol.relative_path.as_path())
            .ok_or(StoreError::Invalid)?;
        let ordinal = ordinals.entry(file).or_default();
        insert(conn, "symbols", *ordinal as usize, file, symbol)?;
        *ordinal += 1;
    }
    ordinals.clear();
    for diagnostic in &artifact.index.diagnostics {
        if selected.is_some_and(|s| !s.contains(&diagnostic.relative_path)) {
            continue;
        }
        let file = *ids
            .get(diagnostic.relative_path.as_path())
            .ok_or(StoreError::Invalid)?;
        let ordinal = ordinals.entry(file).or_default();
        insert(conn, "diagnostics", *ordinal as usize, file, diagnostic)?;
        *ordinal += 1;
    }
    for file in &artifact.relation_files {
        if selected.is_some_and(|s| !s.contains(&file.relative_path)) {
            continue;
        }
        let id = *ids
            .get(file.relative_path.as_path())
            .ok_or(StoreError::Invalid)?;
        conn.execute(
            "INSERT INTO extractions VALUES(?1,?2)",
            params![
                id,
                encode(&(file.extraction.status, file.extraction.unsupported))?
            ],
        )?;
        for (ordinal, candidate) in file.extraction.candidates.iter().enumerate() {
            insert(conn, "candidates", ordinal, id, candidate)?;
        }
    }
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
        params![file, ordinal, encode(value)?],
    )?;
    Ok(())
}
pub fn load_snapshot(path: &Path, root: &Path, max: u64) -> Result<Snapshot> {
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
    read_snapshot(&tx, root, max)
}
fn read_snapshot(tx: &Connection, root: &Path, max: u64) -> Result<Snapshot> {
    let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(StoreError::Incompatible);
    }
    let metadata: CacheMetadata = tx.query_row(
        "SELECT compat,fingerprint_version,codec,root,parse_limit,stamp,integrity_version,artifact_digest FROM meta WHERE id=1",
        [],
        |r| {
            Ok(CacheMetadata {
                compat: r.get(0)?,
                fingerprint: r.get(1)?,
                codec: r.get(2)?,
                root: r.get(3)?,
                limit: r.get(4)?,
                stamp: r.get(5)?,
                integrity_version: r.get(6)?,
                artifact_digest: r.get(7)?,
            })
        },
    )?;
    if metadata.compat != INDEX_COMPAT_VERSION
        || metadata.fingerprint != FINGERPRINT_VERSION
        || metadata.codec != PATH_CODEC
        || metadata.root != native_bytes(root)
        || metadata.limit != max.to_string()
    {
        return Err(StoreError::Incompatible);
    }
    let generation: [u8; 32] = metadata.stamp.try_into().map_err(|_| StoreError::Invalid)?;
    preflight(tx)?;
    if metadata.integrity_version != ARTIFACT_INTEGRITY_VERSION
        || artifact_digest(tx)? != metadata.artifact_digest
    {
        return Err(StoreError::Invalid);
    }
    let mut check = tx.prepare("PRAGMA foreign_key_check")?;
    if check.query([])?.next()?.is_some() {
        return Err(StoreError::Invalid);
    }
    let mut inputs = Inputs::new();
    let mut files = vec![];
    let mut storage_ids = BTreeMap::new();
    let mut statement = tx.prepare("SELECT id,path,input,payload FROM files ORDER BY path")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let path = native_path(row.get(1)?)?;
        let input: FileInput = decode(row.get_ref(2)?.as_blob().map_err(|_| StoreError::Invalid)?)?;
        let mut file: FileAnalysis =
            decode(row.get_ref(3)?.as_blob().map_err(|_| StoreError::Invalid)?)?;
        if !valid_path(&path) || id != file_id(&path) {
            return Err(StoreError::Invalid);
        }
        file.relative_path = path.clone();
        inputs.insert(path, input);
        storage_ids.insert(id, files.len());
        files.push(file);
    }
    let mut index = RepositoryIndex {
        root: root.to_path_buf(),
        git: None,
        scan_warnings: vec![],
        files,
        symbols: vec![],
        diagnostics: vec![],
    };
    let mut symbol_counts = vec![0usize; index.files.len()];
    let mut names = 0usize;
    let mut details = 0usize;
    read_records(tx, "symbols", &storage_ids, |id, bytes| {
        let file = index.files.get(id).ok_or(StoreError::Invalid)?;
        let mut s: Symbol = decode(bytes)?;
        if s.language != file.language || file.status != AnalysisStatus::Analyzed {
            return Err(StoreError::Invalid);
        }
        validate_range(s.range, inputs[&index.files[id].relative_path].size)?;
        validate_range(s.name_range, inputs[&index.files[id].relative_path].size)?;
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
    read_records(tx, "diagnostics", &storage_ids, |id, bytes| {
        let mut d: AnalysisDiagnostic = decode(bytes)?;
        d.relative_path = index
            .files
            .get(id)
            .ok_or(StoreError::Invalid)?
            .relative_path
            .clone();
        if let Some(range) = d.range {
            validate_range(range, inputs[&index.files[id].relative_path].size)?;
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
    let mut statement = tx.prepare("SELECT file,payload FROM extractions ORDER BY file")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let storage_id: i64 = row.get(0)?;
        let id = *storage_ids.get(&storage_id).ok_or(StoreError::Invalid)?;
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
    read_records(tx, "candidates", &storage_ids, |id, bytes| {
        let c: Candidate = decode(bytes)?;
        validate_range(
            c.range,
            inputs[&index
                .files
                .get(id)
                .ok_or(StoreError::Invalid)?
                .relative_path]
                .size,
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
    let relation_files: Vec<_> = extractions
        .into_iter()
        .map(|(id, extraction)| FileRelations {
            relative_path: index.files[id].relative_path.clone(),
            extraction,
        })
        .collect();
    if crate::persistence::generation(&inputs, max) != generation {
        return Err(StoreError::Invalid);
    }
    crate::index::validate_analysis(&index, &relation_files).map_err(|_| StoreError::Invalid)?;
    index.sort();
    Ok(Snapshot {
        artifact: AnalysisArtifact {
            index,
            relation_files,
        },
        inputs,
        generation,
    })
}

/// Compare-and-swap a trusted snapshot under a SQLite write reservation. Only
/// changed/deleted ownership groups are touched; any error rolls the transaction back.
pub fn patch(
    path: &Path,
    expected: &[u8; 32],
    artifact: &AnalysisArtifact,
    max: u64,
    stamp: &[u8; 32],
    inputs: &Inputs,
) -> Result<()> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    setup(&conn)?;
    conn.execute_batch("PRAGMA max_page_count=32768;")?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let old = read_snapshot(&tx, &artifact.index.root, max)?;
    if &old.generation != expected {
        return Err(StoreError::Conflict);
    }
    let changed: BTreeSet<_> = inputs
        .iter()
        .filter(|(p, i)| old.inputs.get(*p) != Some(*i))
        .map(|(p, _)| p.clone())
        .collect();
    for path in old
        .inputs
        .keys()
        .filter(|p| !inputs.contains_key(*p) || changed.contains(*p))
    {
        let id = file_id(path);
        for table in ["candidates", "extractions", "symbols", "diagnostics"] {
            tx.execute(&format!("DELETE FROM {table} WHERE file=?1"), [id])?;
        }
        tx.execute("DELETE FROM files WHERE id=?1", [id])?;
    }
    write_records(&tx, artifact, inputs, Some(&changed))?;
    preflight(&tx)?;
    tx.execute(
        "UPDATE meta SET stamp=?1, artifact_digest=?2 WHERE id=1",
        params![stamp.as_slice(), artifact_digest(&tx)?],
    )?;
    read_snapshot(&tx, &artifact.index.root, max)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
fn save(path: &Path, artifact: &AnalysisArtifact, max: u64, stamp: &[u8; 32]) -> Result<()> {
    // Store codec tests inject native paths without creating source files.
    let inputs = artifact
        .index
        .files
        .iter()
        .map(|f| {
            (
                f.relative_path.clone(),
                FileInput {
                    size: max,
                    stamp: *stamp,
                },
            )
        })
        .collect();
    save_snapshot(
        path,
        artifact,
        max,
        &crate::persistence::generation(&inputs, max),
        &inputs,
    )
}
#[cfg(test)]
fn load(
    path: &Path,
    scan: &RepositoryScan,
    max: u64,
    stamp: &[u8; 32],
) -> Result<AnalysisArtifact> {
    let snapshot = load_snapshot(path, &scan.root, max)?;
    if snapshot.inputs.values().any(|i| &i.stamp != stamp) {
        return Err(StoreError::Stale);
    }
    let mut artifact = snapshot.artifact;
    artifact.index.git = scan.git.as_ref().map(|g| GitMetadata {
        repository_root: g.repository_root.clone(),
        head_commit: g.head_commit.clone(),
        branch: g.branch.clone(),
        is_dirty: g.is_dirty,
    });
    artifact.index.scan_warnings = scan
        .warnings
        .iter()
        .map(|w| ScanWarning {
            path: w.path.clone(),
            message: w.message.clone(),
        })
        .collect();
    crate::index::qualify_paths(&mut artifact.index);
    Ok(artifact)
}

fn read_records(
    conn: &Connection,
    table: &str,
    storage_ids: &BTreeMap<i64, usize>,
    mut consume: impl FnMut(usize, &[u8]) -> Result<()>,
) -> Result<()> {
    let mut statement = conn.prepare(&format!(
        "SELECT file,ordinal,payload FROM {table} ORDER BY file,ordinal"
    ))?;
    let mut rows = statement.query([])?;
    let mut active_file = None;
    let mut expected = 0usize;
    while let Some(row) = rows.next()? {
        let storage_id: i64 = row.get(0)?;
        let ordinal: usize =
            usize::try_from(row.get::<_, i64>(1)?).map_err(|_| StoreError::Invalid)?;
        if active_file != Some(storage_id) {
            active_file = Some(storage_id);
            expected = 0;
        }
        if ordinal != expected {
            return Err(StoreError::Invalid);
        }
        expected += 1;
        let file = *storage_ids.get(&storage_id).ok_or(StoreError::Invalid)?;
        consume(
            file,
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
        assert_eq!(
            Connection::open(&path)
                .unwrap()
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
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
            "PRAGMA user_version=1",
            "PRAGMA user_version=2",
            "PRAGMA user_version=9",
            "UPDATE meta SET codec='unknown'",
            "UPDATE meta SET compat=99",
            "UPDATE meta SET stamp=zeroblob(32)",
            "UPDATE files SET payload=replace(cast(payload as text),'analyzed','future_status')",
            "UPDATE symbols SET ordinal=999 WHERE ordinal=0",
            "UPDATE symbols SET file=999 WHERE ordinal=0",
            "UPDATE files SET path=x'2e2e2f612e7079' WHERE path=x'612e7079'",
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
    #[test]
    fn file_keys_and_child_ordinals_are_file_local() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let db = cache.path().join("cache.sqlite3");
        let (_scan, artifact) = fixture(root.path());
        save(&db, &artifact, 1024, &[1; 32]).unwrap();
        let conn = Connection::open(&db).unwrap();
        let path_bytes = native_bytes(Path::new("a.py"));
        let original_id: i64 = conn
            .query_row("SELECT id FROM files WHERE path=?1", [path_bytes], |r| {
                r.get(0)
            })
            .unwrap();
        let original_ordinals: Vec<i64> = {
            let mut s = conn
                .prepare("SELECT ordinal FROM symbols WHERE file=?1 ORDER BY ordinal")
                .unwrap();
            s.query_map([original_id], |r| r.get(0))
                .unwrap()
                .map(|result| result.unwrap())
                .collect()
        };
        drop(conn);
        std::fs::write(root.path().join("0.py"), "def earlier(): pass\n").unwrap();
        let scan2 = crate::scan::scan_repository(&ScanOptions {
            root: root.path().into(),
            count_lines: false,
            max_line_count_file_bytes: 0,
            probe_git: false,
        })
        .unwrap();
        let (index, relation_files) = crate::index::analyze_scan(
            crate::scan::scan_repository(&ScanOptions {
                root: root.path().into(),
                count_lines: false,
                max_line_count_file_bytes: 0,
                probe_git: false,
            })
            .unwrap(),
            1024,
            true,
        )
        .unwrap();
        save(
            &db,
            &AnalysisArtifact {
                index,
                relation_files,
            },
            1024,
            &[2; 32],
        )
        .unwrap();
        let conn = Connection::open(&db).unwrap();
        let added_id: i64 = conn
            .query_row(
                "SELECT id FROM files WHERE path=?1",
                [native_bytes(Path::new("a.py"))],
                |r| r.get(0),
            )
            .unwrap();
        let ordinals: Vec<i64> = {
            let mut s = conn
                .prepare("SELECT ordinal FROM symbols WHERE file=?1 ORDER BY ordinal")
                .unwrap();
            s.query_map([added_id], |r| r.get(0))
                .unwrap()
                .map(|result| result.unwrap())
                .collect()
        };
        assert_eq!(added_id, original_id);
        assert_eq!(ordinals, original_ordinals);
        let loaded = load(&db, &scan2, 1024, &[2; 32]).unwrap();
        assert_eq!(loaded.index.files[0].relative_path, Path::new("0.py"));
        assert_eq!(loaded.index.files[1].relative_path, Path::new("Makefile"));
        assert_eq!(loaded.index.files[2].relative_path, Path::new("a.py"));
    }
    #[test]
    fn valid_json_payload_mutation_fails_artifact_integrity() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (scan, artifact) = fixture(root.path());
        let db = cache.path().join("cache.sqlite3");
        save(&db, &artifact, 1024, &[1; 32]).unwrap();
        let conn = Connection::open(&db).unwrap();
        let changed = conn.execute(
            "UPDATE symbols SET payload=CAST(replace(cast(payload as text),'one','forged') AS BLOB) WHERE ordinal=0",
                [],
            )
            .unwrap();
        assert_eq!(changed, 1);
        let saved_digest: Vec<u8> = conn
            .query_row("SELECT artifact_digest FROM meta", [], |r| r.get(0))
            .unwrap();
        assert_ne!(artifact_digest(&conn).unwrap(), saved_digest);
        assert!(matches!(
            load(&db, &scan, 1024, &[1; 32]),
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
        let snapshot = load_snapshot(&db, &scan.root, 1024).unwrap();
        let mut inputs = snapshot.inputs;
        inputs.get_mut(&path).unwrap().stamp = [2; 32];
        artifact.index.symbols[0].name = "updated_native".into();
        let generation = crate::persistence::generation(&inputs, 1024);
        patch(
            &db,
            &snapshot.generation,
            &artifact,
            1024,
            &generation,
            &inputs,
        )
        .unwrap();
        let updated = load_snapshot(&db, &scan.root, 1024).unwrap();
        assert_eq!(updated.artifact.index.symbols[0].relative_path, path);
        assert_eq!(updated.artifact.index.symbols[0].name, "updated_native");
    }
    #[test]
    fn stale_writer_conflicts_and_patch_rolls_back_owned_rows() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let db = cache.path().join("cache.sqlite3");
        let (_, artifact) = fixture(root.path());
        save(&db, &artifact, 1024, &[1; 32]).unwrap();
        let snapshot = load_snapshot(&db, &root.path().canonicalize().unwrap(), 1024).unwrap();
        let old_generation = snapshot.generation;
        let mut inputs = snapshot.inputs;
        inputs.get_mut(Path::new("a.py")).unwrap().stamp = [2; 32];
        let new_generation = crate::persistence::generation(&inputs, 1024);
        patch(
            &db,
            &old_generation,
            &artifact,
            1024,
            &new_generation,
            &inputs,
        )
        .unwrap();
        let prior = std::fs::read(&db).unwrap();
        assert!(matches!(
            patch(
                &db,
                &old_generation,
                &artifact,
                1024,
                &new_generation,
                &inputs
            ),
            Err(StoreError::Conflict)
        ));
        assert_eq!(prior, std::fs::read(&db).unwrap());
        // Failure after DELETE/INSERT must restore children, stamps and metadata.
        let mut invalid = artifact;
        invalid.index.symbols[0].name = "x".repeat(MAX_ROW_BYTES + 1);
        inputs.get_mut(Path::new("a.py")).unwrap().stamp = [3; 32];
        assert!(
            patch(
                &db,
                &new_generation,
                &invalid,
                1024,
                &crate::persistence::generation(&inputs, 1024),
                &inputs
            )
            .is_err()
        );
        assert_eq!(prior, std::fs::read(&db).unwrap());
        let still_valid = load_snapshot(&db, &root.path().canonicalize().unwrap(), 1024).unwrap();
        assert_eq!(still_valid.generation, new_generation);
        assert_eq!(still_valid.artifact.index.symbols[0].name, "one");
    }
    #[test]
    fn input_stamps_are_bounded_and_integrity_covered() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let db = cache.path().join("cache.sqlite3");
        let (_, artifact) = fixture(root.path());
        for mutation in [
            "UPDATE files SET input=CAST(json_set(CAST(input AS TEXT),'$.stamp[0]',99) AS BLOB)",
            "UPDATE files SET input=CAST(json_set(CAST(input AS TEXT),'$.size',999999) AS BLOB)",
            "UPDATE files SET input=x'00'",
        ] {
            save(&db, &artifact, 1024, &[1; 32]).unwrap();
            Connection::open(&db)
                .unwrap()
                .execute_batch(mutation)
                .unwrap();
            assert!(load_snapshot(&db, &root.path().canonicalize().unwrap(), 1024).is_err());
        }
    }
}

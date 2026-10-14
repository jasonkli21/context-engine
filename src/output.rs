use crate::model::{Counts, FileRecord, GitMetadata, RepositoryScan, Statistics, aggregate};
use serde::Serialize;
use std::path::Path;

// Only these DTOs define schema v1; domain types deliberately do not derive Serialize.
#[derive(Serialize)]
struct Envelope<T> {
    schema_version: u32,
    command: String,
    data: T,
}
#[derive(Serialize)]
struct CountsDto {
    files: usize,
    bytes: u64,
    counted_lines: u64,
    files_with_line_counts: usize,
}
impl From<&Counts> for CountsDto {
    fn from(c: &Counts) -> Self {
        Self {
            files: c.files,
            bytes: c.bytes,
            counted_lines: c.counted_lines,
            files_with_line_counts: c.files_with_line_counts,
        }
    }
}
#[derive(Serialize)]
struct FileDto {
    path: String,
    language: String,
    role: String,
    size_bytes: u64,
    line_count: Option<u64>,
}
impl From<&FileRecord> for FileDto {
    fn from(f: &FileRecord) -> Self {
        Self {
            path: relative(&f.relative_path),
            language: f.language.name().into(),
            role: f.role.name().into(),
            size_bytes: f.size_bytes,
            line_count: f.line_count,
        }
    }
}
#[derive(Serialize)]
struct LanguageDto {
    language: String,
    counts: CountsDto,
}
#[derive(Serialize)]
struct RoleDto {
    role: String,
    files: usize,
}
#[derive(Serialize)]
struct GitDto {
    repository_root: String,
    head_commit: Option<String>,
    branch: Option<String>,
    is_dirty: Option<bool>,
}
impl From<&GitMetadata> for GitDto {
    fn from(g: &GitMetadata) -> Self {
        Self {
            repository_root: g.repository_root.to_string_lossy().into_owned(),
            head_commit: g.head_commit.clone(),
            branch: g.branch.clone(),
            is_dirty: g.is_dirty,
        }
    }
}
#[derive(Serialize)]
struct WarningDto {
    path: Option<String>,
    message: String,
}
#[derive(Serialize)]
struct ReportDto {
    root: String,
    directory_count: usize,
    totals: CountsDto,
    languages: Vec<LanguageDto>,
    roles: Vec<RoleDto>,
    largest_files: Vec<FileDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<FileDto>>,
    git: Option<GitDto>,
    warnings: Vec<WarningDto>,
}
#[derive(Serialize)]
struct TreeEntry {
    path: String,
    kind: &'static str,
}
#[derive(Serialize)]
struct TreeDto {
    root: String,
    entries: Vec<TreeEntry>,
    warnings: Vec<WarningDto>,
}
fn relative(path: &Path) -> String {
    path.iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
fn warnings(scan: &RepositoryScan) -> Vec<WarningDto> {
    let mut warnings: Vec<_> = scan
        .warnings
        .iter()
        .map(|w| WarningDto {
            path: w.path.as_deref().map(|p| p.to_string_lossy().into_owned()),
            message: w.message.clone(),
        })
        .collect();
    let has_lossy_path = scan.root.to_str().is_none()
        || scan
            .files
            .iter()
            .any(|f| f.relative_path.to_str().is_none())
        || scan.directories.iter().any(|p| p.to_str().is_none())
        || scan
            .git
            .as_ref()
            .is_some_and(|g| g.repository_root.to_str().is_none());
    if has_lossy_path {
        warnings.push(WarningDto { path: None, message: "some paths contain non-UTF-8 bytes; displayed paths use replacement characters and may not be unique".into() });
    }
    warnings
}
fn entries(scan: &RepositoryScan) -> Vec<TreeEntry> {
    let mut entries: Vec<_> = scan
        .directories
        .iter()
        .map(|path| (path, "directory"))
        .chain(scan.files.iter().map(|file| (&file.relative_path, "file")))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
        .into_iter()
        .map(|(path, kind)| TreeEntry {
            path: relative(path),
            kind,
        })
        .collect()
}
pub fn json(command: &str, scan: &RepositoryScan) -> Result<String, serde_json::Error> {
    if command == "tree" {
        return serde_json::to_string_pretty(&Envelope {
            schema_version: 1,
            command: command.into(),
            data: TreeDto {
                root: scan.root.to_string_lossy().into_owned(),
                entries: entries(scan),
                warnings: warnings(scan),
            },
        });
    }
    let stats = aggregate(&scan.files);
    let data = ReportDto {
        root: scan.root.to_string_lossy().into_owned(),
        directory_count: scan.directories.len(),
        totals: (&stats.totals).into(),
        languages: stats
            .languages
            .iter()
            .map(|(lang, c)| LanguageDto {
                language: lang.name().into(),
                counts: c.into(),
            })
            .collect(),
        roles: stats
            .roles
            .iter()
            .map(|(role, files)| RoleDto {
                role: role.name().into(),
                files: *files,
            })
            .collect(),
        largest_files: stats
            .largest_files
            .iter()
            .map(|f| FileDto::from(*f))
            .collect(),
        files: (command == "scan").then(|| scan.files.iter().map(FileDto::from).collect()),
        git: scan.git.as_ref().map(GitDto::from),
        warnings: warnings(scan),
    };
    serde_json::to_string_pretty(&Envelope {
        schema_version: 1,
        command: command.into(),
        data,
    })
}
fn table(stats: &Statistics<'_>, detailed: bool) -> String {
    let mut text = "\nLanguages\n".to_owned();
    for (language, counts) in &stats.languages {
        let percent = if stats.totals.files == 0 {
            0.0
        } else {
            100.0 * counts.files as f64 / stats.totals.files as f64
        };
        text.push_str(&format!(
            "  {:<14} {:>6} files  {:>5.1}%",
            language.name(),
            counts.files,
            percent
        ));
        if detailed {
            text.push_str(&format!(
                "  {} bytes  {} counted lines ({} files)",
                counts.bytes, counts.counted_lines, counts.files_with_line_counts
            ));
        }
        text.push('\n');
    }
    text.push_str("\nRoles\n");
    for (role, files) in &stats.roles {
        text.push_str(&format!("  {:<16} {files}\n", role.name()));
    }
    text.push_str("\nLargest files (up to 10)\n");
    for file in &stats.largest_files {
        text.push_str(&format!(
            "  {}  {} bytes\n",
            relative(&file.relative_path),
            file.size_bytes
        ));
    }
    text
}
pub fn human(command: &str, scan: &RepositoryScan) -> String {
    let mut text = format!("Repository: {}\n", scan.root.display());
    if command == "tree" {
        text.push_str("\nTree\n.\n");
        // A flat relative-path tree also avoids ambiguous indentation for deep paths.
        for entry in entries(scan) {
            text.push_str(&format!(
                "{}{}\n",
                entry.path,
                if entry.kind == "directory" { "/" } else { "" }
            ));
        }
    } else {
        let stats = aggregate(&scan.files);
        text.push_str(&format!("\n{}\n  Files: {}\n  Directories: {}\n  Size: {} bytes\n  Text lines (counted): {}\n  Files with line counts: {} / {}\n", if command == "stats" { "Statistics" } else { "Summary" }, stats.totals.files, scan.directories.len(), stats.totals.bytes, stats.totals.counted_lines, stats.totals.files_with_line_counts, stats.totals.files));
        text.push_str(&table(&stats, command == "stats"));
        text.push_str("\nGit\n");
        if let Some(git) = &scan.git {
            text.push_str(&format!(
                "  Root: {}\n  Branch: {}\n  Head: {}\n  Dirty: {}\n",
                git.repository_root.display(),
                git.branch
                    .as_deref()
                    .unwrap_or("unavailable (possibly detached)"),
                git.head_commit
                    .as_deref()
                    .unwrap_or("unavailable (possibly unborn)"),
                match git.is_dirty {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unavailable",
                }
            ));
        } else {
            text.push_str("  No Git metadata available\n");
        }
    }
    let warnings = warnings(scan);
    if !warnings.is_empty() {
        text.push_str("\nWarnings\n");
        for warning in warnings {
            text.push_str(&format!(
                "  {}{}\n",
                warning.path.map(|p| format!("{p}: ")).unwrap_or_default(),
                warning.message
            ));
        }
    }
    text.trim_end().to_owned()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn non_utf8_paths_are_qualified_at_presentation_boundary() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let scan = RepositoryScan {
            root: "/fixture".into(),
            directories: vec![],
            git: None,
            warnings: vec![],
            files: vec![FileRecord {
                relative_path: OsString::from_vec(b"invalid-\xff.rs".to_vec()).into(),
                language: crate::classify::Language::Rust,
                role: crate::classify::FileRole::Source,
                size_bytes: 0,
                line_count: Some(0),
            }],
        };
        let value: serde_json::Value = serde_json::from_str(&json("scan", &scan).unwrap()).unwrap();
        assert_eq!(value["data"]["files"][0]["path"], "invalid-�.rs");
        assert!(human("scan", &scan).contains("non-UTF-8"));
        assert!(
            value["data"]["warnings"][0]["message"]
                .as_str()
                .unwrap()
                .contains("may not be unique")
        );
    }
}

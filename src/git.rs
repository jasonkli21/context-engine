use crate::model::{GitMetadata, ScanWarning};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(root: &Path, args: &[&str]) -> std::io::Result<Output> {
    let mut command = Command::new("git");
    command
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false", "-C"])
        .arg(root)
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0");
    // Ambient Git overrides must not redirect a scan to an unrelated repository.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(key);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("Git stdout pipe unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("Git stderr pipe unavailable"))?;
    let out_reader = thread::spawn(move || read_capture(stdout, 16 * 1024 * 1024));
    let err_reader = thread::spawn(move || read_capture(stderr, 1024 * 1024));
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Git operation exceeded 15 seconds",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = out_reader
        .join()
        .map_err(|_| std::io::Error::other("Git stdout reader failed"))??;
    let stderr = err_reader
        .join()
        .map_err(|_| std::io::Error::other("Git stderr reader failed"))??;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
fn read_capture(mut reader: impl std::io::Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Git output exceeded metadata limit",
        ));
    }
    Ok(bytes)
}
fn warning(message: String) -> ScanWarning {
    ScanWarning {
        path: None,
        message,
    }
}
fn detail(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}
fn trim_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    bytes
}
fn path_from_output(bytes: Vec<u8>) -> Option<PathBuf> {
    let bytes = trim_newline(bytes);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(OsString::from_vec(bytes)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes)
            .ok()
            .map(OsString::from)
            .map(PathBuf::from)
    }
}
fn optional_text(root: &Path, args: &[&str], warnings: &mut Vec<ScanWarning>) -> Option<String> {
    match run(root, args) {
        Ok(output) if output.status.success() => {
            match String::from_utf8(trim_newline(output.stdout)) {
                Ok(value) if !value.is_empty() => Some(value),
                Ok(_) => None,
                Err(_) => {
                    warnings.push(warning(format!(
                        "Git {} returned non-UTF-8 metadata",
                        args.join(" ")
                    )));
                    None
                }
            }
        }
        // --quiet uses exit 1 for an unborn or detached HEAD.
        Ok(output) if output.status.code() == Some(1) && output.stderr.is_empty() => None,
        Ok(output) => {
            warnings.push(warning(format!(
                "Git {} failed: {}",
                args.join(" "),
                detail(&output)
            )));
            None
        }
        Err(error) => {
            warnings.push(warning(format!(
                "Git {} unavailable: {error}",
                args.join(" ")
            )));
            None
        }
    }
}

fn filters_configured(root: &Path) -> Result<bool, ()> {
    let output = run(
        root,
        &[
            "config",
            "--null",
            "--get-regexp",
            r"^filter\..*\.(clean|process)$",
        ],
    )
    .map_err(|_| ())?;
    if output.status.success() {
        return Ok(!output.stdout.is_empty());
    }
    if output.status.code() == Some(1) && output.stderr.is_empty() {
        return Ok(false);
    }
    Err(())
}

pub fn probe(root: &Path) -> (Option<GitMetadata>, Vec<ScanWarning>) {
    let mut warnings = vec![];
    let output = match run(root, &["rev-parse", "--show-toplevel"]) {
        Ok(output) if output.status.success() => output,
        Ok(output) if detail(&output).contains("not a git repository") => return (None, warnings),
        Ok(output) => {
            warnings.push(warning(format!(
                "Git root probe failed: {}",
                detail(&output)
            )));
            return (None, warnings);
        }
        Err(error) => {
            warnings.push(warning(format!("Git metadata unavailable: {error}")));
            return (None, warnings);
        }
    };
    let Some(repository_root) = path_from_output(output.stdout) else {
        warnings.push(warning(
            "Git root is not representable on this platform".into(),
        ));
        return (None, warnings);
    };
    let head_commit = optional_text(
        root,
        &["rev-parse", "--verify", "--quiet", "HEAD"],
        &mut warnings,
    );
    let branch = optional_text(
        root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        &mut warnings,
    );
    let is_dirty = match filters_configured(root) {
        Ok(true) => {
            warnings.push(warning(
                "Git dirty state unavailable because external Git filters are configured".into(),
            ));
            None
        }
        Err(()) => {
            warnings.push(warning(
                "Git dirty state unavailable because Git filter configuration could not be checked safely".into(),
            ));
            None
        }
        Ok(false) => match run(
            root,
            &[
                "status",
                "--porcelain=v1",
                "--untracked-files=normal",
                "--ignore-submodules=all",
            ],
        ) {
            Ok(output) if output.status.success() => Some(!output.stdout.is_empty()),
            Ok(output) => {
                warnings.push(warning(format!("Git status failed: {}", detail(&output))));
                None
            }
            Err(error) => {
                warnings.push(warning(format!("Git status unavailable: {error}")));
                None
            }
        },
    };
    (
        Some(GitMetadata {
            repository_root,
            head_commit,
            branch,
            is_dirty,
        }),
        warnings,
    )
}

#[path = "git_revision.rs"]
pub(crate) mod revision;

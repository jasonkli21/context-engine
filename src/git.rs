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
    capture(command, 16 * 1024 * 1024, 1024 * 1024).map_err(|error| match error {
        CaptureError::Io(error) => error,
        CaptureError::Limit(_) => std::io::Error::new(std::io::ErrorKind::InvalidData, error),
    })
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum CaptureError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Git {0} exceeded output limit")]
    Limit(&'static str),
}
// One owner always kills/reaps on early failure and joins BOTH readers. Readers
// notify immediately at limit+1 rather than blocking the child on a full pipe.
pub(crate) fn capture(
    mut command: Command,
    out_max: usize,
    err_max: usize,
) -> Result<Output, CaptureError> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let (signal, stop) = std::sync::mpsc::channel();
    let out_signal = signal.clone();
    let out_reader = thread::spawn(move || read_capture(stdout, out_max, out_signal));
    let err_reader = thread::spawn(move || read_capture(stderr, err_max, signal));
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if stop.try_recv().is_ok() {
            break Err(std::io::Error::other("Git capture interrupted"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(error) => break Err(error),
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            break Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Git operation exceeded 15 seconds",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Join before propagating ANY error: no detached readers on timeout/overflow.
    let stdout = out_reader.join();
    let stderr = err_reader.join();
    let stdout = stdout.map_err(|_| std::io::Error::other("Git stdout reader failed"))??;
    let stderr = stderr.map_err(|_| std::io::Error::other("Git stderr reader failed"))??;
    if stdout.len() > out_max {
        return Err(CaptureError::Limit("stdout"));
    }
    if stderr.len() > err_max {
        return Err(CaptureError::Limit("stderr"));
    }
    Ok(Output {
        status: status?,
        stdout,
        stderr,
    })
}
fn read_capture(
    reader: impl Read,
    limit: usize,
    signal: std::sync::mpsc::Sender<()>,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let result = reader
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes);
    if result.is_err() || bytes.len() > limit {
        let _ = signal.send(());
    }
    result?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_child() {
        let Ok(mode) = std::env::var("CTX_CAPTURE_TEST_STREAM") else {
            return;
        };
        use std::io::Write;
        std::fs::write(
            std::env::var_os("CTX_CAPTURE_TEST_PID").unwrap(),
            std::process::id().to_string(),
        )
        .unwrap();
        let bytes = vec![b'x'; 2 * 1024 * 1024];
        if mode == "stdout" {
            let _ = std::io::stdout().write_all(&bytes);
        } else {
            let _ = std::io::stderr().write_all(&bytes);
        }
        // Force parent containment even when the OS pipe can hold all output.
        std::thread::sleep(Duration::from_secs(20));
    }
    pub(super) fn oversized_command(stream: &str, pid_path: &Path) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["--exact", "git::tests::capture_child", "--nocapture"])
            .env("CTX_CAPTURE_TEST_STREAM", stream)
            .env("CTX_CAPTURE_TEST_PID", pid_path);
        c
    }
    #[test]
    fn output_limits_stop_and_reap_both_streams_promptly() {
        for stream in ["stdout", "stderr"] {
            let dir = tempfile::tempdir().unwrap();
            let pid_path = dir.path().join("pid");
            let start = Instant::now();
            let error = capture(oversized_command(stream, &pid_path), 1024, 1024).unwrap_err();
            assert!(
                matches!(error, CaptureError::Limit(s) if s == stream),
                "{error}"
            );
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "{:?}",
                start.elapsed()
            );
            #[cfg(unix)]
            assert!(
                !Command::new("kill")
                    .args(["-0", &std::fs::read_to_string(&pid_path).unwrap()])
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    #[test]
    fn ordinary_capture_preserves_success_and_nonzero_status() {
        let mut command = Command::new("git");
        command.arg("--version");
        let out = capture(command, 1024, 1024).unwrap();
        assert!(out.status.success());
        assert!(out.stdout.starts_with(b"git version "));
        let mut command = Command::new("git");
        command.arg("--ctx-invalid-option");
        let out = capture(command, 1024, 4096).unwrap();
        assert!(!out.status.success());
        assert!(!out.stderr.is_empty());
    }
}

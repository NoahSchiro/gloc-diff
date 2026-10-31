//! The only place gloc runs external processes: shelling out to git.

use std::fmt;
use std::path::PathBuf;
use std::process::Command;

/// A git invocation that failed to run or exited nonzero.
#[derive(Debug)]
pub struct GitError {
    command: String,
    stderr: String,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` failed: {}", self.command, self.stderr.trim())
    }
}

impl std::error::Error for GitError {}

/// Unified diff between the merge-base of `base` and `head` and `head`
/// itself (`git diff base...head`), with zero context and rename detection.
pub fn diff_range(base: &str, head: &str) -> Result<String, GitError> {
    run(&[
        "diff",
        "-U0",
        "--no-color",     // plain parseable text
        "--no-ext-diff",  // plain parseable text
        "--find-renames", // file renames don't count as changes
        &format!("{base}...{head}"),
    ])
}

/// The best common ancestor of `base` and `head`.
pub fn merge_base(base: &str, head: &str) -> Result<String, GitError> {
    run(&["merge-base", base, head]).map(|sha| sha.trim().to_owned())
}

/// The content of one file version: `git show rev:path`.
pub fn blob(rev: &str, path: &str) -> Result<String, GitError> {
    run(&["show", &format!("{rev}:{path}")])
}

/// Absolute path of the working tree root.
///
/// Many of the files we need are relative to this.
/// - Diff output paths
/// - `.gloc.toml`
/// - gitignore files
pub fn toplevel() -> Result<PathBuf, GitError> {
    let path = run(&["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(path.trim_end()))
}

/// Run one git invocation, returning its stdout or an error naming the
/// command and its stderr.
fn run(git_args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(git_args)
        .output()
        .map_err(|err| GitError {
            command: "git".to_string(),
            stderr: err.to_string(),
        })?;

    if !output.status.success() {
        return Err(GitError {
            command: format!("git {}", git_args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

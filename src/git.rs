//! The only place gloc runs external processes: shelling out to git.

use std::fmt;
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
    let git_args = [
        "diff",
        "-U0",
        "--no-color",     // plain parseable text
        "--no-ext-diff",  // plain parseable text
        "--find-renames", // file renames don't count as changes
        &format!("{base}...{head}"),
    ];
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

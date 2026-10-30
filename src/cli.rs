//! The command-line contract

use clap::Parser;

/// Valid arguments to `gloc`
#[derive(Parser, Debug)]
#[command(name = "gloc", version)]
pub struct Args {
    /// Base ref to diff against (branch, tag, or commit), e.g. origin/main.
    /// The diff runs from the merge-base of base and head, so commits that
    /// landed on base after branching are not counted.
    #[arg(long)]
    pub base: String,

    /// Head ref to diff; defaults to the current checkout.
    #[arg(long, default_value = "HEAD")]
    pub head: String,

    /// Fail with exit code 1 when total churn (added + removed lines)
    /// exceeds this number.
    #[arg(long)]
    pub max_total: Option<u64>,
}

//! gloc reports source and test churn between a PR branch and its target
//! branch. For now, we count non-blank lines; per-language classification
//! and output formats come later.

mod cli;
mod count;
mod diff;
mod git;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Args;
use crate::count::{Count, FileCount};

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("gloc: {err}");
            ExitCode::from(2)
        }
    }
}

/// Main entry point
///
/// Exit code 0: within thresholds of acceptable LOC churn.
/// Exit code 1: threshold exceeded.
/// Exit code 2: could not run / failed.
fn run(args: &Args) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let diff_text = git::diff_range(&args.base, &args.head)?;
    let files = diff::parse_diff(&diff_text);
    let counts = count::count_files(&files);
    let total = count::total(&counts);
    print_table(&counts, total);

    if let Some(max_total) = args.max_total {
        if total.churn() > max_total {
            eprintln!(
                "gloc: total churn {} exceeds --max-total {max_total}",
                total.churn()
            );
            return Ok(ExitCode::FAILURE);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn print_table(counts: &[FileCount<'_>], total: Count) {
    let width = counts
        .iter()
        .map(|c| c.path.len())
        .chain(["file".len(), "total".len()])
        .max()
        .unwrap_or(4);

    println!(
        "{:<width$}  {:>7}  {:>7}  {:>7}",
        "file",
        "added",
        "removed",
        "churn",
        width = width
    );
    for c in counts {
        println!(
            "{:<width$}  {:>7}  {:>7}  {:>7}",
            c.path,
            c.count.added,
            c.count.removed,
            c.count.churn(),
            width = width
        );
    }
    println!(
        "{:<width$}  {:>7}  {:>7}  {:>7}",
        "total",
        total.added,
        total.removed,
        total.churn(),
        width = width
    );
}

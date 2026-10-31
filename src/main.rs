//! gloc reports source and test churn between a PR branch and its target
//! branch. File paths are filtered by globs, regexes, and gitignore rules.
//! Non-blank lines are counted per surviving file.

mod cli;
mod config;
mod count;
mod diff;
mod filter;
mod git;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
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

    let repo_root = git::toplevel()?;
    let options = filter::Options::merge(args.filter_options(), config::load(&repo_root)?);
    let changed_dirs: BTreeSet<PathBuf> = files
        .iter()
        .filter_map(|file| Path::new(&file.path).parent())
        .map(Path::to_path_buf)
        .collect();
    let filters = filter::Filters::compile(&options, &repo_root, &changed_dirs)?;

    let counts: Vec<FileCount> = count::count_files(&files)
        .into_iter()
        .filter(|file| filters.keeps(file.path))
        .collect();
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

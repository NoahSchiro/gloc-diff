//! gloc reports source, test, comment, and blank churn between a PR branch
//! and its target branch. File paths are filtered by globs, regexes, and
//! gitignore rules, and every counted line is classed by its language, with
//! unknown files falling back to blank/non-blank counting.

mod classify;
mod cli;
mod config;
mod count;
mod diff;
mod filter;
mod git;
mod globs;
mod lang;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use crate::cli::Args;
use crate::count::{Buckets, FileCount};

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

    // The old side of `base...head` is the merge-base: that is where removed
    // lines came from, so old blobs are fetched from there.
    let merge_base = git::merge_base(&args.base, &args.head)?;

    let mut counts = Vec::new();
    for file in &files {
        if !filters.keeps(&file.path) {
            continue;
        }
        let old_text = old_side_text(file, &merge_base)?;
        let new_text = new_side_text(file, &args.head)?;
        let old_classes = classify::classify_side(file.old_path.as_deref(), old_text.as_deref());
        let new_classes = classify::classify_side(Some(file.path.as_str()), new_text.as_deref());
        counts.push(count::count_file(file, &old_classes, &new_classes));
    }

    let total = count::total(&counts);
    print_table(&counts, total);

    if let Some(max_total) = args.max_total
        && total.churn() > max_total
    {
        eprintln!(
            "gloc: total churn {} exceeds --max-total {max_total}",
            total.churn()
        );
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

/// The old file version, when some hunk removes lines of it. Pure renames
/// and mode-only changes have no such hunks, and binary files have no text.
fn old_side_text(
    file: &diff::FileDiff<'_>,
    merge_base: &str,
) -> Result<Option<String>, git::GitError> {
    if file.is_binary || !file.hunks.iter().any(|hunk| !hunk.removed.is_empty()) {
        return Ok(None);
    }
    match file.old_path.as_deref() {
        Some(path) => git::blob(merge_base, path).map(Some),
        None => Ok(None),
    }
}

/// The new file version, when some hunk adds lines to it.
fn new_side_text(file: &diff::FileDiff<'_>, head: &str) -> Result<Option<String>, git::GitError> {
    if file.is_binary || !file.hunks.iter().any(|hunk| !hunk.added.is_empty()) {
        return Ok(None);
    }
    git::blob(head, &file.path).map(Some)
}

fn print_table(counts: &[FileCount<'_>], total: Buckets) {
    // Width of every numeric column.
    let col_w = 7;
    // Width of one class group (added and removed cols).
    let group_w = 2 * col_w + 2;
    let file_w = counts
        .iter()
        .map(|c| c.path.len())
        .chain(["file", "total"].map(str::len))
        .max()
        .unwrap_or(4);
    let lang_w = counts
        .iter()
        .map(|c| c.lang.len())
        .chain(std::iter::once("lang".len()))
        .max()
        .unwrap_or(4);

    println!(
        "{:<file_w$}  {:<lang_w$}  {:^group_w$}  {:^group_w$}  {:^group_w$}  {:^group_w$}  {:>col_w$}",
        "file",
        "lang",
        "blank",
        "comment",
        "test",
        "source",
        "churn",
        file_w = file_w,
        lang_w = lang_w,
        group_w = group_w,
        col_w = col_w,
    );
    let marks = format!(
        "{:<file_w$}  {:<lang_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}",
        "",
        "",
        "+",
        "-",
        "+",
        "-",
        "+",
        "-",
        "+",
        "-",
        "",
        file_w = file_w,
        lang_w = lang_w,
        col_w = col_w,
    );
    println!("{}", marks.trim_end());

    for count in counts {
        print_row(count.path, count.lang, count.buckets, file_w, lang_w, col_w);
    }
    print_row("total", "", total, file_w, lang_w, col_w);
}

fn print_row(file: &str, lang: &str, buckets: Buckets, file_w: usize, lang_w: usize, col_w: usize) {
    println!(
        "{:<file_w$}  {:<lang_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}  {:>col_w$}",
        file,
        lang,
        buckets.blank.added,
        buckets.blank.removed,
        buckets.comment.added,
        buckets.comment.removed,
        buckets.test.added,
        buckets.test.removed,
        buckets.source.added,
        buckets.source.removed,
        buckets.churn(),
        file_w = file_w,
        lang_w = lang_w,
        col_w = col_w,
    );
}

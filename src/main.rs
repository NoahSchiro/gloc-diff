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
mod syntax;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Args, TableStyle};
use crate::count::{Buckets, FileCount};
use tabled::builder::Builder;
use tabled::settings::{
    Alignment, Modify, Padding, Span, Style,
    object::{Columns, FirstColumn, LastColumn},
};

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

    if args.depth.is_some_and(|depth| depth == 0) {
        return Err("--depth must be at least 1".into());
    }

    let mut counts = Vec::new();
    for file in &files {
        if !filters.keeps(&file.path) {
            continue;
        }
        let old_text = old_side_text(file, &merge_base)?;
        let new_text = new_side_text(file, &args.head)?;
        let old_classes = classify_blob(file.old_path.as_deref(), old_text.as_deref());
        let new_classes = classify_blob(Some(file.path.as_str()), new_text.as_deref());
        counts.push(count::count_file(file, &old_classes, &new_classes));
    }

    let total = count::total(&counts);
    if args.total_only {
        println!("{}", total.churn());
    } else {
        if let Some(depth) = args.depth {
            counts = count::rollup(counts, depth);
        }
        print_table(&counts, total, args.style);
    }

    let failed = exceeded("src", total.source.churn(), args.max_src)
        | exceeded("test", total.test.churn(), args.max_test)
        | exceeded("total", total.churn(), args.max_total);
    if failed {
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

/// Report one exceeded churn gate on stderr.
fn exceeded(name: &str, churn: u64, max: Option<u64>) -> bool {
    match max {
        Some(max) if churn > max => {
            eprintln!("gloc: {name} churn {churn} exceeds --max-{name} {max}");
            true
        }
        _ => false,
    }
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

/// Per-line [`classify::LineClass`] of one file's content.
fn classify_blob(path: Option<&str>, text: Option<&str>) -> Vec<classify::LineClass> {
    let (Some(path), Some(text)) = (path, text) else {
        return Vec::new();
    };
    let lang = lang::for_path(path);
    let is_test_path = lang.is_some_and(|lang| lang.is_test_path(path));
    let (comments, tests) = match lang.and_then(|lang| lang.grammar) {
        Some(grammar) => syntax::ranges(text, &grammar),
        None => (Vec::new(), Vec::new()),
    };
    classify::classify_lines(text, &comments, &tests, is_test_path)
}

/// Print the churn table. `style` picks one of the tabled border presets.
fn print_table(counts: &[FileCount], total: Buckets, style: TableStyle) {
    let mut builder = Builder::new();
    builder.push_record([
        "file", "lang", "blank", "", "comment", "", "test", "", "source", "", "churn",
    ]);
    builder.push_record(["", "", "+", "-", "+", "-", "+", "-", "+", "-", ""]);
    for count in counts {
        builder.push_record(count_row(&count.path, count.lang, count.buckets));
    }
    builder.push_record(count_row("total", "", total));

    let mut table = builder.build();
    table.with(Modify::new(Columns::new(2..)).with(Alignment::right()));
    match style {
        TableStyle::Empty => table
            .with(Style::empty())
            // Borderless: drop the outer edge padding.
            .with(Modify::new(FirstColumn).with(Padding::new(0, 1, 0, 0)))
            .with(Modify::new(LastColumn).with(Padding::new(1, 0, 0, 0))),
        TableStyle::Blank => table.with(Style::blank()),
        TableStyle::Ascii => table.with(Style::ascii()),
        TableStyle::AsciiRounded => table.with(Style::ascii_rounded()),
        TableStyle::Psql => table.with(Style::psql()),
        TableStyle::Markdown => table.with(Style::markdown()),
        TableStyle::Modern => table.with(Style::modern()),
        TableStyle::ModernRounded => table.with(Style::modern_rounded()),
        TableStyle::Sharp => table.with(Style::sharp()),
        TableStyle::Rounded => table.with(Style::rounded()),
        TableStyle::Extended => table.with(Style::extended()),
        TableStyle::Dots => table.with(Style::dots()),
        TableStyle::RestructuredText => table.with(Style::re_structured_text()),
    };
    for col in [2, 4, 6, 8] {
        table.modify((0, col), Span::column(2));
        table.modify((0, col), Alignment::center());
    }
    for line in table.to_string().lines() {
        println!("{}", line.trim_end());
    }
}

/// One table row: the four buckets' added and removed counts, then churn.
fn count_row(file: &str, lang: &str, buckets: Buckets) -> [String; 11] {
    [
        file.to_owned(),
        lang.to_owned(),
        buckets.blank.added.to_string(),
        buckets.blank.removed.to_string(),
        buckets.comment.added.to_string(),
        buckets.comment.removed.to_string(),
        buckets.test.added.to_string(),
        buckets.test.removed.to_string(),
        buckets.source.added.to_string(),
        buckets.source.removed.to_string(),
        buckets.churn().to_string(),
    ]
}

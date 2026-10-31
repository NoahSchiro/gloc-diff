//! The command-line contract

use clap::{Parser, ValueEnum};

use crate::filter;

/// Valid arguments to `gloc`
///
/// Cli args always override arguments in .gloc.toml
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

    /// Only count files matching this gitignore-style glob. Repeatable.
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Skip files matching this gitignore-style glob. Repeatable.
    /// Excludes beat includes.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Same as [`Args::include`] but regex instead of glob
    /// Unioned with [`Args::include`]
    #[arg(long, value_name = "REGEX")]
    pub include_re: Vec<String>,

    /// Same as [`Args::exclude`] but regex instead of glob.
    /// Unioned with [`Args::exclude`]
    #[arg(long, value_name = "REGEX")]
    pub exclude_re: Vec<String>,

    /// Do not apply .gitignore rules.
    #[arg(long)]
    pub no_gitignore: bool,

    /// Border style of the churn table.
    #[arg(long, value_enum, default_value_t = TableStyle::Sharp)]
    pub style: TableStyle,

    /// Provide a summary at this many directory levels instead
    /// of one row per file.
    #[arg(long, value_name = "N")]
    pub depth: Option<usize>,

    /// Only print the total churn for the entire repo.
    #[arg(long, conflicts_with = "depth")]
    pub total_only: bool,
}

/// Border style of the churn table, one of the tabled presets.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TableStyle {
    Empty,
    Blank,
    Ascii,
    AsciiRounded,
    Psql,
    Markdown,
    Modern,
    ModernRounded,
    Sharp,
    Rounded,
    Extended,
    Dots,
    RestructuredText,
}

impl Args {
    /// The filter half of the arguments, in the shared option shape.
    pub fn filter_options(&self) -> filter::Options {
        filter::Options {
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            include_re: self.include_re.clone(),
            exclude_re: self.exclude_re.clone(),
            no_gitignore: self.no_gitignore,
        }
    }
}

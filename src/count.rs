//! Which diff lines count and how they add up.

use std::iter::Sum;
use std::ops::Add;

use crate::diff::FileDiff;

/// Added and removed line counts for one scope (a file, a module, or everything).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub added: u64,
    pub removed: u64,
}

impl Count {
    pub fn churn(self) -> u64 {
        self.added + self.removed
    }
}

impl Add for Count {
    type Output = Count;

    fn add(self, rhs: Count) -> Count {
        Count {
            added: self.added + rhs.added,
            removed: self.removed + rhs.removed,
        }
    }
}

impl Sum for Count {
    fn sum<I: Iterator<Item = Count>>(iter: I) -> Count {
        iter.fold(Count::default(), |acc, count| acc + count)
    }
}

pub struct FileCount<'a> {
    pub path: &'a str,
    pub count: Count,
}

/// Phase 0 counts every non-blank line; the language classifier (phase 2)
/// replaces this with per-line Blank/Comment/Code/Test classes.
fn is_counted(line: &str) -> bool {
    !line.trim().is_empty()
}

/// Get counts for one file
pub fn count_file(file: &FileDiff<'_>) -> Count {
    if file.is_binary {
        return Count::default();
    }
    let mut count = Count::default();
    for hunk in &file.hunks {
        count.added += hunk.added.iter().filter(|line| is_counted(line)).count() as u64;
        count.removed += hunk.removed.iter().filter(|line| is_counted(line)).count() as u64;
    }
    count
}

/// Get Counts for all files
pub fn count_files<'a>(files: &'a [FileDiff<'a>]) -> Vec<FileCount<'a>> {
    files
        .iter()
        .map(|file| FileCount {
            path: &file.path,
            count: count_file(file),
        })
        .collect()
}

/// Get summary count for all files
pub fn total(counts: &[FileCount<'_>]) -> Count {
    counts.iter().map(|c| c.count).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::Hunk;

    fn hunk<'a>(added: &[&'a str], removed: &[&'a str]) -> Hunk<'a> {
        Hunk {
            old_start: 1,
            old_lines: removed.len() as u64,
            new_start: 1,
            new_lines: added.len() as u64,
            added: added.to_vec(),
            removed: removed.to_vec(),
        }
    }

    #[test]
    fn blank_lines_do_not_count() {
        let file = FileDiff {
            path: "a.rs".to_string(),
            is_binary: false,
            hunks: vec![hunk(&["fn a() {}", "", "   "], &["x"])],
        };
        assert_eq!(
            count_file(&file),
            Count {
                added: 1,
                removed: 1
            }
        );
    }

    #[test]
    fn binary_files_contribute_nothing() {
        let file = FileDiff {
            path: "logo.png".to_string(),
            is_binary: true,
            hunks: vec![hunk(&["junk"], &["junk"])],
        };
        assert_eq!(count_file(&file), Count::default());
    }

    #[test]
    fn totals_sum_over_files_and_report_churn() {
        let files = vec![
            FileDiff {
                path: "a.rs".to_string(),
                is_binary: false,
                hunks: vec![hunk(&["a"], &["b", "c"])],
            },
            FileDiff {
                path: "b.bin".to_string(),
                is_binary: true,
                hunks: vec![],
            },
        ];
        let counts = count_files(&files);
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0].path, "a.rs");
        assert_eq!(
            total(&counts),
            Count {
                added: 1,
                removed: 2
            }
        );
        assert_eq!(total(&counts).churn(), 3);
    }
}

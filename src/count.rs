//! This module counts line churn and classifies into into [`Buckets`]
//! (source, test, comment, or blank) each with added and removed counts.

use std::collections::BTreeMap;
use std::iter::Sum;
use std::ops::Add;

use crate::classify::LineClass;
use crate::diff::FileDiff;
use crate::lang;

/// Added and removed line counts for one scope (a bucket, a file, or everything).
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

/// Churn split by line class: the four reported buckets.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Buckets {
    pub source: Count,
    pub test: Count,
    pub comment: Count,
    pub blank: Count,
}

impl Buckets {
    pub fn churn(self) -> u64 {
        self.source.churn() + self.test.churn() + self.comment.churn() + self.blank.churn()
    }

    /// Accumulate one line into its bucket.
    fn record(&mut self, class: LineClass, added: bool) {
        let count = match class {
            LineClass::Code => &mut self.source,
            LineClass::Test => &mut self.test,
            LineClass::Comment => &mut self.comment,
            LineClass::Blank => &mut self.blank,
        };
        if added {
            count.added += 1;
        } else {
            count.removed += 1;
        }
    }
}

impl Add for Buckets {
    type Output = Buckets;

    fn add(self, rhs: Buckets) -> Buckets {
        Buckets {
            source: self.source + rhs.source,
            test: self.test + rhs.test,
            comment: self.comment + rhs.comment,
            blank: self.blank + rhs.blank,
        }
    }
}

impl Sum for Buckets {
    fn sum<I: Iterator<Item = Buckets>>(iter: I) -> Buckets {
        iter.fold(Buckets::default(), |acc, buckets| acc + buckets)
    }
}

/// One file's (or directory's) churn: which language
/// counted it, and the four buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCount {
    pub path: String,
    pub lang: &'static str,
    pub buckets: Buckets,
}

/// Count one file's churn by mapping each hunk line onto the per-line classes
/// of its old and new versions.
pub fn count_file(
    file: &FileDiff<'_>,
    old_classes: &[LineClass],
    new_classes: &[LineClass],
) -> FileCount {
    let lang_name = if file.is_binary {
        "binary"
    } else {
        lang::for_path(&file.path).map_or("unclassified", |lang| lang.name)
    };

    let mut buckets = Buckets::default();
    for hunk in &file.hunks {
        for (i, line) in hunk.added.iter().enumerate() {
            let class = class_at(new_classes, hunk.new_start + i as u64, line);
            buckets.record(class, true);
        }
        for (j, line) in hunk.removed.iter().enumerate() {
            let class = class_at(old_classes, hunk.old_start + j as u64, line);
            buckets.record(class, false);
        }
    }

    FileCount {
        path: file.path.clone(),
        lang: lang_name,
        buckets,
    }
}

/// The class of the hunk line at 1-based `number` in a classified version,
/// or the blank/non-blank fallback when the version has no such line.
fn class_at(classes: &[LineClass], number: u64, line: &str) -> LineClass {
    match number
        .checked_sub(1)
        .and_then(|index| classes.get(index as usize))
    {
        Some(&class) => class,
        None if line.trim().is_empty() => LineClass::Blank,
        None => LineClass::Code,
    }
}

/// The sum of every file's buckets.
pub fn total(counts: &[FileCount]) -> Buckets {
    counts.iter().map(|c| c.buckets).sum()
}

/// Roll per-file counts up into one row per path prefix at `depth`.
/// 
/// Directory rows carry no language. Rows are sorted by path.
pub fn rollup(counts: Vec<FileCount>, depth: usize) -> Vec<FileCount> {
    let mut groups: BTreeMap<String, FileCount> = BTreeMap::new();
    for count in counts {
        let key = group_key(&count.path, depth);
        let group = groups.entry(key.to_owned()).or_insert_with(|| FileCount {
            path: key.to_owned(),
            lang: if key == count.path { count.lang } else { "" },
            buckets: Buckets::default(),
        });
        group.buckets = group.buckets + count.buckets;
    }
    groups.into_values().collect()
}

/// The row path for `path` at grouping `depth`.
fn group_key(path: &str, depth: usize) -> &str {
    let mut end = path.len();
    let mut seen = 0;
    for (offset, byte) in path.bytes().enumerate() {
        if byte == b'/' {
            seen += 1;
            if seen == depth {
                end = offset;
                break;
            }
        }
    }
    &path[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::Hunk;

    fn hunk<'a>(
        old_start: u64,
        new_start: u64,
        added: &[&'a str],
        removed: &[&'a str],
    ) -> Hunk<'a> {
        Hunk {
            old_start,
            old_lines: removed.len() as u64,
            new_start,
            new_lines: added.len() as u64,
            added: added.to_vec(),
            removed: removed.to_vec(),
        }
    }

    fn file<'a>(path: &str, old_path: Option<&str>, hunks: Vec<Hunk<'a>>) -> FileDiff<'a> {
        FileDiff {
            path: path.to_string(),
            old_path: old_path.map(str::to_string),
            is_binary: false,
            hunks,
        }
    }

    #[test]
    fn lines_map_to_their_classified_positions() {
        let file = file(
            "a.xyz",
            Some("a.xyz"),
            vec![hunk(2, 3, &["one", "two"], &["x", "y"])],
        );
        let old = [
            LineClass::Code,
            LineClass::Blank,
            LineClass::Test,
            LineClass::Code,
        ];
        let new = [
            LineClass::Code,
            LineClass::Code,
            LineClass::Comment,
            LineClass::Blank,
            LineClass::Code,
        ];
        let count = count_file(&file, &old, &new);
        assert_eq!(count.lang, "unclassified");
        assert_eq!(count.buckets.source, Count::default());
        assert_eq!(
            count.buckets.test,
            Count {
                added: 0,
                removed: 1
            }
        );
        assert_eq!(
            count.buckets.comment,
            Count {
                added: 1,
                removed: 0
            }
        );
        assert_eq!(
            count.buckets.blank,
            Count {
                added: 1,
                removed: 1
            }
        );
        assert_eq!(count.buckets.churn(), 4);
    }

    #[test]
    fn lines_without_classes_fall_back_to_blank_or_code() {
        let file = file(
            "a.xyz",
            None,
            vec![hunk(1, 1, &["a", "", "  "], &["b", ""])],
        );
        let count = count_file(&file, &[], &[]);
        assert_eq!(count.lang, "unclassified");
        assert_eq!(
            count.buckets.source,
            Count {
                added: 1,
                removed: 1
            }
        );
        assert_eq!(
            count.buckets.blank,
            Count {
                added: 2,
                removed: 1
            }
        );
    }

    #[test]
    fn line_numbers_before_the_hunk_fall_back_too() {
        let file = file("a.xyz", None, vec![hunk(5, 5, &["x"], &[""])]);
        let count = count_file(&file, &[], &[LineClass::Comment]);
        assert_eq!(count.buckets.comment, Count::default());
        assert_eq!(
            count.buckets.source,
            Count {
                added: 1,
                removed: 0
            }
        );
        assert_eq!(
            count.buckets.blank,
            Count {
                added: 0,
                removed: 1
            }
        );
    }

    #[test]
    fn known_languages_show_their_name() {
        let file = file(
            "tests/a.stub",
            Some("tests/a.stub"),
            vec![hunk(1, 1, &["assert"], &[])],
        );
        let count = count_file(&file, &[], &[LineClass::Test]);
        assert_eq!(count.lang, "stub");
        assert_eq!(
            count.buckets.test,
            Count {
                added: 1,
                removed: 0
            }
        );
    }

    #[test]
    fn binary_files_contribute_nothing() {
        let file = FileDiff {
            path: "logo.png".to_string(),
            old_path: Some("logo.png".to_string()),
            is_binary: true,
            hunks: vec![],
        };
        let count = count_file(&file, &[], &[]);
        assert_eq!(count.lang, "binary");
        assert_eq!(count.buckets, Buckets::default());
    }

    #[test]
    fn classes_land_in_their_named_buckets() {
        let mut buckets = Buckets::default();
        buckets.record(LineClass::Code, true);
        buckets.record(LineClass::Test, true);
        buckets.record(LineClass::Comment, true);
        buckets.record(LineClass::Comment, false);
        buckets.record(LineClass::Blank, false);
        assert_eq!(
            buckets.source,
            Count {
                added: 1,
                removed: 0
            }
        );
        assert_eq!(
            buckets.test,
            Count {
                added: 1,
                removed: 0
            }
        );
        assert_eq!(
            buckets.comment,
            Count {
                added: 1,
                removed: 1
            }
        );
        assert_eq!(
            buckets.blank,
            Count {
                added: 0,
                removed: 1
            }
        );
    }

    #[test]
    fn totals_sum_every_bucket() {
        let counts = vec![
            FileCount {
                path: "a".to_owned(),
                lang: "stub",
                buckets: Buckets {
                    source: Count {
                        added: 1,
                        removed: 0,
                    },
                    test: Count {
                        added: 2,
                        removed: 1,
                    },
                    comment: Count::default(),
                    blank: Count::default(),
                },
            },
            FileCount {
                path: "b".to_owned(),
                lang: "unclassified",
                buckets: Buckets {
                    source: Count::default(),
                    test: Count::default(),
                    comment: Count {
                        added: 0,
                        removed: 3,
                    },
                    blank: Count::default(),
                },
            },
        ];
        let total = total(&counts);
        assert_eq!(
            total.source,
            Count {
                added: 1,
                removed: 0
            }
        );
        assert_eq!(
            total.test,
            Count {
                added: 2,
                removed: 1
            }
        );
        assert_eq!(
            total.comment,
            Count {
                added: 0,
                removed: 3
            }
        );
        assert_eq!(total.blank, Count::default());
        assert_eq!(total.churn(), 7);
    }

    /// A count of `added` source lines and nothing else.
    fn source(path: &str, lang: &'static str, added: u64) -> FileCount {
        FileCount {
            path: path.to_owned(),
            lang,
            buckets: Buckets {
                source: Count { added, removed: 0 },
                ..Buckets::default()
            },
        }
    }

    #[test]
    fn rollup_groups_deeper_files_into_their_directory() {
        let rolled = rollup(
            vec![
                source("src/lib.rs", "rust", 1),
                source("src/app.rs", "rust", 2),
                source("tests/x.rs", "rust", 4),
            ],
            1,
        );
        assert_eq!(rolled, vec![source("src", "", 3), source("tests", "", 4),]);
    }

    #[test]
    fn rollup_keeps_files_shallower_than_the_depth_alone() {
        let rolled = rollup(
            vec![
                source("README.md", "unclassified", 1),
                source("src/lib.rs", "rust", 2),
            ],
            1,
        );
        assert_eq!(
            rolled,
            vec![source("README.md", "unclassified", 1), source("src", "", 2),]
        );
    }

    #[test]
    fn rollup_at_depth_two_splits_second_level_directories() {
        let rolled = rollup(
            vec![
                source("src/lib.rs", "rust", 1),
                source("src/app/main.rs", "rust", 2),
            ],
            2,
        );
        assert_eq!(
            rolled,
            vec![source("src/app", "", 2), source("src/lib.rs", "rust", 1),]
        );
    }

    #[test]
    fn rollup_sums_every_bucket() {
        let rolled = rollup(
            vec![
                FileCount {
                    path: "a/x.rs".to_owned(),
                    lang: "rust",
                    buckets: Buckets {
                        source: Count::default(),
                        test: Count {
                            added: 2,
                            removed: 0,
                        },
                        comment: Count {
                            added: 0,
                            removed: 1,
                        },
                        blank: Count::default(),
                    },
                },
                source("a/y.rs", "rust", 3),
            ],
            1,
        );
        assert_eq!(rolled.len(), 1);
        assert_eq!(rolled[0].path, "a");
        assert_eq!(rolled[0].lang, "");
        assert_eq!(
            rolled[0].buckets.test,
            Count {
                added: 2,
                removed: 0
            }
        );
        assert_eq!(
            rolled[0].buckets.source,
            Count {
                added: 3,
                removed: 0
            }
        );
        assert_eq!(
            rolled[0].buckets.comment,
            Count {
                added: 0,
                removed: 1
            }
        );
    }

    #[test]
    fn group_key_takes_the_first_depth_components() {
        assert_eq!(group_key("src/lib.rs", 1), "src");
        assert_eq!(group_key("src/lib.rs", 2), "src/lib.rs");
        assert_eq!(group_key("a/b/c.rs", 2), "a/b");
        assert_eq!(group_key("a/b/c.rs", 5), "a/b/c.rs");
        assert_eq!(group_key("README.md", 1), "README.md");
    }
}

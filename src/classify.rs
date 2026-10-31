//! Per-line classification: whether a line of a file version is blank, a
//! comment, code, or test.

use crate::lang;

/// The class of one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineClass {
    Blank,
    Comment,
    Code,
    Test,
}

/// A 1-based inclusive line span, e.g. the extent of one syntax node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u64,
    pub end: u64,
}

impl LineRange {
    fn contains(&self, line: u64) -> bool {
        self.start <= line && line <= self.end
    }
}

/// Classify every line of one file version.
pub fn classify_lines(
    lines: &[&str],
    comments: &[LineRange],
    tests: &[LineRange],
    is_test_path: bool,
) -> Vec<LineClass> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let number = (index + 1) as u64;
            if comments.iter().any(|range| range.contains(number)) {
                LineClass::Comment
            } else if line.trim().is_empty() {
                LineClass::Blank
            } else if tests.iter().any(|range| range.contains(number)) || is_test_path {
                LineClass::Test
            } else {
                LineClass::Code
            }
        })
        .collect()
}

/// Classify one fetched file version by its path. Note that for now, comments and tests is empty
/// because this is language specific. We haven't built that yet.
pub fn classify_blob(path: &str, text: &str) -> Vec<LineClass> {
    let lines: Vec<&str> = text.lines().collect();
    let is_test_path = lang::for_path(path).is_some_and(|lang| lang.is_test_path(path));
    classify_lines(&lines, &[], &[], is_test_path)
}

/// Per-line classes for one side of a diff: `path` decides language and
/// test-path, `text` the lines.
pub fn classify_side(path: Option<&str>, text: Option<&str>) -> Vec<LineClass> {
    match (path, text) {
        (Some(path), Some(text)) => classify_blob(path, text),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclassified_files_fall_back_to_blank_or_code() {
        let classes = classify_blob("data.xyz", "a\n\n  \nb\n");
        assert_eq!(
            classes,
            [
                LineClass::Code,
                LineClass::Blank,
                LineClass::Blank,
                LineClass::Code
            ]
        );
    }

    #[test]
    fn fixture_code_file_of_the_stub_language() {
        let text = concat!(
            "fn greet(name) {\n", // code
            "  print(name)\n",    // code
            "}\n",                // code
            "\n",                 // blank
            "fn check() {}\n",    // code
        );
        let classes = classify_blob("src/greet.stub", text);
        assert_eq!(
            classes,
            [
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Blank,
                LineClass::Code
            ]
        );
    }

    #[test]
    fn fixture_test_file_of_the_stub_language() {
        let text = concat!(
            "fn check_greet() {\n",      // test: the path is a test path
            "  assert(greet(\"hi\"))\n", // test
            "}\n",                       // test
            "\n",                        // blank
            "fn check_bye() {}\n",       // test
        );
        let classes = classify_blob("tests/greet.stub", text);
        assert_eq!(
            classes,
            [
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Blank,
                LineClass::Test
            ]
        );
    }

    #[test]
    fn fixture_test_file_matched_by_suffix_glob() {
        let classes = classify_blob("src/other_test.stub", "assert\n\nassert\n");
        assert_eq!(
            classes,
            [LineClass::Test, LineClass::Blank, LineClass::Test]
        );
    }

    #[test]
    fn comment_ranges_win_over_blank_lines() {
        let lines = ["/*", "", "*/", "code"];
        let comments = [LineRange { start: 1, end: 3 }];
        let classes = classify_lines(&lines, &comments, &[], false);
        assert_eq!(
            classes,
            [
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Code
            ]
        );
    }

    #[test]
    fn test_ranges_tag_code_but_not_blank_lines() {
        let lines = ["mod tests {", "", "  fn t() {}", "}", "code"];
        let tests = [LineRange { start: 1, end: 4 }];
        let classes = classify_lines(&lines, &[], &tests, false);
        assert_eq!(
            classes,
            [
                LineClass::Test,
                LineClass::Blank,
                LineClass::Test,
                LineClass::Test,
                LineClass::Code
            ]
        );
    }

    #[test]
    fn comments_beat_test_ranges() {
        let lines = ["/* c */", "x"];
        let comments = [LineRange { start: 1, end: 1 }];
        let tests = [LineRange { start: 1, end: 2 }];
        let classes = classify_lines(&lines, &comments, &tests, false);
        assert_eq!(classes, [LineClass::Comment, LineClass::Test]);
    }

    #[test]
    fn test_paths_beat_code_but_not_comments() {
        let lines = ["/* c */", "x", ""];
        let comments = [LineRange { start: 1, end: 1 }];
        let classes = classify_lines(&lines, &comments, &[], true);
        assert_eq!(
            classes,
            [LineClass::Comment, LineClass::Test, LineClass::Blank]
        );
    }

    #[test]
    fn ranges_beyond_the_file_never_match() {
        let lines = ["x"];
        let comments = [LineRange { start: 2, end: 9 }];
        let tests = [LineRange { start: 2, end: 9 }];
        let classes = classify_lines(&lines, &comments, &tests, false);
        assert_eq!(classes, [LineClass::Code]);
    }
}

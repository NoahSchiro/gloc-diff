//! Per-line classification: whether a line of a file version is blank, a
//! comment, code, or test.

/// The class of one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineClass {
    Blank,
    Comment,
    Code,
    Test,
}

/// A byte span of a file version, e.g. the extent of one syntax node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

/// Classify every line of one file version.
///
/// `comments` and `tests` are the extents of comment and inline-test syntax
/// nodes, sorted by start byte. A line is a comment when comment nodes cover
/// it and no code shares it (trailing comments stay code), a test when a
/// test node covers it or the whole file is a test path, and blank when it
/// holds only whitespace.
pub fn classify_lines(
    text: &str,
    comments: &[ByteRange],
    tests: &[ByteRange],
    is_test_path: bool,
) -> Vec<LineClass> {
    let bytes = text.as_bytes();
    let mut comments = Sweep::new(comments);
    let mut tests = Sweep::new(tests);
    let mut classes = Vec::new();
    for span in line_spans(bytes) {
        let (commented, code_outside) = comments.coverage(span.clone(), bytes);
        let class = if commented && !code_outside {
            LineClass::Comment
        } else if text[span.clone()].trim().is_empty() {
            LineClass::Blank
        } else if is_test_path || tests.intersects(span.clone()) {
            LineClass::Test
        } else {
            LineClass::Code
        };
        classes.push(class);
    }
    classes
}

/// Byte spans of each line: newline excluded, any carriage return included,
/// and a trailing newline adds no line.
fn line_spans(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            spans.push(start..offset);
            start = offset + 1;
        }
    }
    if start < bytes.len() {
        spans.push(start..bytes.len());
    }
    spans
}

/// Sorted byte ranges, swept in ascending line order so each range is
/// visited once across the whole file.
struct Sweep<'a> {
    ranges: &'a [ByteRange],
    cursor: usize,
}

impl<'a> Sweep<'a> {
    fn new(ranges: &'a [ByteRange]) -> Self {
        Sweep { ranges, cursor: 0 }
    }

    /// Does any range overlap `span`?
    fn intersects(&mut self, span: std::ops::Range<usize>) -> bool {
        self.seek(span.start);
        self.ranges[self.cursor..]
            .iter()
            .any(|range| range.start < span.end)
    }

    /// Does `span` overlap any range, and does it hold non-whitespace bytes
    /// outside them?
    fn coverage(&mut self, span: std::ops::Range<usize>, bytes: &[u8]) -> (bool, bool) {
        self.seek(span.start);
        let mut covered = false;
        let mut cursor = span.start;
        for &range in &self.ranges[self.cursor..] {
            if range.start >= span.end {
                break;
            }
            covered = true;
            let start = range.start.max(span.start).max(cursor);
            if has_code(bytes, cursor..start) {
                return (true, true);
            }
            cursor = cursor.max(range.end.min(span.end));
        }
        (covered, has_code(bytes, cursor..span.end))
    }

    /// Advance past ranges ending at or before `offset`.
    fn seek(&mut self, offset: usize) {
        while self.cursor < self.ranges.len() && self.ranges[self.cursor].end <= offset {
            self.cursor += 1;
        }
    }
}

/// Does the byte span hold anything besides whitespace?
fn has_code(bytes: &[u8], span: std::ops::Range<usize>) -> bool {
    bytes[span].iter().any(|byte| !byte.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte span of `needle`'s first occurrence in `text`.
    fn span(text: &str, needle: &str) -> ByteRange {
        let start = text.find(needle).expect("needle occurs in text");
        ByteRange {
            start,
            end: start + needle.len(),
        }
    }

    #[test]
    fn unclassified_text_falls_back_to_blank_or_code() {
        assert_eq!(
            classify_lines("a\n\n  \nb\n", &[], &[], false),
            [
                LineClass::Code,
                LineClass::Blank,
                LineClass::Blank,
                LineClass::Code
            ]
        );
    }

    #[test]
    fn test_paths_tag_non_blank_lines() {
        let text = "fn check_greet() {\n  assert(greet(\"hi\"))\n}\n\nfn check_bye() {}\n";
        assert_eq!(
            classify_lines(text, &[], &[], true),
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
    fn comment_ranges_win_over_blank_lines() {
        let text = "/*\n\n*/\ncode\n";
        let comments = [span(text, "/*\n\n*/")];
        assert_eq!(
            classify_lines(text, &comments, &[], false),
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
        let text = "mod tests {\n\n  fn t() {}\n}\ncode\n";
        let tests = [span(text, "mod tests {\n\n  fn t() {}\n}")];
        assert_eq!(
            classify_lines(text, &[], &tests, false),
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
        let text = "/* c */\nx\n";
        let comments = [span(text, "/* c */")];
        let tests = [span(text, "x")];
        assert_eq!(
            classify_lines(text, &comments, &tests, false),
            [LineClass::Comment, LineClass::Test]
        );
    }

    #[test]
    fn test_paths_beat_code_but_not_comments() {
        let text = "/* c */\nx\n\n";
        let comments = [span(text, "/* c */")];
        assert_eq!(
            classify_lines(text, &comments, &[], true),
            [LineClass::Comment, LineClass::Test, LineClass::Blank]
        );
    }

    #[test]
    fn ranges_past_the_text_never_match() {
        let comments = [ByteRange { start: 5, end: 9 }];
        let tests = [ByteRange { start: 5, end: 9 }];
        assert_eq!(
            classify_lines("x\n", &comments, &tests, false),
            [LineClass::Code]
        );
    }

    #[test]
    fn trailing_comments_stay_code() {
        let text = "let x = 1; // note\n";
        let comments = [span(text, "// note")];
        assert_eq!(
            classify_lines(text, &comments, &[], false),
            [LineClass::Code]
        );
    }

    #[test]
    fn indented_comment_lines_are_comments() {
        let text = "  // indented\n";
        let comments = [span(text, "// indented")];
        assert_eq!(
            classify_lines(text, &comments, &[], false),
            [LineClass::Comment]
        );
    }

    #[test]
    fn code_resuming_after_a_block_comment_stays_code() {
        let text = "/* c\n*/ let x = 1;\n";
        let comments = [span(text, "/* c\n*/")];
        assert_eq!(
            classify_lines(text, &comments, &[], false),
            [LineClass::Comment, LineClass::Code]
        );
    }

    #[test]
    fn ranges_ending_at_a_line_start_do_not_cover_it() {
        // A node ending exactly where the next line starts (e.g. a line
        // comment node that includes its newline) must not leak onto it.
        let text = "// c\n\nfn a() {}\n";
        let comments = [span(text, "// c\n")];
        assert_eq!(
            classify_lines(text, &comments, &[], false),
            [LineClass::Comment, LineClass::Blank, LineClass::Code]
        );
    }

    #[test]
    fn crlf_blank_lines_stay_blank() {
        let text = "a\r\n\r\nb\r\n";
        assert_eq!(
            classify_lines(text, &[], &[], false),
            [LineClass::Code, LineClass::Blank, LineClass::Code]
        );
    }

    #[test]
    fn empty_text_has_no_lines_but_a_lone_newline_has_one() {
        assert_eq!(classify_lines("", &[], &[], false), []);
        assert_eq!(classify_lines("\n", &[], &[], false), [LineClass::Blank]);
    }

    #[test]
    fn overlapping_ranges_never_double_count() {
        let text = "/* a */\nfn b() {}\n";
        let comments = [span(text, "/* a */"), span(text, "a */")];
        assert_eq!(
            classify_lines(text, &comments, &[], false),
            [LineClass::Comment, LineClass::Code]
        );
    }
}

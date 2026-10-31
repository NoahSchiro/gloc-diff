//! Parser for `git diff -U0` output:
//! Diff text comes in, per-file hunks out.
//!
//! It is useful to look at the output of git diff -U0 to see what we are parsing:
//!
//! ```text
//! --- a/ui/src/App.css
//! +++ b/ui/src/App.css
//! @@ -4,0 +5,74 @@
//! +
//! +.debug-panel {
//! +  position: fixed;
//! +  top: 10px;
//! -  color: #1c1c1c;
//! +  text-align: left;
//! +  user-select: none;
//! +}
//! ```
//!
//! ... and so on

/// One file's changes.
pub struct FileDiff<'a> {
    pub path: String,             // new file path
    pub old_path: Option<String>, //old file path (if renamed)
    pub is_binary: bool,
    pub hunks: Vec<Hunk<'a>>,
}

/// One `@@` hunk: where each version's lines start, how many lines the
/// header claims for each side, and the content lines with their
/// `-`/`+` prefix stripped.
pub struct Hunk<'a> {
    pub old_start: u64,
    pub old_lines: u64,
    pub new_start: u64,
    pub new_lines: u64,
    pub added: Vec<&'a str>,
    pub removed: Vec<&'a str>,
}

/// Parse a full `git diff` output into one [`FileDiff`] per file section.
pub fn parse_diff(diff_text: &str) -> Vec<FileDiff<'_>> {
    let mut files = Vec::new();
    let mut section: Option<Section<'_>> = None;
    let mut hunk: Option<OpenHunk<'_>> = None;

    for line in diff_text.lines() {
        if let Some(open) = hunk.as_mut() {
            let consumed = try_consume_hunk_line(open, line);
            let finished = consumed && open.is_done();
            if finished {
                let open = hunk.take().expect("hunk is open");
                if let Some(section) = section.as_mut() {
                    section.hunks.push(open.hunk);
                }
            }
            if consumed {
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("diff --git ") {
            close_hunk(&mut hunk, &mut section);
            if let Some(file) = section.take().and_then(Section::into_file) {
                files.push(file);
            }
            section = Some(Section::from_diff_line(rest));
            continue;
        }

        if let Some(rest) = line.strip_prefix("@@ ") {
            close_hunk(&mut hunk, &mut section);
            hunk = parse_hunk_header(rest);
            continue;
        }

        let Some(section) = section.as_mut() else {
            continue;
        };

        if let Some(rest) = line.strip_prefix("--- ") {
            section.old_path = diff_header_path(rest);
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            section.new_path = diff_header_path(rest);
        } else if let Some(rest) = line.strip_prefix("rename from ") {
            section.rename_from = take_path_operand(rest).0;
        } else if let Some(rest) = line.strip_prefix("rename to ") {
            section.rename_to = take_path_operand(rest).0;
        } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            section.is_binary = true;
        }
    }

    close_hunk(&mut hunk, &mut section);
    if let Some(file) = section.take().and_then(Section::into_file) {
        files.push(file);
    }
    files
}

/// Accumulated state for one `diff --git` section, resolved into a
/// [`FileDiff`] once the section closes.
struct Section<'a> {
    old_path: Option<String>,
    new_path: Option<String>,
    rename_from: Option<String>,
    rename_to: Option<String>,
    diff_line_old: Option<String>,
    diff_line_new: Option<String>,
    is_binary: bool,
    hunks: Vec<Hunk<'a>>,
}

impl<'a> Section<'a> {
    fn from_diff_line(rest: &str) -> Self {
        let (diff_line_old, diff_line_new) = parse_diff_git_line(rest);
        Section {
            old_path: None,
            new_path: None,
            rename_from: None,
            rename_to: None,
            diff_line_old,
            diff_line_new,
            is_binary: false,
            hunks: Vec::new(),
        }
    }

    /// The head-side path (`path`) and old-side path (`old_path`): the
    /// `+++ ` path wins for the head side, the `--- ` path for the old side,
    /// then the rename target and source, then the raw `diff --git` operands
    /// (mode-only changes). Sections with no recoverable head-side path never
    /// have hunks, so dropping them loses no churn.
    fn into_file(self) -> Option<FileDiff<'a>> {
        let old_path = self
            .old_path
            .or(self.rename_from)
            .or_else(|| self.diff_line_old.clone());
        let path = self
            .new_path
            .or(self.rename_to)
            .or_else(|| old_path.clone())
            .or(self.diff_line_new)
            .or(self.diff_line_old)?;
        Some(FileDiff {
            path,
            old_path,
            is_binary: self.is_binary,
            hunks: self.hunks,
        })
    }
}

/// A hunk whose content lines are still being consumed.
struct OpenHunk<'a> {
    hunk: Hunk<'a>,
    remaining_old: u64,
    remaining_new: u64,
}

impl OpenHunk<'_> {
    fn is_done(&self) -> bool {
        self.remaining_old == 0 && self.remaining_new == 0
    }
}

/// Try to interpret `line` as content of the open hunk, guided by the line
/// counts from the `@@` header. Content lines starting with `-` or `+` can
/// otherwise collide with `--- `/`+++ ` headers, so the counts decide.
fn try_consume_hunk_line<'a>(open: &mut OpenHunk<'a>, line: &'a str) -> bool {
    match line.as_bytes().first() {
        Some(b'+') if open.remaining_new > 0 => {
            open.hunk.added.push(&line[1..]);
            open.remaining_new -= 1;
        }
        Some(b'-') if open.remaining_old > 0 => {
            open.hunk.removed.push(&line[1..]);
            open.remaining_old -= 1;
        }
        Some(b' ') if open.remaining_old > 0 && open.remaining_new > 0 => {
            open.remaining_old -= 1;
            open.remaining_new -= 1;
        }
        // "\ No newline at end of file": a marker, not content.
        Some(b'\\') => {}
        _ => return false,
    }
    true
}

fn close_hunk<'a>(hunk: &mut Option<OpenHunk<'a>>, section: &mut Option<Section<'a>>) {
    if let Some(open) = hunk.take()
        && let Some(section) = section.as_mut()
    {
        section.hunks.push(open.hunk);
    }
}

/// `@@ -OLD[,OLD_COUNT] +NEW[,NEW_COUNT] @@ [section]`, after the leading
/// `@@ `. A missing `,COUNT` means 1.
fn parse_hunk_header<'a>(rest: &str) -> Option<OpenHunk<'a>> {
    let rest = rest.strip_prefix('-')?;
    let (old_start, rest) = take_u64(rest)?;
    let (old_lines, rest) = take_opt_count(rest);
    let rest = rest.strip_prefix(' ')?.strip_prefix('+')?;
    let (new_start, rest) = take_u64(rest)?;
    let (new_lines, rest) = take_opt_count(rest);
    rest.strip_prefix(" @@")?;
    let hunk = Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        added: Vec::new(),
        removed: Vec::new(),
    };
    Some(OpenHunk {
        remaining_old: hunk.old_lines,
        remaining_new: hunk.new_lines,
        hunk,
    })
}

fn take_u64(s: &str) -> Option<(u64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some((s[..end].parse().ok()?, &s[end..]))
}

fn take_opt_count(s: &str) -> (u64, &str) {
    match s.strip_prefix(',') {
        Some(rest) => match take_u64(rest) {
            Some((count, rest)) => (count, rest),
            None => (1, s),
        },
        None => (1, s),
    }
}

/// Path operand of a `--- `/`+++ ` line. `/dev/null` marks the absent side.
fn diff_header_path(s: &str) -> Option<String> {
    let (path, _) = take_path_operand(s);
    let path = path?;
    (path != "/dev/null").then(|| strip_diff_prefix(&path).to_string())
}

/// Operands of the `diff --git a/x b/y` line. Only a fallback: sections with
/// hunks get their paths from `--- `/`+++ `. Git quotes operands containing
/// spaces or non-ASCII bytes, so unquoted operands never contain spaces.
fn parse_diff_git_line(rest: &str) -> (Option<String>, Option<String>) {
    if rest.starts_with('"') {
        let (old, rest) = take_path_operand(rest);
        let (new, _) = take_path_operand(rest.trim_start());
        return (
            old.map(|p| strip_diff_prefix(&p).to_string()),
            new.map(|p| strip_diff_prefix(&p).to_string()),
        );
    }
    match rest.rfind(" b/") {
        Some(split) => (
            Some(strip_diff_prefix(&rest[..split]).to_string()),
            Some(strip_diff_prefix(&rest[split + 3..]).to_string()),
        ),
        None => (None, None),
    }
}

/// First path operand of a header line. Git quotes paths containing special
/// characters and escapes them C-style, including `\NNN` byte escapes.
/// Returns the decoded path and the remainder of the line.
fn take_path_operand(s: &str) -> (Option<String>, &str) {
    match s.strip_prefix('"') {
        Some(quoted) => {
            let bytes = quoted.as_bytes();
            let mut out: Vec<u8> = Vec::new();
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        let path = String::from_utf8_lossy(&out).into_owned();
                        return (Some(path), &quoted[i + 1..]);
                    }
                    b'\\' if i + 1 < bytes.len() => {
                        i += 1;
                        match bytes[i] {
                            b'\\' => out.push(b'\\'),
                            b'"' => out.push(b'"'),
                            b't' => out.push(b'\t'),
                            b'n' => out.push(b'\n'),
                            b'r' => out.push(b'\r'),
                            b'0'..=b'7' => {
                                let mut value: u32 = 0;
                                let mut digits = 0;
                                while digits < 3
                                    && i < bytes.len()
                                    && (b'0'..=b'7').contains(&bytes[i])
                                {
                                    value = value * 8 + u32::from(bytes[i] - b'0');
                                    digits += 1;
                                    i += 1;
                                }
                                out.push(value as u8);
                                i -= 1; // the loop tail advances again
                            }
                            other => {
                                out.push(b'\\');
                                out.push(other);
                            }
                        }
                        i += 1;
                    }
                    b => {
                        out.push(b);
                        i += 1;
                    }
                }
            }
            // Unterminated quote: keep what was decoded.
            (Some(String::from_utf8_lossy(&out).into_owned()), "")
        }
        None => {
            let end = s.find('\t').unwrap_or(s.len());
            (Some(s[..end].to_string()), &s[end..])
        }
    }
}

/// Remove the `a/`/`b/` prefix git prepends (absent under diff.noprefix).
fn strip_diff_prefix(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additions_and_deletions_with_context() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index 3f2f7a0..b4e9c1d 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
+    println!("hi");
     println!("bye");
-    println!("old");
+    println!("new");
 }
"#;
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "src/main.rs");
        assert!(!file.is_binary);
        assert_eq!(file.hunks.len(), 1);
        let hunk = &file.hunks[0];
        assert_eq!(
            (
                hunk.old_start,
                hunk.old_lines,
                hunk.new_start,
                hunk.new_lines
            ),
            (1, 4, 1, 5)
        );
        assert_eq!(
            hunk.added,
            vec!["    println!(\"hi\");", "    println!(\"new\");"]
        );
        assert_eq!(hunk.removed, vec!["    println!(\"old\");"]);
    }

    #[test]
    fn new_file_has_dev_null_old_path() {
        let diff = r#"diff --git a/new.rs b/new.rs
new file mode 100644
index 0000000..8baef1b
--- /dev/null
+++ b/new.rs
@@ -0,0 +1,2 @@
+fn a() {}
+
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "new.rs");
        let hunk = &files[0].hunks[0];
        assert_eq!((hunk.old_start, hunk.old_lines), (0, 0));
        assert_eq!(hunk.added, vec!["fn a() {}", ""]);
        assert!(hunk.removed.is_empty());
    }

    #[test]
    fn deleted_file_is_named_by_its_old_path() {
        let diff = r#"diff --git a/gone.rs b/gone.rs
deleted file mode 100644
index 8baef1b..0000000
--- a/gone.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn a() {}
-fn b() {}
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "gone.rs");
        assert_eq!(files[0].hunks[0].removed, vec!["fn a() {}", "fn b() {}"]);
    }

    #[test]
    fn rename_with_edits_uses_the_new_path() {
        let diff = r#"diff --git a/old_name.rs b/new_name.rs
similarity index 80%
rename from old_name.rs
rename to new_name.rs
index 1234567..89abcde 100644
--- a/old_name.rs
+++ b/new_name.rs
@@ -1 +1 @@
-fn a() {}
+fn b() {}
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "new_name.rs");
        assert_eq!(files[0].hunks[0].added, vec!["fn b() {}"]);
        assert_eq!(files[0].hunks[0].removed, vec!["fn a() {}"]);
    }

    #[test]
    fn pure_rename_has_no_hunks() {
        let diff = r#"diff --git a/a.txt b/b.txt
similarity index 100%
rename from a.txt
rename to b.txt
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "b.txt");
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn binary_file_has_no_hunks() {
        let diff = r#"diff --git a/logo.png b/logo.png
index 8b13789..baadc0d 100644
Binary files a/logo.png and b/logo.png differ
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "logo.png");
        assert!(files[0].is_binary);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn no_newline_markers_are_not_content() {
        let diff = r#"diff --git a/x.rs b/x.rs
index 1111111..2222222 100644
--- a/x.rs
+++ b/x.rs
@@ -1 +1 @@
-fn old() {}
\ No newline at end of file
+fn new() {}
\ No newline at end of file
"#;
        let files = parse_diff(diff);
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.added, vec!["fn new() {}"]);
        assert_eq!(hunk.removed, vec!["fn old() {}"]);
    }

    #[test]
    fn hunk_header_without_counts_means_one_line() {
        let diff = r#"diff --git a/x.txt b/x.txt
index 1111111..2222222 100644
--- a/x.txt
+++ b/x.txt
@@ -3 +3 @@
-old
+new
"#;
        let files = parse_diff(diff);
        let hunk = &files[0].hunks[0];
        assert_eq!(
            (
                hunk.old_start,
                hunk.old_lines,
                hunk.new_start,
                hunk.new_lines
            ),
            (3, 1, 3, 1)
        );
    }

    #[test]
    fn content_lines_looking_like_headers_stay_content() {
        let diff = r#"diff --git a/tricky.txt b/tricky.txt
index 1111111..2222222 100644
--- a/tricky.txt
+++ b/tricky.txt
@@ -1,2 +1,2 @@
--- not a header
+++ not a header either
"#;
        let files = parse_diff(diff);
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.removed, vec!["-- not a header"]);
        assert_eq!(hunk.added, vec!["++ not a header either"]);
    }

    #[test]
    fn content_lines_looking_like_hunk_headers_stay_content() {
        let diff = r#"diff --git a/tricky.txt b/tricky.txt
index 1111111..2222222 100644
--- a/tricky.txt
+++ b/tricky.txt
@@ -1,2 +1,2 @@
-@@ -1 +1 @@
+@@ -9 +9 @@
"#;
        let files = parse_diff(diff);
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.removed, vec!["@@ -1 +1 @@"]);
        assert_eq!(hunk.added, vec!["@@ -9 +9 @@"]);
    }

    #[test]
    fn quoted_and_escaped_paths_are_decoded() {
        let diff = r#"diff --git "a/dir with space/f.rs" "b/dir with space/f.rs"
index 1111111..2222222 100644
--- "a/dir with space/f.rs"
+++ "b/dir with space/f.rs"
@@ -1 +1 @@
-x
+y
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "dir with space/f.rs");
    }

    #[test]
    fn octal_escapes_decode_to_utf8() {
        let diff = r#"diff --git "a/\303\251.txt" "b/\303\251.txt"
index 1111111..2222222 100644
--- "a/\303\251.txt"
+++ "b/\303\251.txt"
@@ -1 +1 @@
-x
+y
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "é.txt");
    }

    #[test]
    fn multiple_hunks_stay_separate() {
        let diff = r#"diff --git a/x.txt b/x.txt
index 1111111..2222222 100644
--- a/x.txt
+++ b/x.txt
@@ -1 +1 @@
-a
+b
@@ -10 +10 @@
-x
+y
"#;
        let files = parse_diff(diff);
        assert_eq!(files[0].hunks.len(), 2);
        assert_eq!(files[0].hunks[0].old_start, 1);
        assert_eq!(files[0].hunks[0].removed, vec!["a"]);
        assert_eq!(files[0].hunks[1].old_start, 10);
        assert_eq!(files[0].hunks[1].added, vec!["y"]);
    }

    #[test]
    fn empty_diff_yields_no_files() {
        assert!(parse_diff("").is_empty());
        assert!(parse_diff("\n").is_empty());
    }
}

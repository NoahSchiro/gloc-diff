//! End-to-end tests

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// A throwaway git repository under the OS temp dir, removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("gloc-it-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp repo dir");
        git(&dir, ["init", "-b", "main"]);
        git(&dir, ["config", "user.email", "gloc@example.com"]);
        git(&dir, ["config", "user.name", "gloc"]);
        Repo { dir }
    }

    fn commit(&self, path: &str, contents: &str, message: &str) {
        self.write(path, contents);
        git(&self.dir, ["add", path]);
        git(&self.dir, ["commit", "-m", message]);
    }

    /// Write a file and track it even if ignore rules cover it.
    fn commit_forced(&self, path: &str, contents: &str, message: &str) {
        self.write(path, contents);
        git(&self.dir, ["add", "--force", path]);
        git(&self.dir, ["commit", "-m", message]);
    }

    fn write(&self, path: &str, contents: &str) {
        let file = self.dir.join(path);
        fs::create_dir_all(file.parent().expect("path has a parent")).expect("create dirs");
        fs::write(&file, contents).expect("write file");
    }

    fn branch(&self, name: &str) {
        git(&self.dir, ["checkout", "-b", name]);
    }

    fn rename(&self, from: &str, to: &str, message: &str) {
        git(&self.dir, ["mv", from, to]);
        git(&self.dir, ["commit", "-m", message]);
    }

    fn delete(&self, path: &str, message: &str) {
        git(&self.dir, ["rm", path]);
        git(&self.dir, ["commit", "-m", message]);
    }

    fn gloc(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gloc"))
            .current_dir(&self.dir)
            .args(args)
            .output()
            .expect("run gloc")
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn git<'a>(dir: &Path, args: impl IntoIterator<Item = &'a str> + std::fmt::Debug) {
    let args: Vec<&str> = args.into_iter().collect();
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(&args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unclassified_files_fall_back_to_non_blank_counting() {
    let repo = Repo::new();
    repo.commit("src/lib.rs", "fn a() {}\n\nfn b() {}\n", "base");
    repo.branch("feature");
    repo.commit(
        "src/lib.rs",
        "fn a() {}\n\nfn b() {}\nfn c() {}\nfn d() {}\n\n",
        "grow",
    );

    let out = repo.gloc(&["--base", "main"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!(
            "file        lang       blank            comment             test             source         churn\n",
            "                        +        -        +        -        +        -        +        -\n",
            "src/lib.rs  rust        1        0        0        0        0        0        2        0        3\n",
            "total                   1        0        0        0        0        0        2        0        3\n",
        )
    );
}

#[test]
fn stub_language_splits_source_and_test_churn() {
    let repo = Repo::new();
    repo.commit("src/lib.stub", "code a\ncode b\n", "base");
    repo.commit("tests/lib.stub", "assert a\n", "base");
    repo.branch("feature");
    repo.commit("src/lib.stub", "code a\ncode b\ncode c\n", "grow");
    repo.commit("tests/lib.stub", "assert a\nassert b\n\nassert c\n", "grow");

    // src/ is source churn; tests/ matches the stub language's test globs,
    // so its non-blank lines are test churn and its blank line is blank churn.
    let out = repo.gloc(&["--base", "main"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!(
            "file            lang       blank            comment             test             source         churn\n",
            "                            +        -        +        -        +        -        +        -\n",
            "src/lib.stub    stub        0        0        0        0        0        0        1        0        1\n",
            "tests/lib.stub  stub        1        0        0        0        2        0        0        0        3\n",
            "total                       1        0        0        0        2        0        1        0        4\n",
        )
    );

    // The gate counts every bucket: churn is 4.
    let over = repo.gloc(&["--base", "main", "--max-total", "3"]);
    assert_eq!(over.status.code(), Some(1));
    let within = repo.gloc(&["--base", "main", "--max-total", "4"]);
    assert!(within.status.success());
}

#[test]
fn fails_over_the_max_total_threshold() {
    let repo = Repo::new();
    repo.commit("src/lib.rs", "fn a() {}\n", "base");
    repo.branch("feature");
    repo.commit("src/lib.rs", "fn a() {}\nfn b() {}\nfn c() {}\n", "grow");

    let under = repo.gloc(&["--base", "main", "--max-total", "2"]);
    assert!(under.status.success());

    let over = repo.gloc(&["--base", "main", "--max-total", "1"]);
    assert_eq!(over.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&over.stderr).contains("max-total"));
}

#[test]
fn pure_rename_adds_no_churn() {
    let repo = Repo::new();
    repo.commit("old.txt", "one\n", "base");
    repo.branch("feature");
    repo.rename("old.txt", "new.txt", "move");

    let out = repo.gloc(&["--base", "main"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("new.txt"), "table: {stdout}");

    let within = repo.gloc(&["--base", "main", "--max-total", "0"]);
    assert!(within.status.success());
}

#[test]
fn deleted_file_counts_removed_lines() {
    let repo = Repo::new();
    repo.commit("gone.txt", "one\ntwo\nthree\n", "base");
    repo.branch("feature");
    repo.delete("gone.txt", "drop");

    let out = repo.gloc(&["--base", "main"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("gone.txt"),
        "table: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let over = repo.gloc(&["--base", "main", "--max-total", "2"]);
    assert_eq!(over.status.code(), Some(1));
}

#[test]
fn unknown_base_ref_is_an_error_not_a_crash() {
    let repo = Repo::new();
    repo.commit("a.txt", "x\n", "base");

    let out = repo.gloc(&["--base", "does-not-exist"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("failed"));
}

#[test]
fn exclude_glob_drops_files_from_the_table_and_total() {
    let repo = Repo::new();
    repo.commit("src/lib.rs", "fn a()\n", "base");
    repo.commit("Cargo.lock", "one\ntwo\n", "base");
    repo.branch("feature");
    repo.commit("src/lib.rs", "fn a()\nfn b()\n", "grow");
    repo.commit("Cargo.lock", "one\ntwo\nthree\nfour\n", "bump");

    // Three lines of churn: 2 from the lockfile, 1 from the source file.
    let over = repo.gloc(&["--base", "main", "--max-total", "2"]);
    assert_eq!(over.status.code(), Some(1));

    let out = repo.gloc(&["--base", "main", "--exclude", "**/*.lock"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("src/lib.rs"), "table: {stdout}");
    assert!(!stdout.contains("Cargo.lock"), "table: {stdout}");

    let within = repo.gloc(&[
        "--base",
        "main",
        "--exclude",
        "**/*.lock",
        "--max-total",
        "2",
    ]);
    assert!(within.status.success());
}

#[test]
fn include_glob_acts_as_an_allowlist() {
    let repo = Repo::new();
    repo.commit("src/lib.rs", "fn a()\n", "base");
    repo.commit("doc/guide.md", "# guide\n", "base");
    repo.branch("feature");
    repo.commit("src/lib.rs", "fn a()\nfn b()\n", "grow");
    repo.commit("doc/guide.md", "# guide\n# more\n", "grow");

    let out = repo.gloc(&["--base", "main", "--include", "src/**"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("src/lib.rs"), "table: {stdout}");
    assert!(!stdout.contains("doc/guide.md"), "table: {stdout}");
    // Total churn is 1, so a stricter gate passes only because doc/ is out.
    let within = repo.gloc(&["--base", "main", "--include", "src/**", "--max-total", "1"]);
    assert!(within.status.success());
}

#[test]
fn regex_flags_filter_paths() {
    let repo = Repo::new();
    repo.commit("src/gen/old.rs", "fn a()\n", "base");
    repo.commit("src/app.rs", "fn a()\n", "base");
    repo.branch("feature");
    repo.commit("src/gen/old.rs", "fn a()\nfn b()\n", "grow");
    repo.commit("src/app.rs", "fn a()\nfn b()\n", "grow");

    let out = repo.gloc(&["--base", "main", "--exclude-re", "^src/gen/"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("src/gen/old.rs"), "table: {stdout}");
    assert!(stdout.contains("src/app.rs"), "table: {stdout}");

    let out = repo.gloc(&["--base", "main", "--include-re", "gen"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("src/gen/old.rs"), "table: {stdout}");
    assert!(!stdout.contains("src/app.rs"), "table: {stdout}");
}

#[test]
fn config_file_applies_and_cli_overrides_per_option() {
    let repo = Repo::new();
    repo.commit(".gloc.toml", "exclude = [\"**/*.lock\"]\n", "config");
    repo.commit("src/lib.rs", "fn a()\n", "base");
    repo.commit("Cargo.lock", "one\ntwo\n", "base");
    repo.branch("feature");
    repo.commit("src/lib.rs", "fn a()\nfn b()\n", "grow");
    repo.commit("Cargo.lock", "one\ntwo\nthree\nfour\n", "bump");

    // The config file's exclude keeps the lockfile out on its own.
    let out = repo.gloc(&["--base", "main"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("Cargo.lock"), "table: {stdout}");

    // A CLI --exclude replaces the config list for that option, so the
    // lockfile (no longer excluded) shows up while src/ is dropped.
    let out = repo.gloc(&["--base", "main", "--exclude", "src/**"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Cargo.lock"), "table: {stdout}");
    assert!(!stdout.contains("src/lib.rs"), "table: {stdout}");
}

#[test]
fn gitignore_skips_tracked_but_ignored_files() {
    let repo = Repo::new();
    repo.commit(".gitignore", "secret*\n", "ignore rules");
    repo.commit_forced("secret.txt", "one\ntwo\n", "force add");
    repo.commit("src/lib.rs", "fn a()\n", "base");
    repo.branch("feature");
    repo.commit_forced("secret.txt", "one\ntwo\nthree\n", "grow");
    repo.commit("src/lib.rs", "fn a()\nfn b()\n", "grow");

    // Default: the ignored file's churn does not count.
    let out = repo.gloc(&["--base", "main"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("secret.txt"), "table: {stdout}");
    let within = repo.gloc(&["--base", "main", "--max-total", "1"]);
    assert!(within.status.success());

    // --no-gitignore counts it after all.
    let out = repo.gloc(&["--base", "main", "--no-gitignore"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("secret.txt"), "table: {stdout}");
    let over = repo.gloc(&["--base", "main", "--no-gitignore", "--max-total", "1"]);
    assert_eq!(over.status.code(), Some(1));
}

#[test]
fn nested_gitignore_whitelist_applies() {
    let repo = Repo::new();
    repo.commit(".gitignore", "*.log\n", "ignore rules");
    repo.commit("sub/.gitignore", "!keep.log\n", "whitelist");
    repo.commit_forced("sub/keep.log", "a\n", "force add");
    repo.commit_forced("sub/other.log", "x\n", "force add");
    repo.branch("feature");
    repo.commit_forced("sub/keep.log", "a\nb\n", "grow");
    repo.commit_forced("sub/other.log", "x\ny\n", "grow");

    let out = repo.gloc(&["--base", "main"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("sub/keep.log"), "table: {stdout}");
    assert!(!stdout.contains("sub/other.log"), "table: {stdout}");
}

#[test]
fn invalid_patterns_are_errors_not_crashes() {
    let repo = Repo::new();
    repo.commit("a.txt", "x\n", "base");

    let out = repo.gloc(&["--base", "main", "--exclude-re", "("]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid regex"));

    let out = repo.gloc(&["--base", "main", "--exclude", "["]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid glob"));
}

#[test]
fn invalid_config_file_is_an_error() {
    let repo = Repo::new();
    repo.commit("a.txt", "x\n", "base");
    repo.commit(".gloc.toml", "exlude = [\"x\"]\n", "typo");

    let out = repo.gloc(&["--base", "main"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(".gloc.toml"), "stderr: {stderr}");
}

#[test]
fn rust_pr_splits_source_test_comment_and_blank_churn() {
    let repo = Repo::new();
    repo.commit(
        "src/lib.rs",
        concat!(
            "fn greet(name: &str) -> String {\n",
            "    format!(\"hello {name}\")\n",
            "}\n",
        ),
        "base",
    );
    repo.branch("feature");
    repo.commit(
        "src/lib.rs",
        concat!(
            "fn greet(name: &str) -> String {\n",
            "    format!(\"hello {name}\")\n",
            "}\n",
            "\n",                                               // blank
            "/// Greets loudly.\n",                             // comment
            "fn shout(name: &str) -> String {\n",               // source
            "    format!(\"HELLO {name}\") // note\n",          // source: trailing comment
            "}\n",                                              // source
            "\n",                                               // blank
            "#[cfg(test)]\n",                                   // test
            "mod tests {\n",                                    // test
            "    use super::*;\n",                              // test
            "\n",                                               // blank
            "    #[test]\n",                                    // test
            "    fn greets() {\n",                              // test
            "        assert_eq!(greet(\"x\"), \"hello x\");\n", // test
            "    }\n",                                          // test
            "}\n",                                              // test
        ),
        "shout and inline tests",
    );
    repo.commit(
        "tests/it.rs",
        concat!(
            "#[test]\n",
            "fn it_works() {\n",
            "    assert_eq!(2 + 2, 4);\n",
            "}\n",
        ),
        "integration test",
    );
    repo.commit(
        "src/shape.rs",
        concat!(
            "#[derive(Debug)]\n", // an attribute that is not about tests
            "struct Shape;\n",
            "\n",
            "fn area(w: u64, h: u64) -> u64 {\n",
            "    w * h\n",
            "}\n",
        ),
        "shape",
    );

    let out = repo.gloc(&["--base", "main"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!(
            "file          lang       blank            comment             test             source         churn\n",
            "                          +        -        +        -        +        -        +        -\n",
            "src/lib.rs    rust        3        0        1        0        8        0        3        0       15\n",
            "src/shape.rs  rust        1        0        0        0        0        0        5        0        6\n",
            "tests/it.rs   rust        0        0        0        0        4        0        0        0        4\n",
            "total                     4        0        1        0       12        0        8        0       25\n",
        )
    );

    let within = repo.gloc(&["--base", "main", "--max-total", "25"]);
    assert!(within.status.success());
    let over = repo.gloc(&["--base", "main", "--max-total", "24"]);
    assert_eq!(over.status.code(), Some(1));
}

#[test]
fn block_comment_spanning_hunks_counts_as_comment() {
    let repo = Repo::new();
    repo.commit(
        "src/lib.rs",
        concat!(
            "fn a() {}\n",
            "/* long block\n",
            "   comment\n",
            "   end */\n",
            "fn b() {}\n",
        ),
        "base",
    );
    repo.branch("feature");
    // Lines inserted inside the comment, plus code appended after it: two
    // hunks, with the comment's delimiters outside both hunks' context.
    repo.commit(
        "src/lib.rs",
        concat!(
            "fn a() {}\n",
            "/* long block\n",
            "   comment\n",
            "   inserted one\n",
            "   inserted two\n",
            "   end */\n",
            "fn b() {}\n",
            "fn c() {}\n",
        ),
        "grow",
    );

    let out = repo.gloc(&["--base", "main"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!(
            "file        lang       blank            comment             test             source         churn\n",
            "                        +        -        +        -        +        -        +        -\n",
            "src/lib.rs  rust        0        0        2        0        0        0        1        0        3\n",
            "total                   0        0        2        0        0        0        1        0        3\n",
        )
    );
}

#[test]
fn crlf_file_classifies_comments_and_code() {
    let repo = Repo::new();
    repo.commit("src/lib.rs", "fn a() {}\r\n", "base");
    repo.branch("feature");
    repo.commit(
        "src/lib.rs",
        "fn a() {}\r\n// note\r\n\r\nfn b() {}\r\n",
        "grow",
    );

    let out = repo.gloc(&["--base", "main"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!(
            "file        lang       blank            comment             test             source         churn\n",
            "                        +        -        +        -        +        -        +        -\n",
            "src/lib.rs  rust        1        0        1        0        0        0        1        0        3\n",
            "total                   1        0        1        0        0        0        1        0        3\n",
        )
    );

    let within = repo.gloc(&["--base", "main", "--max-total", "3"]);
    assert!(within.status.success());
    let over = repo.gloc(&["--base", "main", "--max-total", "2"]);
    assert_eq!(over.status.code(), Some(1));
}

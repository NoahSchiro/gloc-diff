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
        let file = self.dir.join(path);
        fs::create_dir_all(file.parent().expect("path has a parent")).expect("create dirs");
        fs::write(&file, contents).expect("write file");
        git(&self.dir, ["add", path]);
        git(&self.dir, ["commit", "-m", message]);
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
fn counts_non_blank_churn_in_a_temp_repo() {
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
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Three lines added, one of them blank: churn counts 2.
    assert_eq!(
        stdout,
        concat!(
            "file          added  removed    churn\n",
            "src/lib.rs        2        0        2\n",
            "total             2        0        2\n",
        )
    );
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

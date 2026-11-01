//! Which file paths count toward churn:
//!
//! - include/exclude globs and regexes,
//! - gitignore rules for tracked-but-ignored files.

use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use globset::GlobSet;
use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use regex::RegexSet;

use crate::globs;

/// Raw path filters, before compilation. The CLI and `.gloc.toml` both
/// produce one of these.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Filters {
    /// Allowlist globs.
    pub include: Vec<String>,
    /// Blocklist globs.
    pub exclude: Vec<String>,
    /// Allowlist regexes.
    pub include_re: Vec<String>,
    /// Blocklist regexes.
    pub exclude_re: Vec<String>,
    /// Do not respect .gitignore.
    pub no_gitignore: bool,
}

impl Filters {
    /// Combine CLI and config-file filters per filter.
    pub fn merge(cli: Filters, file: Filters) -> Filters {
        fn pick(cli: Vec<String>, file: Vec<String>) -> Vec<String> {
            if cli.is_empty() { file } else { cli }
        }
        Filters {
            include: pick(cli.include, file.include),
            exclude: pick(cli.exclude, file.exclude),
            include_re: pick(cli.include_re, file.include_re),
            exclude_re: pick(cli.exclude_re, file.exclude_re),
            no_gitignore: cli.no_gitignore || file.no_gitignore,
        }
    }
}

/// One gitignore file.
struct GitignoreSource {
    dir: PathBuf,
    matcher: Gitignore,
}

impl fmt::Debug for GitignoreSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitignoreSource")
            .field("dir", &self.dir)
            .field("globs", &self.matcher.len())
            .finish()
    }
}

/// Compiled filters deciding which file paths count toward churn.
#[derive(Debug)]
pub struct Matchers {
    include_globs: GlobSet,
    include_regexes: RegexSet,
    exclude_globs: GlobSet,
    exclude_regexes: RegexSet,
    gitignores: Vec<GitignoreSource>,
}

/// Why raw filters could not be compiled.
#[derive(Debug)]
pub enum FilterError {
    Globs {
        source: globs::GlobError,
    },
    Regex {
        source: regex::Error,
    },
    Gitignore {
        path: PathBuf,
        source: ignore::Error,
    },
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterError::Globs { source } => write!(f, "{source}"),
            FilterError::Regex { source } => write!(f, "invalid regex: {source}"),
            FilterError::Gitignore { path, source } => {
                write!(
                    f,
                    "cannot read ignore rules from {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for FilterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FilterError::Globs { source } => Some(source),
            FilterError::Regex { source } => Some(source),
            FilterError::Gitignore { source, .. } => Some(source),
        }
    }
}

impl Matchers {
    /// Compile raw filters into matchers.
    pub fn compile(
        filters: &Filters,
        repo_root: &Path,
        changed_dirs: &BTreeSet<PathBuf>,
    ) -> Result<Matchers, FilterError> {
        Ok(Matchers {
            include_globs: compile_globs(&filters.include)?,
            include_regexes: compile_regexes(&filters.include_re)?,
            exclude_globs: compile_globs(&filters.exclude)?,
            exclude_regexes: compile_regexes(&filters.exclude_re)?,
            gitignores: if filters.no_gitignore {
                Vec::new()
            } else {
                load_gitignores(repo_root, changed_dirs)?
            },
        })
    }

    /// Does provided `path` pass the filter?
    pub fn keeps(&self, path: &str) -> bool {
        if self.exclude_globs.is_match(path) || self.exclude_regexes.is_match(path) {
            return false;
        }
        if self.gitignore_ignored(Path::new(path)) {
            return false;
        }
        let allowlist_active = !self.include_globs.is_empty() || !self.include_regexes.is_empty();
        !allowlist_active
            || self.include_globs.is_match(path)
            || self.include_regexes.is_match(path)
    }

    /// Does provided `path` get .gitignored?
    fn gitignore_ignored(&self, path: &Path) -> bool {
        let chain: Vec<&Path> = path.ancestors().collect();
        for node in chain.into_iter().rev() {
            match self.gitignore_decision(node, node != path) {
                Some(true) => return true,
                Some(false) if node == path => return false,
                _ => {}
            }
        }
        false
    }

    /// The decision of the highest-precedence ignore source whose directory
    /// contains `node`: `Some(true)` ignored, `Some(false)` whitelisted,
    /// `None` no opinion.
    fn gitignore_decision(&self, node: &Path, is_dir: bool) -> Option<bool> {
        self.gitignores
            .iter()
            .rev()
            .filter(|source| source.dir.as_os_str().is_empty() || node.starts_with(&source.dir))
            .find_map(|source| match source.matcher.matched(node, is_dir) {
                Match::None => None,
                Match::Ignore(_) => Some(true),
                Match::Whitelist(_) => Some(false),
            })
    }
}

/// Compile include/exclude globs into one set, gitignore-style.
fn compile_globs(patterns: &[String]) -> Result<GlobSet, FilterError> {
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();
    globs::compile(&patterns).map_err(|source| FilterError::Globs { source })
}

/// Compile include/exclude regexes.
fn compile_regexes(patterns: &[String]) -> Result<RegexSet, FilterError> {
    RegexSet::new(patterns).map_err(|source| FilterError::Regex { source })
}

/// Prioritize `.git/info/exclude`, then every relevant `.gitignore` from the root
/// down. Later sources override earlier ones, as in git.
fn load_gitignores(
    repo_root: &Path,
    changed_dirs: &BTreeSet<PathBuf>,
) -> Result<Vec<GitignoreSource>, FilterError> {
    let mut sources = Vec::new();
    let info_exclude = repo_root.join(".git").join("info").join("exclude");
    if info_exclude.is_file() {
        sources.push(load_gitignore(Path::new(""), &info_exclude)?);
    }
    for dir in gitignore_dirs(changed_dirs) {
        let file = repo_root.join(&dir).join(".gitignore");
        if file.is_file() {
            sources.push(load_gitignore(&dir, &file)?);
        }
    }
    Ok(sources)
}

/// Read one .gitignore file, anchored at `dir`.
fn load_gitignore(dir: &Path, file: &Path) -> Result<GitignoreSource, FilterError> {
    let mut builder = GitignoreBuilder::new(dir);
    if let Some(source) = builder.add(file) {
        return Err(FilterError::Gitignore {
            path: file.to_owned(),
            source,
        });
    }
    let matcher = builder.build().map_err(|source| FilterError::Gitignore {
        path: file.to_owned(),
        source,
    })?;
    Ok(GitignoreSource {
        dir: dir.to_owned(),
        matcher,
    })
}

/// Directories whose `.gitignore` may apply to a changed file. Every
/// ancestor of every changed directory, sorted shallow to deep (same
/// precedence git gives gitignore files.)
fn gitignore_dirs(changed_dirs: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut dirs: Vec<PathBuf> = changed_dirs
        .iter()
        .flat_map(|dir| dir.ancestors())
        .filter(|dir| seen.insert(dir.to_path_buf()))
        .map(Path::to_path_buf)
        .collect();
    dirs.sort_by_key(|dir| dir.components().count());
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn opts(
        include: &[&str],
        exclude: &[&str],
        include_re: &[&str],
        exclude_re: &[&str],
    ) -> Filters {
        Filters {
            include: include.iter().map(|s| s.to_string()).collect(),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            include_re: include_re.iter().map(|s| s.to_string()).collect(),
            exclude_re: exclude_re.iter().map(|s| s.to_string()).collect(),
            no_gitignore: false,
        }
    }

    /// A root that does not exist, so no ignore files are ever found.
    fn bare_filters(filters: &Filters, paths: &[&str]) -> Matchers {
        compile_for_paths(filters, Path::new("/gloc-test-root-does-not-exist"), paths)
    }

    fn compile_for_paths(filters: &Filters, root: &Path, paths: &[&str]) -> Matchers {
        let dirs: BTreeSet<PathBuf> = paths
            .iter()
            .map(|path| {
                Path::new(path)
                    .parent()
                    .expect("test path has a parent")
                    .to_owned()
            })
            .collect();
        Matchers::compile(filters, root, &dirs).expect("filters compile")
    }

    fn kept(filters: &Filters, path: &str) -> bool {
        bare_filters(filters, &[path]).keeps(path)
    }

    /// A throwaway directory under the OS temp dir, removed on drop.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("gloc-filter-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create temp dir");
            TempRoot(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn write(&self, rel: &str, contents: &str) {
            let file = self.0.join(rel);
            fs::create_dir_all(file.parent().expect("rel path has a parent")).expect("create dirs");
            fs::write(file, contents).expect("write file");
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn excludes_beat_includes() {
        let filters = opts(&["**/*.rs"], &["tests/**"], &[], &[]);
        assert!(kept(&filters, "src/main.rs"));
        assert!(!kept(&filters, "tests/cli.rs"));
        assert!(!kept(&filters, "README.md"));
    }

    #[test]
    fn include_acts_as_an_allowlist() {
        let filters = opts(&["src/**"], &[], &[], &[]);
        assert!(kept(&filters, "src/main.rs"));
        assert!(!kept(&filters, "doc/guide.md"));
        assert!(!kept(&filters, "main.rs"));
    }

    #[test]
    fn include_globs_and_regexes_are_unioned() {
        let filters = opts(&["src/**"], &[], &["^lib/"], &[]);
        assert!(kept(&filters, "src/main.rs"));
        assert!(kept(&filters, "lib/old.rs"));
        assert!(!kept(&filters, "doc/guide.md"));
    }

    #[test]
    fn exclude_globs_and_regexes_are_unioned() {
        let filters = opts(&[], &["**/*.lock"], &[], &["vendor"]);
        assert!(!kept(&filters, "Cargo.lock"));
        assert!(!kept(&filters, "a/b/Cargo.lock"));
        assert!(!kept(&filters, "third_party/vendor/x.rs"));
        assert!(kept(&filters, "src/main.rs"));
    }

    #[test]
    fn basename_globs_match_at_any_depth() {
        let filters = opts(&[], &["*.lock"], &[], &[]);
        assert!(!kept(&filters, "Cargo.lock"));
        assert!(!kept(&filters, "a/b/Cargo.lock"));
        assert!(kept(&filters, "a/b/lock"));
    }

    #[test]
    fn anchored_globs_stay_at_the_root() {
        let filters = opts(&[], &["tests/**"], &[], &[]);
        assert!(!kept(&filters, "tests/cli.rs"));
        assert!(kept(&filters, "a/tests/cli.rs"));
    }

    #[test]
    fn directory_patterns_cover_their_contents() {
        let filters = opts(&[], &["build"], &[], &[]);
        assert!(!kept(&filters, "build/out.o"));
        assert!(!kept(&filters, "a/build/out.o"));
        assert!(kept(&filters, "a/building/out.o"));
    }

    #[test]
    fn regexes_search_the_whole_path() {
        let filters = opts(&[], &[], &[], &["^src/gen/"]);
        assert!(!kept(&filters, "src/gen/old.rs"));
        assert!(kept(&filters, "src/app/gen.rs"));
    }

    #[test]
    fn cli_lists_replace_config_lists_per_option() {
        let cli = opts(&["src/**"], &[], &[], &[]);
        let file = Filters {
            include: vec!["docs/**".into()],
            exclude: vec!["**/*.lock".into()],
            include_re: vec!["^lib".into()],
            exclude_re: vec!["vend".into()],
            no_gitignore: true,
        };
        let merged = Filters::merge(cli, file);
        assert_eq!(merged.include, ["src/**"]);
        assert_eq!(merged.exclude, ["**/*.lock"]);
        assert_eq!(merged.include_re, ["^lib"]);
        assert_eq!(merged.exclude_re, ["vend"]);
        assert!(merged.no_gitignore);
    }

    #[test]
    fn config_applies_when_the_cli_is_silent() {
        let merged = Filters::merge(Filters::default(), opts(&["a"], &["b"], &[], &[]));
        assert_eq!(merged.include, ["a"]);
        assert_eq!(merged.exclude, ["b"]);
        assert!(!merged.no_gitignore);
    }

    #[test]
    fn gitignore_rules_drop_tracked_files() {
        let root = TempRoot::new();
        root.write(".gitignore", "generated*\n*.log\n");
        let filters = compile_for_paths(
            &opts(&[], &[], &[], &[]),
            root.path(),
            &["src/generated.rs"],
        );
        assert!(filters.keeps("src/main.rs"));
        assert!(!filters.keeps("generated.txt"));
        assert!(!filters.keeps("src/generated.rs"));
        assert!(!filters.keeps("a/b/debug.log"));
    }

    #[test]
    fn gitignore_can_be_disabled() {
        let root = TempRoot::new();
        root.write(".gitignore", "secret*\n");
        let mut filters = opts(&[], &[], &[], &[]);
        filters.no_gitignore = true;
        let filters = compile_for_paths(&filters, root.path(), &["secret.txt"]);
        assert!(filters.keeps("secret.txt"));
    }

    #[test]
    fn ignored_directories_cover_their_contents() {
        let root = TempRoot::new();
        root.write(".gitignore", "build/\nnode_modules\n/logs\n");
        let filters = compile_for_paths(&opts(&[], &[], &[], &[]), root.path(), &["build/out.o"]);
        assert!(!filters.keeps("build/out.o"));
        // A trailing slash matches a directory of that name at any depth; a
        // leading slash anchors the pattern at the repo root.
        assert!(!filters.keeps("deep/build/out.o"));
        assert!(!filters.keeps("deep/node_modules/out.o"));
        assert!(!filters.keeps("logs/cleanup.log"));
        assert!(filters.keeps("deep/logs/cleanup.log"));
    }

    #[test]
    fn nested_gitignore_overrides_shallow_rules() {
        let root = TempRoot::new();
        root.write(".gitignore", "*.log\n");
        root.write("sub/.gitignore", "!keep.log\n");
        let filters = compile_for_paths(&opts(&[], &[], &[], &[]), root.path(), &["sub/keep.log"]);
        assert!(filters.keeps("sub/keep.log"));
        assert!(!filters.keeps("sub/other.log"));
        assert!(!filters.keeps("top.log"));
    }

    #[test]
    fn nested_rules_do_not_leak_to_sibling_directories() {
        let root = TempRoot::new();
        root.write("sub/.gitignore", "drop.txt\n");
        let filters = compile_for_paths(
            &opts(&[], &[], &[], &[]),
            root.path(),
            &["sub/drop.txt", "other/drop.txt"],
        );
        assert!(!filters.keeps("sub/drop.txt"));
        assert!(filters.keeps("other/drop.txt"));
    }

    #[test]
    fn info_exclude_applies_at_the_root() {
        let root = TempRoot::new();
        root.write(".git/info/exclude", "secret*\n");
        let filters = compile_for_paths(&opts(&[], &[], &[], &[]), root.path(), &["secret.txt"]);
        assert!(!filters.keeps("secret.txt"));
        assert!(filters.keeps("src/main.rs"));
    }
}

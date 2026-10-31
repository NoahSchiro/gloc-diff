//! Gitignore-style glob patterns compiled into path matchers.
//!
//! Shared by the path filters and the per-language test globs.

use std::fmt;

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};

/// Why patterns could not be compiled.
#[derive(Debug)]
pub enum GlobError {
    Pattern {
        pattern: String,
        source: globset::Error,
    },
    Set {
        source: globset::Error,
    },
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobError::Pattern { pattern, source } => {
                write!(f, "invalid glob {pattern:?}: {source}")
            }
            GlobError::Set { source } => write!(f, "failed to build glob set: {source}"),
        }
    }
}

impl std::error::Error for GlobError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GlobError::Pattern { source, .. } | GlobError::Set { source } => Some(source),
        }
    }
}

/// Compile gitignore-style patterns into one matcher. Patterns are trimmed;
/// empty ones are skipped. Unanchored patterns (no `/`) match at any depth;
/// a pattern naming a directory also covers its contents.
pub fn compile(patterns: &[&str]) -> Result<GlobSet, GlobError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        let globs = variants(pattern).map_err(|source| GlobError::Pattern {
            pattern: pattern.to_owned(),
            source,
        })?;
        for glob in globs {
            builder.add(glob);
        }
    }
    builder.build().map_err(|source| GlobError::Set { source })
}

/// The matcher variants covering one pattern: the pattern itself, plus its
/// contents when it names a directory.
fn variants(pattern: &str) -> Result<Vec<Glob>, globset::Error> {
    // Anchoring is decided before the trailing slash: gitignore treats
    // `build/` as a directory named build at any depth, not a root path.
    let trimmed = pattern.strip_suffix('/').unwrap_or(pattern);
    let anchored = trimmed.contains('/');
    let mut normalized = trimmed.strip_prefix('/').unwrap_or(trimmed).to_owned();
    if trimmed.len() != pattern.len() {
        normalized.push_str("/**");
    }
    if !anchored {
        normalized.insert_str(0, "**/");
    }
    Ok(vec![
        GlobBuilder::new(&normalized)
            .literal_separator(true)
            .build()?,
        // The contents variant: `**` matches zero components, so it also
        // covers the matched directory itself.
        GlobBuilder::new(&format!("{normalized}/**"))
            .literal_separator(true)
            .build()?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(patterns: &[&str]) -> GlobSet {
        compile(patterns).expect("patterns compile")
    }

    #[test]
    fn unanchored_patterns_match_at_any_depth() {
        let set = matcher(&["*.lock"]);
        assert!(set.is_match("Cargo.lock"));
        assert!(set.is_match("a/b/Cargo.lock"));
        assert!(!set.is_match("a/b/lock"));
    }

    #[test]
    fn directory_patterns_cover_their_contents() {
        let set = matcher(&["build/"]);
        assert!(set.is_match("build/out.o"));
        assert!(set.is_match("a/build/out.o"));
        assert!(!set.is_match("a/building/out.o"));
    }

    #[test]
    fn double_star_patterns_match_everywhere() {
        let set = matcher(&["**/tests/**"]);
        assert!(set.is_match("tests/a.rs"));
        assert!(set.is_match("crates/a/tests/a.rs"));
        assert!(!set.is_match("src/a.rs"));
        assert!(!set.is_match("tests"));
    }

    #[test]
    fn empty_patterns_match_nothing() {
        let set = matcher(&["", "   "]);
        assert!(!set.is_match("anything"));
    }

    #[test]
    fn invalid_patterns_are_reported_with_the_pattern() {
        let err = compile(&["["]).expect_err("invalid pattern");
        assert!(matches!(err, GlobError::Pattern { pattern, .. } if pattern == "["));
    }
}

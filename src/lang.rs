//! The language registry: which language handles a path, and which paths
//! count as tests.
//!
//! A [`Lang`] carries everything the classifier needs per language. The stub
//! language proves the classification pipeline end to end; real languages
//! (with grammars and tree-sitter queries for comments and inline tests)
//! register here in later phases.

use std::path::Path;

use crate::globs;

/// Everything gloc knows about one language.
pub struct Lang {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub test_globs: &'static [&'static str],
}

impl Lang {
    /// Does this language's default test globs match `path`?
    pub fn is_test_path(&self, path: &str) -> bool {
        globs::compile(self.test_globs)
            .map(|set| set.is_match(path))
            .expect("built-in test globs compile")
    }
}

/// Every language gloc knows. Extensions must not repeat across languages,
/// so [`for_path`] stays unambiguous.
pub static ALL: &[Lang] = &[Lang {
    name: "stub",
    extensions: &["stub"],
    test_globs: &["**/tests/**", "*_test.stub"],
}];

/// The language handling `path`, matched by file extension. `None` for files
/// gloc cannot classify; they fall back to blank/non-blank counting.
pub fn for_path(path: &str) -> Option<&'static Lang> {
    let ext = Path::new(path).extension()?.to_str()?;
    ALL.iter().find(|lang| {
        lang.extensions
            .iter()
            .any(|known| known.eq_ignore_ascii_case(ext))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_by_extension_case_insensitively() {
        assert!(for_path("a.stub").is_some());
        assert!(for_path("dir/A.STUB").is_some());
        assert!(for_path("a.rs").is_none());
        assert!(for_path("noext").is_none());
        assert!(for_path(".hidden").is_none());
    }

    #[test]
    fn test_globs_match_tests_at_any_depth() {
        let lang = for_path("src/x.stub").expect("stub is registered");
        assert!(lang.is_test_path("tests/x.stub"));
        assert!(lang.is_test_path("crates/a/tests/x.stub"));
        assert!(lang.is_test_path("src/other_test.stub"));
        assert!(lang.is_test_path("deep/other_test.stub"));
        assert!(!lang.is_test_path("src/x.stub"));
        assert!(!lang.is_test_path("src/test_x.stub"));
    }

    #[test]
    fn extensions_are_unique_across_languages() {
        let mut extensions: Vec<&str> = ALL
            .iter()
            .flat_map(|lang| lang.extensions.iter().copied())
            .collect();
        extensions.sort_unstable();
        extensions.dedup();
        let total: usize = ALL.iter().map(|lang| lang.extensions.len()).sum();
        assert_eq!(extensions.len(), total);
    }
}

//! The language registry: which language handles a path, and which paths
//! count as tests.
//!
//! A [`Lang`] carries everything the classifier needs per language: where
//! test code lives by path, and how to parse comments and inline tests
//! out of the text.

use std::path::Path;

use tree_sitter::Language;

use crate::globs;

/// How to parse and query one language's syntax.
#[derive(Clone, Copy)]
pub struct Grammar {
    /// Loads the tree-sitter grammar.
    pub language: fn() -> Language,
    /// Query capturing every comment.
    pub comment_query: &'static str,
    /// Query capturing every inline test node.
    pub test_query: &'static str,
}

/// Everything gloc knows about one language.
pub struct Lang {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub test_globs: &'static [&'static str],
    pub grammar: Option<Grammar>,
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
pub static ALL: &[Lang] = &[
    Lang {
        name: "rust",
        extensions: &["rs"],
        test_globs: &["**/tests/**", "*_test.rs"],
        grammar: Some(Grammar {
            language: || tree_sitter_rust::LANGUAGE.into(),
            comment_query: include_str!("lang/rust/comment.scm"),
            test_query: include_str!("lang/rust/test.scm"),
        }),
    },
    // For testing only
    Lang {
        name: "stub",
        extensions: &["stub"],
        test_globs: &["**/tests/**", "*_test.stub"],
        grammar: None,
    },
];

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
        assert!(for_path("src/lib.rs").is_some());
        assert!(for_path("noext").is_none());
        assert!(for_path(".hidden").is_none());
    }

    #[test]
    fn stub_test_globs_match_tests_at_any_depth() {
        let lang = for_path("src/x.stub").expect("stub is registered");
        assert!(lang.is_test_path("tests/x.stub"));
        assert!(lang.is_test_path("crates/a/tests/x.stub"));
        assert!(lang.is_test_path("src/other_test.stub"));
        assert!(lang.is_test_path("deep/other_test.stub"));
        assert!(!lang.is_test_path("src/x.stub"));
        assert!(!lang.is_test_path("src/test_x.stub"));
    }

    #[test]
    fn rust_test_globs_cover_test_directories_and_suffixes() {
        let lang = for_path("src/lib.rs").expect("rust is registered");
        assert!(lang.is_test_path("tests/cli.rs"));
        assert!(lang.is_test_path("crates/a/tests/cli.rs"));
        assert!(lang.is_test_path("src/payment_test.rs"));
        assert!(!lang.is_test_path("src/lib.rs"));
        assert!(!lang.is_test_path("src/tests.rs"));
    }

    #[test]
    fn rust_has_a_grammar_and_the_stub_does_not() {
        assert!(for_path("a.rs").is_some_and(|lang| lang.grammar.is_some()));
        assert!(for_path("a.stub").is_some_and(|lang| lang.grammar.is_none()));
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

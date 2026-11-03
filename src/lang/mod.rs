//! The language registry: which language handles a path, and which paths
//! count as tests.
//!
//! A [`Lang`] carries everything the classifier needs per language: where
//! test code lives by path, and how to parse comments and inline tests
//! out of the text. Languages register from submodules of this one.

#[cfg(feature = "css")]
mod css;

#[cfg(any(feature = "javascript", feature = "typescript"))]
mod ecmascript;

#[cfg(feature = "html")]
mod html;

#[cfg(feature = "javascript")]
mod javascript;

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "rust")]
mod rust;

#[cfg(feature = "typescript")]
mod typescript;

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
    /// Query capturing every inline test node. `None` for languages with
    /// no inline-test convention.
    pub test_query: Option<&'static str>,
}

/// Everything gloc knows about one language.
#[derive(Clone, Copy)]
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
    #[cfg(feature = "rust")]
    rust::LANG,
    #[cfg(feature = "python")]
    python::LANG,
    #[cfg(feature = "javascript")]
    javascript::LANG,
    #[cfg(feature = "typescript")]
    typescript::LANG,
    #[cfg(feature = "typescript")]
    typescript::LANG_TSX,
    #[cfg(feature = "html")]
    html::LANG,
    #[cfg(feature = "css")]
    css::LANG,
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
        #[cfg(feature = "rust")]
        assert!(for_path("src/lib.rs").is_some());
        #[cfg(feature = "python")]
        assert!(for_path("src/app.py").is_some());
        #[cfg(feature = "javascript")]
        assert!(for_path("src/app.js").is_some());
        #[cfg(feature = "typescript")]
        assert!(for_path("src/app.ts").is_some());
        #[cfg(feature = "html")]
        assert!(for_path("src/index.html").is_some());
        #[cfg(feature = "css")]
        assert!(for_path("src/site.css").is_some());
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

    #[cfg(feature = "rust")]
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

//! The Rust language registration for gloc's registry.

use super::{Grammar, Lang};

/// The Rust language
pub static LANG: Lang = Lang {
    name: "rust",
    extensions: &["rs"],
    test_globs: &["**/tests/**", "*_test.rs"],
    grammar: Some(Grammar {
        language: || tree_sitter_rust::LANGUAGE.into(),
        comment_query: include_str!("rust/comment.scm"),
        test_query: Some(include_str!("rust/test.scm")),
    }),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_globs_cover_test_directories_and_suffixes() {
        assert!(LANG.is_test_path("tests/cli.rs"));
        assert!(LANG.is_test_path("crates/a/tests/cli.rs"));
        assert!(LANG.is_test_path("src/payment_test.rs"));
        assert!(!LANG.is_test_path("src/lib.rs"));
        assert!(!LANG.is_test_path("src/tests.rs"));
    }
}

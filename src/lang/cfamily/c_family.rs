//! C and C++ grammars share where test files live, and the syntax queries.

/// Where test files live in C/C++ projects.
pub const TEST_GLOBS: &[&str] = &["**/tests/**", "test_*.*", "*_test.*"];

/// Query capturing every comment.
pub const COMMENT_QUERY: &str = include_str!("comment.scm");

/// Query capturing every inline test node.
pub const TEST_QUERY: &str = include_str!("test.scm");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_globs_cover_the_common_conventions() {
        let keeps = |path: &str| {
            crate::globs::compile(TEST_GLOBS)
                .expect("built-in test globs compile")
                .is_match(path)
        };
        assert!(keeps("tests/main.c"));
        assert!(keeps("src/test_main.cpp"));
        assert!(keeps("src/main_test.cc"));
        assert!(!keeps("src/main.c"));
        assert!(!keeps("src/test-utils.h"));
    }
}

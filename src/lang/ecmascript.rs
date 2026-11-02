//! JavaScript and TypeScript grammars share where test files
//! live, and the syntax queries.

/// Where test files live in JavaScript/TypeScript projects.
pub const TEST_GLOBS: &[&str] = &[
    "**/tests/**",
    "**/__tests__/**",
    "*.test.*",
    "*.spec.*",
];

/// Query capturing every comment.
pub const COMMENT_QUERY: &str = include_str!("ecmascript/comment.scm");

/// Query capturing every inline test node.
pub const TEST_QUERY: &str = include_str!("ecmascript/test.scm");

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
        assert!(keeps("tests/app.js"));
        assert!(keeps("src/__tests__/app.js"));
        assert!(keeps("src/app.test.js"));
        assert!(keeps("src/app.spec.tsx"));
        assert!(!keeps("src/app.js"));
        assert!(!keeps("src/test-utils.js"));
    }
}

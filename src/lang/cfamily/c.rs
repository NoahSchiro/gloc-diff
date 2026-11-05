//! The C language registration for gloc's registry.

use super::{COMMENT_QUERY, TEST_GLOBS, TEST_QUERY};
use crate::lang::{Grammar, Lang};

/// The C language
pub static LANG: Lang = Lang {
    name: "c",
    extensions: &["c", "h"],
    test_globs: TEST_GLOBS,
    grammar: Some(Grammar {
        language: || tree_sitter_c::LANGUAGE.into(),
        comment_query: COMMENT_QUERY,
        test_query: Some(TEST_QUERY),
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the C grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("c has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_test_directories_and_suffixes() {
        assert!(LANG.is_test_path("tests/main.c"));
        assert!(LANG.is_test_path("src/test_payment.c"));
        assert!(LANG.is_test_path("src/payment_test.c"));
        // A test_*.h header is test support code, like test_*.py in Python.
        assert!(LANG.is_test_path("src/test_utils.h"));
        assert!(!LANG.is_test_path("src/main.c"));
    }

    #[test]
    fn line_and_block_comments_are_comment_lines() {
        let text = concat!(
            "// line\n",       // comment
            "/* block\n",      // comment
            "   ends */\n",    // comment
            "/** doc */\n",    // comment
            "int main() {\n",  // code
            "    return 0;\n", // code
            "}\n",             // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn comment_markers_inside_strings_stay_code() {
        let text = "const char *s = \"// not a comment\";\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn trailing_comments_stay_code() {
        let text = "int x = 1; // note\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn unity_test_functions_are_test_lines() {
        let text = concat!(
            "void test_adds(void) {\n",           // test
            "    TEST_ASSERT_EQUAL(2, 1 + 1);\n", // test
            "}\n",                                // test
            "int helper(void) {\n",               // code
            "    return 1;\n",                    // code
            "}\n",                                // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }
}

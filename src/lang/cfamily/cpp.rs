//! The C++ language registration for gloc's registry.

use super::{COMMENT_QUERY, TEST_GLOBS, TEST_QUERY};
use crate::lang::{Grammar, Lang};

/// The C++ language
pub static LANG: Lang = Lang {
    name: "cpp",
    extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
    test_globs: TEST_GLOBS,
    grammar: Some(Grammar {
        language: || tree_sitter_cpp::LANGUAGE.into(),
        comment_query: COMMENT_QUERY,
        test_query: Some(TEST_QUERY),
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the C++ grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("cpp has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_test_directories_and_suffixes() {
        assert!(LANG.is_test_path("tests/widget.cpp"));
        assert!(LANG.is_test_path("src/test_widget.cpp"));
        assert!(LANG.is_test_path("src/widget_test.cc"));
        assert!(!LANG.is_test_path("src/widget.cpp"));
    }

    #[test]
    fn line_and_block_comments_are_comment_lines() {
        let text = concat!(
            "// line\n",              // comment
            "/* block\n",             // comment
            "   ends */\n",           // comment
            "namespace n {\n",        // code
            "class Widget {\n",       // code
            " public:\n",             // code
            "  /// Docs for w.\n",    // comment
            "  int w = 0; // note\n", // code
            "};\n",                   // code
            "}\n",                    // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Comment,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn comment_markers_inside_strings_stay_code() {
        let text = "const char *s = \"/* not a comment */\";\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn gtest_and_catch2_suites_are_test_lines() {
        let text = concat!(
            "TEST(Suite, greets) {\n",    // test
            "  EXPECT_EQ(1, 1);\n",       // test
            "}\n",                        // test
            "TEST_F(Fixture, waits) {\n", // test
            "  ASSERT_TRUE(true);\n",     // test
            "}\n",                        // test
            "TEST_CASE(\"sums\") {\n",    // test
            "  REQUIRE(1 == 1);\n",       // test
            "}\n",                        // test
            "int helper() {\n",           // code
            "  return 1;\n",              // code
            "}\n",                        // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
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

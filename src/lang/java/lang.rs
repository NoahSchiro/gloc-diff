//! The Java language registration for gloc's registry.

use crate::lang::{Grammar, Lang};

/// The Java language
pub static LANG: Lang = Lang {
    name: "java",
    extensions: &["java"],
    test_globs: &[
        "**/src/test/java/**",
        "**/Test*.java",
        "**/*Test.java",
        "**/*Tests.java",
        "**/*TestCase.java",
    ],
    grammar: Some(Grammar {
        language: || tree_sitter_java::LANGUAGE.into(),
        comment_query: include_str!("comment.scm"),
        test_query: Some(include_str!("test.scm")),
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the java grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("java has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_maven_layout_and_surefire_names() {
        assert!(LANG.is_test_path("src/test/java/demo/GreeterTest.java"));
        assert!(LANG.is_test_path("backend/src/test/java/demo/GreeterTest.java"));
        assert!(LANG.is_test_path("src/main/java/demo/TestUtils.java"));
        assert!(LANG.is_test_path("src/main/java/demo/GreeterTests.java"));
        assert!(LANG.is_test_path("src/main/java/demo/GreeterTestCase.java"));
        assert!(!LANG.is_test_path("src/main/java/demo/Greeter.java"));
        assert!(!LANG.is_test_path("src/main/java/demo/Contest.java"));
        assert!(!LANG.is_test_path("src/main/java/demo/Latest.java"));
    }

    #[test]
    fn line_block_and_doc_comments_are_comment_lines() {
        let text = concat!(
            "// line\n",                             // comment
            "/* block\n",                            // comment
            "   ends */\n",                          // comment
            "/** Docs. */\n",                        // comment
            "class A {\n",                           // code
            "    int one() { return 1; } // note\n", // code: trailing comment
            "}\n",                                   // code
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
        let text = "final String s = \"// not a comment\";\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn trailing_comments_stay_code() {
        let text = "int x = 1; // note\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn junit_annotated_methods_are_test_lines() {
        let text = concat!(
            "class A {\n",                           // code
            "    @Test\n",                           // test
            "    void greets() {\n",                 // test
            "        assertEquals(2, 1 + 1);\n",     // test
            "    }\n",                               // test
            "    @Test(timeout = 100)\n",            // test
            "    void greetsQuickly() {}\n",         // test
            "    @ParameterizedTest\n",              // test
            "    @ValueSource(strings = {\"x\"})\n", // test
            "    void greetsEach(String name) {}\n", // test
            "}\n",                                   // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
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
            ]
        );
    }

    #[test]
    fn nested_test_classes_are_test_lines() {
        let text = concat!(
            "class A {\n",                 // code
            "    @Nested\n",               // test
            "    class WhenEmpty {\n",     // test
            "        void helper() {}\n",  // test
            "        @Test\n",             // test
            "        void isEmpty() {}\n", // test
            "    }\n",                     // test
            "}\n",                         // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn other_annotations_are_code() {
        let text = concat!(
            "class A {\n",                      // code
            "    @Override\n",                  // code
            "    public String toString() {\n", // code
            "        return \"A\";\n",          // code
            "    }\n",                          // code
            "}\n",                              // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }
}

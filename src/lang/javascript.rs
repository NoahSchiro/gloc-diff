//! The JavaScript language registration for gloc's registry.

use super::ecmascript;
use super::{Grammar, Lang};

/// The JavaScript language
pub static LANG: Lang = Lang {
    name: "javascript",
    extensions: &["js", "mjs", "cjs", "jsx"],
    test_globs: ecmascript::TEST_GLOBS,
    grammar: Some(Grammar {
        language: || tree_sitter_javascript::LANGUAGE.into(),
        comment_query: ecmascript::COMMENT_QUERY,
        test_query: ecmascript::TEST_QUERY,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the javascript grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("javascript has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_test_directories_and_suffixes() {
        assert!(LANG.is_test_path("tests/app.test.js"));
        assert!(LANG.is_test_path("src/__tests__/app.js"));
        assert!(LANG.is_test_path("src/app.test.mjs"));
        assert!(!LANG.is_test_path("src/app.js"));
    }

    #[test]
    fn line_and_block_comments_are_comment_lines() {
        let text = concat!(
            "// line\n",      // comment
            "/* block\n",     // comment
            "   ends */\n",   // comment
            "const x = 1;\n", // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn describe_and_it_calls_are_test_lines() {
        let text = concat!(
            "describe(\"greet\", () => {\n",                 // test
            "  it(\"greets\", () => {\n",                    // test
            "    expect(greet(\"x\")).toBe(\"hello x\");\n", // test
            "  });\n",                                       // test
            "});\n",                                         // test
            "function helper() {\n",                         // code
            "}\n",                                           // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn skipped_focused_and_modified_forms_are_test_lines() {
        let text = concat!(
            "describe.only(\"a\", () => {});\n", // test
            "xit(\"b\", () => {});\n",           // test
            "beforeEach(() => {});\n",           // code
        );
        assert_eq!(
            classes(text),
            vec![LineClass::Test, LineClass::Test, LineClass::Code]
        );
    }

    #[test]
    fn comment_markers_inside_strings_stay_code() {
        let text = "const s = \"// not a comment\";\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }
}

//! The Python language registration for gloc's registry.

use super::{Grammar, Lang};

/// The Python language
pub static LANG: Lang = Lang {
    name: "python",
    extensions: &["py"],
    test_globs: &["**/tests/**", "test_*.py", "*_test.py"],
    grammar: Some(Grammar {
        language: || tree_sitter_python::LANGUAGE.into(),
        comment_query: include_str!("python/comment.scm"),
        test_query: Some(include_str!("python/test.scm")),
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the python grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("python has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_test_directories_and_suffixes() {
        assert!(LANG.is_test_path("tests/test_app.py"));
        assert!(LANG.is_test_path("packages/a/tests/test_app.py"));
        assert!(LANG.is_test_path("src/test_payment.py"));
        assert!(LANG.is_test_path("src/payment_test.py"));
        assert!(!LANG.is_test_path("src/payment.py"));
        assert!(!LANG.is_test_path("src/tests.py"));
    }

    #[test]
    fn line_comments_and_docstrings_are_comment_lines() {
        let text = concat!(
            "# plain\n",                // comment
            "def greet(name):\n",       // code
            "    \"\"\"Greet.\"\"\"\n", // comment: docstring
            "    return name\n",        // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Code,
                LineClass::Comment,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn multiline_module_docstrings_are_comment_lines() {
        let text = concat!(
            "\"\"\"Module doc.\n", // comment
            "More.\n",             // comment
            "\"\"\"\n",            // comment
            "x = 1\n",             // code
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
    fn test_functions_are_test_lines() {
        let text = concat!(
            "import pytest\n",           // code
            "\n",                        // blank
            "@pytest.mark.slow\n",       // test: decorator inside decorated_definition
            "def test_adds():\n",        // test
            "    assert True\n",         // test
            "\n",                        // blank
            "async def test_waits():\n", // test
            "    await sleep(0)\n",      // test
            "\n",                        // blank
            "def helper():\n",           // code
            "    return 1\n",            // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Blank,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Blank,
                LineClass::Test,
                LineClass::Test,
                LineClass::Blank,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn testcase_subclasses_are_test_lines() {
        let text = concat!(
            "import unittest\n",                      // code
            "\n",                                     // blank
            "class TestGreets(unittest.TestCase):\n", // test
            "    def test_one(self):\n",              // test
            "        self.assertTrue(True)\n",        // test
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Blank,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
            ]
        );
    }

    #[test]
    fn strings_assigned_to_names_stay_code() {
        let text = "TEMPLATE = \"// not a comment\"\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn trailing_comments_stay_code() {
        let text = "x = 1  # note\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }
}

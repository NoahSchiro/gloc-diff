//! The Go language registration for gloc's registry.

use crate::lang::{Grammar, Lang};

/// The Go language
pub static LANG: Lang = Lang {
    name: "go",
    extensions: &["go"],
    // Go has no tests/ directory convention. Tests live next to the code
    // they exercise, in *_test.go files.
    test_globs: &["*_test.go"],
    grammar: Some(Grammar {
        language: || tree_sitter_go::LANGUAGE.into(),
        comment_query: include_str!("comment.scm"),
        test_query: Some(include_str!("test.scm")),
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the go grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("go has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_test_files_next_to_their_source() {
        assert!(LANG.is_test_path("sum_test.go"));
        assert!(LANG.is_test_path("internal/sum/sum_test.go"));
        assert!(!LANG.is_test_path("internal/sum/sum.go"));
        assert!(!LANG.is_test_path("internal/testdata/sum.go"));
    }

    #[test]
    fn line_and_block_comments_are_comment_lines() {
        let text = concat!(
            "// line\n",                  // comment
            "/* block\n",                 // comment
            "   ends */\n",               // comment
            "// Sum adds.\n",             // comment
            "func Sum(a, b int) int {\n", // code
            "\treturn a + b // note\n",   // code: trailing comment
            "}\n",                        // code
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
        let text = "const s = \"// not a comment\"\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn trailing_comments_stay_code() {
        let text = "var x = 1 // note\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn test_benchmark_and_fuzz_functions_are_test_lines() {
        let text = concat!(
            "func TestSum(t *testing.T) {\n",           // test
            "\tt.Run(\"ones\", func(t *testing.T) {\n", // test: subtest
            "\t\tif Sum(1, 1) != 2 {\n",                // test
            "\t\t\tt.Fatal(\"bad\")\n",                 // test
            "\t\t}\n",                                  // test
            "\t})\n",                                   // test
            "}\n",                                      // test
            "func BenchmarkSum(b *testing.B) {\n",      // test
            "\tfor i := 0; i < b.N; i++ {\n",           // test
            "\t\tSum(1, 2)\n",                          // test
            "\t}\n",                                    // test
            "}\n",                                      // test
            "func FuzzSum(f *testing.F) {\n",           // test
            "\tf.Add(1, 2)\n",                          // test
            "}\n",                                      // test
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
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
            ]
        );
    }

    #[test]
    fn lowercase_or_suffixless_names_are_code() {
        let text = concat!(
            "func testSum(t *testing.T) {}\n",      // code: not run by go test
            "func Test(t *testing.T) {}\n",         // code: no suffix
            "func Testify() {}\n",                  // code: lowercase suffix
            "func Benchmarking(b *testing.B) {}\n", // code: lowercase suffix
            "func (s Suite) TestSum() {}\n",        // code: method, not a test
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }
}

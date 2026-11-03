//! The TypeScript language registration for gloc's registry.

use super::ecmascript;
use super::{Grammar, Lang};

/// The TypeScript language
pub static LANG: Lang = Lang {
    name: "typescript",
    extensions: &["ts", "mts", "cts"],
    test_globs: ecmascript::TEST_GLOBS,
    grammar: Some(Grammar {
        language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        comment_query: ecmascript::COMMENT_QUERY,
        test_query: ecmascript::TEST_QUERY,
    }),
};

/// `.tsx` needs its own grammar. JSX tag syntax conflicts with the
/// generics syntax of the plain TypeScript grammar. From the perspective
/// of gloc, it is still TypeScript.
pub static LANG_TSX: Lang = Lang {
    name: "typescript",
    extensions: &["tsx"],
    test_globs: ecmascript::TEST_GLOBS,
    grammar: Some(Grammar {
        language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        comment_query: ecmascript::COMMENT_QUERY,
        test_query: ecmascript::TEST_QUERY,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as `lang`'s grammar and the shared queries see them.
    fn classes(lang: &Lang, text: &str) -> Vec<LineClass> {
        let grammar = lang.grammar.expect("typescript has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn test_globs_cover_test_directories_and_suffixes() {
        assert!(LANG.is_test_path("tests/app.test.ts"));
        assert!(LANG.is_test_path("src/__tests__/app.ts"));
        assert!(LANG_TSX.is_test_path("src/app.test.tsx"));
        assert!(!LANG.is_test_path("src/app.ts"));
    }

    #[test]
    fn typescript_syntax_and_test_calls_are_classified() {
        let text = concat!(
            "interface Greetable {\n",                       // code
            "  name: string;\n",                             // code
            "}\n",                                           // code
            "describe<Greetable>(\"greet\", () => {\n",      // test
            "  it(\"greets\", () => {\n",                    // test
            "    expect(greet(\"x\")).toBe(\"hello x\");\n", // test
            "  });\n",                                       // test
            "});\n",                                         // test
            "const n: number = 1;\n",                        // code
        );
        assert_eq!(
            classes(&LANG, text),
            vec![
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
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
    fn tsx_files_parse_with_the_tsx_grammar() {
        let text = concat!(
            "// widget test\n",                     // comment
            "describe(\"widget\", () => {\n",       // test
            "  it(\"renders\", () => {\n",          // test
            "    render(<Widget name=\"x\" />);\n", // test
            "  });\n",                              // test
            "});\n",                                // test
        );
        assert_eq!(
            classes(&LANG_TSX, text),
            vec![
                LineClass::Comment,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
            ]
        );
    }
}

//! The CSS language registration for gloc's registry.

use super::{Grammar, Lang};

/// The CSS language
pub static LANG: Lang = Lang {
    name: "css",
    extensions: &["css"],
    test_globs: &[],
    grammar: Some(Grammar {
        language: || tree_sitter_css::LANGUAGE.into(),
        comment_query: include_str!("css/comment.scm"),
        // No inline-test convention.
        test_query: None,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the css grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("css has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn comments_are_comment_lines_and_rules_are_code() {
        let text = concat!(
            "/* base */\n",     // comment
            "body {\n",         // code
            "  margin: 0;\n",   // code
            "}\n",              // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn multiline_comments_are_comment_lines() {
        let text = concat!(
            "/* spans\n",       // comment
            "   lines */\n",    // comment
            "a { color: red; }\n", // code
        );
        assert_eq!(
            classes(text),
            vec![LineClass::Comment, LineClass::Comment, LineClass::Code]
        );
    }
}

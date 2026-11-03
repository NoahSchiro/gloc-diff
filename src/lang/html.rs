//! The HTML language registration for gloc's registry.

use super::{Grammar, Lang};

/// The HTML language
pub static LANG: Lang = Lang {
    name: "html",
    extensions: &["html", "htm"],
    test_globs: &[],
    grammar: Some(Grammar {
        language: || tree_sitter_html::LANGUAGE.into(),
        comment_query: include_str!("html/comment.scm"),
        // No inline-test convention.
        test_query: None,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};
    use crate::syntax::ranges;

    /// Classes of `text` as the html grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let grammar = LANG.grammar.expect("html has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn comments_are_comment_lines_and_markup_is_code() {
        let text = concat!(
            "<!DOCTYPE html>\n",        // code
            "<html>\n",                 // code
            "  <!-- header note -->\n", // comment
            "  <body>\n",               // code
            "    <h1>Hi</h1>\n",        // code
            "  </body>\n",              // code
            "</html>\n",                // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Code,
                LineClass::Comment,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn multiline_comments_are_comment_lines() {
        let text = concat!(
            "<!-- spans\n",   // comment
            "   lines -->\n", // comment
            "<p>x</p>\n",     // code
        );
        assert_eq!(
            classes(text),
            vec![LineClass::Comment, LineClass::Comment, LineClass::Code]
        );
    }

    #[test]
    fn comment_markers_in_text_stay_code() {
        let text = "<p>not <!-- a comment --> a marker</p>\n";
        // The real comment is a comment; the words around it stay code.
        let classes = classes(text);
        assert_eq!(classes, vec![LineClass::Code]);
    }
}

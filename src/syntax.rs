//! The tree-sitter mechanism.
//!
//! Parse one file version with its language's grammar and extract the byte
//! extents of comments and inline tests.

use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator, Tree};

use crate::classify::ByteRange;
use crate::lang::Grammar;

/// Comment and inline-test byte ranges of one file version, sorted by
/// start.
pub fn ranges(text: &str, grammar: &Grammar) -> (Vec<ByteRange>, Vec<ByteRange>) {
    let language = (grammar.language)();
    let tree = parse(text, &language);
    let mut comments: Vec<ByteRange> =
        captured(&tree, text, &language, grammar.comment_query, "comment")
            .into_iter()
            .map(|node| ByteRange {
                start: node.start_byte(),
                end: node.end_byte(),
            })
            .collect();
    let mut tests: Vec<ByteRange> = captured(&tree, text, &language, grammar.test_query, "test")
        .into_iter()
        .map(|node| ByteRange {
            start: over_attributes(node),
            end: node.end_byte(),
        })
        .collect();
    // Two query patterns (or overlapping nodes) can yield out-of-order
    // ranges; the classifier sweeps them in line order.
    comments.sort_unstable();
    tests.sort_unstable();
    (comments, tests)
}

/// Parse `text` with the grammar's language.
fn parse(text: &str, language: &Language) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("built-in grammar matches the tree-sitter version");
    parser
        .parse(text, None)
        .expect("parser has a language and no timeout")
}

/// Every node captured as `name` by a built-in query, in match order.
fn captured<'a>(
    tree: &'a Tree,
    text: &str,
    language: &Language,
    query_src: &'static str,
    name: &str,
) -> Vec<Node<'a>> {
    let query = Query::new(language, query_src).expect("built-in query compiles");
    let index = query
        .capture_index_for_name(name)
        .expect("built-in query defines the capture");
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    let mut nodes = Vec::new();
    while let Some(m) = matches.next() {
        nodes.extend(m.nodes_for_capture_index(index));
    }
    nodes
}

/// Start offset pulled back over immediately preceding attribute items, so
/// stacked attributes (e.g. `#[should_panic]` above `#[test]`) count as test.
fn over_attributes(node: Node<'_>) -> usize {
    let mut start = node.start_byte();
    let mut sibling = node.prev_named_sibling();
    while let Some(attr) = sibling.filter(|sibling| sibling.kind() == "attribute_item") {
        start = attr.start_byte();
        sibling = attr.prev_named_sibling();
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{LineClass, classify_lines};

    /// Classes of `text` as the rust grammar and queries see them.
    fn classes(text: &str) -> Vec<LineClass> {
        let lang = crate::lang::for_path("a.rs").expect("rust is registered");
        let grammar = lang.grammar.expect("rust has a grammar");
        let (comments, tests) = ranges(text, &grammar);
        classify_lines(text, &comments, &tests, false)
    }

    #[test]
    fn line_block_and_doc_comments_are_comment_lines() {
        let text = concat!(
            "//! inner doc\n",      // comment
            "/// outer doc\n",      // comment
            "// plain\n",           // comment
            "/* block */\n",        // comment
            "/** doc block */\n",   // comment
            "/*! inner block */\n", // comment
            "fn a() {}\n",          // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn blank_lines_inside_block_comments_are_comments() {
        let text = "/* a\n\nb\n*/\nfn a() {}\n";
        assert_eq!(
            classes(text),
            vec![
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Comment,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn nested_block_comments_are_one_comment() {
        let text = "/* a /* b */ c */\nfn a() {}\n";
        assert_eq!(classes(text), vec![LineClass::Comment, LineClass::Code]);
    }

    #[test]
    fn comment_markers_inside_strings_stay_code() {
        let text = "let s = \"// not a comment\";\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn trailing_comments_stay_code() {
        let text = "let x = 1; // note\n";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }

    #[test]
    fn code_resuming_after_a_block_comment_stays_code() {
        let text = "/* c\n*/ let x = 1;\n";
        assert_eq!(classes(text), vec![LineClass::Comment, LineClass::Code]);
    }

    #[test]
    fn cfg_test_module_and_test_fn_are_test_lines() {
        let text = concat!(
            "fn a() {}\n",     // code
            "#[cfg(test)]\n",  // test
            "mod tests {\n",   // test
            "    #[test]\n",   // test
            "    fn b() {}\n", // test
            "}\n",             // test
            "fn c() {}\n",     // code
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
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn stacked_attributes_are_test_lines() {
        let text = concat!(
            "#[should_panic]\n", // test: widened over the stacked attribute
            "#[test]\n",         // test
            "fn a() {}\n",       // test
            "fn b() {}\n",       // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Test,
                LineClass::Test,
                LineClass::Test,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn attributes_with_inner_whitespace_are_test_lines() {
        let text = concat!(
            "#[ test ]\n",        // test
            "#[ cfg( test ) ]\n", // test
            "mod tests {}\n",     // test
        );
        assert_eq!(
            classes(text),
            vec![LineClass::Test, LineClass::Test, LineClass::Test]
        );
    }

    #[test]
    fn other_attributes_and_cfgs_are_code() {
        let text = concat!(
            "#[derive(Debug)]\n",        // code
            "struct S;\n",               // code
            "#[cfg(feature = \"x\")]\n", // code
            "fn a() {}\n",               // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Code,
                LineClass::Code,
                LineClass::Code
            ]
        );
    }

    #[test]
    fn test_fn_inside_an_impl_block_is_test() {
        let text = concat!(
            "struct S;\n",     // code
            "impl S {\n",      // code
            "    #[test]\n",   // test
            "    fn a() {}\n", // test
            "}\n",             // code
        );
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Code,
                LineClass::Test,
                LineClass::Test,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn crlf_line_endings_classify_the_same() {
        let text = "fn a() {}\r\n// note\r\n\r\nfn b() {}\r\n";
        assert_eq!(
            classes(text),
            vec![
                LineClass::Code,
                LineClass::Comment,
                LineClass::Blank,
                LineClass::Code,
            ]
        );
    }

    #[test]
    fn text_without_a_trailing_newline_classifies() {
        let text = "fn a() {} // note";
        assert_eq!(classes(text), vec![LineClass::Code]);
    }
}

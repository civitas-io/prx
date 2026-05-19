use tree_sitter::Node;

use super::create_parser;

fn is_comment_node(node: &Node) -> bool {
    matches!(
        node.kind(),
        "comment" | "line_comment" | "block_comment" | "hash_comment" | "heredoc_body"
    )
}

fn collect_comment_ranges(node: Node, ranges: &mut Vec<(usize, usize)>) {
    if is_comment_node(&node) {
        ranges.push((node.start_byte(), node.end_byte()));
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_comment_ranges(child, ranges);
    }
}

pub fn strip_comments(source: &str, ext: &str) -> String {
    let mut parser = match create_parser(ext) {
        Some(p) => p,
        None => return collapse_blank_lines(source),
    };

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return collapse_blank_lines(source),
    };

    let mut ranges = Vec::new();
    collect_comment_ranges(tree.root_node(), &mut ranges);

    if ranges.is_empty() {
        return collapse_blank_lines(source);
    }

    let bytes = source.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut pos = 0;

    for (start, end) in &ranges {
        if *start > pos {
            result.extend_from_slice(&bytes[pos..*start]);
        }
        pos = *end;
    }
    if pos < bytes.len() {
        result.extend_from_slice(&bytes[pos..]);
    }

    let stripped = String::from_utf8_lossy(&result);
    collapse_blank_lines(&stripped)
}

fn collapse_blank_lines(text: &str) -> String {
    let mut result = Vec::new();
    let mut prev_blank = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push("");
            }
            prev_blank = true;
        } else {
            result.push(line);
            prev_blank = false;
        }
    }

    while result.last() == Some(&"") {
        result.pop();
    }

    let mut out = result.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_rust_line_comments() {
        let src = "// header comment\nfn main() {\n    // inner\n    let x = 1;\n}\n";
        let result = strip_comments(src, "rs");
        assert!(!result.contains("// header"));
        assert!(!result.contains("// inner"));
        assert!(result.contains("fn main()"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn strips_python_comments() {
        let src = "# comment\ndef hello():\n    # inner\n    return 1\n";
        let result = strip_comments(src, "py");
        assert!(!result.contains("# comment"));
        assert!(!result.contains("# inner"));
        assert!(result.contains("def hello():"));
        assert!(result.contains("return 1"));
    }

    #[test]
    fn strips_block_comments() {
        let src = "/* block */\nfn main() {\n    let x = 1;\n}\n";
        let result = strip_comments(src, "rs");
        assert!(!result.contains("block"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn collapses_consecutive_blank_lines() {
        let src = "fn a() {}\n\n\n\n\nfn b() {}\n";
        let result = strip_comments(src, "rs");
        assert!(!result.contains("\n\n\n"));
        assert!(result.contains("fn a()"));
        assert!(result.contains("fn b()"));
    }

    #[test]
    fn unsupported_language_still_collapses() {
        let src = "line1\n\n\n\nline2\n";
        let result = strip_comments(src, "xyz");
        assert_eq!(result, "line1\n\nline2\n");
    }

    #[test]
    fn preserves_string_contents() {
        let src = "fn main() {\n    let s = \"// not a comment\";\n}\n";
        let result = strip_comments(src, "rs");
        assert!(result.contains("// not a comment"));
    }

    #[test]
    fn empty_file() {
        let result = strip_comments("", "rs");
        assert_eq!(result, "\n");
    }

    #[test]
    fn no_comments_unchanged() {
        let src = "fn main() {\n    let x = 1;\n}\n";
        let result = strip_comments(src, "rs");
        assert_eq!(result, src);
    }

    #[test]
    fn js_comments() {
        let src = "// comment\nfunction hello() {\n    /* block */\n    return 1;\n}\n";
        let result = strip_comments(src, "js");
        assert!(!result.contains("// comment"));
        assert!(!result.contains("block"));
        assert!(result.contains("function hello()"));
    }
}

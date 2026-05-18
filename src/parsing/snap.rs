use tree_sitter::{Node, Parser};

use super::languages::language_for_extension;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapTarget {
    Function,
    Class,
    Block,
}

pub struct SnapResult {
    pub start_line: usize,
    pub end_line: usize,
    pub target_kind: String,
    pub target_name: Option<String>,
}

pub fn snap_to_structure(
    source: &str,
    ext: &str,
    line: usize,
    target: SnapTarget,
) -> Option<SnapResult> {
    let lang = language_for_extension(ext)?;
    let mut parser = Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let byte_offset = line_to_byte_offset(source, line)?;
    let node = tree
        .root_node()
        .descendant_for_byte_range(byte_offset, byte_offset)?;

    find_enclosing(node, source, target)
}

fn find_enclosing(start: Node, source: &str, target: SnapTarget) -> Option<SnapResult> {
    let mut current = Some(start);
    while let Some(node) = current {
        if matches_target(node.kind(), target) {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(String::from);
            return Some(SnapResult {
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                target_kind: node.kind().to_string(),
                target_name: name,
            });
        }
        current = node.parent();
    }
    None
}

fn matches_target(kind: &str, target: SnapTarget) -> bool {
    match target {
        SnapTarget::Function => matches!(
            kind,
            "function_item"
                | "function_definition"
                | "function_declaration"
                | "method_definition"
                | "method_declaration"
                | "arrow_function"
                | "generator_function_declaration"
        ),
        SnapTarget::Class => matches!(
            kind,
            "class_declaration"
                | "class_definition"
                | "struct_item"
                | "struct_specifier"
                | "impl_item"
                | "trait_item"
                | "interface_declaration"
                | "enum_item"
                | "enum_declaration"
        ),
        SnapTarget::Block => matches!(
            kind,
            "block"
                | "statement_block"
                | "compound_statement"
                | "function_item"
                | "function_definition"
                | "function_declaration"
                | "class_declaration"
                | "class_definition"
                | "if_statement"
                | "if_expression"
                | "for_statement"
                | "for_expression"
                | "while_statement"
                | "while_expression"
                | "match_expression"
                | "switch_statement"
        ),
    }
}

fn line_to_byte_offset(source: &str, line: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let target = line - 1;
    let mut current_line = 0;
    for (offset, ch) in source.char_indices() {
        if current_line == target {
            return Some(offset);
        }
        if ch == '\n' {
            current_line += 1;
        }
    }
    if current_line == target {
        Some(source.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_to_function_in_rust() {
        let src = "fn outer() {\n    let x = 1;\n    let y = 2;\n}\n";
        let result = snap_to_structure(src, "rs", 2, SnapTarget::Function).unwrap();
        assert_eq!(result.start_line, 1);
        assert_eq!(result.end_line, 4);
        assert_eq!(result.target_name.as_deref(), Some("outer"));
    }

    #[test]
    fn snap_to_class_in_python() {
        let src =
            "class Foo:\n    def bar(self):\n        pass\n\n    def baz(self):\n        pass\n";
        let result = snap_to_structure(src, "py", 3, SnapTarget::Class).unwrap();
        assert_eq!(result.start_line, 1);
        assert_eq!(result.target_name.as_deref(), Some("Foo"));
    }

    #[test]
    fn snap_to_function_in_python() {
        let src = "class Foo:\n    def bar(self):\n        x = 1\n        y = 2\n";
        let result = snap_to_structure(src, "py", 3, SnapTarget::Function).unwrap();
        assert_eq!(result.target_name.as_deref(), Some("bar"));
        assert_eq!(result.start_line, 2);
    }

    #[test]
    fn no_match_returns_none() {
        let src = "let x = 1;\n";
        let result = snap_to_structure(src, "rs", 1, SnapTarget::Function);
        assert!(result.is_none());
    }

    #[test]
    fn unsupported_language() {
        let result = snap_to_structure("hello", "xyz", 1, SnapTarget::Function);
        assert!(result.is_none());
    }

    #[test]
    fn line_zero_returns_none() {
        let result = snap_to_structure("fn f() {}", "rs", 0, SnapTarget::Function);
        assert!(result.is_none());
    }

    #[test]
    fn line_beyond_file_returns_none() {
        let result = snap_to_structure("fn f() {}", "rs", 100, SnapTarget::Function);
        assert!(result.is_none());
    }
}

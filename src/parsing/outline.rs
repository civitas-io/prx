use tree_sitter::{Node, Parser};

use super::languages::language_for_extension;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Type,
    Const,
    Module,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Method => write!(f, "method"),
            Self::Class => write!(f, "class"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Interface => write!(f, "interface"),
            Self::Type => write!(f, "type"),
            Self::Const => write!(f, "const"),
            Self::Module => write!(f, "module"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    pub children: Vec<Symbol>,
}

pub fn extract_symbols(source: &str, ext: &str) -> Vec<Symbol> {
    let lang = match language_for_extension(ext) {
        Some(l) => l,
        None => return vec![],
    };
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return vec![];
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let lines: Vec<&str> = source.lines().collect();
    let mut symbols = Vec::new();
    collect_symbols(tree.root_node(), source, &lines, &mut symbols);
    symbols
}

fn collect_symbols(node: Node, source: &str, lines: &[&str], symbols: &mut Vec<Symbol>) {
    let kind_opt = classify_node(node.kind());
    if let Some(kind) = kind_opt {
        if let Some(name) = extract_name(node, source) {
            let start_line = node.start_position().row + 1;
            let end_line = node.end_position().row + 1;
            let signature = lines
                .get(start_line - 1)
                .map(|l| l.trim().to_string())
                .unwrap_or_default();

            let mut children = Vec::new();
            let is_container = matches!(
                kind,
                SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Interface
                    | SymbolKind::Module
            );
            if is_container {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    collect_symbols(child, source, lines, &mut children);
                }
            }

            symbols.push(Symbol {
                name,
                kind,
                start_line,
                end_line,
                signature,
                children,
            });
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, source, lines, symbols);
    }
}

fn classify_node(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_item"
        | "function_definition"
        | "function_declaration"
        | "arrow_function"
        | "generator_function_declaration" => Some(SymbolKind::Function),

        "method_definition" | "method_declaration" => Some(SymbolKind::Method),

        "class_declaration" | "class_definition" => Some(SymbolKind::Class),

        "struct_item" | "struct_specifier" => Some(SymbolKind::Struct),

        "enum_item" | "enum_declaration" | "enum_specifier" => Some(SymbolKind::Enum),

        "trait_item" => Some(SymbolKind::Trait),

        "interface_declaration" => Some(SymbolKind::Interface),

        "type_alias_declaration" | "type_item" => Some(SymbolKind::Type),

        "const_item" | "lexical_declaration" => Some(SymbolKind::Const),

        "mod_item" | "module" => Some(SymbolKind::Module),

        _ => None,
    }
}

fn extract_name(node: Node, source: &str) -> Option<String> {
    // Most node types have a `name` field
    if let Some(name_node) = node.child_by_field_name("name") {
        return name_node
            .utf8_text(source.as_bytes())
            .ok()
            .map(String::from);
    }
    // JS arrow functions: look for variable declarator parent
    if node.kind() == "arrow_function" {
        if let Some(parent) = node.parent() {
            if parent.kind() == "variable_declarator" {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    return name_node
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(String::from);
                }
            }
        }
    }
    // Fallback: for const declarations, try the first declarator
    if node.kind() == "lexical_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    return name_node
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(String::from);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_functions_and_structs() {
        let src = r#"
fn hello() {}

struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

enum Color {
    Red,
    Blue,
}
"#;
        let symbols = extract_symbols(src, "rs");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"), "missing hello: {names:?}");
        assert!(names.contains(&"Point"), "missing Point: {names:?}");
        assert!(names.contains(&"Color"), "missing Color: {names:?}");
    }

    #[test]
    fn python_functions_and_classes() {
        let src = r#"
def greet(name):
    print(f"Hello {name}")

class User:
    def __init__(self, name):
        self.name = name

    def display(self):
        print(self.name)
"#;
        let symbols = extract_symbols(src, "py");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"User"), "missing User: {names:?}");

        let user = symbols.iter().find(|s| s.name == "User").unwrap();
        let method_names: Vec<&str> = user.children.iter().map(|s| s.name.as_str()).collect();
        assert!(
            method_names.contains(&"__init__"),
            "missing __init__: {method_names:?}"
        );
        assert!(
            method_names.contains(&"display"),
            "missing display: {method_names:?}"
        );
    }

    #[test]
    fn javascript_functions() {
        let src = r#"
function processData(data) {
    return data.map(x => x * 2);
}

class Handler {
    handle(req) {
        return req;
    }
}
"#;
        let symbols = extract_symbols(src, "js");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"processData"),
            "missing processData: {names:?}"
        );
        assert!(names.contains(&"Handler"), "missing Handler: {names:?}");
    }

    #[test]
    fn go_functions() {
        let src = r#"
package main

func main() {
    fmt.Println("hello")
}

func add(a int, b int) int {
    return a + b
}
"#;
        let symbols = extract_symbols(src, "go");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"), "missing main: {names:?}");
        assert!(names.contains(&"add"), "missing add: {names:?}");
    }

    #[test]
    fn signature_is_first_line() {
        let src = "fn process(data: &[u8]) -> Result<(), Error> {\n    Ok(())\n}\n";
        let symbols = extract_symbols(src, "rs");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.starts_with("fn process(data: &[u8])"));
    }

    #[test]
    fn unsupported_extension_returns_empty() {
        let symbols = extract_symbols("some content", "xyz");
        assert!(symbols.is_empty());
    }

    #[test]
    fn line_numbers_are_one_indexed() {
        let src = "fn first() {}\nfn second() {}\n";
        let symbols = extract_symbols(src, "rs");
        assert_eq!(symbols[0].start_line, 1);
        assert_eq!(symbols[1].start_line, 2);
    }
}

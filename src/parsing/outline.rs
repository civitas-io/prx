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

/// Flattened symbol with kind serialized as a string, for cross-module consumers.
#[derive(Debug, Clone)]
pub struct FlatSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
}

impl Symbol {
    /// Recursively flatten this symbol and its children into a flat list.
    pub fn flatten(&self) -> Vec<FlatSymbol> {
        let mut out = vec![FlatSymbol {
            name: self.name.clone(),
            kind: self.kind.to_string(),
            start_line: self.start_line,
            end_line: self.end_line,
            signature: self.signature.clone(),
        }];
        for child in &self.children {
            out.extend(child.flatten());
        }
        out
    }
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
    if let Some((kind, name)) = classify_hcl_block(node, source)
        .or_else(|| classify_makefile_rule(node, source))
        .or_else(|| classify_elixir_call(node, source))
    {
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let signature = lines
            .get(start_line - 1)
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        let mut children = Vec::new();
        if matches!(kind, SymbolKind::Module) {
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

fn classify_hcl_block(node: Node, source: &str) -> Option<(SymbolKind, String)> {
    if node.kind() != "block" {
        return None;
    }
    let mut cursor = node.walk();
    let mut block_type = None;
    let mut labels = Vec::new();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" if block_type.is_none() => {
                block_type = child.utf8_text(source.as_bytes()).ok().map(String::from);
            }
            "string_lit" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    labels.push(text.trim_matches('"').to_string());
                }
            }
            _ => {}
        }
    }
    let bt = block_type?;
    let kind = match bt.as_str() {
        "resource" | "data" | "module" => SymbolKind::Module,
        "variable" | "output" | "locals" => SymbolKind::Const,
        "provider" | "terraform" => SymbolKind::Module,
        _ => SymbolKind::Module,
    };
    let name = if labels.is_empty() {
        bt
    } else {
        format!("{} {}", bt, labels.join(" "))
    };
    Some((kind, name))
}

fn classify_makefile_rule(node: Node, source: &str) -> Option<(SymbolKind, String)> {
    match node.kind() {
        "rule" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "targets" {
                    let name = child.utf8_text(source.as_bytes()).ok()?;
                    return Some((SymbolKind::Function, name.trim().to_string()));
                }
            }
            None
        }
        "variable_assignment" => {
            if let Some(first_child) = node.child(0) {
                if first_child.kind() == "word" {
                    let name = first_child.utf8_text(source.as_bytes()).ok()?;
                    return Some((SymbolKind::Const, name.to_string()));
                }
            }
            None
        }
        _ => None,
    }
}

fn classify_elixir_call(node: Node, source: &str) -> Option<(SymbolKind, String)> {
    if node.kind() != "call" {
        return None;
    }
    let target = node.child(0)?;
    if target.kind() != "identifier" {
        return None;
    }
    let target_name = target.utf8_text(source.as_bytes()).ok()?;
    let kind = match target_name {
        "def" | "defp" => SymbolKind::Function,
        "defmodule" => SymbolKind::Module,
        "defprotocol" => SymbolKind::Interface,
        "defstruct" => SymbolKind::Struct,
        _ => return None,
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "arguments" {
            let mut inner = child.walk();
            for arg in child.children(&mut inner) {
                if matches!(arg.kind(), "alias" | "identifier") {
                    let name = arg.utf8_text(source.as_bytes()).ok()?;
                    return Some((kind, name.to_string()));
                }
            }
        }
    }
    None
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

        "struct_item" | "struct_specifier" | "struct_declaration" => Some(SymbolKind::Struct),

        "enum_item" | "enum_declaration" | "enum_specifier" => Some(SymbolKind::Enum),

        "trait_item" | "trait_declaration" => Some(SymbolKind::Trait),

        "interface_declaration" => Some(SymbolKind::Interface),

        "protocol_declaration" => Some(SymbolKind::Interface),

        "type_alias_declaration" | "type_item" => Some(SymbolKind::Type),

        "const_item" => Some(SymbolKind::Const),

        "mod_item" | "module" => Some(SymbolKind::Module),

        "object_declaration" => Some(SymbolKind::Class),

        "namespace_declaration" => Some(SymbolKind::Module),

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
    // Kotlin/Swift: name is a type_identifier or simple_identifier child (no field)
    if matches!(
        node.kind(),
        "class_declaration"
            | "object_declaration"
            | "protocol_declaration"
            | "namespace_declaration"
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "type_identifier" | "identifier") {
                return child.utf8_text(source.as_bytes()).ok().map(String::from);
            }
        }
    }
    if node.kind() == "function_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "simple_identifier" {
                return child.utf8_text(source.as_bytes()).ok().map(String::from);
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

    #[test]
    fn kotlin_functions_and_classes() {
        let src = "fun main() {}\nclass Foo {}\nobject Singleton {}\n";
        let symbols = extract_symbols(src, "kt");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"), "missing main: {names:?}");
        assert!(names.contains(&"Foo"), "missing Foo: {names:?}");
        assert!(names.contains(&"Singleton"), "missing Singleton: {names:?}");
    }

    #[test]
    fn swift_functions_and_types() {
        let src = "func greet() {}\nclass MyClass {}\nprotocol MyProto {}\n";
        let symbols = extract_symbols(src, "swift");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"MyClass"), "missing MyClass: {names:?}");
        assert!(names.contains(&"MyProto"), "missing MyProto: {names:?}");
    }

    #[test]
    fn csharp_classes_and_methods() {
        let src = "namespace Foo {\n  class Bar {\n    void Method() {}\n  }\n  interface IBaz {}\n  struct Point {}\n  enum Color { Red }\n}\n";
        let symbols = extract_symbols(src, "cs");
        let all_names: Vec<String> = symbols
            .iter()
            .flat_map(|s| {
                let mut names = vec![s.name.clone()];
                names.extend(s.children.iter().flat_map(|c| {
                    let mut n = vec![c.name.clone()];
                    n.extend(c.children.iter().map(|gc| gc.name.clone()));
                    n
                }));
                names
            })
            .collect();
        assert!(
            all_names.contains(&"Foo".to_string()),
            "missing Foo: {all_names:?}"
        );
        assert!(
            all_names.contains(&"Bar".to_string()),
            "missing Bar: {all_names:?}"
        );
        assert!(
            all_names.contains(&"IBaz".to_string()),
            "missing IBaz: {all_names:?}"
        );
        assert!(
            all_names.contains(&"Point".to_string()),
            "missing Point: {all_names:?}"
        );
        assert!(
            all_names.contains(&"Color".to_string()),
            "missing Color: {all_names:?}"
        );
    }

    #[test]
    fn php_functions_and_classes() {
        let src = "<?php\nfunction greet() {}\nclass User {}\ninterface Loggable {}\ntrait HasName {}\nenum Color { case Red; }\n";
        let symbols = extract_symbols(src, "php");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing greet: {names:?}");
        assert!(names.contains(&"User"), "missing User: {names:?}");
        assert!(names.contains(&"Loggable"), "missing Loggable: {names:?}");
        assert!(names.contains(&"HasName"), "missing HasName: {names:?}");
        assert!(names.contains(&"Color"), "missing Color: {names:?}");
    }

    #[test]
    fn hcl_resources_and_variables() {
        let src = "variable \"name\" {\n  type = string\n}\nresource \"aws_instance\" \"web\" {\n  ami = \"ami-123\"\n}\n";
        let symbols = extract_symbols(src, "tf");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("variable")),
            "missing variable: {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("aws_instance")),
            "missing resource: {names:?}"
        );
    }

    #[test]
    fn makefile_rules_and_variables() {
        let src = "CC = gcc\n\nall: build test\n\nbuild:\n\t$(CC) -o app main.c\n";
        let symbols = extract_symbols(src, "makefile");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"CC"), "missing CC variable: {names:?}");
        assert!(names.contains(&"all"), "missing all rule: {names:?}");
        assert!(names.contains(&"build"), "missing build rule: {names:?}");
    }

    #[test]
    fn elixir_modules_and_functions() {
        let src = "defmodule MyApp do\n  def hello do :ok end\n  defp private_fn do :ok end\nend\n";
        let symbols = extract_symbols(src, "ex");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyApp"), "missing MyApp: {names:?}");
        let module = symbols.iter().find(|s| s.name == "MyApp").unwrap();
        let child_names: Vec<&str> = module.children.iter().map(|s| s.name.as_str()).collect();
        assert!(
            child_names.contains(&"hello"),
            "missing hello: {child_names:?}"
        );
        assert!(
            child_names.contains(&"private_fn"),
            "missing private_fn: {child_names:?}"
        );
    }
}

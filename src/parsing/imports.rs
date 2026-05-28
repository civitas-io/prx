use tree_sitter::{Node, Parser};

use super::languages::language_for_extension;

/// Extract import paths from `source` using a tree-sitter parse, dispatched by `ext`.
pub fn extract_imports(source: &str, ext: &str) -> Vec<String> {
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
    let mut imports = Vec::new();
    collect_imports(tree.root_node(), source, ext, &mut imports);
    imports
}

fn collect_imports(node: Node, source: &str, ext: &str, imports: &mut Vec<String>) {
    let consumed = match ext {
        "rs" => extract_rust(node, source, imports),
        "py" | "pyi" => extract_python(node, source, imports),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "mts" | "cts" => {
            extract_js(node, source, imports)
        }
        "go" => extract_go(node, source, imports),
        "java" => extract_java(node, source, imports),
        "c" | "h" | "cpp" | "hpp" | "cc" | "hxx" | "cxx" | "hh" => extract_c(node, source, imports),
        "rb" => extract_ruby(node, source, imports),
        _ => false,
    };
    if consumed {
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_imports(child, source, ext, imports);
    }
}

fn extract_rust(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    match node.kind() {
        "use_declaration" => {
            if let Some(arg) = node.child_by_field_name("argument") {
                push_text(arg, source, imports);
            }
            true
        }
        "extern_crate_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                push_text(name, source, imports);
            }
            true
        }
        "mod_item" => {
            if let Some(name) = node.child_by_field_name("name") {
                push_text(name, source, imports);
            }
            false
        }
        _ => false,
    }
}

fn extract_python(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    match node.kind() {
        "import_from_statement" => {
            if let Some(m) = node.child_by_field_name("module_name") {
                push_text(m, source, imports);
            }
            true
        }
        "import_statement" => {
            let mut cursor = node.walk();
            for child in node.children_by_field_name("name", &mut cursor) {
                let target = if child.kind() == "aliased_import" {
                    child.child_by_field_name("name").unwrap_or(child)
                } else {
                    child
                };
                push_text(target, source, imports);
            }
            true
        }
        _ => false,
    }
}

fn extract_js(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    match node.kind() {
        "import_statement" => {
            if let Some(s) = node.child_by_field_name("source") {
                push_string_literal(s, source, imports);
            }
            true
        }
        "export_statement" => {
            if let Some(s) = node.child_by_field_name("source") {
                push_string_literal(s, source, imports);
                return true;
            }
            false
        }
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                let func_kind = func.kind();
                let func_text = func.utf8_text(source.as_bytes()).unwrap_or("");
                let is_module_call =
                    func_kind == "import" || func_text == "require" || func_text == "import";
                if is_module_call {
                    if let Some(args) = node.child_by_field_name("arguments") {
                        let mut cursor = args.walk();
                        for arg in args.children(&mut cursor) {
                            if arg.kind() == "string" {
                                push_string_literal(arg, source, imports);
                                break;
                            }
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn extract_go(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "import_spec" {
        if let Some(path) = node.child_by_field_name("path") {
            push_string_literal(path, source, imports);
        }
        true
    } else {
        false
    }
}

fn extract_java(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "import_declaration" {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if matches!(child.kind(), "scoped_identifier" | "identifier") {
                push_text(child, source, imports);
                break;
            }
        }
        true
    } else {
        false
    }
}

fn extract_c(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "preproc_include" {
        if let Some(path) = node.child_by_field_name("path") {
            if let Ok(text) = path.utf8_text(source.as_bytes()) {
                let stripped = text.trim_matches(|c: char| matches!(c, '"' | '\'' | '<' | '>'));
                if !stripped.is_empty() {
                    imports.push(stripped.to_string());
                }
            }
        }
        true
    } else {
        false
    }
}

fn extract_ruby(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "call" {
        if let Some(method) = node.child_by_field_name("method") {
            let m_text = method.utf8_text(source.as_bytes()).unwrap_or("");
            if m_text == "require" || m_text == "require_relative" {
                if let Some(args) = node.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    for arg in args.children(&mut cursor) {
                        if arg.kind() == "string" {
                            push_string_literal(arg, source, imports);
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn push_text(node: Node, source: &str, imports: &mut Vec<String>) {
    if let Ok(text) = node.utf8_text(source.as_bytes()) {
        if !text.is_empty() {
            imports.push(text.to_string());
        }
    }
}

fn push_string_literal(node: Node, source: &str, imports: &mut Vec<String>) {
    if let Ok(text) = node.utf8_text(source.as_bytes()) {
        let stripped = text.trim_matches(|c: char| matches!(c, '"' | '\'' | '`'));
        if !stripped.is_empty() {
            imports.push(stripped.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_use_and_mod() {
        let src = "use crate::commands::read;\nmod search;\nextern crate serde;\n";
        let imports = extract_imports(src, "rs");
        assert_eq!(imports, vec!["crate::commands::read", "search", "serde"]);
    }

    #[test]
    fn python_from_and_import() {
        let src = "from foo.bar import baz\nimport os\nfrom pathlib import Path\n";
        let imports = extract_imports(src, "py");
        assert_eq!(imports, vec!["foo.bar", "os", "pathlib"]);
    }

    #[test]
    fn js_from_and_require() {
        let src = "import React from 'react';\nconst fs = require('fs');\nimport { foo } from './utils';\n";
        let imports = extract_imports(src, "js");
        assert_eq!(imports, vec!["react", "fs", "./utils"]);
    }

    #[test]
    fn go_imports() {
        let src = "import (\n\t\"fmt\"\n\t\"os\"\n)\n";
        let imports = extract_imports(src, "go");
        assert_eq!(imports, vec!["fmt", "os"]);
    }

    #[test]
    fn java_imports() {
        let src = "import java.util.List;\nimport static org.junit.Assert.assertEquals;\n";
        let imports = extract_imports(src, "java");
        assert_eq!(
            imports,
            vec!["java.util.List", "org.junit.Assert.assertEquals"]
        );
    }

    #[test]
    fn c_includes() {
        let src = "#include <stdio.h>\n#include \"myheader.h\"\n";
        let imports = extract_imports(src, "c");
        assert_eq!(imports, vec!["stdio.h", "myheader.h"]);
    }

    #[test]
    fn ruby_require() {
        let src = "require 'json'\nrequire_relative 'helper'\n";
        let imports = extract_imports(src, "rb");
        assert_eq!(imports, vec!["json", "helper"]);
    }

    #[test]
    fn unsupported_returns_empty() {
        let imports = extract_imports("anything", "xyz");
        assert!(imports.is_empty());
    }

    #[test]
    fn empty_source() {
        assert!(extract_imports("", "rs").is_empty());
    }

    #[test]
    fn typescript_from() {
        let src = "import { Component } from '@angular/core';\n";
        let imports = extract_imports(src, "ts");
        assert_eq!(imports, vec!["@angular/core"]);
    }

    #[test]
    fn rust_multi_path_use() {
        let src = "use std::{io, fs};\n";
        let imports = extract_imports(src, "rs");
        assert!(
            !imports.is_empty(),
            "expected at least one import from grouped use"
        );
        assert!(
            imports.iter().any(|i| i.contains("std")),
            "expected `std` somewhere in {imports:?}"
        );
    }

    #[test]
    fn python_multiline_import() {
        let src = "from foo import (\n    bar,\n    baz\n)\n";
        let imports = extract_imports(src, "py");
        assert_eq!(imports, vec!["foo"]);
    }

    #[test]
    fn js_reexport() {
        let src = "export { foo } from './utils';\n";
        let imports = extract_imports(src, "js");
        assert_eq!(imports, vec!["./utils"]);
    }

    #[test]
    fn js_dynamic_import() {
        let src = "const m = import('./module');\nimport('./lazy');\n";
        let imports = extract_imports(src, "js");
        assert_eq!(imports, vec!["./module", "./lazy"]);
    }

    #[test]
    fn ts_type_import() {
        let src = "import type { Foo } from './types';\n";
        let imports = extract_imports(src, "ts");
        assert_eq!(imports, vec!["./types"]);
    }
}

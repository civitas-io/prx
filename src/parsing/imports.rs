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
        "sh" | "bash" | "zsh" => extract_bash(node, source, imports),
        "css" => extract_css(node, source, imports),
        "html" | "htm" => extract_html(node, source, imports),
        "kt" | "kts" => extract_kotlin(node, source, imports),
        "swift" => extract_swift(node, source, imports),
        "cs" => extract_csharp(node, source, imports),
        "php" => extract_php(node, source, imports),
        "ex" | "exs" => extract_elixir(node, source, imports),
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

fn extract_bash(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "command" {
        if let Some(name) = node.child_by_field_name("name") {
            let cmd = name.utf8_text(source.as_bytes()).unwrap_or("");
            if cmd == "source" || cmd == "." {
                if let Some(arg) = node.child_by_field_name("argument") {
                    push_text(arg, source, imports);
                    return true;
                }
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "word" && child.id() != name.id() {
                        push_text(child, source, imports);
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn extract_css(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "import_statement" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string_value" || child.kind() == "call_expression" {
                push_string_literal(child, source, imports);
                return true;
            }
        }
    }
    false
}

fn extract_html(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "script_element" || node.kind() == "style_element" || node.kind() == "element"
    {
        let mut is_relevant = node.kind() == "script_element" || node.kind() == "style_element";
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
                let mut tag_cursor = child.walk();
                for attr in child.children(&mut tag_cursor) {
                    if attr.kind() == "tag_name" {
                        let tag = attr.utf8_text(source.as_bytes()).unwrap_or("");
                        if tag == "link" || tag == "script" {
                            is_relevant = true;
                        }
                    }
                    if attr.kind() == "attribute" && is_relevant {
                        let mut ac = attr.walk();
                        let mut name = "";
                        let mut val_node = None;
                        for a in attr.children(&mut ac) {
                            if a.kind() == "attribute_name" {
                                name = a.utf8_text(source.as_bytes()).unwrap_or("");
                            }
                            if a.kind() == "quoted_attribute_value" || a.kind() == "attribute_value"
                            {
                                val_node = Some(a);
                            }
                        }
                        if name == "src" || name == "href" {
                            if let Some(val) = val_node {
                                push_string_literal(val, source, imports);
                            }
                        }
                    }
                }
            }
        }
        return is_relevant;
    }
    false
}

fn extract_kotlin(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "import_header" {
        if let Some(id) = node.child_by_field_name("identifier") {
            push_text(id, source, imports);
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    push_text(child, source, imports);
                    break;
                }
            }
        }
        return true;
    }
    false
}

fn extract_swift(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "import_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                push_text(child, source, imports);
                return true;
            }
        }
        return true;
    }
    false
}

fn extract_csharp(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "using_directive" {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if matches!(child.kind(), "identifier" | "qualified_name") {
                push_text(child, source, imports);
                break;
            }
        }
        return true;
    }
    false
}

fn extract_php(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    match node.kind() {
        "namespace_use_declaration" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "namespace_use_clause" {
                    let mut inner = child.walk();
                    for c in child.named_children(&mut inner) {
                        if c.kind() == "qualified_name" || c.kind() == "name" {
                            push_text(c, source, imports);
                            break;
                        }
                    }
                }
            }
            true
        }
        "expression_statement" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if matches!(
                    child.kind(),
                    "require_expression"
                        | "require_once_expression"
                        | "include_expression"
                        | "include_once_expression"
                ) {
                    let mut inner = child.walk();
                    for c in child.children(&mut inner) {
                        if c.kind() == "string" {
                            push_string_literal(c, source, imports);
                            return true;
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn extract_elixir(node: Node, source: &str, imports: &mut Vec<String>) -> bool {
    if node.kind() == "call" {
        if let Some(target) = node.child(0) {
            if target.kind() == "identifier" {
                let name = target.utf8_text(source.as_bytes()).unwrap_or("");
                if matches!(name, "import" | "alias" | "use" | "require") {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "arguments" {
                            let mut inner = child.walk();
                            for arg in child.children(&mut inner) {
                                if arg.kind() == "alias" || arg.kind() == "identifier" {
                                    push_text(arg, source, imports);
                                    return true;
                                }
                            }
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

    #[test]
    fn bash_source() {
        let src = "source ./config.sh\n. /etc/profile\n";
        let imports = extract_imports(src, "sh");
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.contains("config")));
    }

    #[test]
    fn css_import() {
        let src = "@import 'reset.css';\n@import \"theme.css\";\n";
        let imports = extract_imports(src, "css");
        assert!(!imports.is_empty());
    }

    #[test]
    fn html_script_src() {
        let src =
            "<script src=\"app.js\"></script>\n<link href=\"style.css\" rel=\"stylesheet\">\n";
        let imports = extract_imports(src, "html");
        assert!(!imports.is_empty());
    }

    #[test]
    fn json_no_imports() {
        let src = "{\"key\": \"value\"}\n";
        let imports = extract_imports(src, "json");
        assert!(imports.is_empty());
    }

    #[test]
    fn kotlin_imports() {
        let src = "import java.util.List\nimport kotlin.io.println\n";
        let imports = extract_imports(src, "kt");
        assert_eq!(imports, vec!["java.util.List", "kotlin.io.println"]);
    }

    #[test]
    fn swift_import() {
        let src = "import Foundation\nimport UIKit\n";
        let imports = extract_imports(src, "swift");
        assert_eq!(imports, vec!["Foundation", "UIKit"]);
    }

    #[test]
    fn csharp_using() {
        let src = "using System;\nusing System.IO;\n";
        let imports = extract_imports(src, "cs");
        assert_eq!(imports, vec!["System", "System.IO"]);
    }

    #[test]
    fn php_use_and_require() {
        let src = "<?php\nuse Illuminate\\Database\\Model;\nrequire_once 'helper.php';\n";
        let imports = extract_imports(src, "php");
        assert!(
            imports.iter().any(|i| i.contains("Illuminate")),
            "missing use: {imports:?}"
        );
        assert!(
            imports.iter().any(|i| i.contains("helper")),
            "missing require: {imports:?}"
        );
    }

    #[test]
    fn elixir_import_alias_use() {
        let src = "defmodule MyApp do\n  import Ecto.Query\n  alias MyApp.Repo\n  use GenServer\n  require Logger\nend\n";
        let imports = extract_imports(src, "ex");
        assert!(
            imports.iter().any(|i| i.contains("Ecto")),
            "missing import: {imports:?}"
        );
        assert!(
            imports.iter().any(|i| i.contains("Repo")),
            "missing alias: {imports:?}"
        );
        assert!(
            imports.iter().any(|i| i.contains("GenServer")),
            "missing use: {imports:?}"
        );
        assert!(
            imports.iter().any(|i| i.contains("Logger")),
            "missing require: {imports:?}"
        );
    }
}

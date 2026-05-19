use std::sync::OnceLock;

use regex::Regex;

static RE_RUST: OnceLock<Option<Regex>> = OnceLock::new();
static RE_PYTHON: OnceLock<Option<Regex>> = OnceLock::new();
static RE_JS: OnceLock<Option<Regex>> = OnceLock::new();
static RE_GO: OnceLock<Option<Regex>> = OnceLock::new();
static RE_JAVA: OnceLock<Option<Regex>> = OnceLock::new();
static RE_C: OnceLock<Option<Regex>> = OnceLock::new();
static RE_RUBY: OnceLock<Option<Regex>> = OnceLock::new();

fn get_regex<'a>(lock: &'a OnceLock<Option<Regex>>, pattern: &str) -> Option<&'a Regex> {
    lock.get_or_init(|| Regex::new(pattern).ok()).as_ref()
}

pub fn extract_imports(source: &str, ext: &str) -> Vec<String> {
    match ext {
        "rs" => extract_with(
            source,
            &RE_RUST,
            r"^\s*(?:use|extern\s+crate|mod)\s+([\w:]+)",
        ),
        "py" => extract_python(source),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => extract_js(source),
        "go" => extract_with(source, &RE_GO, r#"^\s*"([^"]+)""#),
        "java" => extract_with(source, &RE_JAVA, r"^\s*import\s+(?:static\s+)?([\w.]+)\s*;"),
        "c" | "h" | "cpp" | "hpp" | "cc" | "hxx" => {
            extract_with(source, &RE_C, r#"^\s*#include\s+["<]([^">]+)[">]"#)
        }
        "rb" => extract_with(
            source,
            &RE_RUBY,
            r#"^\s*require(?:_relative)?\s+['"]([^'"]+)['"]"#,
        ),
        _ => vec![],
    }
}

fn extract_with(source: &str, lock: &OnceLock<Option<Regex>>, pattern: &str) -> Vec<String> {
    let re = match get_regex(lock, pattern) {
        Some(r) => r,
        None => return vec![],
    };
    let mut result = Vec::new();
    for line in source.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(m) = caps.get(1) {
                result.push(m.as_str().to_string());
            }
        }
    }
    result
}

fn extract_python(source: &str) -> Vec<String> {
    let re = match get_regex(
        &RE_PYTHON,
        r"^\s*(?:from\s+([\w.]+)\s+import|import\s+([\w.]+))",
    ) {
        Some(r) => r,
        None => return vec![],
    };
    let mut result = Vec::new();
    for line in source.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(m) = caps.get(1).or_else(|| caps.get(2)) {
                result.push(m.as_str().to_string());
            }
        }
    }
    result
}

fn extract_js(source: &str) -> Vec<String> {
    let re = match get_regex(
        &RE_JS,
        r#"(?:from\s+['"]([^'"]+)['"]|require\(\s*['"]([^'"]+)['"]\s*\))"#,
    ) {
        Some(r) => r,
        None => return vec![],
    };
    let mut result = Vec::new();
    for line in source.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(m) = caps.get(1).or_else(|| caps.get(2)) {
                result.push(m.as_str().to_string());
            }
        }
    }
    result
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
}

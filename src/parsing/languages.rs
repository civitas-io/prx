use tree_sitter::Language;

pub fn language_for_extension(ext: &str) -> Option<Language> {
    match ext {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "py" | "pyi" => Some(tree_sitter_python::LANGUAGE.into()),
        "js" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "ts" | "mts" | "cts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "c" | "h" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(tree_sitter_cpp::LANGUAGE.into()),
        "rb" => Some(tree_sitter_ruby::LANGUAGE.into()),
        "sh" | "bash" | "zsh" => Some(tree_sitter_bash::LANGUAGE.into()),
        "json" => Some(tree_sitter_json::LANGUAGE.into()),
        "html" | "htm" => Some(tree_sitter_html::LANGUAGE.into()),
        "css" => Some(tree_sitter_css::LANGUAGE.into()),
        _ => None,
    }
}

pub fn language_name_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "py" | "pyi" => Some("python"),
        "js" | "mjs" | "cjs" | "jsx" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some("cpp"),
        "rb" => Some("ruby"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "json" => Some("json"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_extensions_return_language() {
        let exts = [
            "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "h", "cpp", "cc", "rb", "sh",
            "bash", "json", "html", "css",
        ];
        for ext in exts {
            assert!(
                language_for_extension(ext).is_some(),
                "no language for .{ext}"
            );
        }
    }

    #[test]
    fn unknown_returns_none() {
        assert!(language_for_extension("xyz").is_none());
        assert!(language_for_extension("md").is_none());
        assert!(language_for_extension("toml").is_none());
    }

    #[test]
    fn names_match_extensions() {
        assert_eq!(language_name_for_extension("rs"), Some("rust"));
        assert_eq!(language_name_for_extension("py"), Some("python"));
        assert_eq!(language_name_for_extension("ts"), Some("typescript"));
        assert_eq!(language_name_for_extension("tsx"), Some("tsx"));
        assert_eq!(language_name_for_extension("xyz"), None);
    }

    #[test]
    fn parsers_can_parse() {
        let test_cases = [
            ("rs", "fn main() {}"),
            ("py", "def hello(): pass"),
            ("js", "function f() {}"),
            ("ts", "function f(): void {}"),
            ("go", "package main\nfunc main() {}"),
            ("java", "class Foo {}"),
            ("c", "int main() { return 0; }"),
            ("cpp", "int main() { return 0; }"),
            ("rb", "def hello; end"),
            ("sh", "echo hello"),
            ("json", "{\"a\": 1}"),
            ("html", "<div>hi</div>"),
            ("css", "body { color: red; }"),
        ];
        for (ext, src) in test_cases {
            let lang = language_for_extension(ext).unwrap();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&lang).unwrap();
            let tree = parser.parse(src, None);
            assert!(tree.is_some(), "failed to parse .{ext}");
        }
    }
}

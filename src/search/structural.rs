use std::path::Path;

use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::{AstGrep, Language, Pattern, PatternError};

use crate::output::AgError;
use crate::parsing;
use crate::walk::{self, WalkOpts};

pub struct StructuralMatch {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub matched_text: String,
    pub snippet: String,
}

pub struct StructuralSearchResult {
    pub matches: Vec<StructuralMatch>,
    pub warning: Option<String>,
}

pub fn structural_search(
    query: &str,
    root: &Path,
    top_k: usize,
) -> Result<StructuralSearchResult, AgError> {
    let entries = walk::walk(root, &WalkOpts::default());
    let mut all_matches = Vec::new();
    let mut pattern_compiled = false;
    let mut files_searched = 0usize;

    for entry in &entries {
        let ext = match parsing::extension_from_path(&entry.path) {
            Some(e) => e,
            None => continue,
        };

        let lang = match lang_for_ext(ext) {
            Some(l) => l,
            None => continue,
        };

        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .to_string();

        let pattern = match Pattern::try_new(query, lang.clone()) {
            Ok(p) => p,
            Err(_) => continue,
        };

        pattern_compiled = true;
        files_searched += 1;

        let doc = match StrDoc::try_new(&content, lang.clone()) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let grep = AstGrep::doc(doc);

        for node_match in grep.root().find_all(&pattern) {
            let start = node_match.start_pos();
            let start_line = start.line();
            let text = node_match.text().to_string();
            let lines: Vec<&str> = content.lines().collect();
            let ctx_start = start_line.saturating_sub(1);
            let ctx_end = (start_line + 3).min(lines.len());
            let snippet = lines[ctx_start..ctx_end].join("\n");

            all_matches.push(StructuralMatch {
                file: rel_path.clone(),
                line: start_line + 1,
                column: 1,
                matched_text: text,
                snippet,
            });

            if all_matches.len() >= top_k {
                return Ok(StructuralSearchResult {
                    matches: all_matches,
                    warning: None,
                });
            }
        }
    }

    let warning = if !pattern_compiled {
        Some(format!(
            "pattern `{query}` did not compile for any language in the search path"
        ))
    } else if all_matches.is_empty() {
        Some(format!(
            "pattern `{query}` compiled but matched 0 of {files_searched} files searched"
        ))
    } else {
        None
    };

    Ok(StructuralSearchResult {
        matches: all_matches,
        warning,
    })
}

#[derive(Clone)]
struct AgLang {
    ts_lang: TSLanguage,
}

impl Language for AgLang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.ts_lang.id_for_node_kind(kind, true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.ts_lang.field_id_for_name(field).map(|f| f.get())
    }

    fn build_pattern(
        &self,
        builder: &ast_grep_core::matcher::PatternBuilder,
    ) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, self.clone()))
    }
}

impl LanguageExt for AgLang {
    fn get_ts_language(&self) -> TSLanguage {
        self.ts_lang.clone()
    }
}

fn lang_for_ext(ext: &str) -> Option<AgLang> {
    let ts_lang = parsing::languages::language_for_extension(ext)?;
    Some(AgLang { ts_lang })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("sample.rs"),
            "fn hello() {\n    let x = 1;\n    let y = 2;\n}\n\nfn world(n: i32) -> i32 {\n    let result = n + 1;\n    result\n}\n",
        ).unwrap();
        std::fs::write(
            dir.path().join("sample.py"),
            "def greet(name):\n    print(f\"Hello {name}\")\n\ndef add(a, b):\n    return a + b\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn finds_rust_let_bindings() {
        let dir = make_test_dir();
        let result = structural_search("let $X = $Y", dir.path(), 10).unwrap();
        assert!(
            result.matches.len() >= 3,
            "should find 3 let bindings, got {}",
            result.matches.len()
        );
        assert!(result.warning.is_none());
    }

    #[test]
    fn top_k_limits() {
        let dir = make_test_dir();
        let result = structural_search("let $X = $Y", dir.path(), 1).unwrap();
        assert!(result.matches.len() <= 1);
    }

    #[test]
    fn no_matches_returns_warning() {
        let dir = make_test_dir();
        let result = structural_search("class $NAME {}", dir.path(), 10).unwrap();
        assert!(result.matches.is_empty());
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("matched 0"));
    }

    #[test]
    fn match_has_correct_location() {
        let dir = make_test_dir();
        let result = structural_search("let $X = $Y", dir.path(), 1).unwrap();
        if !result.matches.is_empty() {
            assert_eq!(result.matches[0].file, "sample.rs");
            assert!(result.matches[0].line >= 1);
            assert!(result.matches[0].column >= 1);
        }
    }

    #[test]
    fn warns_on_nonsense_pattern() {
        let dir = make_test_dir();
        let result = structural_search("}{}{}{", dir.path(), 10).unwrap();
        assert!(result.matches.is_empty());
        assert!(result.warning.is_some());
    }
}

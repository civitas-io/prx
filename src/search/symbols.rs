use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::parsing::outline::{self, SymbolKind};

const SYMBOLS_FILE: &str = "symbols.bin";

/// A single symbol definition location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDef {
    pub file: String,
    pub line: usize,
    pub kind: String,
}

/// Lightweight symbol index: maps symbol names to definition locations + reference counts.
///
/// Built at index time from tree-sitter outlines. For symbol queries, this provides
/// direct lookup instead of relying on BM25 (which cannot distinguish definition
/// from usage when a symbol appears in hundreds of import lines).
pub struct SymbolIndex {
    /// Symbol name (case-sensitive) -> list of definitions
    pub defs: HashMap<String, Vec<SymbolDef>>,
    /// Symbol name -> number of chunks that mention it (reference count)
    pub ref_counts: HashMap<String, u32>,
}

#[derive(Serialize, Deserialize)]
struct SerializedSymbolIndex {
    defs: Vec<(String, Vec<SymbolDef>)>,
    ref_counts: Vec<(String, u32)>,
}

impl SymbolIndex {
    /// Build symbol index from file paths and their contents.
    ///
    /// Extracts symbol definitions via tree-sitter outlines, then counts how many
    /// chunks reference each symbol name.
    pub fn build(
        file_paths: &[String],
        reader: impl Fn(&str) -> Option<String>,
        chunk_texts: &[String],
    ) -> Self {
        let mut defs: HashMap<String, Vec<SymbolDef>> = HashMap::new();

        for path in file_paths {
            let ext = path.rsplit('.').next().unwrap_or("");
            let source = match reader(path) {
                Some(s) => s,
                None => continue,
            };

            let symbols = outline::extract_symbols(&source, ext);
            collect_defs(&symbols, path, &mut defs);
        }

        let ref_counts = count_references(&defs, chunk_texts);

        SymbolIndex { defs, ref_counts }
    }

    /// Look up definitions for a symbol name.
    ///
    /// Returns definitions sorted by reference count (most-referenced first).
    pub fn lookup(&self, name: &str) -> Vec<&SymbolDef> {
        let Some(definitions) = self.defs.get(name) else {
            return vec![];
        };

        let mut result: Vec<&SymbolDef> = definitions.iter().collect();
        result.sort_by(|a, b| {
            let key_a = format!("{}:{}", a.file, a.line);
            let key_b = format!("{}:{}", b.file, b.line);
            let refs_a = self.ref_counts.get(name).copied().unwrap_or(0);
            let refs_b = self.ref_counts.get(name).copied().unwrap_or(0);
            refs_b.cmp(&refs_a).then(key_a.cmp(&key_b))
        });
        result
    }

    /// Look up definitions with a case-insensitive fallback.
    ///
    /// Tries exact match first. If no results, tries case-insensitive match.
    pub fn lookup_flexible(&self, name: &str) -> Vec<&SymbolDef> {
        let exact = self.lookup(name);
        if !exact.is_empty() {
            return exact;
        }

        let lower = name.to_lowercase();
        let mut results = Vec::new();
        for (key, defs) in &self.defs {
            if key.to_lowercase() == lower {
                results.extend(defs.iter());
            }
        }
        results.sort_by(|a, b| {
            let key_a = format!("{}:{}", a.file, a.line);
            let key_b = format!("{}:{}", b.file, b.line);
            key_a.cmp(&key_b)
        });
        results
    }

    /// Persist symbol index to disk (follows graph.rs pattern).
    pub fn save(&self, dir: &Path) -> Result<(), std::io::Error> {
        let serialized = SerializedSymbolIndex {
            defs: self
                .defs
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            ref_counts: self
                .ref_counts
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        };
        let bytes =
            bincode::serialize(&serialized).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(dir.join(SYMBOLS_FILE), bytes)
    }

    /// Load symbol index from disk.
    pub fn load(dir: &Path) -> Result<Self, std::io::Error> {
        let bytes = std::fs::read(dir.join(SYMBOLS_FILE))?;
        let serialized: SerializedSymbolIndex =
            bincode::deserialize(&bytes).map_err(|e| std::io::Error::other(e.to_string()))?;

        Ok(SymbolIndex {
            defs: serialized.defs.into_iter().collect(),
            ref_counts: serialized.ref_counts.into_iter().collect(),
        })
    }
}

/// Recursively collect definitions from symbol tree (handles nested symbols like methods-of-class).
fn collect_defs(
    symbols: &[outline::Symbol],
    file: &str,
    defs: &mut HashMap<String, Vec<SymbolDef>>,
) {
    for sym in symbols {
        let kind_str = match sym.kind {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Interface => "interface",
            SymbolKind::Type => "type",
            SymbolKind::Const => "const",
            SymbolKind::Module => "module",
        };

        defs.entry(sym.name.clone()).or_default().push(SymbolDef {
            file: file.to_string(),
            line: sym.start_line,
            kind: kind_str.to_string(),
        });

        if !sym.children.is_empty() {
            collect_defs(&sym.children, file, defs);
        }
    }
}

/// Count how many chunks mention each symbol name.
fn count_references(
    defs: &HashMap<String, Vec<SymbolDef>>,
    chunk_texts: &[String],
) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();

    for name in defs.keys() {
        if name.len() < 3 {
            continue;
        }
        let mut count: u32 = 0;
        for text in chunk_texts {
            if text.contains(name.as_str()) {
                count += 1;
            }
        }
        counts.insert(name.clone(), count);
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_files() -> Vec<(String, String)> {
        vec![
            (
                "src/manager.py".to_string(),
                "class ConfigurationManager:\n    def get(self):\n        return self.config\n".to_string(),
            ),
            (
                "src/handler.py".to_string(),
                "from manager import ConfigurationManager\ndef handle():\n    mgr = ConfigurationManager()\n".to_string(),
            ),
            (
                "src/auth.rs".to_string(),
                "pub fn authenticate(user: &User) -> Token {\n    validate(user)\n}\n".to_string(),
            ),
        ]
    }

    fn make_chunks() -> Vec<String> {
        vec![
            "class ConfigurationManager:\n    def get(self):\n        return self.config".to_string(),
            "from manager import ConfigurationManager\ndef handle():\n    mgr = ConfigurationManager()".to_string(),
            "import ConfigurationManager\nuse_it()".to_string(),
            "pub fn authenticate(user: &User) -> Token {\n    validate(user)\n}".to_string(),
        ]
    }

    #[test]
    fn build_extracts_definitions() {
        let files = make_files();
        let chunks = make_chunks();
        let idx = SymbolIndex::build(
            &files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            |path| {
                files
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, c)| c.clone())
            },
            &chunks,
        );

        assert!(idx.defs.contains_key("ConfigurationManager"));
        assert!(idx.defs.contains_key("authenticate"));
    }

    #[test]
    fn lookup_returns_definitions() {
        let files = make_files();
        let chunks = make_chunks();
        let idx = SymbolIndex::build(
            &files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            |path| {
                files
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, c)| c.clone())
            },
            &chunks,
        );

        let results = idx.lookup("ConfigurationManager");
        assert!(!results.is_empty());
        assert!(results.iter().any(|d| d.file == "src/manager.py"));
        assert!(results.iter().any(|d| d.kind == "class"));
    }

    #[test]
    fn reference_counting_works() {
        let files = make_files();
        let chunks = make_chunks();
        let idx = SymbolIndex::build(
            &files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            |path| {
                files
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, c)| c.clone())
            },
            &chunks,
        );

        let refs = idx
            .ref_counts
            .get("ConfigurationManager")
            .copied()
            .unwrap_or(0);
        assert_eq!(refs, 3);

        let refs = idx.ref_counts.get("authenticate").copied().unwrap_or(0);
        assert_eq!(refs, 1);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let files = make_files();
        let chunks = make_chunks();
        let idx = SymbolIndex::build(
            &files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            |path| {
                files
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, c)| c.clone())
            },
            &chunks,
        );

        let dir = tempfile::tempdir().unwrap();
        idx.save(dir.path()).unwrap();

        let loaded = SymbolIndex::load(dir.path()).unwrap();
        assert_eq!(loaded.defs.len(), idx.defs.len());
        assert_eq!(loaded.ref_counts.len(), idx.ref_counts.len());
        assert!(loaded.defs.contains_key("ConfigurationManager"));
        assert_eq!(
            loaded.ref_counts.get("ConfigurationManager"),
            idx.ref_counts.get("ConfigurationManager")
        );
    }

    #[test]
    fn lookup_flexible_case_insensitive() {
        let files = make_files();
        let chunks = make_chunks();
        let idx = SymbolIndex::build(
            &files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            |path| {
                files
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, c)| c.clone())
            },
            &chunks,
        );

        let exact = idx.lookup_flexible("ConfigurationManager");
        assert!(!exact.is_empty());

        let fallback = idx.lookup_flexible("configurationmanager");
        assert!(!fallback.is_empty());
    }

    #[test]
    fn empty_input() {
        let idx = SymbolIndex::build(&[], |_| None, &[]);
        assert!(idx.defs.is_empty());
        assert!(idx.ref_counts.is_empty());
        assert!(idx.lookup("anything").is_empty());
    }

    #[test]
    fn nested_symbols_collected() {
        let files = vec![(
            "src/app.py".to_string(),
            "class App:\n    def run(self):\n        pass\n    def stop(self):\n        pass\n"
                .to_string(),
        )];
        let chunks = vec!["class App:\n    def run(self):\n        pass".to_string()];
        let idx = SymbolIndex::build(
            &files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            |path| {
                files
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, c)| c.clone())
            },
            &chunks,
        );

        assert!(idx.defs.contains_key("App"));
        assert!(idx.defs.contains_key("run"));
        assert!(idx.defs.contains_key("stop"));
    }
}

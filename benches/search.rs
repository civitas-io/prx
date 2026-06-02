use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

fn make_bench_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    let rust_src = r#"
use std::collections::HashMap;

pub struct SearchEngine {
    index: HashMap<String, Vec<usize>>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self { index: HashMap::new() }
    }

    pub fn add_document(&mut self, id: usize, content: &str) {
        for word in content.split_whitespace() {
            self.index.entry(word.to_lowercase()).or_default().push(id);
        }
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        self.index.get(&query.to_lowercase()).cloned().unwrap_or_default()
    }
}

fn process_batch(items: &[String]) -> Vec<String> {
    items.iter().filter(|s| !s.is_empty()).cloned().collect()
}

fn validate_input(input: &str) -> Result<(), String> {
    if input.is_empty() {
        return Err("empty input".to_string());
    }
    Ok(())
}
"#;

    let python_src = r#"
import json
from typing import List, Dict, Optional

class DataProcessor:
    def __init__(self, config: Dict):
        self.config = config
        self.cache: Dict[str, any] = {}

    def process(self, data: List[Dict]) -> List[Dict]:
        results = []
        for item in data:
            if self.validate(item):
                results.append(self.transform(item))
        return results

    def validate(self, item: Dict) -> bool:
        return "id" in item and "value" in item

    def transform(self, item: Dict) -> Dict:
        return {
            "id": item["id"],
            "processed_value": item["value"] * 2,
        }

def load_config(path: str) -> Optional[Dict]:
    try:
        with open(path) as f:
            return json.load(f)
    except FileNotFoundError:
        return None
"#;

    let js_src = r#"
export class EventEmitter {
    constructor() {
        this.listeners = new Map();
    }

    on(event, callback) {
        if (!this.listeners.has(event)) {
            this.listeners.set(event, []);
        }
        this.listeners.get(event).push(callback);
        return this;
    }

    emit(event, ...args) {
        const callbacks = this.listeners.get(event) || [];
        callbacks.forEach(cb => cb(...args));
    }

    off(event, callback) {
        const callbacks = this.listeners.get(event) || [];
        this.listeners.set(event, callbacks.filter(cb => cb !== callback));
    }
}

export function debounce(fn, delay) {
    let timer;
    return (...args) => {
        clearTimeout(timer);
        timer = setTimeout(() => fn(...args), delay);
    };
}
"#;

    std::fs::write(dir.path().join("engine.rs"), rust_src).unwrap();
    std::fs::write(dir.path().join("processor.py"), python_src).unwrap();
    std::fs::write(dir.path().join("events.js"), js_src).unwrap();

    for i in 0..20 {
        let content = format!(
            "pub fn generated_fn_{i}(x: i32) -> i32 {{\n    x + {i}\n}}\n\n\
             pub fn helper_{i}() -> String {{\n    format!(\"helper {i}\")\n}}\n"
        );
        std::fs::write(dir.path().join(format!("gen_{i}.rs")), content).unwrap();
    }

    dir
}

fn bench_bm25_index_build(c: &mut Criterion) {
    let dir = make_bench_dir();
    let root = dir.path();

    c.bench_function("bm25_index_build", |b| {
        b.iter(|| {
            let entries = prx::walk::walk(root, &prx::walk::WalkOpts::default());
            let mut texts = Vec::new();
            for entry in &entries {
                let content = match std::fs::read_to_string(&entry.path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let rel = entry
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&entry.path)
                    .to_string_lossy()
                    .to_string();
                let ext = prx::parsing::extension_from_path(&entry.path);
                let chunks = prx::chunking::chunk_file(&content, &rel, ext);
                for chunk in &chunks {
                    texts.push(prx::index::sparse::enrich_for_bm25(
                        &chunk.content,
                        &chunk.file_path,
                    ));
                }
            }
            prx::index::sparse::SparseIndex::build(&texts)
        })
    });
}

fn bench_bm25_query(c: &mut Criterion) {
    let dir = make_bench_dir();
    let root = dir.path();

    let entries = prx::walk::walk(root, &prx::walk::WalkOpts::default());
    let mut texts = Vec::new();
    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel = entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .to_string();
        let ext = prx::parsing::extension_from_path(&entry.path);
        let chunks = prx::chunking::chunk_file(&content, &rel, ext);
        for chunk in &chunks {
            texts.push(prx::index::sparse::enrich_for_bm25(
                &chunk.content,
                &chunk.file_path,
            ));
        }
    }
    let index = prx::index::sparse::SparseIndex::build(&texts);

    let queries = [
        "search engine",
        "process data",
        "event emitter",
        "validate input",
        "generated_fn",
    ];

    c.bench_function("bm25_query", |b| {
        b.iter(|| {
            for q in &queries {
                let _ = index.query(q, 5);
            }
        })
    });
}

fn bench_literal_search(c: &mut Criterion) {
    let dir = make_bench_dir();

    c.bench_function("literal_search", |b| {
        b.iter(|| {
            let args = prx::commands::search::SearchArgs {
                query: "fn ".to_string(),
                path: dir.path().to_string_lossy().to_string(),
                literal: true,
                top_k: 10,
                ..Default::default()
            };
            let _ = prx::commands::search::run(args);
        })
    });
}

fn bench_persistent_index_build(c: &mut Criterion) {
    let dir = make_bench_dir();

    c.bench_function("persistent_index_build", |b| {
        b.iter(|| {
            let _ = prx::index::persist::build_and_save(dir.path(), "builtin");
        })
    });
}

fn bench_incremental_index_noop(c: &mut Criterion) {
    let dir = make_bench_dir();
    prx::index::persist::build_and_save(dir.path(), "builtin").unwrap();

    c.bench_function("incremental_index_noop", |b| {
        b.iter(|| {
            let _ = prx::index::persist::build_and_save(dir.path(), "builtin");
        })
    });
}

criterion_group!(
    benches,
    bench_bm25_index_build,
    bench_bm25_query,
    bench_literal_search,
    bench_persistent_index_build,
    bench_incremental_index_noop,
);
criterion_main!(benches);

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::parsing::imports;

pub struct ImportGraph {
    pub paths: Vec<String>,
    pub path_to_id: HashMap<String, u32>,
    pub forward: Vec<Vec<u32>>,
    pub reverse: Vec<Vec<u32>>,
}

#[derive(Serialize, Deserialize)]
struct SerializedGraph {
    paths: Vec<String>,
    edges: Vec<(u32, u32)>,
}

impl ImportGraph {
    pub fn build_full(file_paths: &[String], reader: impl Fn(&str) -> Option<String>) -> Self {
        let (paths, path_to_id, suffix_index) = build_path_index(file_paths);
        let n = paths.len();
        let mut forward = vec![Vec::new(); n];

        for (i, path) in paths.iter().enumerate() {
            let ext = path.rsplit('.').next().unwrap_or("");
            if let Some(source) = reader(path) {
                let raw_imports = imports::extract_imports(&source, ext);
                for imp in raw_imports {
                    for &target in resolve_import(&imp, &suffix_index, &paths, i as u32).iter() {
                        if target != i as u32 {
                            forward[i].push(target);
                        }
                    }
                }
                forward[i].sort();
                forward[i].dedup();
            }
        }

        let reverse = build_reverse(&forward, n);

        ImportGraph {
            paths,
            path_to_id,
            forward,
            reverse,
        }
    }

    pub fn build_partial(
        seed_paths: &[&str],
        all_paths: &[String],
        reader: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let (paths, path_to_id, suffix_index) = build_path_index(all_paths);
        let n = paths.len();
        let mut forward = vec![Vec::new(); n];

        for seed in seed_paths {
            let Some(&id) = path_to_id.get(*seed) else {
                continue;
            };
            let ext = seed.rsplit('.').next().unwrap_or("");
            if let Some(source) = reader(seed) {
                let raw_imports = imports::extract_imports(&source, ext);
                for imp in raw_imports {
                    for &target in resolve_import(&imp, &suffix_index, &paths, id).iter() {
                        if target != id {
                            forward[id as usize].push(target);
                        }
                    }
                }
                forward[id as usize].sort();
                forward[id as usize].dedup();
            }
        }

        let reverse = build_reverse(&forward, n);

        ImportGraph {
            paths,
            path_to_id,
            forward,
            reverse,
        }
    }

    pub fn neighbors_within(&self, seeds: &[u32], max_hops: u8) -> HashMap<u32, u8> {
        let mut visited: HashMap<u32, u8> = HashMap::new();
        let mut queue: VecDeque<(u32, u8)> = VecDeque::new();

        for &seed in seeds {
            visited.insert(seed, 0);
            queue.push_back((seed, 0));
        }

        while let Some((node, hop)) = queue.pop_front() {
            if hop >= max_hops {
                continue;
            }
            let next_hop = hop + 1;

            let neighbors = self
                .forward
                .get(node as usize)
                .into_iter()
                .flatten()
                .chain(self.reverse.get(node as usize).into_iter().flatten());

            for &neighbor in neighbors {
                if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(neighbor) {
                    e.insert(next_hop);
                    queue.push_back((neighbor, next_hop));
                }
            }
        }

        visited
    }

    pub fn save(&self, dir: &Path) -> Result<(), std::io::Error> {
        let mut edges = Vec::new();
        for (from, targets) in self.forward.iter().enumerate() {
            for &to in targets {
                edges.push((from as u32, to));
            }
        }
        let serialized = SerializedGraph {
            paths: self.paths.clone(),
            edges,
        };
        let bytes =
            postcard::to_allocvec(&serialized).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(dir.join("imports.bin"), bytes)
    }

    pub fn load(dir: &Path) -> Result<Self, std::io::Error> {
        let bytes = std::fs::read(dir.join("imports.bin"))?;
        let serialized: SerializedGraph =
            postcard::from_bytes(&bytes).map_err(|e| std::io::Error::other(e.to_string()))?;

        let n = serialized.paths.len();
        let mut forward = vec![Vec::new(); n];
        for (from, to) in &serialized.edges {
            if (*from as usize) < n && (*to as usize) < n {
                forward[*from as usize].push(*to);
            }
        }

        let reverse = build_reverse(&forward, n);
        let path_to_id: HashMap<String, u32> = serialized
            .paths
            .iter()
            .enumerate()
            .map(|(i, p)| (p.clone(), i as u32))
            .collect();

        Ok(ImportGraph {
            paths: serialized.paths,
            path_to_id,
            forward,
            reverse,
        })
    }
}

type PathIndex = (Vec<String>, HashMap<String, u32>, HashMap<String, Vec<u32>>);

fn build_path_index(file_paths: &[String]) -> PathIndex {
    let mut paths = Vec::new();
    let mut path_to_id = HashMap::new();
    let mut suffix_index: HashMap<String, Vec<u32>> = HashMap::new();

    for (i, path) in file_paths.iter().enumerate() {
        let id = i as u32;
        paths.push(path.clone());
        path_to_id.insert(path.clone(), id);

        let stem = strip_extension(path);
        let parts: Vec<&str> = stem.split('/').collect();
        for start in 0..parts.len() {
            let suffix = parts[start..].join("/");
            suffix_index.entry(suffix).or_default().push(id);
        }
    }

    (paths, path_to_id, suffix_index)
}

fn strip_extension(path: &str) -> &str {
    match path.rfind('.') {
        Some(i) if i > 0 && !path[i..].contains('/') => &path[..i],
        _ => path,
    }
}

fn normalize_import(raw: &str) -> String {
    let s = raw.replace("::", "/");
    let s = if s.starts_with("./") || s.starts_with("../") {
        s.trim_start_matches("../")
            .trim_start_matches("./")
            .to_string()
    } else {
        s.replace('.', "/")
    };
    let mut result = s;
    for prefix in &["crate/", "self/", "super/"] {
        if let Some(stripped) = result.strip_prefix(prefix) {
            result = stripped.to_string();
            break;
        }
    }
    result
}

fn resolve_import(
    raw: &str,
    suffix_index: &HashMap<String, Vec<u32>>,
    paths: &[String],
    importer_id: u32,
) -> Vec<u32> {
    let normalized = normalize_import(raw);

    if let Some(ids) = suffix_index.get(&normalized) {
        if ids.len() <= 5 {
            return ids.clone();
        }
        return pick_closest(ids, paths, importer_id);
    }

    let trimmed = match normalized.rfind('/') {
        Some(i) => &normalized[..i],
        None => return vec![],
    };
    if let Some(ids) = suffix_index.get(trimmed) {
        if ids.len() <= 5 {
            return ids.clone();
        }
        return pick_closest(ids, paths, importer_id);
    }

    vec![]
}

fn pick_closest(candidates: &[u32], paths: &[String], importer_id: u32) -> Vec<u32> {
    let importer_dir = paths
        .get(importer_id as usize)
        .and_then(|p| p.rfind('/').map(|i| &p[..i]))
        .unwrap_or("");

    let mut scored: Vec<(u32, usize)> = candidates
        .iter()
        .map(|&id| {
            let candidate_dir = paths
                .get(id as usize)
                .and_then(|p| p.rfind('/').map(|i| &p[..i]))
                .unwrap_or("");
            let shared = common_prefix_len(importer_dir, candidate_dir);
            (id, shared)
        })
        .collect();

    scored.sort_by_key(|s| std::cmp::Reverse(s.1));
    scored.truncate(2);
    scored.into_iter().map(|(id, _)| id).collect()
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.split('/')
        .zip(b.split('/'))
        .take_while(|(x, y)| x == y)
        .count()
}

fn build_reverse(forward: &[Vec<u32>], n: usize) -> Vec<Vec<u32>> {
    let mut reverse = vec![Vec::new(); n];
    for (from, targets) in forward.iter().enumerate() {
        for &to in targets {
            if (to as usize) < n {
                reverse[to as usize].push(from as u32);
            }
        }
    }
    reverse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_full_creates_edges() {
        let paths = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/utils.rs".to_string(),
        ];
        let graph = ImportGraph::build_full(&paths, |path| match path {
            "src/main.rs" => Some("use crate::lib;\nuse crate::utils;\n".to_string()),
            "src/lib.rs" => Some("use crate::utils;\n".to_string()),
            _ => Some(String::new()),
        });

        assert!(!graph.forward[0].is_empty());
        assert_eq!(graph.paths.len(), 3);
    }

    #[test]
    fn neighbors_within_bfs() {
        let paths = vec!["a.rs", "b.rs", "c.rs", "d.rs"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let graph = ImportGraph {
            path_to_id: paths
                .iter()
                .enumerate()
                .map(|(i, p)| (p.clone(), i as u32))
                .collect(),
            paths,
            forward: vec![vec![1], vec![2], vec![3], vec![]],
            reverse: vec![vec![], vec![0], vec![1], vec![2]],
        };

        let hop_map = graph.neighbors_within(&[0], 2);
        assert_eq!(hop_map.get(&0), Some(&0));
        assert_eq!(hop_map.get(&1), Some(&1));
        assert_eq!(hop_map.get(&2), Some(&2));
        assert_eq!(hop_map.get(&3), None);
    }

    #[test]
    fn cycle_does_not_loop() {
        let paths = vec!["a.rs", "b.rs"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let graph = ImportGraph {
            path_to_id: paths
                .iter()
                .enumerate()
                .map(|(i, p)| (p.clone(), i as u32))
                .collect(),
            paths,
            forward: vec![vec![1], vec![0]],
            reverse: vec![vec![1], vec![0]],
        };

        let hop_map = graph.neighbors_within(&[0], 5);
        assert_eq!(hop_map.len(), 2);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let paths = vec!["a.rs".to_string(), "b.rs".to_string()];
        let graph = ImportGraph {
            path_to_id: paths
                .iter()
                .enumerate()
                .map(|(i, p)| (p.clone(), i as u32))
                .collect(),
            paths: paths.clone(),
            forward: vec![vec![1], vec![]],
            reverse: vec![vec![], vec![0]],
        };

        graph.save(dir.path()).unwrap();
        let loaded = ImportGraph::load(dir.path()).unwrap();

        assert_eq!(loaded.paths, paths);
        assert_eq!(loaded.forward[0], vec![1]);
        assert!(!loaded.reverse[1].is_empty());
    }

    #[test]
    fn normalize_strips_prefixes() {
        assert_eq!(normalize_import("crate::foo::bar"), "foo/bar");
        assert_eq!(normalize_import("self::utils"), "utils");
        assert_eq!(normalize_import("./components/Button"), "components/Button");
        assert_eq!(normalize_import("foo.bar.baz"), "foo/bar/baz");
    }

    #[test]
    fn strip_extension_works() {
        assert_eq!(strip_extension("src/main.rs"), "src/main");
        assert_eq!(strip_extension("lib.py"), "lib");
        assert_eq!(strip_extension("noext"), "noext");
    }

    #[test]
    fn partial_build_only_reads_seeds() {
        let all = vec![
            "src/a.rs".to_string(),
            "src/b.rs".to_string(),
            "src/c.rs".to_string(),
        ];
        let read_count = std::cell::Cell::new(0u32);
        let graph = ImportGraph::build_partial(&["src/a.rs"], &all, |_path| {
            read_count.set(read_count.get() + 1);
            Some("use crate::b;\n".to_string())
        });
        assert_eq!(read_count.get(), 1);
        assert!(!graph.forward[0].is_empty());
        assert!(graph.forward[1].is_empty());
    }

    #[test]
    fn empty_paths() {
        let graph = ImportGraph::build_full(&[], |_| None);
        assert!(graph.paths.is_empty());
        assert!(graph.neighbors_within(&[], 2).is_empty());
    }

    #[test]
    fn resolve_picks_closest_when_ambiguous() {
        let paths = vec![
            "src/components/index.ts".to_string(),
            "src/utils/index.ts".to_string(),
            "src/api/index.ts".to_string(),
            "src/models/index.ts".to_string(),
            "src/components/button.ts".to_string(),
        ];
        let (_, _, suffix_index) = build_path_index(&paths);

        let result = resolve_import("./index", &suffix_index, &paths, 4);
        assert!(!result.is_empty());
        assert!(
            result.contains(&0),
            "should resolve to src/components/index.ts (same dir as importer)"
        );
    }

    #[test]
    fn resolve_does_not_bail_on_common_names() {
        let paths: Vec<String> = (0..10).map(|i| format!("pkg{i}/utils.py")).collect();
        let (_, _, suffix_index) = build_path_index(&paths);

        let result = resolve_import("utils", &suffix_index, &paths, 0);
        assert!(
            !result.is_empty(),
            "should not bail on >3 matches — should pick closest"
        );
        assert!(result.len() <= 2, "should return at most 2 candidates");
    }

    #[test]
    fn common_prefix_len_works() {
        assert_eq!(common_prefix_len("src/components", "src/components"), 2);
        assert_eq!(common_prefix_len("src/components", "src/utils"), 1);
        assert_eq!(common_prefix_len("lib/foo", "src/bar"), 0);
        assert_eq!(common_prefix_len("", "src"), 0);
    }
}

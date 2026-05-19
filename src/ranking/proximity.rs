use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::search::graph::ImportGraph;

const PROXIMITY_BOOST_MULTIPLIER: f32 = 0.25;
const MAX_HOPS: u8 = 2;
const SEED_TOP_K: usize = 8;
const HOP_DECAY: f32 = 0.5;

pub fn boost_proximity(
    scores: &mut HashMap<usize, f32>,
    file_paths: &[String],
    graph: &ImportGraph,
) {
    let max_score = scores.values().copied().fold(0.0_f32, f32::max);
    if max_score == 0.0 {
        return;
    }

    let mut per_file: HashMap<u32, f32> = HashMap::new();
    for (&cid, &s) in scores.iter() {
        if let Some(p) = file_paths.get(cid) {
            if let Some(&fid) = graph.path_to_id.get(p) {
                *per_file.entry(fid).or_insert(0.0) += s;
            }
        }
    }

    let threshold = max_score * 0.1;
    let mut seeds: Vec<(u32, f32)> = per_file
        .into_iter()
        .filter(|(_, s)| *s >= threshold)
        .collect();
    seeds.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    seeds.truncate(SEED_TOP_K);
    let seed_set: HashSet<u32> = seeds.iter().map(|(i, _)| *i).collect();
    let seed_ids: Vec<u32> = seeds.into_iter().map(|(i, _)| i).collect();

    let hop_map = graph.neighbors_within(&seed_ids, MAX_HOPS);

    let unit = max_score * PROXIMITY_BOOST_MULTIPLIER;
    for (&cid, score) in scores.iter_mut() {
        let Some(p) = file_paths.get(cid) else {
            continue;
        };
        let Some(&fid) = graph.path_to_id.get(p) else {
            continue;
        };
        if seed_set.contains(&fid) {
            continue;
        }
        if let Some(&hop) = hop_map.get(&fid) {
            if hop > 0 {
                *score += unit * HOP_DECAY.powi(hop as i32 - 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph() -> ImportGraph {
        // auth → handler → utils, unrelated is isolated
        // 10 total files so SEED_TOP_K (8) doesn't swallow all scored files
        let mut paths: Vec<String> = vec![
            "src/auth.rs",
            "src/handler.rs",
            "src/utils.rs",
            "src/unrelated.rs",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        for i in 0..6 {
            paths.push(format!("src/filler_{i}.rs"));
        }
        let n = paths.len();
        let mut forward = vec![Vec::new(); n];
        forward[0] = vec![1]; // auth imports handler
        forward[1] = vec![2]; // handler imports utils
        let mut reverse = vec![Vec::new(); n];
        reverse[1] = vec![0];
        reverse[2] = vec![1];

        ImportGraph {
            path_to_id: paths
                .iter()
                .enumerate()
                .map(|(i, p)| (p.clone(), i as u32))
                .collect(),
            paths,
            forward,
            reverse,
        }
    }

    #[test]
    fn boosts_neighbors() {
        let graph = make_graph();
        // auth (0) is top-scored seed. handler (1) and utils (2) are neighbors.
        let mut scores: HashMap<usize, f32> =
            HashMap::from([(0, 1.0), (1, 0.01), (2, 0.01), (3, 0.01)]);
        let paths = graph.paths.clone();

        boost_proximity(&mut scores, &paths, &graph);

        assert!(scores[&1] > 0.01, "1-hop neighbor should be boosted");
        assert!(scores[&2] > 0.01, "2-hop neighbor should be boosted");
        assert_eq!(scores[&3], 0.01, "unrelated should not change");
    }

    #[test]
    fn hop_decay_applied() {
        let graph = make_graph();
        let mut scores: HashMap<usize, f32> = HashMap::from([(0, 1.0), (1, 0.001), (2, 0.001)]);
        let paths = graph.paths.clone();

        boost_proximity(&mut scores, &paths, &graph);

        let boost_1hop = scores[&1] - 0.001;
        let boost_2hop = scores[&2] - 0.001;
        assert!(
            boost_1hop > boost_2hop,
            "1-hop ({boost_1hop}) should get more boost than 2-hop ({boost_2hop})"
        );
    }

    #[test]
    fn seeds_not_boosted() {
        let graph = make_graph();
        let mut scores: HashMap<usize, f32> = HashMap::from([(0, 1.0)]);
        let paths = graph.paths.clone();

        let original = scores[&0];
        boost_proximity(&mut scores, &paths, &graph);

        assert_eq!(scores[&0], original, "seed should not self-boost");
    }

    #[test]
    fn empty_scores_no_panic() {
        let graph = make_graph();
        let mut scores: HashMap<usize, f32> = HashMap::new();
        boost_proximity(&mut scores, &[], &graph);
        assert!(scores.is_empty());
    }

    #[test]
    fn zero_max_score_no_boost() {
        let graph = make_graph();
        let mut scores: HashMap<usize, f32> = HashMap::from([(0, 0.0), (1, 0.0)]);
        let paths = graph.paths.clone();

        boost_proximity(&mut scores, &paths, &graph);

        assert_eq!(scores[&0], 0.0);
        assert_eq!(scores[&1], 0.0);
    }
}

use assert_cmd::Command;
use serde::Deserialize;

#[derive(Deserialize)]
struct NdcgDataset {
    queries: Vec<NdcgQuery>,
    #[serde(default)]
    root: Option<String>,
}

#[derive(Deserialize)]
struct NdcgQuery {
    query: String,
    category: String,
    relevant: Vec<RelevantFile>,
}

#[derive(Deserialize)]
struct RelevantFile {
    file: String,
    relevance: u32,
}

#[derive(Deserialize)]
struct SearchEnvelope {
    data: SearchData,
}

#[derive(Deserialize)]
struct SearchData {
    matches: Vec<SearchMatch>,
}

#[derive(Deserialize)]
struct SearchMatch {
    file: String,
}

fn dcg(relevances: &[f64], k: usize) -> f64 {
    relevances
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, &rel)| rel / (i as f64 + 2.0).log2())
        .sum()
}

fn ndcg_at_k(relevances: &[f64], k: usize) -> f64 {
    let actual_dcg = dcg(relevances, k);
    if actual_dcg == 0.0 {
        return 0.0;
    }

    let mut ideal = relevances.to_vec();
    ideal.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let ideal_dcg = dcg(&ideal, k);

    if ideal_dcg == 0.0 {
        0.0
    } else {
        actual_dcg / ideal_dcg
    }
}

fn ag() -> Command {
    let mut cmd = Command::cargo_bin("prx").unwrap();
    cmd.env("PRX_STATS_FILE", "/dev/null");
    cmd.env("PRX_ERRORS_FILE", "/dev/null");
    cmd
}

fn run_ndcg(dataset_file: &str, label: &str) {
    let dataset_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks")
        .join(dataset_file);
    let dataset_str = std::fs::read_to_string(&dataset_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", dataset_path.display()));
    let dataset: NdcgDataset = serde_json::from_str(&dataset_str).unwrap();

    let search_root = match &dataset.root {
        Some(root) => root.clone(),
        None => {
            let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
            src_dir.to_string_lossy().to_string()
        }
    };
    let src_path = search_root;

    let mut all_ndcg5 = Vec::new();
    let mut all_ndcg10 = Vec::new();
    let mut by_category: std::collections::HashMap<String, Vec<f64>> =
        std::collections::HashMap::new();

    for q in &dataset.queries {
        let output = ag()
            .args(["search", &q.query, &src_path, "--top-k", "10"])
            .output()
            .unwrap();

        if !output.status.success() {
            eprintln!("SKIP query {:?}: command failed", q.query);
            continue;
        }

        let envelope: SearchEnvelope = match serde_json::from_slice(&output.stdout) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("SKIP query {:?}: parse error: {e}", q.query);
                continue;
            }
        };

        let relevances: Vec<f64> = envelope
            .data
            .matches
            .iter()
            .map(|m| {
                q.relevant
                    .iter()
                    .find(|r| {
                        m.file == r.file || m.file.ends_with(&r.file) || r.file.ends_with(&m.file)
                    })
                    .map(|r| r.relevance as f64)
                    .unwrap_or(0.0)
            })
            .collect();

        let n5 = ndcg_at_k(&relevances, 5);
        let n10 = ndcg_at_k(&relevances, 10);
        all_ndcg5.push(n5);
        all_ndcg10.push(n10);
        by_category.entry(q.category.clone()).or_default().push(n10);
    }

    let mean_ndcg5 = if all_ndcg5.is_empty() {
        0.0
    } else {
        all_ndcg5.iter().sum::<f64>() / all_ndcg5.len() as f64
    };
    let mean_ndcg10 = if all_ndcg10.is_empty() {
        0.0
    } else {
        all_ndcg10.iter().sum::<f64>() / all_ndcg10.len() as f64
    };

    eprintln!("\n=== NDCG Results ({label}) ===");
    eprintln!("Queries evaluated: {}", all_ndcg10.len());
    eprintln!("Mean NDCG@5:  {mean_ndcg5:.4}");
    eprintln!("Mean NDCG@10: {mean_ndcg10:.4}");

    for (cat, scores) in &by_category {
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        eprintln!("  {cat}: NDCG@10 = {mean:.4} (n={})", scores.len());
    }

    let results = serde_json::json!({
        "label": label,
        "mean_ndcg5": mean_ndcg5,
        "mean_ndcg10": mean_ndcg10,
        "queries_evaluated": all_ndcg10.len(),
        "by_category": by_category.iter().map(|(k, v)| {
            (k.clone(), serde_json::json!({
                "mean_ndcg10": v.iter().sum::<f64>() / v.len() as f64,
                "count": v.len(),
            }))
        }).collect::<serde_json::Map<String, serde_json::Value>>(),
    });

    let results_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks");
    let results_file = format!(
        "ndcg_results_{}.json",
        label.replace(' ', "_").to_lowercase()
    );
    let results_path = results_dir.join(&results_file);
    let _ = std::fs::write(
        &results_path,
        serde_json::to_string_pretty(&results).unwrap(),
    );
    eprintln!("Results written to {}", results_path.display());

    assert!(
        !all_ndcg10.is_empty(),
        "no queries were evaluated successfully"
    );
}

#[test]
fn ndcg_at_10_measurement() {
    run_ndcg("ndcg_dataset.json", "prx");
}

#[test]
fn ndcg_at_10_fiddler() {
    let dataset_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks")
        .join("ndcg_dataset_fiddler.json");
    if !dataset_path.exists() {
        eprintln!("SKIP: fiddler dataset not found");
        return;
    }
    let root_path = std::path::Path::new("/Users/jeryn/workspace/projects/fiddler");
    if !root_path.exists() {
        eprintln!("SKIP: fiddler repo not found at {}", root_path.display());
        return;
    }
    run_ndcg("ndcg_dataset_fiddler.json", "fiddler");
}

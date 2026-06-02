#!/usr/bin/env python3
"""Run prx bench-ndcg across all candidate models and benchmark repos.

Requires:
    1. A release build of prx: cargo build --release
    2. Distilled models in models/eval/: python scripts/distill_eval_models.py
    3. Each benchmark repo indexed: prx index <repo-path>

Usage:
    python scripts/run_model_eval.py [--prx ./target/release/prx]
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

BENCHMARKS_DIR = Path("benchmarks/repos")
MODELS_DIR = Path("models/eval")

# "builtin" means no --model-path flag (uses the embedded model)
BUILTIN_KEY = "builtin"


def discover_models(models_dir: Path) -> dict[str, Path | None]:
    """Find all eval models plus the builtin baseline."""
    models = {BUILTIN_KEY: None}
    if models_dir.exists():
        for entry in sorted(models_dir.iterdir()):
            if entry.is_dir() and (entry / "model.safetensors").exists():
                models[entry.name] = entry
    return models


def discover_datasets(benchmarks_dir: Path) -> dict[str, Path]:
    """Find all benchmark dataset JSON files."""
    datasets = {}
    if benchmarks_dir.exists():
        for f in sorted(benchmarks_dir.glob("*.json")):
            datasets[f.stem] = f
    return datasets


def run_bench(prx: str, dataset: Path, model_path: Path | None) -> dict | None:
    """Run prx bench-ndcg for one dataset + model combination."""
    cmd = [prx, "bench-ndcg", str(dataset)]
    if model_path is not None:
        cmd.extend(["--model-path", str(model_path)])

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        if result.returncode != 0:
            print(f"    FAILED (exit {result.returncode}): {result.stderr[:200]}")
            return None
        return json.loads(result.stdout)
    except subprocess.TimeoutExpired:
        print("    TIMEOUT (600s)")
        return None
    except json.JSONDecodeError:
        print(f"    BAD JSON: {result.stdout[:200]}")
        return None


def main():
    parser = argparse.ArgumentParser(description="Run model evaluation benchmark")
    parser.add_argument(
        "--prx",
        default="./target/release/prx",
        help="Path to prx binary (default: ./target/release/prx)",
    )
    parser.add_argument(
        "--models-dir",
        default=str(MODELS_DIR),
        help="Directory containing eval models (default: models/eval)",
    )
    parser.add_argument(
        "--only-model",
        default=None,
        help="Run only this model key (e.g. potion-code-16M)",
    )
    parser.add_argument(
        "--only-repo",
        default=None,
        help="Run only this repo (e.g. ripgrep)",
    )
    args = parser.parse_args()

    if not os.path.exists(args.prx):
        print(f"Error: prx binary not found at {args.prx}")
        print("Run: cargo build --release")
        sys.exit(1)

    models = discover_models(Path(args.models_dir))
    datasets = discover_datasets(BENCHMARKS_DIR)

    if not datasets:
        print(f"Error: no benchmark datasets found in {BENCHMARKS_DIR}/")
        sys.exit(1)

    if args.only_model:
        if args.only_model not in models:
            print(f"Error: model '{args.only_model}' not found. Available: {list(models.keys())}")
            sys.exit(1)
        models = {args.only_model: models[args.only_model]}

    if args.only_repo:
        if args.only_repo not in datasets:
            print(f"Error: repo '{args.only_repo}' not found. Available: {list(datasets.keys())}")
            sys.exit(1)
        datasets = {args.only_repo: datasets[args.only_repo]}

    print(f"Models: {list(models.keys())}")
    print(f"Repos:  {list(datasets.keys())}")
    print(f"Total runs: {len(models) * len(datasets)}")
    print()

    # results[model_key][repo_name] = {"ndcg5": ..., "ndcg10": ..., "by_category": ...}
    results: dict[str, dict[str, dict]] = {}
    total = len(models) * len(datasets)
    done = 0

    for model_key, model_path in models.items():
        results[model_key] = {}
        for repo_name, dataset_path in datasets.items():
            done += 1
            label = f"[{done}/{total}] {model_key} x {repo_name}"
            print(f"  {label}...", end=" ", flush=True)

            start = time.time()
            data = run_bench(args.prx, dataset_path, model_path)
            elapsed = time.time() - start

            if data and "data" in data:
                inner = data["data"]
                ndcg10 = inner.get("ndcg10", 0.0)
                ndcg5 = inner.get("ndcg5", 0.0)
                results[model_key][repo_name] = {
                    "ndcg5": ndcg5,
                    "ndcg10": ndcg10,
                    "by_category": inner.get("by_category", {}),
                    "misses": len(inner.get("misses", [])),
                    "queries": inner.get("queries_evaluated", 0),
                }
                print(f"NDCG@10={ndcg10:.4f}  ({elapsed:.1f}s)")
            else:
                results[model_key][repo_name] = None
                print(f"FAILED ({elapsed:.1f}s)")

    print_comparison_table(results, list(datasets.keys()))
    save_results(results)


def print_comparison_table(results: dict, repos: list[str]):
    """Print a formatted comparison table."""
    print("\n" + "=" * 80)
    print("MODEL COMPARISON — NDCG@10")
    print("=" * 80)

    header = f"{'Model':<22}"
    for repo in repos:
        header += f" {repo:>9}"
    header += f" {'MEAN':>9}"
    print(header)
    print("-" * len(header))

    model_means = {}
    for model_key, repo_results in results.items():
        row = f"{model_key:<22}"
        scores = []
        for repo in repos:
            r = repo_results.get(repo)
            if r is not None:
                score = r["ndcg10"]
                scores.append(score)
                row += f" {score:>9.4f}"
            else:
                row += f" {'FAIL':>9}"
        mean = sum(scores) / len(scores) if scores else 0.0
        model_means[model_key] = mean
        row += f" {mean:>9.4f}"
        print(row)

    print("-" * len(header))

    # Highlight best model
    if model_means:
        best = max(model_means, key=model_means.get)
        baseline = model_means.get(BUILTIN_KEY, 0.0)
        best_score = model_means[best]
        if baseline > 0:
            lift = ((best_score - baseline) / baseline) * 100
            print(f"\nBest: {best} (mean NDCG@10 = {best_score:.4f}, {lift:+.1f}% vs builtin)")
        else:
            print(f"\nBest: {best} (mean NDCG@10 = {best_score:.4f})")

    # Per-category breakdown for best model
    if model_means:
        best = max(model_means, key=model_means.get)
        print(f"\nPer-category breakdown for {best}:")
        cat_scores: dict[str, list[float]] = {}
        for repo_result in results[best].values():
            if repo_result and "by_category" in repo_result:
                for cat, stats in repo_result["by_category"].items():
                    cat_scores.setdefault(cat, []).append(stats.get("ndcg10", 0.0))

        if cat_scores:
            for cat in sorted(cat_scores.keys()):
                scores = cat_scores[cat]
                mean = sum(scores) / len(scores) if scores else 0.0
                print(f"  {cat:<20} {mean:.4f} (n={len(scores)})")


def save_results(results: dict):
    """Save raw results to JSON for further analysis."""
    output_path = "benchmarks/model_eval_results.json"
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nRaw results saved to {output_path}")


if __name__ == "__main__":
    main()

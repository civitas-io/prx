#!/usr/bin/env python3
"""Distill candidate code embedding models into Model2Vec format for evaluation.

Produces static Model2Vec models that prx can load via --model-path.
potion-code-16M is already Model2Vec and is downloaded directly.
Full transformer models (CodeRankEmbed, Jina, CodeXEmbed) are distilled.

Usage:
    pip install model2vec sentence-transformers
    python scripts/distill_eval_models.py [--only MODEL_KEY] [--dims 256]

Output:
    models/eval/<model-name>/ directories, each with model.safetensors + tokenizer.json
"""

import argparse
import os
import sys
import time

os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "1"

# ---------------------------------------------------------------------------
# Candidate models for the prx embedding shootout.
#
# "type" determines how we obtain the Model2Vec static model:
#   - "m2v_download": already a Model2Vec model on HuggingFace, download directly
#   - "distill": full transformer, distill via model2vec.distill()
#
# "dim" is the *source* embedding dimension. PCA reduction is controlled by
# the --dims flag (default 256 for standard tier, 512 for large tier eval).
# ---------------------------------------------------------------------------
CANDIDATES = {
    "potion-code-16M": {
        "hf_id": "minishlab/potion-code-16M",
        "type": "m2v_download",
        "source_dim": 256,
    },
    "coderankedembed": {
        "hf_id": "nomic-ai/CodeRankEmbed",
        "type": "distill",
        "source_dim": 768,
    },
    "jina-code-v2": {
        "hf_id": "jinaai/jina-embeddings-v2-base-code",
        "type": "distill",
        "source_dim": 768,
    },
    "codexembed-400m": {
        "hf_id": "Salesforce/SFR-Embedding-Code-400M_R",
        "type": "distill",
        "source_dim": 768,
    },
    "codexembed-2b": {
        "hf_id": "Salesforce/SFR-Embedding-Code-2B_R",
        "type": "distill",
        "source_dim": 2048,
    },
}


def download_m2v_model(hf_id: str, output_dir: str) -> None:
    """Download an existing Model2Vec model from HuggingFace."""
    from model2vec import StaticModel

    print(f"  Downloading Model2Vec model from {hf_id}...")
    model = StaticModel.from_pretrained(hf_id)
    model.save_pretrained(output_dir)
    print(f"  Saved to {output_dir}/")


def distill_model(hf_id: str, output_dir: str, pca_dims: int) -> None:
    """Distill a full transformer into Model2Vec static format."""
    from model2vec.distill import distill, distill_from_model

    print(f"  Distilling {hf_id} -> {pca_dims}d Model2Vec...")
    try:
        model = distill(hf_id, pca_dims=pca_dims, trust_remote_code=True)
    except Exception:
        # Custom-architecture models need explicit model+tokenizer loading
        from transformers import AutoModel, AutoTokenizer
        print(f"  Standard distill failed, loading via AutoModel...")
        tokenizer = AutoTokenizer.from_pretrained(hf_id, trust_remote_code=True)
        base_model = AutoModel.from_pretrained(hf_id, trust_remote_code=True)
        model = distill_from_model(base_model, tokenizer, pca_dims=pca_dims)
    model.save_pretrained(output_dir)
    print(f"  Saved to {output_dir}/")


def main():
    parser = argparse.ArgumentParser(description="Distill candidate models for prx eval")
    parser.add_argument(
        "--only",
        type=str,
        default=None,
        help=f"Process only this model key. Choices: {', '.join(CANDIDATES.keys())}",
    )
    parser.add_argument(
        "--dims",
        type=int,
        default=256,
        help="PCA target dimensionality for distilled models (default: 256)",
    )
    parser.add_argument(
        "--output-root",
        type=str,
        default="models/eval",
        help="Root directory for output models (default: models/eval)",
    )
    args = parser.parse_args()

    if args.only and args.only not in CANDIDATES:
        print(f"Error: unknown model key '{args.only}'")
        print(f"Available: {', '.join(CANDIDATES.keys())}")
        sys.exit(1)

    models_to_process = (
        {args.only: CANDIDATES[args.only]} if args.only else CANDIDATES
    )

    os.makedirs(args.output_root, exist_ok=True)

    results = {}
    for key, spec in models_to_process.items():
        output_dir = os.path.join(args.output_root, key)
        print(f"\n[{key}] {spec['hf_id']}")

        if os.path.exists(os.path.join(output_dir, "model.safetensors")):
            print(f"  Already exists at {output_dir}/, skipping. Delete to re-run.")
            results[key] = "skipped"
            continue

        os.makedirs(output_dir, exist_ok=True)
        start = time.time()

        try:
            if spec["type"] == "m2v_download":
                download_m2v_model(spec["hf_id"], output_dir)
            else:
                distill_model(spec["hf_id"], output_dir, args.dims)

            elapsed = time.time() - start
            size_mb = os.path.getsize(os.path.join(output_dir, "model.safetensors")) / 1e6
            print(f"  Done in {elapsed:.1f}s, safetensors size: {size_mb:.1f} MB")
            results[key] = f"ok ({size_mb:.1f} MB, {elapsed:.1f}s)"
        except Exception as e:
            print(f"  FAILED: {e}")
            results[key] = f"FAILED: {e}"

    print("\n" + "=" * 60)
    print("Summary:")
    for key, status in results.items():
        print(f"  {key:25s} {status}")
    print(f"\nModels saved to: {args.output_root}/")
    print(f"Run eval with: python scripts/run_model_eval.py")


if __name__ == "__main__":
    main()

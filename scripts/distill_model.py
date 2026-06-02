#!/usr/bin/env python3
"""Distill a code-specific embedding model into Model2Vec format for prx.

Usage:
    pip install model2vec
    python scripts/distill_model.py

Output:
    models/codesage-m2v-256/ directory with:
    - model.safetensors (the weights, ~8 MB)
    - tokenizer.json (the tokenizer)

These can be loaded by prx's DenseIndex via include_bytes! or file path.
"""

from model2vec import distill

MODELS = {
    "standard": {
        "source": "codesage/codesage-base-v2",
        "pca_dims": 256,
        "output": "models/codesage-m2v-256",
    },
    # Uncomment to also distill the large tier:
    # "large": {
    #     "source": "jinaai/jina-embeddings-v3",
    #     "pca_dims": 512,
    #     "output": "models/jina-code-m2v-512",
    # },
}

if __name__ == "__main__":
    for tier, config in MODELS.items():
        print(f"Distilling {tier} tier: {config['source']} -> {config['output']}")
        print(f"  PCA dims: {config['pca_dims']}")

        model = distill(
            config["source"],
            pca_dims=config["pca_dims"],
        )

        model.save_pretrained(config["output"])
        print(f"  Saved to {config['output']}/")
        print()

    print("Done. Next steps:")
    print("  1. Check model.safetensors size (should be ~8 MB for standard)")
    print("  2. Run: prx bench-ndcg --model-path models/codesage-m2v-256/")
    print("  3. Compare NDCG@10 against builtin baseline")

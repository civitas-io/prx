//! Downloadable embedding model tier registry.
//!
//! prx ships with a small built-in model embedded in the binary. Larger,
//! code-specific tiers are downloaded on demand from GitHub Releases and
//! cached under `~/.prx/models/<dir_name>/`. Each tier pins a SHA-256
//! of its archive plus the unpacked `model.safetensors` and `tokenizer.json`.
//!
//! Downloads use system `curl` + `tar` to avoid pulling additional HTTP /
//! decompression crates into the runtime binary.

use std::path::{Path, PathBuf};

/// A downloadable embedding model tier.
pub struct ModelTier {
    pub name: &'static str,
    pub display_name: &'static str,
    pub url: &'static str,
    pub archive_sha256: &'static str,
    pub safetensors_sha256: &'static str,
    pub tokenizer_sha256: &'static str,
    /// Embedding dimensionality; informational metadata for callers.
    #[allow(dead_code)]
    pub dim: usize,
    pub dir_name: &'static str,
}

pub static STANDARD: ModelTier = ModelTier {
    name: "standard",
    display_name: "CodeSage-M2V-256",
    url: "https://github.com/civitas-io/prx/releases/download/models-v1/codesage-m2v-256.tar.gz",
    archive_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    safetensors_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    tokenizer_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    dim: 256,
    dir_name: "codesage-m2v-256",
};

pub static LARGE: ModelTier = ModelTier {
    name: "large",
    display_name: "Jina-Code-M2V-512",
    url: "https://github.com/civitas-io/prx/releases/download/models-v1/jina-code-m2v-512.tar.gz",
    archive_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    safetensors_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    tokenizer_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
    dim: 512,
    dir_name: "jina-code-m2v-512",
};

/// Look up a model tier by name. Accepts `"standard"` or `"large"`.
pub fn get_tier(name: &str) -> Option<&'static ModelTier> {
    match name {
        "standard" => Some(&STANDARD),
        "large" => Some(&LARGE),
        _ => None,
    }
}

/// Return the local directory where a model should be stored.
///
/// Returns `$PRX_MODELS_DIR/<dir_name>/` if set (test/CI override), else
/// `~/.prx/models/<dir_name>/`.
pub fn model_dir(tier: &ModelTier) -> PathBuf {
    let base = if let Ok(dir) = std::env::var("PRX_MODELS_DIR") {
        PathBuf::from(dir)
    } else {
        dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".prx")
            .join("models")
    };
    base.join(tier.dir_name)
}

/// Check if a model is already downloaded and valid.
///
/// Returns `true` if both `model.safetensors` and `tokenizer.json` exist and,
/// when their pinned SHA-256 is not the all-zero placeholder, the on-disk
/// contents match.
pub fn is_model_ready(tier: &ModelTier) -> bool {
    let dir = model_dir(tier);
    let safetensors = dir.join("model.safetensors");
    let tokenizer = dir.join("tokenizer.json");

    if !safetensors.exists() || !tokenizer.exists() {
        return false;
    }

    if !is_placeholder_hash(tier.safetensors_sha256)
        && !verify_sha256(&safetensors, tier.safetensors_sha256)
    {
        return false;
    }

    if !is_placeholder_hash(tier.tokenizer_sha256)
        && !verify_sha256(&tokenizer, tier.tokenizer_sha256)
    {
        return false;
    }

    true
}

/// Download and extract a model from GitHub Releases.
///
/// Uses system `curl` for the download and `tar` for extraction so we do not
/// pull a HTTP client and gzip / tar parser into the runtime binary. Verifies
/// the archive's SHA-256 against `tier.archive_sha256` unless the pinned hash
/// is the all-zero placeholder.
pub fn download_model(tier: &ModelTier) -> Result<PathBuf, String> {
    let dir = model_dir(tier);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create model dir: {e}"))?;

    let archive_path = dir.join("model.tar.gz");

    eprintln!("Downloading {} model ({})...", tier.display_name, tier.name);
    eprintln!("  From: {}", tier.url);

    let status = std::process::Command::new("curl")
        .args(["-fSL", "--progress-bar", "-o"])
        .arg(&archive_path)
        .arg(tier.url)
        .status()
        .map_err(|e| format!("curl not found: {e}"))?;

    if !status.success() {
        let _ = std::fs::remove_file(&archive_path);
        return Err("Download failed. Check your network connection.".to_string());
    }

    if !is_placeholder_hash(tier.archive_sha256) {
        let data =
            std::fs::read(&archive_path).map_err(|e| format!("Failed to read archive: {e}"))?;
        let hash = sha256_hex(&data);
        if hash != tier.archive_sha256 {
            let _ = std::fs::remove_file(&archive_path);
            return Err(format!(
                "Archive hash mismatch: expected {}, got {hash}",
                tier.archive_sha256
            ));
        }
    }

    let status = std::process::Command::new("tar")
        .args(["xzf"])
        .arg(&archive_path)
        .arg("-C")
        .arg(&dir)
        .status()
        .map_err(|e| format!("tar not found: {e}"))?;

    if !status.success() {
        return Err("Extraction failed.".to_string());
    }

    let _ = std::fs::remove_file(&archive_path);
    eprintln!("  Saved to {}", dir.display());
    Ok(dir)
}

/// Ensure a model is available locally, downloading if missing or stale.
pub fn ensure_model(tier: &ModelTier) -> Result<PathBuf, String> {
    if is_model_ready(tier) {
        return Ok(model_dir(tier));
    }
    download_model(tier)
}

fn is_placeholder_hash(hex: &str) -> bool {
    hex.chars().all(|c| c == '0')
}

fn verify_sha256(path: &Path, expected: &str) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    sha256_hex(&data) == expected
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_tier_known_names() {
        assert_eq!(get_tier("standard").map(|t| t.name), Some("standard"));
        assert_eq!(get_tier("large").map(|t| t.name), Some("large"));
    }

    #[test]
    fn get_tier_unknown_returns_none() {
        assert!(get_tier("nope").is_none());
        assert!(get_tier("builtin").is_none());
        assert!(get_tier("").is_none());
    }

    #[test]
    fn placeholder_detection() {
        assert!(is_placeholder_hash(
            "0000000000000000000000000000000000000000000000000000000000000000"
        ));
        assert!(!is_placeholder_hash(
            "ca6159081a6e96cebe4ad878e5e8437bfccc761e8db16223370149cd2faa6c0b"
        ));
    }

    #[test]
    fn sha256_hex_matches_known_value() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn model_dir_respects_env_override() {
        let tier = &STANDARD;
        // SAFETY: tests run sequentially within this module's process; the var is
        // read only by model_dir() invoked synchronously below.
        unsafe {
            std::env::set_var("PRX_MODELS_DIR", "/tmp/prx-test-models");
        }
        let dir = model_dir(tier);
        unsafe {
            std::env::remove_var("PRX_MODELS_DIR");
        }
        assert_eq!(
            dir,
            PathBuf::from("/tmp/prx-test-models").join(tier.dir_name)
        );
    }

    #[test]
    fn is_model_ready_false_for_missing_dir() {
        let tier = &STANDARD;
        unsafe {
            std::env::set_var(
                "PRX_MODELS_DIR",
                "/tmp/prx-test-models-definitely-does-not-exist-xyz123",
            );
        }
        let ready = is_model_ready(tier);
        unsafe {
            std::env::remove_var("PRX_MODELS_DIR");
        }
        assert!(!ready);
    }
}

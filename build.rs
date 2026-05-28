//! Build script: ensure `models/` contains the three embedded artifacts.
//!
//! Run order on a clean checkout:
//!   1. Skip if `models/potion-retrieval-32M.safetensors` already exists and its
//!      embeddings tensor is F16 (idempotent cache hit).
//!   2. Read each artifact from `$PRX_MODELS_DIR` if set, else download from
//!      Hugging Face with a synchronous `ureq` call.
//!   3. SHA-256 verify each downloaded F32 / tokenizer payload against the
//!      pinned hash. Mismatch is a hard build error.
//!   4. Rewrite the safetensors file with the `embeddings` tensor converted
//!      from F32 to F16 (other tensors preserved verbatim).
//!
//! Files land in `models/` (alongside `Cargo.toml`) because `src/` uses
//! `include_bytes!("../models/…")` — paths relative to the source file, which
//! cannot reach `OUT_DIR`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

const MODEL_FILE: &str = "potion-retrieval-32M.safetensors";
const MODEL_URL: &str =
    "https://huggingface.co/minishlab/potion-code-16M/resolve/main/model.safetensors";
const MODEL_SHA256: &str = "ca6159081a6e96cebe4ad878e5e8437bfccc761e8db16223370149cd2faa6c0b";

const M2V_TOK_FILE: &str = "model2vec_tokenizer.json";
const M2V_TOK_URL: &str =
    "https://huggingface.co/minishlab/potion-code-16M/resolve/main/tokenizer.json";
const M2V_TOK_SHA256: &str = "8e84217af15e70e8127c855435fc3d8a4cd91d7bbe686f72e75f188118ec78ae";

const CL100K_FILE: &str = "cl100k_base.json";
const CL100K_URL: &str = "https://huggingface.co/Xenova/gpt-4/resolve/main/tokenizer.json";
const CL100K_SHA256: &str = "239eb2359f79c38497476671aaa835e01fb43d42743c612a8514a0dfa2ac93a2";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PRX_MODELS_DIR");
    println!("cargo:rerun-if-changed=models/{MODEL_FILE}");
    println!("cargo:rerun-if-changed=models/{M2V_TOK_FILE}");
    println!("cargo:rerun-if-changed=models/{CL100K_FILE}");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let models_dir = PathBuf::from(manifest_dir).join("models");

    if let Err(e) = ensure_models(&models_dir) {
        eprintln!("\nbuild.rs: failed to provision model files\n  {e}\n");
        eprintln!("Set PRX_MODELS_DIR=<path> to use pre-downloaded files for offline builds.");
        std::process::exit(1);
    }
}

fn ensure_models(models_dir: &Path) -> Result<(), String> {
    let model_path = models_dir.join(MODEL_FILE);
    let m2v_path = models_dir.join(M2V_TOK_FILE);
    let cl_path = models_dir.join(CL100K_FILE);

    if model_path.exists()
        && m2v_path.exists()
        && cl_path.exists()
        && is_safetensors_f16(&model_path).unwrap_or(false)
    {
        return Ok(());
    }

    fs::create_dir_all(models_dir)
        .map_err(|e| format!("create_dir_all {}: {e}", models_dir.display()))?;

    let source_dir = env::var("PRX_MODELS_DIR").ok().map(PathBuf::from);

    fetch_verified(
        &m2v_path,
        M2V_TOK_URL,
        M2V_TOK_SHA256,
        source_dir.as_deref(),
    )?;
    fetch_verified(&cl_path, CL100K_URL, CL100K_SHA256, source_dir.as_deref())?;

    ensure_model_f16(&model_path, source_dir.as_deref())?;

    Ok(())
}

fn fetch_verified(
    dst: &Path,
    url: &str,
    sha256_hex: &str,
    source_dir: Option<&Path>,
) -> Result<(), String> {
    if dst.exists() {
        let existing = fs::read(dst).map_err(|e| format!("read {}: {e}", dst.display()))?;
        if sha256_hex_of(&existing) == sha256_hex {
            return Ok(());
        }
    }

    let bytes = load_bytes(dst, url, source_dir)?;
    let got = sha256_hex_of(&bytes);
    if got != sha256_hex {
        return Err(format!(
            "SHA-256 mismatch for {}\n  expected: {sha256_hex}\n  got:      {got}\n  source:   {}",
            dst.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
            source_dir
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| url.to_string()),
        ));
    }

    fs::write(dst, &bytes).map_err(|e| format!("write {}: {e}", dst.display()))?;
    Ok(())
}

fn ensure_model_f16(dst: &Path, source_dir: Option<&Path>) -> Result<(), String> {
    if dst.exists() && is_safetensors_f16(dst).unwrap_or(false) {
        return Ok(());
    }

    let bytes = load_bytes(dst, MODEL_URL, source_dir)?;

    let bytes = if is_safetensors_f16_bytes(&bytes).unwrap_or(false) {
        bytes
    } else {
        let got = sha256_hex_of(&bytes);
        if got != MODEL_SHA256 {
            return Err(format!(
                "SHA-256 mismatch for {MODEL_FILE}\n  expected: {MODEL_SHA256}\n  got:      {got}",
            ));
        }
        convert_embeddings_to_f16(&bytes)?
    };

    fs::write(dst, &bytes).map_err(|e| format!("write {}: {e}", dst.display()))?;
    Ok(())
}

fn load_bytes(dst: &Path, url: &str, source_dir: Option<&Path>) -> Result<Vec<u8>, String> {
    let filename = dst
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("invalid filename: {}", dst.display()))?;

    if let Some(src_dir) = source_dir {
        let src = src_dir.join(filename);
        return fs::read(&src).map_err(|e| {
            format!(
                "read PRX_MODELS_DIR file {}: {e}\n(unset PRX_MODELS_DIR to download from network)",
                src.display()
            )
        });
    }

    download(url)
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();

    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {url}\n  {e}"))?;

    resp.body_mut()
        .with_config()
        .limit(128 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("read body from {url}: {e}"))
}

fn sha256_hex_of(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn is_safetensors_f16(path: &Path) -> Result<bool, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    is_safetensors_f16_bytes(&bytes)
}

fn is_safetensors_f16_bytes(bytes: &[u8]) -> Result<bool, String> {
    let header = parse_header(bytes)?;
    let tensors = header
        .as_object()
        .ok_or_else(|| "safetensors header is not a JSON object".to_string())?;
    let emb = tensors
        .get("embeddings")
        .ok_or_else(|| "no `embeddings` tensor in safetensors header".to_string())?;
    let dtype = emb
        .get("dtype")
        .and_then(|d| d.as_str())
        .ok_or_else(|| "`embeddings` tensor has no dtype".to_string())?;
    Ok(dtype == "F16")
}

fn parse_header(bytes: &[u8]) -> Result<serde_json::Value, String> {
    if bytes.len() < 8 {
        return Err("safetensors file shorter than 8-byte header length prefix".into());
    }
    let mut hl = [0u8; 8];
    hl.copy_from_slice(&bytes[..8]);
    let header_len = u64::from_le_bytes(hl) as usize;
    let end = 8usize
        .checked_add(header_len)
        .ok_or("header length overflow")?;
    if bytes.len() < end {
        return Err(format!(
            "safetensors header truncated: need {end} bytes, file has {}",
            bytes.len()
        ));
    }
    serde_json::from_slice(&bytes[8..end]).map_err(|e| format!("parse safetensors header: {e}"))
}

/// Rewrite a safetensors blob, converting the `embeddings` tensor from F32 to
/// F16 and preserving every other tensor verbatim. Tensor order in the output
/// matches the order in the input header so byte offsets stay sequential.
fn convert_embeddings_to_f16(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let header = parse_header(bytes)?;
    let mut hl = [0u8; 8];
    hl.copy_from_slice(&bytes[..8]);
    let header_len = u64::from_le_bytes(hl) as usize;
    let data_start = 8 + header_len;

    let obj = header
        .as_object()
        .ok_or("safetensors header is not a JSON object")?;

    let mut tensor_names: Vec<&str> = obj
        .iter()
        .filter(|(k, _)| !k.starts_with("__"))
        .map(|(k, _)| k.as_str())
        .collect();
    tensor_names.sort_by_key(|name| {
        obj.get(*name)
            .and_then(|t| t.get("data_offsets"))
            .and_then(|o| o.get(0))
            .and_then(|n| n.as_u64())
            .unwrap_or(u64::MAX)
    });

    let mut new_header = serde_json::Map::new();
    let mut new_data: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut cursor: u64 = 0;

    for name in &tensor_names {
        let tensor = obj
            .get(*name)
            .ok_or_else(|| format!("missing tensor {name}"))?;
        let dtype = tensor
            .get("dtype")
            .and_then(|d| d.as_str())
            .ok_or_else(|| format!("tensor {name} has no dtype"))?;
        let shape_val = tensor
            .get("shape")
            .ok_or_else(|| format!("tensor {name} has no shape"))?
            .clone();
        let offsets = tensor
            .get("data_offsets")
            .and_then(|o| o.as_array())
            .ok_or_else(|| format!("tensor {name} has no data_offsets"))?;
        if offsets.len() != 2 {
            return Err(format!("tensor {name}: data_offsets must have 2 elements"));
        }
        let off_start = offsets[0].as_u64().ok_or("data_offsets[0] not u64")? as usize;
        let off_end = offsets[1].as_u64().ok_or("data_offsets[1] not u64")? as usize;
        let abs_start = data_start.checked_add(off_start).ok_or("offset overflow")?;
        let abs_end = data_start.checked_add(off_end).ok_or("offset overflow")?;
        if abs_end > bytes.len() {
            return Err(format!(
                "tensor {name}: data range [{abs_start}, {abs_end}) exceeds file size {}",
                bytes.len()
            ));
        }
        let raw = &bytes[abs_start..abs_end];

        let (out_bytes, out_dtype): (Vec<u8>, &str) = if *name == "embeddings" && dtype == "F32" {
            if raw.len() % 4 != 0 {
                return Err(format!(
                    "embeddings F32 data length {} is not a multiple of 4",
                    raw.len()
                ));
            }
            let mut out = Vec::with_capacity(raw.len() / 2);
            for chunk in raw.chunks_exact(4) {
                let f = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let h = half::f16::from_f32(f);
                out.extend_from_slice(&h.to_le_bytes());
            }
            (out, "F16")
        } else {
            (raw.to_vec(), dtype)
        };

        let len = out_bytes.len() as u64;
        let mut entry = serde_json::Map::new();
        entry.insert(
            "dtype".to_string(),
            serde_json::Value::String(out_dtype.into()),
        );
        entry.insert("shape".to_string(), shape_val);
        entry.insert(
            "data_offsets".to_string(),
            serde_json::Value::Array(vec![cursor.into(), (cursor + len).into()]),
        );
        new_header.insert((*name).to_string(), serde_json::Value::Object(entry));
        new_data.extend_from_slice(&out_bytes);
        cursor += len;
    }

    if let Some(meta) = obj.get("__metadata__") {
        new_header.insert("__metadata__".to_string(), meta.clone());
    }

    let mut header_bytes =
        serde_json::to_vec(&serde_json::Value::Object(new_header)).map_err(|e| e.to_string())?;
    while header_bytes.len() % 8 != 0 {
        header_bytes.push(b' ');
    }

    let mut out = Vec::with_capacity(8 + header_bytes.len() + new_data.len());
    out.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&new_data);
    Ok(out)
}

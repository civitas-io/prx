use std::sync::OnceLock;

static TOKENIZER: OnceLock<Option<tokenizers::Tokenizer>> = OnceLock::new();

static TOKENIZER_BYTES: &[u8] = include_bytes!("../models/cl100k_base.json");

pub fn count_fast(text: &str) -> usize {
    text.len() / 4
}

pub fn count_exact(text: &str) -> usize {
    let tok = TOKENIZER.get_or_init(load_tokenizer);
    match tok {
        Some(t) => match t.encode(text, false) {
            Ok(encoding) => encoding.get_ids().len(),
            Err(_) => count_fast(text),
        },
        None => count_fast(text),
    }
}

pub fn count(text: &str, use_exact: bool) -> usize {
    if use_exact {
        count_exact(text)
    } else {
        count_fast(text)
    }
}

fn load_tokenizer() -> Option<tokenizers::Tokenizer> {
    if TOKENIZER_BYTES.is_empty() {
        return None;
    }
    tokenizers::Tokenizer::from_bytes(TOKENIZER_BYTES).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_approximation() {
        assert_eq!(count_fast(""), 0);
        assert_eq!(count_fast("abcd"), 1);
        assert_eq!(count_fast("hello world!"), 3);
    }

    #[test]
    fn fast_on_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let fast = count_fast(code);
        assert!(fast > 0);
        assert!(fast < code.len());
    }

    #[test]
    fn exact_falls_back_when_no_tokenizer() {
        // With an empty placeholder file, exact falls back to fast
        let result = count_exact("hello world");
        assert!(result > 0);
    }

    #[test]
    fn count_dispatches_correctly() {
        let text = "hello world test";
        let fast = count(text, false);
        let exact = count(text, true);
        assert!(fast > 0);
        assert!(exact > 0);
    }

    #[test]
    fn fast_and_exact_within_range() {
        // When tokenizer is loaded, exact and fast should be within 2x of each other
        // When tokenizer is not loaded (placeholder), exact == fast
        let text = "fn process_request(req: &Request) -> Response { let data = req.body(); }";
        let fast = count_fast(text);
        let exact = count_exact(text);
        let ratio = (fast as f64) / (exact as f64);
        assert!(
            ratio > 0.3 && ratio < 3.0,
            "ratio {ratio} out of expected range"
        );
    }
}

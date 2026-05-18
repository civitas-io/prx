use ndarray::{Array1, Array2};
use std::collections::HashMap;
use std::sync::OnceLock;

static MODEL2VEC_TOKENIZER: OnceLock<Option<tokenizers::Tokenizer>> = OnceLock::new();
static MODEL2VEC_TOKENIZER_BYTES: &[u8] = include_bytes!("../../models/model2vec_tokenizer.json");

pub struct DenseIndex {
    vocab: HashMap<String, usize>,
    weights: Array2<f32>,
    chunk_embeddings: Array2<f32>,
    use_hf_tokenizer: bool,
}

impl DenseIndex {
    pub fn new(vocab: HashMap<String, usize>, weights: Array2<f32>) -> Self {
        let dim = weights.ncols();
        Self {
            vocab,
            weights,
            chunk_embeddings: Array2::zeros((0, dim)),
            use_hf_tokenizer: true,
        }
    }

    pub fn without_hf_tokenizer(vocab: HashMap<String, usize>, weights: Array2<f32>) -> Self {
        let dim = weights.ncols();
        Self {
            vocab,
            weights,
            chunk_embeddings: Array2::zeros((0, dim)),
            use_hf_tokenizer: false,
        }
    }

    pub fn dim(&self) -> usize {
        self.weights.ncols()
    }

    pub fn vocab(&self) -> &HashMap<String, usize> {
        &self.vocab
    }

    pub fn weights(&self) -> &Array2<f32> {
        &self.weights
    }

    pub fn embed_text(&self, text: &str) -> Array1<f32> {
        let dim = self.dim();
        let token_ids = tokenize_for_embedding(text, &self.vocab, self.use_hf_tokenizer);
        let mut sum = Array1::zeros(dim);
        let mut count = 0usize;

        for idx in &token_ids {
            if *idx < self.weights.nrows() {
                sum += &self.weights.row(*idx);
                count += 1;
            }
        }

        if count > 0 {
            sum /= count as f32;
        }
        l2_normalize(&mut sum);
        sum
    }

    pub fn index_chunks(&mut self, texts: &[&str]) {
        let dim = self.dim();
        let mut embeddings = Array2::zeros((texts.len(), dim));
        for (i, text) in texts.iter().enumerate() {
            let emb = self.embed_text(text);
            embeddings.row_mut(i).assign(&emb);
        }
        self.chunk_embeddings = embeddings;
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        if self.chunk_embeddings.nrows() == 0 {
            return vec![];
        }

        let query_vec = self.embed_text(query);
        let mut scores: Vec<(usize, f32)> = self
            .chunk_embeddings
            .rows()
            .into_iter()
            .enumerate()
            .map(|(i, row)| (i, row.dot(&query_vec)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }
}

fn tokenize_for_embedding(text: &str, vocab: &HashMap<String, usize>, use_hf: bool) -> Vec<usize> {
    if use_hf {
        let tokenizer = MODEL2VEC_TOKENIZER.get_or_init(|| {
            if MODEL2VEC_TOKENIZER_BYTES.is_empty() {
                return None;
            }
            tokenizers::Tokenizer::from_bytes(MODEL2VEC_TOKENIZER_BYTES).ok()
        });

        if let Some(tok) = tokenizer {
            if let Ok(encoding) = tok.encode(text, false) {
                return encoding
                    .get_ids()
                    .iter()
                    .map(|&id| id as usize)
                    .filter(|id| *id < vocab.len())
                    .collect();
            }
        }
    }

    text.split_whitespace()
        .flat_map(|word| {
            word.split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|s| !s.is_empty())
                .filter_map(|s| vocab.get(&s.to_lowercase()).copied())
        })
        .collect()
}

fn l2_normalize(vec: &mut Array1<f32>) {
    let norm = vec.dot(vec).sqrt();
    if norm > 1e-10 {
        *vec /= norm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> DenseIndex {
        let mut vocab = HashMap::new();
        vocab.insert("hello".to_string(), 0);
        vocab.insert("world".to_string(), 1);
        vocab.insert("foo".to_string(), 2);
        vocab.insert("bar".to_string(), 3);
        vocab.insert("auth".to_string(), 4);
        vocab.insert("login".to_string(), 5);

        // 8-dim vectors to allow more distinct directions
        let weights = Array2::from_shape_vec(
            (6, 8),
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // hello
                0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // world
                0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, // foo
                0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, // bar
                0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, // auth
                0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, // login
            ],
        )
        .unwrap();

        DenseIndex::without_hf_tokenizer(vocab, weights)
    }

    #[test]
    fn embed_text_produces_unit_vector() {
        let idx = test_index();
        let vec = idx.embed_text("hello world");
        let norm = vec.dot(&vec).sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "norm = {norm}");
    }

    #[test]
    fn embed_unknown_tokens_gives_zero() {
        let idx = test_index();
        let vec = idx.embed_text("zzzzz qqqqq");
        let norm = vec.dot(&vec).sqrt();
        assert!(norm < 1e-6, "expected zero vector for unknown tokens");
    }

    #[test]
    fn similar_texts_have_high_cosine() {
        let idx = test_index();
        let a = idx.embed_text("auth login");
        let b = idx.embed_text("login auth");
        let sim = a.dot(&b);
        assert!(sim > 0.9, "expected high similarity, got {sim}");
    }

    #[test]
    fn different_texts_have_low_cosine() {
        let idx = test_index();
        let a = idx.embed_text("hello world");
        let b = idx.embed_text("foo bar");
        let sim = a.dot(&b);
        assert!(sim < 0.1, "expected low similarity, got {sim}");
    }

    #[test]
    fn search_returns_ranked_results() {
        let mut idx = test_index();
        idx.index_chunks(&["hello world", "foo bar", "auth login"]);

        let results = idx.search("auth login", 3);
        assert!(!results.is_empty());
        // The "auth login" chunk should have the highest similarity to "auth login" query
        let best = results[0].0;
        assert_eq!(
            best, 2,
            "auth login chunk should rank first, got chunk {best}"
        );
        assert!(results[0].1 > results[1].1, "scores should be descending");
    }

    #[test]
    fn search_empty_index() {
        let idx = test_index();
        let results = idx.search("hello", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn tokenize_returns_valid_ids() {
        let idx = test_index();
        let ids = tokenize_for_embedding("hello world", &idx.vocab, false);
        assert!(!ids.is_empty());
        for id in &ids {
            assert!(*id < idx.weights.nrows());
        }
    }

    #[test]
    fn dim_matches_weights() {
        let idx = test_index();
        assert_eq!(idx.dim(), 8);
    }
}

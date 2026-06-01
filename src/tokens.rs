pub fn count_fast(text: &str) -> usize {
    text.len() / 4
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
}

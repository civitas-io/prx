use std::path::Path;
use xxhash_rust::xxh3::xxh3_128;

pub fn hash_bytes(data: &[u8]) -> String {
    format!("{:032x}", xxh3_128(data))
}

#[allow(dead_code)]
pub fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let data = std::fs::read(path)?;
    Ok(hash_bytes(&data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn deterministic() {
        let a = hash_bytes(b"hello world");
        let b = hash_bytes(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn different_inputs_different_hashes() {
        let a = hash_bytes(b"hello");
        let b = hash_bytes(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn hex_length_is_32() {
        let h = hash_bytes(b"test");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn empty_input() {
        let h = hash_bytes(b"");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn hash_file_works() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"file content").unwrap();
        let h = hash_file(tmp.path()).unwrap();
        assert_eq!(h.len(), 32);
        assert_eq!(h, hash_bytes(b"file content"));
    }

    #[test]
    fn hash_file_not_found() {
        let result = hash_file(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }
}

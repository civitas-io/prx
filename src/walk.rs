use std::path::{Path, PathBuf};

const DEFAULT_MAX_FILE_SIZE: u64 = 1_048_576; // 1 MB
const BINARY_CHECK_BYTES: usize = 8192;

pub struct WalkEntry {
    pub path: PathBuf,
    pub size: u64,
    #[allow(dead_code)]
    pub language: Option<String>,
}

pub struct WalkOpts {
    pub max_file_size: u64,
}

impl Default for WalkOpts {
    fn default() -> Self {
        let max = std::env::var("PRX_MAX_FILE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_FILE_SIZE);
        Self { max_file_size: max }
    }
}

pub fn walk(root: &Path, opts: &WalkOpts) -> Vec<WalkEntry> {
    let mut entries = Vec::new();

    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .add_custom_ignore_filename(".prxignore")
        .build();

    for result in walker.flatten() {
        let ft = match result.file_type() {
            Some(ft) if ft.is_file() => ft,
            _ => continue,
        };
        let _ = ft;

        let path = result.path().to_path_buf();

        if path.components().any(|c| c.as_os_str() == ".prx") {
            continue;
        }

        let size = result.metadata().map(|m| m.len()).unwrap_or(0);
        if size > opts.max_file_size {
            continue;
        }

        if is_binary(&path) {
            continue;
        }

        let language = detect_language(&path);
        entries.push(WalkEntry {
            path,
            size,
            language,
        });
    }

    entries
}

fn is_binary(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    use std::io::Read;
    let mut buf = [0u8; BINARY_CHECK_BYTES];
    let mut reader = std::io::BufReader::new(file);
    let n = reader.read(&mut buf).unwrap_or(0);
    buf[..n].contains(&0)
}

fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    if let Some(name) = crate::parsing::languages::language_name_for_extension(ext) {
        return Some(name.to_string());
    }
    let lang = match ext {
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "lua" => "lua",
        "r" | "R" => "r",
        "scala" | "sc" => "scala",
        "zig" => "zig",
        _ => return None,
    };
    Some(lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // git init so .gitignore is respected by the ignore crate
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.py"), "def hello(): pass").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "plain text").unwrap();

        let mut bin = std::fs::File::create(dir.path().join("image.bin")).unwrap();
        bin.write_all(&[0u8; 100]).unwrap();

        std::fs::write(dir.path().join(".gitignore"), "ignored.txt\n").unwrap();
        std::fs::write(dir.path().join("ignored.txt"), "should be skipped").unwrap();

        dir
    }

    #[test]
    fn skips_binary_files() {
        let dir = create_test_dir();
        let entries = walk(dir.path(), &WalkOpts::default());
        let paths: Vec<_> = entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(!paths.contains(&"image.bin"));
    }

    #[test]
    fn respects_gitignore() {
        let dir = create_test_dir();
        let entries = walk(dir.path(), &WalkOpts::default());
        let paths: Vec<_> = entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(!paths.contains(&"ignored.txt"));
    }

    #[test]
    fn detects_languages() {
        let dir = create_test_dir();
        let entries = walk(dir.path(), &WalkOpts::default());
        let rs = entries
            .iter()
            .find(|e| e.path.file_name().unwrap() == "main.rs")
            .unwrap();
        assert_eq!(rs.language.as_deref(), Some("rust"));

        let py = entries
            .iter()
            .find(|e| e.path.file_name().unwrap() == "lib.py")
            .unwrap();
        assert_eq!(py.language.as_deref(), Some("python"));
    }

    #[test]
    fn unknown_extension_returns_none() {
        let dir = create_test_dir();
        let entries = walk(dir.path(), &WalkOpts::default());
        let txt = entries
            .iter()
            .find(|e| e.path.file_name().unwrap() == "notes.txt");
        if let Some(entry) = txt {
            assert_eq!(entry.language, None);
        }
    }

    #[test]
    fn skips_files_over_max_size() {
        let dir = TempDir::new().unwrap();
        let big_path = dir.path().join("big.rs");
        let data = vec![b'a'; 2_000_000]; // 2 MB
        std::fs::write(&big_path, &data).unwrap();

        let entries = walk(
            dir.path(),
            &WalkOpts {
                max_file_size: 1_000_000,
            },
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn prxignore_support() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".prxignore"), "vendor/\n").unwrap();
        std::fs::create_dir(dir.path().join("vendor")).unwrap();
        std::fs::write(dir.path().join("vendor").join("dep.rs"), "// vendored").unwrap();
        std::fs::write(dir.path().join("src.rs"), "// source").unwrap();

        let entries = walk(dir.path(), &WalkOpts::default());
        let paths: Vec<_> = entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(!paths.contains(&"dep.rs"));
        assert!(paths.contains(&"src.rs"));
    }
}

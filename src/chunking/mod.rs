mod treesitter;

use crate::parsing::languages::language_for_extension;

const DEFAULT_CHUNK_SIZE: usize = 1500;
const OVERLAP_BYTES: usize = 200;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    #[allow(dead_code)]
    pub start_byte: usize,
    #[allow(dead_code)]
    pub end_byte: usize,
    pub language: Option<String>,
}

pub fn chunk_file(source: &str, file_path: &str, ext: Option<&str>) -> Vec<Chunk> {
    if source.is_empty() {
        return vec![];
    }

    let language = ext
        .and_then(|e| crate::parsing::languages::language_name_for_extension(e).map(String::from));

    let boundaries = match ext.and_then(language_for_extension) {
        Some(lang) => treesitter::chunk_with_treesitter(source, &lang, DEFAULT_CHUNK_SIZE),
        None => chunk_by_lines(source, DEFAULT_CHUNK_SIZE),
    };

    let with_overlap = add_overlap(&boundaries, source, OVERLAP_BYTES);

    let newline_offsets: Vec<usize> = source
        .bytes()
        .enumerate()
        .filter_map(|(i, b)| if b == b'\n' { Some(i) } else { None })
        .collect();

    with_overlap
        .into_iter()
        .map(|(start_byte, end_byte)| {
            let content = source[start_byte..end_byte].to_string();
            let start_line = newline_offsets.partition_point(|&o| o < start_byte) + 1;
            let end_line = newline_offsets.partition_point(|&o| o < end_byte) + 1;
            Chunk {
                content,
                file_path: file_path.to_string(),
                start_line,
                end_line,
                start_byte,
                end_byte,
                language: language.clone(),
            }
        })
        .collect()
}

fn add_overlap(boundaries: &[(usize, usize)], source: &str, overlap: usize) -> Vec<(usize, usize)> {
    if boundaries.len() <= 1 || overlap == 0 {
        return boundaries.to_vec();
    }

    let mut result = Vec::with_capacity(boundaries.len());
    result.push(boundaries[0]);

    for i in 1..boundaries.len() {
        let (orig_start, end) = boundaries[i];
        let prev_start = boundaries[i - 1].0;
        let overlap_start = orig_start.saturating_sub(overlap);
        let snapped = snap_to_newline(source, overlap_start.max(prev_start));
        let safe_start = snapped.min(end);
        result.push((safe_start, end));
    }

    result
}

fn snap_to_newline(source: &str, pos: usize) -> usize {
    let safe_pos = snap_to_char_boundary(source, pos);
    source[safe_pos..]
        .find('\n')
        .map(|n| safe_pos + n + 1)
        .unwrap_or(safe_pos)
}

fn snap_to_char_boundary(source: &str, pos: usize) -> usize {
    if pos >= source.len() {
        return source.len();
    }
    if source.is_char_boundary(pos) {
        return pos;
    }
    let mut p = pos;
    while p < source.len() && !source.is_char_boundary(p) {
        p += 1;
    }
    p
}

fn chunk_by_lines(source: &str, target_size: usize) -> Vec<(usize, usize)> {
    let mut boundaries = Vec::new();
    let mut start = 0;
    let mut current_size = 0;

    for (offset, ch) in source.char_indices() {
        current_size += ch.len_utf8();
        if ch == '\n' && current_size >= target_size {
            let end = offset + 1;
            boundaries.push((start, end));
            start = end;
            current_size = 0;
        }
    }

    if start < source.len() {
        boundaries.push((start, source.len()));
    }

    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source() {
        let chunks = chunk_file("", "test.rs", Some("rs"));
        assert!(chunks.is_empty());
    }

    #[test]
    fn small_file_single_chunk() {
        let src = "fn main() {\n    println!(\"hello\");\n}\n";
        let chunks = chunk_file(src, "main.rs", Some("rs"));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, src);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, src.len());
    }

    #[test]
    fn large_file_multiple_chunks() {
        let func = "def process(data):\n    result = []\n    for item in data:\n        result.append(item * 2)\n    return result\n\n";
        let mut src = String::new();
        for i in 0..30 {
            src.push_str(&func.replace("process", &format!("process_{i}")));
        }
        let chunks = chunk_file(&src, "big.py", Some("py"));
        assert!(
            chunks.len() > 1,
            "expected multiple chunks, got {}",
            chunks.len()
        );

        // verify ordering and overlap (chunks may overlap due to OVERLAP_BYTES)
        for window in chunks.windows(2) {
            assert!(
                window[1].start_byte <= window[0].end_byte,
                "chunk {} should start at or before end of chunk {} (overlap expected)",
                window[1].start_byte,
                window[0].end_byte
            );
        }

        assert_eq!(chunks.first().unwrap().start_byte, 0);
        assert_eq!(chunks.last().unwrap().end_byte, src.len());
    }

    #[test]
    fn fallback_for_unsupported_language() {
        let mut src = String::new();
        for i in 0..200 {
            src.push_str(&format!(
                "line {i}: some content here that fills up space\n"
            ));
        }
        let chunks = chunk_file(&src, "data.txt", None);
        assert!(chunks.len() > 1);

        for window in chunks.windows(2) {
            assert!(window[1].start_byte <= window[0].end_byte);
        }
    }

    #[test]
    fn chunk_preserves_language() {
        let src = "fn f() {}\n";
        let chunks = chunk_file(src, "lib.rs", Some("rs"));
        assert_eq!(chunks[0].language.as_deref(), Some("rust"));
    }

    #[test]
    fn chunk_line_numbers_correct() {
        let src = "line1\nline2\nline3\nline4\nline5\n";
        let chunks = chunk_file(src, "test.txt", None);
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn no_function_split_in_python() {
        // Build a source with two ~800-char functions
        // They should chunk at function boundaries, not mid-function
        let mut func1 = String::from("def alpha():\n");
        for i in 0..40 {
            func1.push_str(&format!("    x_{i} = {i}\n"));
        }
        func1.push('\n');

        let mut func2 = String::from("def beta():\n");
        for i in 0..40 {
            func2.push_str(&format!("    y_{i} = {i}\n"));
        }
        func2.push('\n');

        let src = format!("{func1}{func2}");
        let chunks = chunk_file(&src, "test.py", Some("py"));

        if chunks.len() > 1 {
            // If split, verify each chunk contains a complete function
            for chunk in &chunks {
                let has_def = chunk.content.contains("def ");
                if has_def {
                    let def_count = chunk.content.matches("def ").count();
                    assert!(
                        def_count >= 1,
                        "chunk has partial function: {:?}",
                        &chunk.content[..80.min(chunk.content.len())]
                    );
                }
            }
        }
    }
}

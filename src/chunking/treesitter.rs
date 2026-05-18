use tree_sitter::{Language, Node, Parser};

pub fn chunk_with_treesitter(
    source: &str,
    language: &Language,
    target_size: usize,
) -> Vec<(usize, usize)> {
    let mut parser = Parser::new();
    if parser.set_language(language).is_err() {
        return fallback(source);
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return fallback(source),
    };

    let boundaries = merge_nodes(tree.root_node(), target_size);
    if boundaries.is_empty() {
        return vec![(0, source.len())];
    }

    let merged = post_merge(boundaries, target_size);
    fill_gaps(merged, source.len())
}

/// Recursively merge adjacent sibling AST nodes until accumulated size
/// reaches the target. When a single node exceeds the target, recurse
/// into its children.
fn merge_nodes(node: Node, target_size: usize) -> Vec<(usize, usize)> {
    if node.child_count() == 0 {
        return vec![(node.start_byte(), node.end_byte())];
    }

    let mut groups = Vec::new();
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    let mut i = 0;

    while i < children.len() {
        let child = children[i];
        let start = child.start_byte();
        let mut end = child.end_byte();
        let mut length = end - start;
        i += 1;

        if length > target_size {
            groups.extend(merge_nodes(child, target_size));
            continue;
        }

        while i < children.len() {
            let next = children[i];
            let next_length = next.end_byte() - next.start_byte();
            if length + next_length > target_size {
                break;
            }
            end = next.end_byte();
            length += next_length;
            i += 1;
        }

        groups.push((start, end));
    }

    groups
}

/// Merge adjacent small boundaries that together fit within the target.
fn post_merge(boundaries: Vec<(usize, usize)>, target_size: usize) -> Vec<(usize, usize)> {
    let mut merged = Vec::new();

    let mut i = 0;
    while i < boundaries.len() {
        let (start, mut end) = boundaries[i];
        i += 1;

        while i < boundaries.len() {
            let (_, next_end) = boundaries[i];
            if next_end - start > target_size {
                break;
            }
            end = next_end;
            i += 1;
        }

        merged.push((start, end));
    }

    merged
}

/// Extend each chunk to cover gaps (whitespace between AST nodes)
/// so chunks are contiguous and span the entire source.
fn fill_gaps(mut boundaries: Vec<(usize, usize)>, source_len: usize) -> Vec<(usize, usize)> {
    if boundaries.is_empty() {
        return vec![(0, source_len)];
    }
    boundaries[0].0 = 0;
    for i in 1..boundaries.len() {
        boundaries[i].0 = boundaries[i - 1].1;
    }
    if let Some(last) = boundaries.last_mut() {
        last.1 = source_len;
    }
    boundaries
}

fn fallback(source: &str) -> Vec<(usize, usize)> {
    vec![(0, source.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_lang() -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn python_lang() -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    #[test]
    fn single_function_under_target() {
        let src = "fn hello() {\n    println!(\"hi\");\n}\n";
        let chunks = chunk_with_treesitter(src, &rust_lang(), 1500);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, src.len()));
    }

    #[test]
    fn multiple_functions_merge_within_target() {
        let src = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let chunks = chunk_with_treesitter(src, &rust_lang(), 1500);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn splits_at_function_boundaries() {
        let mut src = String::new();
        for i in 0..20 {
            src.push_str(&format!(
                "fn func_{i}() {{\n    let x = {i};\n    let y = {i} * 2;\n    println!(\"{{x}} {{y}}\");\n}}\n\n"
            ));
        }
        let chunks = chunk_with_treesitter(&src, &rust_lang(), 500);
        assert!(chunks.len() > 1, "expected split, got 1 chunk");

        for (start, end) in &chunks {
            let slice = &src[*start..*end];
            let opens = slice.matches('{').count();
            let closes = slice.matches('}').count();
            assert_eq!(opens, closes, "unbalanced braces in chunk: {slice:?}");
        }
    }

    #[test]
    fn contiguous_no_gaps() {
        let mut src = String::new();
        for i in 0..30 {
            src.push_str(&format!("def f_{i}():\n    x = {i}\n    return x\n\n"));
        }
        let chunks = chunk_with_treesitter(&src, &python_lang(), 500);

        for window in chunks.windows(2) {
            assert_eq!(
                window[0].1, window[1].0,
                "gap at {}-{}",
                window[0].1, window[1].0
            );
        }

        assert_eq!(chunks.first().unwrap().0, 0);
        assert_eq!(chunks.last().unwrap().1, src.len());
    }

    #[test]
    fn oversized_single_function() {
        let mut body = String::from("fn big() {\n");
        for i in 0..200 {
            body.push_str(&format!("    let var_{i} = {i};\n"));
        }
        body.push_str("}\n");
        let chunks = chunk_with_treesitter(&body, &rust_lang(), 500);
        assert!(chunks.len() > 1, "expected split of oversized function");
    }
}

use regex::Regex;
use std::sync::LazyLock;

static IDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap());

pub fn split_identifier(token: &str) -> Vec<String> {
    let lower = token.to_lowercase();

    let parts: Vec<String> = if token.contains('_') {
        lower
            .split('_')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect()
    } else {
        split_camel_case(token)
            .iter()
            .map(|s| s.to_lowercase())
            .collect()
    };

    if parts.len() >= 2 {
        let mut result = vec![lower];
        result.extend(parts);
        result
    } else {
        vec![lower]
    }
}

fn split_camel_case(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut start = 0;

    for i in 1..chars.len() {
        let split = if chars[i].is_uppercase() && chars[i - 1].is_lowercase() {
            // fooBar -> foo | Bar
            true
        } else if i + 1 < chars.len()
            && chars[i].is_uppercase()
            && chars[i + 1].is_lowercase()
            && chars[i - 1].is_uppercase()
        {
            // HTTPResponse -> HTTP | Response
            true
        } else {
            false
        };

        if split {
            let part: String = chars[start..i].iter().collect();
            if !part.is_empty() {
                parts.push(part);
            }
            start = i;
        }
    }

    let last: String = chars[start..].iter().collect();
    if !last.is_empty() {
        parts.push(last);
    }

    parts
}

pub fn tokenize(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    for m in IDENT_RE.find_iter(text) {
        result.extend(split_identifier(m.as_str()));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case() {
        let r = split_identifier("my_func");
        assert_eq!(r, vec!["my_func", "my", "func"]);
    }

    #[test]
    fn camel_case() {
        let r = split_identifier("getHTTPResponse");
        assert_eq!(r, vec!["gethttpresponse", "get", "http", "response"]);
    }

    #[test]
    fn pascal_case() {
        let r = split_identifier("HandlerStack");
        assert_eq!(r, vec!["handlerstack", "handler", "stack"]);
    }

    #[test]
    fn simple_word() {
        let r = split_identifier("simple");
        assert_eq!(r, vec!["simple"]);
    }

    #[test]
    fn tokenize_full_text() {
        let tokens = tokenize("fn getHTTPResponse(req: &Request) -> Response");
        assert!(tokens.contains(&"fn".to_string()));
        assert!(tokens.contains(&"gethttpresponse".to_string()));
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"http".to_string()));
        assert!(tokens.contains(&"response".to_string()));
        assert!(tokens.contains(&"req".to_string()));
    }

    #[test]
    fn xml_parser_case() {
        let r = split_identifier("XMLParser");
        assert_eq!(r, vec!["xmlparser", "xml", "parser"]);
    }

    #[test]
    fn leading_underscore() {
        let r = split_identifier("_private");
        assert_eq!(r, vec!["_private"]);
    }
}

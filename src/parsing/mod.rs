pub mod languages;
pub mod outline;
pub mod snap;

use std::path::Path;
use tree_sitter::Parser;

use languages::language_for_extension;

pub fn create_parser(ext: &str) -> Option<Parser> {
    let lang = language_for_extension(ext)?;
    let mut parser = Parser::new();
    parser.set_language(&lang).ok()?;
    Some(parser)
}

pub fn extension_from_path(path: &Path) -> Option<&str> {
    path.extension()?.to_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_parser_for_known_extension() {
        let parser = create_parser("rs");
        assert!(parser.is_some());
    }

    #[test]
    fn create_parser_for_unknown_extension() {
        let parser = create_parser("xyz");
        assert!(parser.is_none());
    }

    #[test]
    fn extension_from_path_extracts() {
        assert_eq!(extension_from_path(Path::new("foo.rs")), Some("rs"));
        assert_eq!(extension_from_path(Path::new("a/b/c.py")), Some("py"));
        assert_eq!(extension_from_path(Path::new("noext")), None);
    }
}

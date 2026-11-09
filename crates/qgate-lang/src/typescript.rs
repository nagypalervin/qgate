//! TypeScript and TSX adapter, built on `tree-sitter-typescript`.

use tree_sitter::Parser;

use crate::{FileModel, Language, ParseError};

/// Parse TypeScript or TSX source into a [`FileModel`].
pub(crate) fn parse(language: Language, source: &str) -> Result<FileModel, ParseError> {
    let grammar = match language {
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
    };
    let mut parser = Parser::new();
    parser
        .set_language(&grammar.into())
        .map_err(|e| ParseError::Parser {
            message: e.to_string(),
        })?;
    let _tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::Parser {
            message: "tree-sitter returned no tree".to_owned(),
        })?;
    Ok(FileModel::default())
}

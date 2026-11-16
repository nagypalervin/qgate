//! Rust adapter, built on `tree-sitter-rust` (design of
//! `add-rust-analysis`).

use crate::common::parse_tree;
use crate::{FileModel, ParseError};

/// Parse Rust source into a [`FileModel`].
pub(crate) fn parse(source: &str) -> Result<FileModel, ParseError> {
    parse_tree(&tree_sitter_rust::LANGUAGE.into(), source)?;
    Ok(FileModel::default())
}

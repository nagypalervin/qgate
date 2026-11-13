//! C# adapter, built on `tree-sitter-c-sharp` (design of
//! `add-csharp-analysis`).

use crate::common::parse_tree;
use crate::{FileModel, ParseError};

/// Parse C# source into a [`FileModel`].
pub(crate) fn parse(source: &str) -> Result<FileModel, ParseError> {
    let _tree = parse_tree(&tree_sitter_c_sharp::LANGUAGE.into(), source)?;
    Ok(FileModel::default())
}

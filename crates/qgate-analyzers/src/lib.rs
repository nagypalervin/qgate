//! Analyzers compare a base and a head revision and emit findings.
//!
//! Planned analyzers:
//!
//! - `architecture`: forbidden dependencies between layers (M2)
//! - `complexity`: per-function complexity increase (M1)
//! - `duplication`: near-duplicate blocks introduced by a change (M2)
//! - `tests`: changed code without changed tests, coverage drop (M3)
//!
//! Status: placeholder.

use qgate_core::Finding;

/// Something that inspects a change and reports what got worse.
pub trait Analyzer {
    /// Stable name used in config and in rule ids, e.g. `complexity`.
    fn name(&self) -> &'static str;

    /// Run the analysis and return findings. An empty list means
    /// nothing got worse.
    fn analyze(&self) -> Vec<Finding>;
}

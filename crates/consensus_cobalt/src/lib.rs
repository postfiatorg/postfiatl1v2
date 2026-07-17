#![allow(clippy::too_many_arguments)]

include!("core_types.rs");
include!("validator_admission_policy.rs");
include!("cobalt_cover_extractor.rs");
include!("trust_graph_governance.rs");
include!("rbc_abba_mvba.rs");
include!("dabc_registry.rs");
include!("internal_validation.rs");

#[cfg(test)]
mod tests {
    include!("tests.rs");
}

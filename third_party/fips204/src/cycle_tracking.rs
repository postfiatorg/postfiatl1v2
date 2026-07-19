//! SP1-only measurement hooks.
//!
//! These hooks are compiled to no-ops unless `sp1-cycle-tracking` is enabled.
//! Static command strings avoid allocation and keep the tracker perturbation
//! bounded and repeatable.

#[cfg(feature = "sp1-cycle-tracking")]
#[inline(never)]
/// Writes one static SP1 cycle-tracker command to standard output.
pub(crate) fn write(command: &'static [u8]) {
    sp1_zkvm::io::write(1, command);
}

#[cfg(not(feature = "sp1-cycle-tracking"))]
#[inline(always)]
/// Compiles measurement hooks away in production and native builds.
pub(crate) const fn write(_command: &'static [u8]) {}

macro_rules! tracked {
    ($label:literal, $expression:expr) => {{
        crate::cycle_tracking::write(
            concat!("cycle-tracker-report-start:", $label, "\n").as_bytes(),
        );
        let tracked_result = $expression;
        crate::cycle_tracking::write(concat!("cycle-tracker-report-end:", $label, "\n").as_bytes());
        tracked_result
    }};
}

pub(crate) use tracked;

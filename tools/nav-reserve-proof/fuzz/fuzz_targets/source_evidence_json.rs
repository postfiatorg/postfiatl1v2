#![no_main]

use libfuzzer_sys::fuzz_target;
use reserve_proof_types::{SourceEvidenceV1, MAX_EVIDENCE_BYTES};

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_EVIDENCE_BYTES {
        return;
    }
    if let Ok(evidence) = serde_json::from_slice::<SourceEvidenceV1>(data) {
        let _ = evidence.class();
        let _ = evidence.commitment();
    }
});

# Initial E1 mismatch review

- Frozen corpus SHA-256: `42ed266ba207136eec560f8be14c904c2e63ffe305e188860e4ff04731cd5fd2`
- Classification SHA-256: `90eee779ec246901c16d66b6391079b90568430198c9a68dcd447eefd2d5b368`
- Cases: 10,240
- Complete agreements: 1,706 deliberately invalid strict-inequality boundary cases
- Frozen disagreements: 8,534 valid graph cases
- Oracle review: the first and second oracles agreed on validity, compatibility, fully linked pairs among correct validators, strong support, and strongly connected closures in every case; no oracle correction is indicated.
- Production review: all 10,240 production attempts stopped before graph construction with `Cobalt domain genesis_hash must be 96 lowercase hex characters`.
- Classification: the harness adapter supplied a 64-character genesis fixture. This is an adapter defect, not a production Cobalt defect.
- Follow-up regression review: after correcting the genesis fixture, the focused production-graph construction test exposed the same 64-versus-96 width defect in the adapter's registry-root fixture.
- Remediation: correct both adapter fixture hashes to the production-required 96 lowercase hex characters, retain the focused regression test, and rerun this unchanged corpus.

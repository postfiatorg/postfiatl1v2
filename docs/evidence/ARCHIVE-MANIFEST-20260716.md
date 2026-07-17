# Raw evidence archive manifest — 2026-07-16

- Archive identifier: `postfiatl1v2-docs-evidence-20260716`
- Archive format: deterministic uncompressed POSIX tar
- Source prefix: `docs/evidence/`
- Regular files: `1283`
- Source bytes (`du -sb`): `36326352`
- Archive bytes: `38144000`
- SHA-256: `ac6911368cb199e475dce8fce2309ffd18811ab9c6ca5048aae9a85084cb5eea`
- Access class: restricted engineering evidence; not a public release asset
- Public disposition: 1,273 raw files removed; ten redaction-safe source
  summaries kept with this manifest and the raw-evidence policy

Verification used a full archive listing count and a byte-for-byte extraction
hash of a shielded-ingress batch before the source copy was removed. The raw
archive includes historical devnet material and must be handled as potentially
privacy-sensitive even after notes are spent.

## Publication-excluded media

- Archive identifier: `postfiatl1v2-publication-excluded-media-20260716`
- Contents: 19 unreferenced/historical wallet screenshots and the locally
  downloaded VeriLLM research PDF (arXiv `2509.24257`)
- Regular files: `20`
- Archive bytes: `3235840`
- SHA-256: `be670b538db5a56d2c00ef4c4fc1cecd07c649687f45d01d50510ea6964caf37`
- Access class: engineering/design history; not a public source artifact

The retained Cobalt reference PDF is not part of this archive: it remains a
hash-pinned source reference with a maintained Markdown extraction. The removed
VeriLLM PDF is available from its canonical DOI
`https://doi.org/10.48550/arXiv.2509.24257`; the public source tree need not
redistribute a downloaded copy.

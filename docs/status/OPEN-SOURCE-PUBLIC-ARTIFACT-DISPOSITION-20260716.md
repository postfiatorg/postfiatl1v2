# Public Binary and Media Artifact Disposition

Date: 2026-07-16
Status: complete candidate-tree binary/media inventory; fail-closed regression enabled

The publication candidate contains exactly 14 tracked binary or media paths,
representing 13 distinct blobs. Every path is classified and SHA-256 pinned by
`scripts/test-public-artifact-policy`; any addition, removal, or byte change
fails CI until it receives an explicit review and disposition. Raw evidence,
screenshots, downloaded duplicate papers, report archives, and operator captures
are not accepted publication artifacts.

| Class | Paths | Disposition and reason |
| --- | ---: | --- |
| SP1 aggregate fixtures | 2 | Keep as small consensus-verifier positive vectors; exact bytes are test inputs, not generated release output. |
| Asset-Orchard parameters | 1 | Keep as the active `k=15` proving/verifying parameter input; runtime metadata and tests hash-bind it. |
| Active Asset-Orchard verifier keys | 2 | Keep because live verification loads these exact pinned assemblies and fails closed on drift. |
| Historical Asset-Orchard verifier keys | 2 | Keep because authenticated archive replay needs the exact old circuit keys; they are explicitly under `artifacts/replay`, never selected for live actions. |
| Cobalt research source PDF | 1 | Keep as the sole redistributed research PDF because its hash, source, license/retention rationale, and Markdown extraction are recorded. |
| In-tree upstream Halo2 proof fixture | 1 | Keep as part of the hash-verified upstream test corpus and provenance package; it accompanies the pinned upstream dependency and is not evidence of a PostFiat proof-system reimplementation. |
| Extension icons | 3 | Keep as minimal public product assets. |
| Wallet WASM | 2 paths / 1 blob | Keep in both package layouts so a source checkout builds the web wallet and extension without fetching opaque bytes. Both paths are byte-identical, Git stores one blob, the deterministic build command remaps builder paths, and two rebuilds reproduced SHA-256 `395576c1efa2fc5115e94df17645f1fb0f5584fd5ce4f7677e6e3539258ea5a2`. |

Largest retained artifact: 2,097,220 bytes. No tracked screenshot, raw browser
capture, compressed report archive, downloaded duplicate research PDF, or
unclassified binary remains in the candidate tree.

This inventory does not treat build caches, local virtual environments,
`node_modules`, `target`, `site`, or untracked reports as source artifacts; the
release/archive builder must continue excluding them.

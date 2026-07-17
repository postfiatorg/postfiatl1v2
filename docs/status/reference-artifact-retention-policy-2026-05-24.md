# Reference Artifact Retention Policy

Status: active repo-shape policy.
Date: 2026-05-24

## Scope

This policy covers tracked reference or generated artifacts that legitimately
remain above the 5,000-line oversized-file threshold after the emergency source
refactor.

## Decisions

| Artifact | Decision | Reason | Guard |
| --- | --- | --- | --- |
| `docs/references/cobalt-bft-governance-in-open-networks.pdf` | Retain in Git as a hash-pinned reference artifact. | The PDF is the local source reference for the Cobalt Markdown extraction and governance docs, is small enough for ordinary Git retention at 500,545 bytes, and has no local Git LFS policy configured. Keeping it local preserves reproducibility if the external arXiv URL changes or is unavailable. | `docs/status/oversized-file-baseline.json` records the SHA-256, byte count, and line count; `scripts/check-reference-artifacts` verifies them. |
| `reports/testnet-cobalt-canonical-artifacts/testnet-cobalt-collusion-threshold-normalized-v1.json` | Retain in Git as one canonical generated packet body. | Forty full generated copies were replaced by hash-bound manifests. This one body is the reconstruction source for those manifests. | `scripts/check-cobalt-report-references` verifies canonical artifact hashes and reconstructed original report hashes. |
| `reports/testnet-cobalt-canonical-artifacts/testnet-cobalt-strict-launch-expected-fail-normalized-*.json` | Retain in Git as canonical generated strict-launch report bodies. | Four oversized strict-launch report copies were replaced by hash-bound manifests. These bodies are the reconstruction source for those manifests. | `scripts/check-cobalt-report-references` verifies canonical artifact hashes and reconstructed original report hashes. |

## Rule

Do not add a new tracked artifact above 5,000 lines unless one of these is true:

- it is split or sharded below the threshold;
- it is replaced by a hash-bound reference manifest plus a canonical artifact;
- it is added to `docs/status/oversized-file-baseline.json` with an explicit
  retention reason and, for reference artifacts, a SHA-256 checked by
  `scripts/check-reference-artifacts`.

# Security Policy

## Reporting a Vulnerability

Do not open a public GitHub issue for security vulnerabilities.

Use the repository's GitHub **Report a vulnerability** / private security
advisory form. This is the only published intake channel; the project does not
publish an unauthenticated fallback email address or PGP key. If GitHub's form
is unavailable, do not open a public issue or disclose exploit details—wait for
the private channel to recover.

Include:
- A description of the vulnerability and its impact.
- Steps to reproduce or a proof of concept.
- Any relevant logs, hashes, or transaction IDs.

You will receive an acknowledgment within 72 hours. If the report is
accepted, a fix and advisory will be prepared and coordinated disclosure
will follow.

## Supported Versions

PostFiat L1 is in the controlled-testnet phase. Only the latest `main`
branch is supported for security fixes.

## Security Status

PostFiat L1 is pre-mainnet controlled-testnet software. The codebase has
internal review, replay tests, and controlled-network evidence, but it should
not be treated as externally audited production infrastructure.

## Threat Model Summary

| Adversary | Capability | Mitigation |
| --- | --- | --- |
| Byzantine validator | Up to `f=floor((n-1)/3)` of the active set can equivocate, withhold proposals, or stop voting | Distinct-voter certificates and persist-before-sign safety state protect both versioned modes: legacy direct-certificate finality and activated consensus v2 prepare/precommit finality |
| Network adversary | Can delay, reorder, replay, or drop messages between validators | Domain-bound signatures and replay checks protect safety; activated consensus v2 advances views only with signed timeout certificates, while loss of the normal quorum still halts progress |
| Post-quantum adversary | Targets long-lived authorization signatures | ML-DSA-65 (FIPS 204) protects implemented account and validator authorization; no SLH-DSA recovery-key path is currently implemented |
| Privacy adversary | Observes public chain data, mempool, and network metadata | Orchard/Halo2 shielded settlement with nullifier set; privacy floor calibration; RPC resource limits |

## Security Boundaries

- Validator keys, operator keys, and service secrets are never committed to git.
- The `.gitignore` excludes `target/`, `site/`, node data directories, and local key material.
- Consensus and state-transition code is designed to be deterministic and to avoid panic paths on untrusted inputs.
- RPC inputs, file inputs, and network messages are treated as untrusted and bounds-validated.

## Current Security Limitations

- Validator keys are plaintext software-key files protected by host filesystem
  permissions. Production HSM/remote-signer custody is not implemented.
- Consensus state uses a size-bounded JSON/JSONL store with a synced ordered
  commit journal and cross-process mutation locks. It is not a transactional
  indexed production engine; long-running validator and RPC services fail
  closed without the explicit `--unsafe-devnet-json-storage` acknowledgement.
- The FastPay owned-object lane exposes only signed, state-validated mutation
  methods under normal RPC startup. Operators may disable it during an incident
  with `--disable-owned-lane`; unsafe unsigned wrap/unwrap methods remain absent.
  Versioned ordered recovery confirms or cancels abandoned locks and permanently
  fences the prior object version. External deployment/operations evidence for
  that recovery path remains a real-value gate.
- FastSwap's dual-owner Confirm-or-Cancel protocol is implemented and has local
  six-process safety, conservation, catch-up, and restart evidence. Source
  availability is not a claim that a public or real-value network has activated
  the lane.
- Public RPC requires an authenticated TLS edge; the node rejects public and
  wildcard plaintext binds.
- This policy is not an external audit report or a mainnet-readiness claim.

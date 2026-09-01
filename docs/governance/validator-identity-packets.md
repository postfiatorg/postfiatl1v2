# Validator Identity Packets

**Status:** Full frozen corpus published — 2026-09-01

## Purpose

Validator keys and claimed domains do not by themselves provide enough context
for institutional reputation or association scoring. The identity-packet stage
turns those minimal coordinates into cited, reviewable external evidence before
any H200 score is requested.

The frozen corpus covers all 35 validators in the matching Ripple and XRP
Ledger Foundation publisher lists and all 20 validators in the current
completed PostFiat UNL, round 20.

## Pipeline

For each validator:

1. freeze the network, validator master key, claimed domain, upstream
   domain-verification value, publisher membership, and metadata source;
2. pass an exact validator-specific initial prompt to an independent
   `corbanu --search exec` session;
3. publish the returned Markdown packet and complete JSONL exec log;
4. validate headings, coordinate fidelity, business-summary structure,
   machine-readable JSON, packet/log equality, and hashes; and
5. pass only the frozen Markdown packet bytes and their hashes into the later
   pinned H200 scoring run.

The H200 phase does not browse, rerun Corbanu, or call external inference from
consensus. Identity packets remain `SHADOW_ONLY` external evidence.

## Packet contract

Every packet contains:

- validator coordinates and claimed-domain status;
- official URLs and public identity;
- supported aliases;
- a single neutral 90–160 word business-reference summary;
- public X handle;
- incorporation and operating regions;
- activities;
- estimated public-profile size;
- cited evidence and explicit uncertainties; and
- a machine-readable summary matching the prose.

Unsupported fields are null or “Not established.” Domain claims, registry
labels, list membership, and organizational branding are not silently promoted
into proof of current validator-key control.

## Published evidence

The complete artifact is under
`benchmarks/ai-governance/validator-identity-packets-20260901/`.

It includes 55 coordinate inputs, 55 exact prompts, 55 Markdown packets, 55
Corbanu exec JSONL logs, 55 stderr captures, 55 run receipts, a human-readable
index, a structured index, a deterministic finalizer, and a corpus manifest.

| Artifact set | SHA-256 |
| --- | --- |
| Frozen validator corpus | `7687dcd9a23638dca4e0fbe50c2dd3782c6db89fa645802cd5dd9586feb87f27` |
| Prompt template | `48a03cabd80cfd0f8fac6ef57cdc700ce1ea45c88a87b9fbdf7f9ee0f6d3769b` |
| All 55 packets | `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd` |
| All 55 exec logs | `3a72e90a410df1c1ce0681f63d5d581ab70d724bda35362f8a03078a305493b1` |

The final verification result is **55/55 PASS**.

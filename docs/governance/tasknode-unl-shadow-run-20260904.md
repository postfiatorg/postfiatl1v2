# Task Node UNL real-data shadow run — 2026-09-04

**Mode:** `SHADOW_ONLY`

**Outcome:** Successful fail-closed run. The derived candidate set was empty,
the 20-validator baseline was unchanged, and all three observed ledger accounts
held on named missing evidence. This note authorizes no live action.

## Read-only source and freeze

Task Node source revision
`ab1f2457afdc41b8a195899a6b5d37c412f7cbe0` documents the PFT Ledger RPC at
`http://178.156.143.199:5005` and the archive at
`wss://ws-archive.testnet.postfiat.org` in
`docs/wiki/architecture/pftl-live-task-replay.md`. This repository does not
document an equivalent PFT Ledger endpoint.

Only unauthenticated reads were made. No transaction was prepared or submitted,
no credential or key store was read, and no Task Node service or website was
accessed.

| Probe | Result |
| --- | --- |
| `server_info` at `https://rpc.testnet.postfiat.org` | HTTP 200; retained ledgers `5950054-6012252` |
| `server_info` at `https://rpc.testnet.postfiat.org:5006/` | Connection refused; skipped |
| `server_info` at `http://178.156.143.199:5005` | HTTP 200; retained ledgers `6011631-6012252` |
| `server_info` at `wss://ws-archive.testnet.postfiat.org` | Success; complete ledgers `1-6012268`; selected |

The frozen query used `account_tx`, forward order, ledger range
`1-6012256`, limit 200, for
`rpHvzMCKZ7JrzGfRseXohC3RsMWqcnEKkA`. The anchor is validated ledger
`6012256`, hash
`14ac80ccb897dd9b4c2aad37f6be0fe52bdd2269f0cfea34fd6605357164c52d`,
closed at `2026-09-04T11:57:22Z`.

The account had 49 validated transactions and no continuation marker, so its
history was complete through the anchor. All 49 were successful payments. They
span ledgers `5620845-6003432` and close times
`2026-08-21T19:21:52Z` through `2026-09-04T04:32:31Z`. The records identify
two counterparties and three wallets including the Task Node distribution
wallet. Forty-eight transactions carried `pf.ptr/v4` memos: 47 encrypted
reward pointers and one encrypted context pointer.

A bounded completeness probe then read 730 records for
`rPo8GkCA9YMKzuJGTHbj11kdVfPqSJHxNx` in four pages through its end of history,
and 1,221 records for `rwdm72S9YVKkZjeADKU2bbUMuY4vPnSfH7` in seven pages.
The latter stopped at marker `{"ledger":482263,"seq":0}` when the aggregate
2,000-record cap was reached, so counterparty funding history remained
incomplete. Those bulk counterparty responses are summarized in the source
manifest rather than retained.

## Baseline and adapter inputs

The baseline and seed list are the 20 entries in
`benchmarks/ai-governance/institution-reputation-unl-20260901/sources/postfiat-current-unl.json`
at repository revision `deaaa5a280765869af0d5a472921710711b9a37f`. The file
records completed round 20 and has SHA-256
`b4edcf3d752d854f516ea3826a390da036535f33463b7802d240b86454a796b1`.
That content hash is used as the shadow registry root. The three validators
whose frozen identity coordinates say `Post Fiat` are excluded from the equal
seed set as Foundation-bound.

The round-19 lag view came from the public, unauthenticated read
`https://scoring-testnet.postfiat.org/api/scoring/rounds/19/outputs/selected_unl.json`.
Its 20-validator response has SHA-256
`1fb36067ef30d183b1e7443d68bbda5a547302ba3824b5d16b4f7d6d96050862`.
The later public round 21 was observed but was not substituted for the
repository-pinned round-20 baseline.

The adapter received:

- 49 real funding-transfer records in the 180-day window ending at the ledger
  anchor;
- empty binding, signed work-digest, publishing-key, ledger-digest-snapshot,
  vouch, and co-work inputs;
- `null` for the unavailable published funding exclusion list;
- incomplete funding-history/window flags because the bounded counterparty
  pull reached its cap; and
- three coverage-only candidate rows for the observed wallets, with all
  nullable Admission Policy V1 facts absent.

The coverage rows are not validator identities. Their `public_key_hash` values
hash public wallet transaction signing keys observed in the snapshot; those are
not validator keys. Unique `unresolved-*` control-group strings satisfy the
runner's non-null input syntax only; they are not evidence and cannot admit a
candidate.

## Shadow result

The CLI was run twice over the same directory. Both output files had SHA-256
`1998fafb9131331c66fb74446ac19b101440f925e8c56f72f3a636f05800767f`;
the report's internal hash is
`3000bc768071c7f1c72a5e1ac3e4aef4aef6f95237fe2b5b35d99adb6f20b661`.

The public-edge stage held on
`missing_funding_exclusion_list:funding_exclusions`, so its atomic result
contained **0 funding edges** and the trust graph remained unavailable. No
account was eligible. No validator was added or removed. The churn guard
allowed the no-change result and reported 100.0% one-round overlap and 81.8%
two-round overlap (`18/22`) against round 19.

Every observed-account row held with the same complete reason set:

| Observed account | Verdict and named reasons |
| --- | --- |
| `rPo8GkCA9YMKzuJGTHbj11kdVfPqSJHxNx` | **HOLD** — `binding_missing`, `work_digest_missing`, `edge_extraction_hold`, `trust_graph_unavailable`, `missing_accountability`, `missing_rho`, `missing_reliability`, `missing_operator_manifest_signature`, `missing_domain_control`, `missing_cobalt_linkedness`, `missing_model_classification`, `missing_required_evidence` |
| `rpHvzMCKZ7JrzGfRseXohC3RsMWqcnEKkA` | **HOLD** — `binding_missing`, `work_digest_missing`, `edge_extraction_hold`, `trust_graph_unavailable`, `missing_accountability`, `missing_rho`, `missing_reliability`, `missing_operator_manifest_signature`, `missing_domain_control`, `missing_cobalt_linkedness`, `missing_model_classification`, `missing_required_evidence` |
| `rwdm72S9YVKkZjeADKU2bbUMuY4vPnSfH7` | **HOLD** — `binding_missing`, `work_digest_missing`, `edge_extraction_hold`, `trust_graph_unavailable`, `missing_accountability`, `missing_rho`, `missing_reliability`, `missing_operator_manifest_signature`, `missing_domain_control`, `missing_cobalt_linkedness`, `missing_model_classification`, `missing_required_evidence` |

The named failed fields were
`validator.identity.tasknode_binding`,
`validator.admission.accountability_score`,
`validator.admission.rho_score`,
`validator.trust_graph.edges`,
`validator.trust_graph.cluster_seat_cap`,
`validator.performance.uptime_window_bps`,
`validator.operator_manifest.signature_valid`,
`validator.identity.key_domain_binding.status`,
`validator.cobalt.linkedness_safe`, and
`validator.model.operator_independence_classification`.

## Evidence that must begin appearing

- **Validator binding memos:** Phase 0's validator-key-to-wallet CLI and
  `validator.identity.tasknode_binding.*` fields must produce the public memo,
  wallet address, transaction hash, challenge digest, and both signatures.
- **Signed per-account work digests:** Phase 0's Task Node work-digest item must
  publish each account/window digest and the publishing-key material needed to
  verify it. The digest must enumerate and reconcile the ledger pointers used
  for accountability.
- **Signed public vouch memos:** these map to the proposal's Phase 2 vouch-edge
  rollout, not Phase 0. They must be public ledger memos; private messages are
  never eligible. Without a seed-connected public edge source, observed
  accounts cannot clear the connectivity floor.

A future non-empty derivation also needs a valid published exchange/Foundation
funding exclusion list, complete funding coverage for the evaluated window and
first-funder history, replayable co-work facts when used, and the remaining
Admission Policy V1 evidence named above. None may be inferred from these
coverage rows.

## Retained evidence

The bounded evidence package is in
`docs/governance/tasknode-unl-shadow-run-20260904/`:

- `archive-rpc-response.json`, `ledger-snapshot.json`, and
  `pointer-memos.json` retain the bounded public ledger evidence;
- `source-manifest.json` records endpoints, exact counts, hashes, probes, and
  coverage limits;
- `shadow-input.json` and its referenced JSON files are the exact runner
  inputs; and
- `shadow-report.json` is the byte-identical derived result.

`build_evidence.py` deterministically rebuilds the local input files from the
retained responses and performs no network access.

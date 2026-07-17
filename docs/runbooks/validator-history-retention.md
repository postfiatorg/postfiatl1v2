# Validator History Retention

Status: controlled-testnet runbook draft
Date: 2026-05-13
Audience: validator operators, protocol engineering, release engineering

## Purpose

PostFiat validators should not be mandatory full-history servers. This is
especially important because ML-DSA signatures, registry-backed certificates,
future privacy payloads, and RPC indexes make chain history materially heavier
than legacy elliptic-curve payment chains.

The controlled-testnet target is a partial-history validator mode:

- validators keep current state;
- validators keep recent block, receipt, certificate, registry, governance, and
  batch evidence needed for safety and catch-up;
- validators keep checkpoints/snapshots and enough replay evidence to verify
  continuity;
- old full payload history moves to archive/indexer nodes;
- pruning fails closed if required recent evidence, registry history, or
  checkpoint continuity is missing.

This is similar in operating shape to the XRP Ledger `rippled` split between
recent validated ledger retention, online deletion, advisory deletion, and
separate full-history operation.

## XRPL Operating Analogy

The useful `rippled` concepts are:

- `online_delete`: retain a configured number of recent validated ledgers, then
  delete older local history.
- `advisory_delete`: require an operator command or schedule before deletion,
  so pruning can run during off-peak windows.
- `[ledger_history]`: configure how much validated history to backfill from
  peers, bounded by retention.
- Full-history servers are a separate operating posture from ordinary
  validators.

PostFiat should not copy the config names blindly, but it should copy the
operator distinction: validator nodes validate with bounded recent history;
archive/indexer nodes serve long-range historical data.

## Proposed PostFiat Config Surface

Controlled-testnet validators should expose explicit history policy in the node
config or validator manifest:

```toml
[history]
mode = "partial"
retain_recent_blocks = 50000
retain_recent_receipts = 50000
retain_recent_certificates = 50000
retain_recent_batches = 50000
retain_recent_governance = 100000
checkpoint_interval_blocks = 1000
minimum_replay_window_blocks = 5000
backfill_window_blocks = 5000
advisory_prune = true
archive_handoff_required = true
fail_closed_on_missing_recent_evidence = true
```

Initial controlled-testnet defaults can be smaller while block cadence is slow,
but the knobs must exist before public claims. Production defaults must be based
on measured disk growth, block cadence, certificate size, and operator hardware.

## Validator Responsibilities

A partial-history validator must retain:

- latest durable ledger/state;
- current and genesis validator registry roots;
- registry history needed to verify every retained block certificate;
- recent block headers and block records;
- recent receipt records;
- recent block certificates and timeout certificates;
- recent batch archive payloads required for catch-up/replay;
- governance records affecting the retained window;
- checkpoint manifests and state roots;
- pruning journal describing what was deleted and why;
- archive handoff proof when `archive_handoff_required` is enabled.

A validator may prune:

- full historical batch payloads older than the replay window;
- full historical receipts older than the configured receipt window;
- historical transaction bodies after their block/receipt roots and archive
  handoff are durable;
- old RPC/indexer-only material that is not needed for consensus replay.

A validator must not prune:

- current state;
- genesis configuration;
- current validator registry;
- registry history required to verify retained certificates;
- unresolved governance/action evidence;
- checkpoints required for state continuity;
- recent evidence required for catch-up, stale-vote rejection, or post-change
  governance replay.

## Archive/Indexer Responsibilities

Archive and indexer nodes are responsible for:

- full historical block payloads;
- full receipt history;
- account history;
- transaction lookup;
- historical finality proofs;
- long-range replay packages;
- explorer queries;
- data export for auditors and external reviewers.

Archive nodes must verify data before serving it. They are not consensus
authorities; they are historical evidence providers.

The current controlled-testnet role policy is
`docs/status/controlled-testnet-history-roles.json`. It makes the validator
role explicitly partial-history/advisory-prune, requires archive handoff and
source backfill, and declares at least one full-history archive role for
controlled testnet. `scripts/testnet-history-role-policy-smoke` verifies that
policy file against the latest history-retention smoke report.

## Required Commands

The controlled-testnet CLI should grow these operator commands:

```bash
postfiat-node history-status --data-dir <validator-data>
postfiat-node history-prune-plan --data-dir <validator-data>
postfiat-node history-archive-handoff-create --data-dir <validator-data> --from-height <h> --to-height <h> --output <archive-handoff.json>
postfiat-node history-archive-handoff-verify --data-dir <validator-data> --proof-file <archive-handoff.json>
postfiat-node history-prune --data-dir <validator-data> --up-to-height <height>
postfiat-node history-prune-recover --data-dir <validator-data>
postfiat-node history-checkpoint-rebuild-from-archive --data-dir <validator-data> --backup-file <checkpoint-v1.backup.json>
postfiat-node history-can-prune --data-dir <validator-data> --up-to-height <height>
postfiat-node history-backfill --data-dir <validator-data> --source-rpc <url>
postfiat-node archive-export-window --from-height <h> --to-height <h> --output <archive-window.json>
postfiat-node archive-window-verify --bundle-file <archive-window.json>
postfiat-node archive-window-import --data-dir <validator-data> --bundle-file <archive-window.json>
postfiat-node archive-window-backfill --data-dir <validator-data> --source-host <archive-host> --source-rpc-port <port> --from-height <h> --to-height <h>
```

Implemented now:

- `history-status`: reports mode, retention policy, local block range, receipt
  count, archived batch count, governance history counts, block verification,
  checkpoint/prune-journal file sizes, and relevant storage-file sizes.
- `history-prune-plan`: non-destructive dry run that computes a safe boundary
  from retention windows and fails closed while inside retention, when block
  verification fails, or when archive handoff proof is required but unavailable.
- `history-can-prune`: alias for the same dry-run planner.
- `history-archive-handoff-create`: writes a deterministic proof binding a
  handed-off block range to chain id, genesis hash, protocol version, block
  range root, batch payload root, receipt root, first/last block hash, and proof
  hash.
- `history-archive-handoff-verify`: recomputes the handoff proof from local
  history and rejects tampered or stale handoff files.
- `history-prune`: destructive advisory prune that requires a verified archive
  handoff proof, writes `history_checkpoint.json`, removes covered block,
  batch-archive, and receipt entries, verifies the retained suffix from the
  checkpoint, and appends `history_prune_journal.json`.
- `history-prune-recover`: completes an interrupted prune from
  `history_prune_pending.json`, rewrites the active files idempotently, verifies
  from the checkpoint, appends the journal record if missing, and clears the
  pending marker.
- `history-checkpoint-rebuild-from-archive`: offline-only recovery for an
  unverifiable `postfiat-history-checkpoint-v1`. It ignores the legacy economic
  state, replays contiguous imported archive windows from genesis, verifies the
  rebuilt prefix and retained suffix in isolated shadow stores, writes the
  requested exact backup, and atomically installs checkpoint v2. Stop the node
  before running it; a missing/gapped/tampered archive or changed live
  checkpoint fails before replacement.
- `archive-export-window`: writes a deterministic archive-window bundle
  containing the selected block records, archived batch payloads, receipts,
  archive-handoff proof, and bundle hash.
- `archive-window-verify`: verifies an archive-window bundle hash and recomputes
  the handoff proof from bundled contents.
- `archive-window-import`: verifies a bundle against local genesis and stores it
  under `history_archive_windows/` with `history_archive_index.json` without
  mutating the active retained block log.
- read-only RPC `archive_window`: full-history/archive nodes can serve a
  deterministic archive-window bundle over the bounded RPC service.
- `archive-window-backfill`: fetches an archive window from a source RPC node,
  checks source chain domain against the local validator, writes the fetched
  bundle into the operator work directory, verifies/imports it into
  `history_archive_windows/`, and leaves active retained block history pruned.
- RPC SDK request/response validation for `archive_window`: external tools can
  build bounded archive-window requests and reject malformed bundle responses.
- `scripts/testnet-history-role-policy-smoke`: verifies the controlled-testnet
  history-role policy, archive/indexer responsibilities, gate declarations, and
  the latest partial-history/archive-window backfill evidence.

Current checkpoint behavior:

- `history_checkpoint.json` is an operator-local replay checkpoint, not a
  consensus payload.
- `history_prune_pending.json` is a short-lived local recovery marker written
  before checkpoint/active-file rewrites begin.
- The checkpoint stores the pruned boundary height/hash/state root, the archive
  handoff roots, and replay state needed to verify future retained blocks.
- Block verification starts from the checkpoint when earlier local blocks have
  been pruned.
- New blocks after pruning use the checkpoint block hash as the parent hash
  when no retained local block exists.
- `ordered_batches.json` remains durable because it is part of the current
  replicated state root; only block records, archived batch payloads, and
  receipt rows covered by the handoff are removed.
- Checkpoint v2 commits cumulative native fee burns and requires live native
  custody plus cumulative burns to equal genesis supply. Checkpoint v1 is
  deliberately refused and can only be recovered by the archive-backed command
  above; it is never grandfathered or trusted as an economic replay base.

`history-prune` should be blocked unless:

- block verification passes through the retained tip;
- retained registry history can verify every retained certificate;
- the checkpoint before the prune boundary is valid;
- all retained receipts match block receipt roots;
- archive handoff is present when required;
- the target height is older than all configured retention windows;
- no active release gate depends on the evidence being pruned.

## RPC Surface

Public RPC should expose the history posture:

- current height;
- complete local history range;
- retained receipt range;
- retained batch-payload range;
- latest checkpoint height/hash;
- archive mode: `validator_partial`, `archive`, or `full_history`;
- pruning mode: `automatic`, `advisory`, or `disabled`;
- whether historical tx lookup was served locally or by archive backend.

This gives external clients an XRP-like expectation: a validator may be healthy
without being a full-history node.

## Release Gate

Controlled-testnet release should require:

- history config is present in the validator package;
- `history-status` reports the expected mode;
- prune dry-run refuses unsafe deletion;
- destructive prune writes a checkpoint and prune journal;
- interrupted prune recovery clears pending state and preserves verification;
- post-prune block append and `verify-blocks` work from the checkpoint;
- catch-up works from retained recent evidence;
- source-driven archive backfill works from a full-history/archive RPC source
  after local pruning;
- the history-role policy smoke passes and the P0/release reports link its
  evidence;
- release packages do not contain private keys or machine credentials in
  history manifests.

## Open Decisions

- Exact controlled-testnet retention counts.
- Whether pruning is automatic, advisory-only, or disabled for the first
  controlled cut.
- Production archive node topology and independent archive/indexer onboarding.
- How receipt roots, batch roots, and account-history indexes are exposed for
  long-range proof verification.

## References

- XRPL online deletion:
  `https://xrpl.org/docs/infrastructure/configuration/data-retention/online-deletion`
- XRPL configure online deletion:
  `https://xrpl.org/docs/infrastructure/configuration/data-retention/configure-online-deletion`
- XRPL advisory deletion:
  `https://xrpl.org/docs/infrastructure/configuration/data-retention/configure-advisory-deletion`
- XRPL full history:
  `https://xrpl.org/docs/infrastructure/configuration/data-retention/configure-full-history`

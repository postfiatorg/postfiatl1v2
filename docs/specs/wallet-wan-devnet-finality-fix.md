# Wallet WAN Devnet Finality Fix

Status: implementation spec
Date: 2026-06-27
Scope: PostFiat L1 wallet submission, WAN devnet validator liveness, and RPC write/finality behavior

## Problem Statement

The PostFiat web wallet must be able to send user-signed transactions on the
WAN devnet without the wallet, the web server, or the operator helper owning all
validator keys or local validator data directories.

The current system has the right primitives, but the operational and helper
paths are mixed:

- `apply-batch` is being used as a convenient local/devnet finalization path.
  That requires local filesystem access to validator data directories and is not
  a wallet/testnet transaction path.
- The WAN devnet RPC endpoint can answer reads, but the latest observed height
  was stalled at `472`; a read-only or stalled RPC can show balances while
  being unable to finalize sends.
- The Python wallet helper can submit signed transactions to RPC, but its local
  "finalize now" mode still uses `mempool-batch` plus `apply-batch`.
- The existing RPC finality endpoint runs a peer-certified mempool round but
  currently requires the serving validator to be the local proposer. That makes
  it unsuitable as a general always-on wallet endpoint when proposer rotation
  selects another validator.

This must be fixed before the wallet can be treated as demo-ready or
controlled-testnet-ready.

## Design Principle

A normal wallet transaction should look like XRPL-style wallet flow:

1. The wallet derives/holds user keys locally.
2. The wallet signs the transaction locally.
3. The wallet submits the signed transaction to a write-enabled RPC edge.
4. The validator network orders and certifies the transaction.
5. The wallet polls transaction status or receives finality evidence.

The wallet must not need validator private keys, proposer keys, SSH access, or
local validator data directories.

## Terminology

| Term | Meaning |
| --- | --- |
| Wallet submit path | Browser or Python wallet signs a user transaction and sends it to RPC. |
| RPC write edge | A validator RPC service with mempool submission enabled. |
| Consensus loop | Long-running validator services that propose, vote, certify, apply, and converge blocks. |
| Local harness path | Development shortcut using local data dirs and `apply-batch`. |
| Peer-certified round | One-shot transport command that obtains validator votes and forms a certificate. |
| Finality RPC | RPC method that accepts a signed transaction and returns certified finality evidence. |

## Existing Code Facts

The implementation must preserve these existing boundaries:

- `python/postfiat_rpc/wallet.py::send_pft` already signs and submits through
  `mempool_submit_signed_transfer` / `mempool_submit_signed_payment_v2`.
- The same helper only runs `mempool-batch` and `apply-batch` when
  `finalize_data_dir` is supplied. That branch is local harness behavior and
  must not be used for WAN devnet wallet sends.
- `crates/node/src/rpc_cli.rs` recognizes `mempool_submit_signed_transfer`,
  `mempool_submit_signed_payment_v2`, `mempool_submit_signed_asset_transaction`,
  `mempool_submit_signed_offer_transaction`, and related write methods.
- `rpc-serve` write methods are intentionally gated by `--allow-mempool-submit`
  or `--allow-mempool-submit-finality`.
- `mempool_submit_signed_transfer_finality` currently invokes
  `transport_peer_certified_mempool_round`, but sets `require_local_proposer:
  true`. That is acceptable for a controlled one-shot run when the local node is
  proposer, but it is not a general wallet API.
- `apply-batch` is an operator/local state-apply command. It is not a public
  wallet RPC method and must remain out of the wallet path.

## Target Architecture

```text
Browser wallet
  signs tx in WASM
  submits signed tx over WebSocket proxy
        |
        v
RPC write edge
  validates RPC envelope and rate limits
  admits signed tx to local mempool
  returns tx_id immediately
        |
        v
Validator consensus services
  proposer includes pending mempool txs
  validators re-execute and vote
  quorum certificate is formed
  certified block is applied and propagated
        |
        v
Wallet finality
  polls tx/receipts/account_tx/status
  shows pending -> finalized/failed
```

No validator private key crosses into the wallet, web server, or static web
frontend.

## Required Behavior

### Wallet UX

The web wallet must expose chain/write health separately from account balance:

- `Online/readable`: RPC status and read methods respond.
- `Writable`: RPC advertises or proves mempool submit is enabled.
- `Finalizing`: the chain height is advancing and submitted transactions can
  reach receipts.
- `Stalled`: RPC reads work, but height does not advance within the configured
  window.
- `Read-only`: RPC reads work, but write submit methods are disabled.

When the wallet cannot send, it must say why:

- "RPC is read-only; transaction submission is disabled."
- "Network is stalled at height N; transaction may remain pending."
- "Transaction admitted to mempool; waiting for validator finality."
- "Transaction finalized in block H."

It must not show a generic "offline" for every failure class.

### Python Helpers

The Python wallet helpers must have explicit modes:

| Mode | Intended use | Behavior |
| --- | --- | --- |
| `submit_only` | WAN/testnet wallet path | Sign and submit to RPC, return `tx_id`, no local finalization. |
| `submit_and_poll` | WAN/testnet wallet path | Sign, submit, poll `tx`/`receipts`/`account_tx` until finality or timeout. |
| `local_apply` | local harness only | Use `mempool-batch` plus `apply-batch` against local validator dirs. |
| `peer_certified` | controlled operator path | Use peer-certified transport with explicit topology/key files. |

The default for a network endpoint must be `submit_only` or
`submit_and_poll`, never `local_apply`.

### RPC Write Edge

`rpc-serve` must support a stable wallet-facing write profile:

```bash
postfiat-node rpc-serve \
  --data-dir <validator-data-dir> \
  --port 27650 \
  --bind-host 127.0.0.1 \
  --allow-mempool-submit \
  --max-mempool-submit-per-peer <N> \
  --max-mempool-submit-total <N> \
  --keep-alive
```

For public or internet-facing deployment, the RPC edge must sit behind the
existing WebSocket proxy or another rate-limited front door. Plain TCP
`rpc-serve` should not be treated as an unauthenticated public write endpoint
without an explicit operator decision.

### Consensus Services

The WAN devnet must run long-lived validator services, not rely on ad hoc
single-shot commands for normal wallet sends.

Each validator must provide:

- a process supervisor or systemd unit;
- restart-on-failure;
- logs;
- health endpoint or status command;
- voting/proposal service for the active topology;
- convergence monitoring for height, state root, and block tip.

A healthy six-validator WAN devnet must have at least five validators available
to vote and must continue producing blocks under normal conditions.

## Required Code Changes

### 1. Add Explicit Network Send Mode To Python Helpers

Update `python/postfiat_rpc/wallet.py`:

- Keep `send_pft(... finalize_data_dir=...)` working for local tests.
- Add or expose a first-class `send_pft_and_poll_finality(...)` helper.
- Add result fields:
  - `submit_mode`;
  - `pending`;
  - `finalized`;
  - `finality_receipt`;
  - `finality_timeout`.
- Ensure WAN examples and `scripts/pftl-transfer.py send` do not require
  `--finalize-data-dir`.

### 2. Add RPC Capability Discovery

Add a read method or extend `server_info` / `status` so clients can discover:

- `read_only`;
- `mempool_submit_enabled`;
- `mempool_submit_finality_enabled`;
- current `block_height`;
- `last_block_time` or equivalent block freshness signal;
- `mempool_pending`;
- rate limits.

This avoids guessing from failed submit attempts.

### 3. Make Wallet Submit Use Capability-Aware UX

Update `wallet-web`:

- call capability/status on load and periodically;
- show distinct read/write/finality health;
- block the Send button only for known impossible states;
- allow submit in writable-but-not-finalizing state only with explicit pending
  warning;
- after submit, poll finality by `tx_id`.

### 4. Remove WAN Guidance That Uses `apply-batch`

Documentation and scripts must state:

- `apply-batch` is local harness/operator tooling.
- WAN/testnet sends are signed RPC submits.
- Validator keys are only used by validator services, not wallets.

### 5. Fix Or Replace General Finality RPC

Choose one of these designs:

#### Option A: Persistent Consensus Loop Preferred

Run validator services continuously. RPC submit admits to mempool only. Finality
is produced by the normal consensus loop. Wallets poll for receipts.

This is closest to normal L1 operation and should be the default controlled
testnet target.

#### Option B: Proposer-Agnostic Finality RPC

Keep `mempool_submit_signed_transfer_finality`, but make it robust when the
local RPC node is not proposer:

- route/forward the signed transaction to the current proposer; or
- support a request to the proposer service through topology; or
- allow the local node to submit to mempool and wait for the proposer loop.

Do not disable proposer rotation rules or fake nonzero view evidence.

Option B is useful for demos, but it must not create a centralized sequencer
semantics by accident.

## Operational Fix For Current WAN Devnet

Before testing wallet sends, restore chain liveness:

1. Check all six validator hosts:
   - process running;
   - node status;
   - height;
   - state root;
   - block tip;
   - logs for vote/proposal errors.
2. Restart or resync any stalled validator.
3. Start the long-running peer-certified validator/consensus services.
4. Verify at least five validators are voting.
5. Verify height advances by at least two blocks without manual one-shot
   certification.
6. Restart or confirm RPC write edge with `--allow-mempool-submit`.
7. Submit one signed transfer through RPC and verify receipt finality.

## Stage Gates

Agents implementing this spec must mark each stage as one of:

- `not_started`
- `in_progress`
- `blocked`
- `complete`

Do not mark a stage `complete` unless the evidence listed for that stage exists
and has been inspected. Do not advance to wallet send testing while the fleet
or RPC write edge is marked `blocked`.

Use this status block at the top of any handoff or implementation report:

```text
wallet-wan-devnet-finality-fix status
Stage 0 - baseline audit: <status> - <evidence path or blocker>
Stage 1 - local harness preservation: <status> - <evidence path or blocker>
Stage 2 - WAN fleet liveness: <status> - <evidence path or blocker>
Stage 3 - RPC write edge: <status> - <evidence path or blocker>
Stage 4 - helper/API split: <status> - <evidence path or blocker>
Stage 5 - wallet UX/finality: <status> - <evidence path or blocker>
Stage 6 - regression and docs: <status> - <evidence path or blocker>
Final decision: <ship/hold> - <reason>
```

### Stage 0: Baseline Audit

Purpose: establish current facts before changing code or restarting services.

Entry criteria:

- clean or intentionally documented working tree;
- current branch recorded;
- current WAN RPC endpoint known.

Required actions:

- Query WAN RPC `status` twice over a short interval.
- Record block height, block tip, state root, `last_run_unix`,
  `mempool_pending`, `chain_id`, and `validator_count`.
- Classify current network state as:
  - `read_unreachable`;
  - `readable_but_stalled`;
  - `readable_and_advancing`;
  - `writable_unknown`;
  - `writable_enabled`;
  - `writable_disabled`.
- Identify whether any currently running wallet/proxy service is pointing at
  WAN devnet or local devnet.

Evidence:

- JSON report under an implementation run directory, for example:
  `reports/wallet-wan-devnet-finality/stage0-baseline-<timestamp>.json`.
- Include exact command lines or script names used to collect the report.

Pass rule:

- Stage passes when the report exists and clearly states whether the chain is
  advancing.

Agent note:

- If height is stalled, stop repeated wallet send attempts. Move to Stage 2.
- If RPC is read-only, do not treat wallet failures as frontend bugs yet.

### Stage 1: Local Harness Preservation

Purpose: prove existing local harness behavior still works while WAN behavior
is being corrected.

Entry criteria:

- Stage 0 complete.

Required actions:

- Run `cargo fmt --check`.
- Run `cargo check --workspace`.
- Run Python wallet helper tests.
- Run or preserve a local devnet test proving `local_apply` mode still uses
  local validator dirs intentionally.

Evidence:

- Test output summary.
- Any local devnet report path used.

Pass rule:

- Rust check passes.
- Python helper tests pass.
- Local apply behavior remains explicitly local-only.

Agent note:

- Do not remove `apply-batch`; quarantine it to local/operator flows.

### Stage 2: WAN Fleet Liveness

Purpose: restore normal validator block production before wallet testing.

Entry criteria:

- Stage 0 says WAN devnet is stalled or liveness is unproven.

Required actions:

- Check each validator host in the active topology:
  - process status;
  - latest height;
  - state root;
  - block tip;
  - validator key identity;
  - recent logs;
  - vote/proposal activity.
- Restart or resync stalled validators.
- Start long-running validator/peer-certified consensus services.
- Observe the fleet long enough to prove height advances without manual
  `apply-batch`.

Evidence:

- Per-validator health table.
- Logs or service status showing running validator processes.
- A convergence report with at least five validators on the same height/root.
- A liveness report showing height advanced by at least two blocks.

Pass rule:

- At least five of six validators are voting or otherwise available for quorum.
- Height advances without manual `apply-batch`.
- Voting validators converge on the same root/tip.

Blocked rule:

- Mark `blocked` if fewer than five validators can be restored, keys are
  missing, hosts are unreachable, or state roots cannot converge.

Agent note:

- This is the first hard gate. Do not run browser wallet send demos while this
  stage is not `complete`.

### Stage 3: RPC Write Edge

Purpose: expose a bounded wallet-safe transaction submission path.

Entry criteria:

- Stage 2 complete.

Required actions:

- Start or restart RPC with `--allow-mempool-submit`.
- Keep rate limits explicit:
  - `--max-mempool-submit-per-peer`;
  - `--max-mempool-submit-total`.
- If using a WebSocket proxy, verify the proxy points at the write-enabled RPC.
- Add or verify an RPC capability response that tells clients whether write
  submission is enabled.
- Submit a deliberately invalid signed transfer and confirm it is rejected with
  a clear validation error.
- Submit a valid signed transfer and confirm it is admitted.

Evidence:

- RPC startup command or service unit.
- RPC capability/status JSON.
- Rejection report for invalid transaction.
- Admission report for valid transaction.

Pass rule:

- Read methods respond.
- Write capability is discoverable.
- Invalid signed input is rejected.
- Valid signed input is admitted without validator keys in the client.

Blocked rule:

- Mark `blocked` if write methods are disabled, hidden behind a read-only proxy,
  or unbounded/unrate-limited.

Agent note:

- Mempool admission is not finality. Do not mark Stage 5 complete from Stage 3
  evidence.

### Stage 4: Helper/API Split

Purpose: make it impossible for normal WAN helper paths to silently fall back
to local `apply-batch`.

Entry criteria:

- Stage 1 complete.
- Stage 3 complete or mocked in tests.

Required actions:

- Add explicit send modes:
  - `submit_only`;
  - `submit_and_poll`;
  - `local_apply`;
  - `peer_certified` if still needed for operator flows.
- Ensure `scripts/pftl-transfer.py send` defaults to submit/poll behavior for
  RPC endpoints.
- Ensure `local_apply` requires explicit local validator dir flags.
- Add tests proving WAN/testnet send mode does not call `apply-batch`.
- Add tests proving `local_apply` still calls the local harness path only when
  explicitly requested.

Evidence:

- Unit test output.
- Code references for send mode dispatch.
- Example command lines for WAN send and local harness send.

Pass rule:

- Tests prove mode separation.
- WAN send examples require no validator key paths and no validator data dirs.

Blocked rule:

- Mark `blocked` if the only working send helper still requires local
  `apply-batch`.

Agent note:

- This stage is about API correctness. It does not prove live network finality
  by itself.

### Stage 5: Wallet UX And Finality

Purpose: prove the browser wallet can send through the correct network path.

Entry criteria:

- Stage 2 complete.
- Stage 3 complete.
- Stage 4 complete.

Required actions:

- Update wallet-web to show separate:
  - readable status;
  - writable status;
  - finalizing/height-advancing status;
  - stalled/read-only reasons.
- Create or import a browser wallet.
- Submit a PFT transfer from the browser without validator keys.
- Show pending state after mempool admission.
- Poll finality by `tx_id`.
- Show finalized receipt and updated balances.
- Capture browser console/page errors.

Evidence:

- Playwright report or equivalent browser automation output.
- Screenshot or DOM assertion for readable/writable/finalizing state.
- Submitted `tx_id`.
- Receipt/finality report.
- Before/after balances.

Pass rule:

- Browser wallet send reaches finalized receipt.
- UI does not call the transaction finalized until receipt/finality exists.
- Zero page errors.

Blocked rule:

- Mark `blocked` if the wallet can only submit but finality never arrives.
- Mark `blocked` if the UI requires validator keys, local data dirs, or manual
  `apply-batch`.

Agent note:

- This is the second hard gate. Do not say the wallet is fixed until this stage
  is complete.

### Stage 6: Regression And Docs

Purpose: lock the corrected architecture into tests and docs.

Entry criteria:

- Stage 5 complete.

Required actions:

- Update `docs/specs/web-wallet.md` to reference this finality model.
- Update runbooks that currently imply WAN sends should use `apply-batch`.
- Add regression tests for:
  - read-only RPC status;
  - stalled chain status;
  - WAN send mode not using local validator dirs;
  - wallet UI status text for read-only/stalled/finalized.
- Run required checks:
  - `cargo fmt --check`;
  - `cargo check --workspace`;
  - Python wallet tests;
  - wallet-web build;
  - MkDocs strict build.

Evidence:

- Test command outputs.
- Updated doc paths.
- Final implementation summary with all stage statuses.

Pass rule:

- All required checks pass.
- The final report marks Stage 0 through Stage 6 `complete`.

Agent note:

- This is the release hygiene gate. If it is skipped, future agents will repeat
  the `apply-batch` confusion.

## Security And Correctness Requirements

- User private keys remain browser/Python-wallet local.
- Validator keys remain on validator hosts or controlled operator machines.
- RPC admission must validate signatures before mempool acceptance.
- RPC write paths must be rate-limited and bounded by request size.
- Finality polling must not treat mempool admission as finality.
- Stalled-chain detection must not fabricate success.
- Proposer rotation must remain protocol-correct.
- Nonzero view use must require real timeout-certificate evidence.

## Rollout Plan

### Phase 1: Documentation And Health Diagnosis

- Land this spec.
- Add a WAN devnet health runbook.
- Produce a current fleet health report.
- Do not run repeated wallet send tests until fleet liveness is restored.

### Phase 2: Restore WAN Devnet Services

- Deploy/restart long-running validator services.
- Verify block production and convergence.
- Enable a bounded RPC write edge.
- Record liveness and write-capability evidence.

### Phase 3: Helper And Wallet Fixes

- Add explicit Python send modes.
- Add RPC capability discovery.
- Update wallet-web status model and send flow.
- Add tests that prevent WAN paths from using `apply-batch`.

### Phase 4: End-To-End Evidence

- Run two consecutive wallet sends through the browser.
- Run two Python `scripts/pftl-transfer.py send` transfers in submit/poll mode.
- Verify receipts and balances.
- Archive reports under the normal evidence workflow.

## Non-Goals

- Public mainnet decentralization claims.
- Requiring independent external validators for controlled-testnet readiness.
- Giving wallets validator keys.
- Turning the RPC service into a custodial signer.
- Bypassing validator re-execution or quorum certificates.

## Decision

The canonical wallet path is signed RPC submission plus validator finality. The
`apply-batch` path remains a local harness tool only. The immediate blocker is
WAN devnet liveness and RPC write-edge configuration, followed by helper and UX
changes that make the correct path explicit and prevent accidental fallback to
local validator-control assumptions.

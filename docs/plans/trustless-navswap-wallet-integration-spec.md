# Trustless NAVSwap Wallet Integration Spec

Date: 2026-06-29
Repo scope: `postfiatl1v2` live wallet, wallet proxy/RPC, and StakeHub NAVSwap orchestration
Status: implementation spec

## Executive Summary

The live wallet already has a `Swap` screen with route scaffolding for
transparent, private, and OTC swaps. StakeHub already has extensive NAVSwap
work: transparent PFTL-only NAV roundtrips, shielded `pfUSDC -> a651`
Asset-Orchard swaps, direct private egress, public bridge-out, receipt
rendering, and non-browser E2E scripts.

The missing product work is not "invent NAVSwap". It is to move the proven
StakeHub flow behind a wallet-safe interface:

```text
wallet-local custody and signing
  -> wallet-facing NAVSwap adapter
  -> untrusted proof/relay/orchestration services
  -> PFTL validators, Asset-Orchard, NAV proof state, and bridge settlement
```

The browser wallet must remain the custody boundary. StakeHub can supply
implementation evidence, long-running proof runners, route status, and relay
orchestration, but a production wallet route must not depend on StakeHub
operator key files, demo wallet state, global dashboard status, or server-side
authority over the user's destination, amount, asset, note, or spend.

Trustless here means:

- the wallet signs all user spends locally;
- private note openings, spend keys, view keys, seed material, passphrases, and
  owner key files never leave the wallet boundary;
- the adapter can relay and prove, but cannot steal funds or silently change
  the transaction the user approved;
- the wallet verifies NAV proof freshness, asset IDs, route terms, receipts,
  validator certificates, and privacy disclosures before it marks the swap
  complete.

## Repos Researched

### StakeHub

Relevant files:

- `StakeHub/docs/current-sprint/shielded-nav-swap-agent-handoff.md`
- `StakeHub/docs/current-sprint/end-to-end-shielding-privacy-requirement.md`
- `StakeHub/docs/status/shielded-nav-swap-audit.md`
- `StakeHub/dashboard/BACKEND_SPEC.md`
- `StakeHub/scripts/shielded-nav-swap-e2e-live.py`
- `StakeHub/scripts/shielded-navswap-ux-live.py`
- `StakeHub/stakehub/dashboard_server.py`

Findings:

- StakeHub exposes a loopback operator dashboard API:
  - `GET /api/shielded-nav-swap/nav-check?phase=before|after`
  - `GET /api/shielded-nav-swap/balances`
  - `GET /api/shielded-nav-swap/snapshot?phase=before|after`
  - `GET /api/shielded-nav-swap/status`
  - `POST /api/shielded-nav-swap/action`
- Implemented action names include `bridge_in`, `shield_ingress`, `prewarm`,
  `shield_swap`, `private_egress`, `bridge_out`, and
  `transparent_roundtrip`.
- The current private route shape is:

```text
public USDC or pfUSDC funding
  -> public PFTL pfUSDC
  -> public ingress into Asset-Orchard
  -> private pfUSDC note
  -> private pfUSDC <-> a651 swap
  -> private a651 note
  -> direct private egress
  -> public a651 exit receipt
  -> bridge_out public NAV exit, redeem, withdraw, and settlement
```

- The direct private-egress primitive exists and has produced live receipts.
- StakeHub runs forbidden-field scans to fail public artifacts that expose note
  openings or private key material.
- StakeHub docs correctly warn that direct private egress hides the spent note
  opening, but still reveals public destination, public asset, public amount,
  fee, policy/disclosure fields, nullifier, anchor, proof material, and timing.
- The current dashboard API is not itself wallet-ready because it is an
  operator workflow. Several actions read local demo wallet key files and
  synchronous/global status, rather than taking wallet-signed user payloads.

### postfiatl1v2

Relevant files:

- `wallet-web/src/components/Swap.jsx`
- `wallet-web/src/lib/swap-server.js`
- `wallet-web/src/lib/utils.js`
- `docs/plans/proper-private-nav-swap-plan.md`
- `docs/runbooks/private-nav-otc-shielded-swap-wan-devnet.md`
- `docs/runbooks/shielded-swap-performance-handoff.md`
- `docs/specs/asset-orchard-swap-circuit-design-v2.md`
- `docs/status/xrpl-feature-parity-burndown.md`
- `docs/python/xrp-style-transactions.md`
- `docs/python/wallet-functions.md`
- `docs/rpc/methods.md`
- `docs/navcoins/uniswap-pool.md`

Findings:

- The wallet has a `Swap` component with route choices for transparent,
  private, and OTC.
- The wallet already imports canonical `pfUSDC` and `a651` asset IDs.
- The wallet's current transparent handler is not a NAVSwap. It builds a simple
  issued-asset transfer to the user's own address.
- The wallet's private route calls generic `/api/swap/*` paths, while StakeHub
  implements `/api/shielded-nav-swap/*`.
- `swap-server.js` states the right custody rule: the swap server must never
  receive the user's seed, passphrase, or private keys. The spec below makes
  that rule enforceable at the protocol boundary.
- L1 docs distinguish the transparent economic roundtrip from the shielded
  local proof path. The transparent full roundtrip has been measured around
  two minutes; shielded proving can be fast only through warm long-lived prover
  paths, not cold one-shot CLIs.
- The existing "atomic swap" implementation is `ESCROW-009`: deterministic
  two-sided atomic settlement templates for PFT/issued-asset swaps. It uses
  reciprocal escrow-create legs with one shared condition hash, symmetric
  settlement ids, and the same fulfillment to finish both legs. It is exposed
  through CLI/RPC as `atomic-settlement-template` /
  `atomic_settlement_template` and through Python as
  `build_atomic_swap_template(...)`.
- The current atomic settlement template is same-chain PFTL escrow plumbing. It
  does not by itself bridge to Ethereum or execute Uniswap. It also currently
  validates exactly one native `PFT` leg and one issued-asset leg, so NAVCoin
  issued-asset-to-issued-asset atomic swaps need either a P2 extension or an
  explicit PFT intermediary.
- The live Ethereum `a651/USDC` pool is a historical launch venue, not the
  final PFTL canonical bridge. `docs/navcoins/uniswap-pool.md` states that it
  is not a live cross-chain bridge for a651 and that the pool is not the PFTL
  NAV/supply ledger.

### Canonical NAVCoin / Uniswap Research

Relevant files:

- `postfiatorg.github.io/content/research/canonical-navcoin-transaction.md`
- `postfiatorg.github.io/content/research/trustless-pftl-uniswap-bridges.md`
- `StakeHub/docs/navcoin-uniswap-launch-plan.md`
- `StakeHub/zk/contracts/src/navcoin/NavCoin.sol`
- `StakeHub/zk/contracts/src/navcoin/NavBridgeController.sol`
- `StakeHub/zk/contracts/src/navcoin/NavCoinV4LaunchHelper.sol`
- `postfiatl1v2/docs/status/arbitrum-contracts-code-review-2026-06-19.md`

Findings:

- The canonical NAVCoin architecture says PFTL is the source of truth and
  Ethereum is a venue. Wrapped Ethereum NAVCoin should exist only after a
  verified PFTL packet.
- The canonical transaction notes explicitly require a new bridge-aware
  Ethereum wrapped NAVCoin and a new Uniswap pool that trades the wrapped token
  released by the verified/atomic handoff path, not the old standalone token.
- The current StakeHub `NavCoin` can lock its controller; the deployed a651
  token/controller should be treated as a legacy boundary, not something to
  repoint into a PFTL bridge.
- Current `NavBridgeController` supports owner-gated remote burn/mint
  authorization, not verifier-driven PFTL finality. It is useful launch
  scaffolding but not a trustless PFTL bridge controller.
- `NAVGuardHook.sol` in the L1 repo was reviewed as "Uniswap-v4-shaped", not a
  real v4 hook. A production pool must either use a hookless pool plus a
  separate verified bridge controller, or implement a real Uniswap v4 hook with
  faithful v4 callback tests.

## Product Target

The live wallet should offer four route families, with the first two usable
before exposing any advanced OTC flow:

### Route A: Transparent NAVSwap

Use when the user prioritizes determinism, auditability, and simpler failure
recovery over privacy.

```text
public pfUSDC balance
  -> public primary mint / NAV money-in
  -> public a651 balance
  -> optional public NAV exit
  -> optional pfUSDC burn-to-redeem / withdraw / settle
```

Properties:

- public asset, amount, sender, destination, NAV checkpoint, and receipts;
- no Asset-Orchard notes;
- wallet signs all user-owned PFTL operations locally;
- adapter may relay certified batches and bridge operations;
- useful as the first wallet integration target because it proves end-to-end
  NAV accounting without private-note custody.
- primary NAV money-in is not fixed inventory: the user pays a counted
  settlement asset such as `pfUSDC`, the route prices from the finalized
  pre-inflow NAV checkpoint, mints fractional native NAVCoin to the user, and
  only then adds the user's settlement inflow to reserves;
- a large primary subscription, for example 100,000 USDC-equivalent at a
  finalized 1,000 USDC/NAV checkpoint, fills as about 100 NAVCoin before fees
  and deterministic rounding. The user's own inflow must not raise the NAV
  price used for that same fill.

### Route B: Shielded NAVSwap

Use when the user wants the private Asset-Orchard middle.

```text
public pfUSDC balance
  -> public Asset-Orchard ingress
  -> private pfUSDC note
  -> private pfUSDC <-> a651 swap
  -> private a651 note
  -> hold privately, private-transfer later, or direct private egress
  -> public a651 exit
  -> optional bridge_out
```

Properties:

- public ingress reveals asset, amount, and timing;
- private middle hides raw note owner, recipient, asset IDs, values, and
  bilateral price from the public swap action;
- direct private egress hides the spent note opening but reveals public exit
  destination, asset, amount, fee, nullifier, anchor, proof material, policy
  fields, and timing;
- not timing-anonymous and not amount-anonymous in v1;
- wallet owns note scanning, note selection, spend authorization, and private
  witness handling.

### Route C: PFTL Atomic Settlement

Use when two PFTL wallets want a same-chain atomic swap using the existing
escrow rails.

```text
wallet A creates escrow leg A
wallet B creates escrow leg B
both legs share one condition hash and settlement id
same fulfillment finishes both legs
cancel_after lets each side recover if the swap does not complete
```

Properties:

- implemented as `ESCROW-009` deterministic atomic settlement templates;
- current template supports exactly one native `PFT` leg and one issued-asset
  leg;
- template building does not sign or submit anything;
- each wallet must verify the two legs, approve its own escrow-create
  operation, and later approve/submit the finish operation;
- the wallet must show both escrow ids, the shared condition hash, finish
  height, cancel height, assets, amounts, and counterparties before signing.

### Route D: PFTL-to-Uniswap Atomic Handoff

Use when a user wants native PFTL NAVCoin to become a Uniswap-tradable
Ethereum venue representation without inventory fronting.

The target route is:

```text
pfUSDC or another counted PFTL settlement asset
  -> public primary NAV money-in at finalized pre-inflow NAV
  -> native PFTL NAVCoin
  -> public PFTL lock/debit under a bridge packet
  -> finalized PFTL receipt
  -> Ethereum verifier / atomic handoff controller
  -> bridge-aware wrapped NAVCoin
  -> new Uniswap pool for wrapped NAVCoin / USDC
```

Properties:

- the wallet must not target the existing legacy `a651/USDC` pool for this
  trustless route;
- a new bridge-aware wrapped NAVCoin token and new Uniswap pool are required;
- the wrapped token mints or releases only after verified PFTL packets;
- Uniswap is execution only, not the source of NAV truth;
- Uniswap buys and sells transfer existing wrapped NAVCoin from pool or
  market-maker liquidity. They do not mint canonical NAVCoin supply;
- pool seed wrapped NAVCoin must come from canonical primary NAV issuance plus
  bridge export, with the seed packet and reserve checkpoint bound into the
  route config. It must not be unexplained operator inventory;
- the handoff must have timeout/refund semantics: either the Ethereum packet is
  consumed or the PFTL-side lock/debit becomes safely refundable, but not both;
- optional `mint_and_swap_uniswap` must bind router/path or pool id, token in,
  token out, amount in, minimum output, recipient, deadline, and failure
  behavior.

## Non-Goals

- Do not embed the StakeHub operator dashboard directly in the wallet.
- Do not let a remote service hold user wallet key files.
- Do not call a route "private end-to-end cash-out" unless the executed route
  produced a private-egress receipt and the UI labels all public boundary
  disclosures.
- Do not make the server responsible for choosing the user's final destination
  after approval.
- Do not present `bridge_out` as private. It is the public post-egress or
  disclosed fallback leg.
- Do not block the first transparent integration on the full shielded proving
  performance plan.
- Do not call the legacy Ethereum `a651/USDC` pool the trustless PFTL Uniswap
  route. It is a secondary-market venue and historical launch artifact.
- Do not claim ESCROW-009 solves cross-chain PFTL-to-Uniswap handoff by itself.
  It is a same-chain reciprocal escrow template. The Uniswap handoff needs
  bridge packet verification, replay protection, and refund safety.

## Trust Boundaries

### Wallet Boundary

The wallet owns:

- seed/passphrase handling;
- account keys and owner keys;
- Asset-Orchard spending/viewing material;
- private notes and encrypted note store;
- note scanning using local view material;
- local signing of PFTL account operations and owned/spend actions;
- user approval screens;
- verification of prepared actions before signing;
- verification of receipts and certificates after submission.

### NAVSwap Adapter Boundary

The adapter may be implemented in wallet proxy, a companion local service, or a
new server that wraps StakeHub/L1 tooling. It may:

- quote routes;
- expose NAV proof snapshots;
- expose liquidity availability;
- prepare unsigned action plans;
- prewarm proving material;
- relay signed actions to validators;
- poll validator certificates and bridge receipts;
- stream run-scoped status to the wallet;
- store public receipts and run artifacts.

The adapter must not:

- receive seed material, passphrases, account private keys, owner key files,
  spend authorization keys, viewing keys, note openings, or unencrypted private
  note files;
- mutate route terms after user approval;
- mark a run complete without wallet-verifiable receipts;
- hide whether a step is public, private, or disclosed.

### Proof Runner Boundary

For shielded actions, proof generation touches private witness data. The safe
default is a local wallet-controlled prover:

- browser WASM/WebWorker if performance is acceptable;
- local helper daemon controlled by the wallet if browser proving is too slow;
- remote proof service only for public or already-blinded work that does not
  reveal private witnesses.

If a proof job requires note openings or spend secrets to leave the wallet, the
route is custodial and must be disabled in the trustless wallet UI.

### Relay and Validator Boundary

Relays and validators are not trusted for custody. Validators verify consensus
rules, proofs, nullifiers, retained anchors, NAV invariants, and certificates.
The wallet verifies final receipts and quorum evidence.

## Wallet-Facing API

The wallet should not call StakeHub's operator endpoints directly. Add a
wallet-facing NAVSwap adapter with run-scoped resources.

Base path:

```text
/api/navswap
```

### Capabilities

```http
GET /api/navswap/capabilities
```

Returns:

```json
{
  "ok": true,
  "routes": [
    "transparent_navswap",
    "shielded_navswap",
    "pftl_atomic_settlement",
    "uniswap_atomic_handoff"
  ],
  "assets": {
    "pfUSDC": {"asset_id": "...", "decimals": 6},
    "a651": {"asset_id": "...", "decimals": 6},
    "wrapped_navcoin": {
      "chain_id": 1,
      "token": null,
      "status": "requires_bridge_aware_redeployment"
    }
  },
  "atomic_settlement": {
    "template_schema": "postfiat-atomic-settlement-template-v1",
    "supported_pairs": ["PFT/issued_asset"],
    "issued_issued_supported": false
  },
  "uniswap_handoff": {
    "legacy_a651_pool_supported": false,
    "requires_new_bridge_aware_token": true,
    "requires_new_pool": true,
    "verifier_status": "not_deployed"
  },
  "prover": {
    "local_required_for_private_witness": true,
    "remote_private_witness_allowed": false,
    "warm": false
  },
  "feeds": ["sse", "websocket"],
  "chain_id": "postfiat-wan-devnet",
  "validator_count": 6,
  "bft_quorum": 5
}
```

### Atomic Settlement Template

```http
POST /api/navswap/atomic-templates
```

This wraps the existing PFTL `atomic_settlement_template` RPC. It is for
same-chain reciprocal escrow swaps, not Uniswap execution.

The wallet adapter accepts whole-number amounts/heights as strings from the
browser, normalizes them to safe integers for the current RPC, then calls the
RPC twice: once with the displayed legs and once with the legs swapped. The
adapter response is rejected unless the swapped RPC result preserves the
`settlement_id` and `condition_hash` and swaps the two escrow ids cleanly.

Request:

```json
{
  "left_owner": "pf...",
  "left_recipient": "pf...",
  "left_asset_id": "PFT",
  "left_amount": "1000000",
  "right_owner": "pf...",
  "right_recipient": "pf...",
  "right_asset_id": "asset-id...",
  "right_amount": "250000",
  "condition": "wallet-generated-secret-or-hashlock-label",
  "finish_after": 0,
  "cancel_after": 123456
}
```

Response:

```json
{
  "ok": true,
  "schema": "postfiat-navswap-atomic-template-v1",
  "verification": {
    "schema": "postfiat-atomic-settlement-template-v1",
    "settlement_id": "...",
    "condition_hash": "...",
    "left_escrow_id": "...",
    "right_escrow_id": "..."
  },
  "symmetry": {
    "schema": "postfiat-navswap-atomic-template-symmetry-v1",
    "stable": true,
    "settlement_id": "...",
    "condition_hash": "...",
    "left_escrow_id": "...",
    "right_escrow_id": "..."
  },
  "result": {
    "schema": "postfiat-atomic-settlement-template-v1",
    "left": {
      "escrow_id": "...",
      "operation": {}
    },
    "right": {
      "escrow_id": "...",
      "operation": {}
    }
  }
}
```

Wallet requirements:

- reject the template unless both legs match the user's displayed terms;
- reject the template unless adapter symmetry verification is present and
  stable;
- reject unsupported pairs unless the L1 template has been extended beyond
  exactly-one-PFT validation;
- ask each wallet to sign only its own escrow-create operation;
- after both escrow creates are accepted, reveal or submit the fulfillment only
  through the approved finish flow;
- show cancel/recovery height before either side signs.

### Uniswap Handoff Quote

```http
POST /api/navswap/uniswap-handoff/quotes
```

This is the trustless route from native PFTL NAVCoin into a new bridge-aware
Uniswap venue representation. It must not quote against the legacy standalone
`a651/USDC` pool as if that pool were bridge-aware.

Request:

```json
{
  "source_asset_id": "pftl-navcoin-asset-id",
  "amount_atoms": "1000000",
  "destination_chain_id": 1,
  "destination_address": "0x...",
  "destination_action": "mint_only",
  "swap": null
}
```

For a one-transaction Ethereum mint-and-swap, `destination_action` becomes
`mint_and_swap_uniswap` and `swap` must bind:

```json
{
  "router": "0x...",
  "token_in": "bridge-aware-wrapped-navcoin",
  "token_out": "USDC",
  "pool_id_or_path_hash": "0x...",
  "amount_in": "1000000",
  "min_amount_out": "990000",
  "recipient": "0x...",
  "deadline": 1782691200
}
```

The quote must return the exact bridge packet fields, verifier mode, expiry,
refund parameters, and whether the route is disabled because the bridge-aware
token/pool has not been redeployed.

### NAV Proof Snapshot

```http
GET /api/navswap/nav-proof?asset_id=a651
```

Returns the wallet-verifiable NAV summary:

- chain ID;
- current PFTL height;
- NAV epoch;
- reserve packet hash;
- freshness deadline height;
- NAV per unit;
- supply;
- proof status;
- source receipt hashes.

The wallet must reject stale or missing NAV proof data before signing a NAVSwap
route.

### Quote

```http
POST /api/navswap/quotes
```

Request:

```json
{
  "route": "transparent_navswap",
  "from_asset": "pfUSDC",
  "to_asset": "a651",
  "amount_atoms": "1000000",
  "wallet_address": "pf...",
  "destination_ref": "pftl:pf...",
  "max_slippage_bps": 0
}
```

Response:

```json
{
  "ok": true,
  "quote_id": "navq_...",
  "expires_at_height": 123456,
  "route": "transparent_navswap",
  "from_asset_id": "8751...",
  "to_asset_id": "dcdd...",
  "input_atoms": "1000000",
  "expected_output_atoms": "196850",
  "fees_atoms": "0",
  "nav_proof_hash": "sha256:...",
  "reserve_packet_hash": "...",
  "privacy_label": "public",
  "disclosures": ["sender", "destination", "asset", "amount", "timing"],
  "requires_wallet_signatures": true
}
```

Quotes are not spend authority. They are route terms the wallet must bind into
the signed action request.

### Create Run

```http
POST /api/navswap/runs
```

Request:

```json
{
  "quote_id": "navq_...",
  "wallet_address": "pf...",
  "idempotency_key": "wallet-generated-uuid"
}
```

Response:

```json
{
  "ok": true,
  "run_id": "navrun_...",
  "status": "waiting_user_signature",
  "route": "transparent_navswap",
  "current_step": "prepare_primary_mint",
  "events_url": "/api/navswap/runs/navrun_.../events"
}
```

### Prepare Step

```http
POST /api/navswap/runs/{run_id}/steps/{step_id}/prepare
```

The adapter returns a `WalletActionRequest`:

```json
{
  "ok": true,
  "run_id": "navrun_...",
  "step_id": "prepare_primary_mint",
  "action_request": {
    "schema": "pftl.wallet_action_request.v1",
    "chain_id": "postfiat-wan-devnet",
    "quote_id": "navq_...",
    "route": "transparent_navswap",
    "kind": "pftl_asset_operation",
    "canonical_bytes_hex": "...",
    "canonical_hash": "sha256:...",
    "display": {
      "from_asset": "pfUSDC",
      "to_asset": "a651",
      "input_atoms": "1000000",
      "expected_output_atoms": "196850",
      "destination": "pf...",
      "privacy_label": "public"
    },
    "must_verify": {
      "from_asset_id": "8751...",
      "to_asset_id": "dcdd...",
      "reserve_packet_hash": "...",
      "expires_at_height": 123456
    }
  }
}
```

The wallet must reconstruct or parse the canonical request, compare every
displayed field to the quote and user input, then request user approval.

### Submit Signed Step

```http
POST /api/navswap/runs/{run_id}/steps/{step_id}/submit
```

Request:

```json
{
  "wallet_action_hash": "sha256:...",
  "signatures": [
    {
      "address": "pf...",
      "public_key_hex": "...",
      "signature_hex": "..."
    }
  ],
  "proofs": [],
  "idempotency_key": "wallet-generated-uuid"
}
```

Response:

```json
{
  "ok": true,
  "run_id": "navrun_...",
  "step_id": "prepare_primary_mint",
  "status": "submitted",
  "tx_ids": ["..."],
  "receipt_ids": ["..."]
}
```

### Run Status

```http
GET /api/navswap/runs/{run_id}
GET /api/navswap/runs/{run_id}/receipts
GET /api/navswap/runs/{run_id}/events
GET /api/navswap/runs/{run_id}/stream
```

The `/stream` endpoint is the Server-Sent Events feed for run progress. It
emits an initial `navswap_run_snapshot`, zero or more `navswap_run_update`
events, and a terminal `navswap_run_done` event. `/events` remains the JSON
history endpoint, and WebSocket can be added later as an equivalent transport.
The wallet should not have to refresh to see progress.

Run statuses:

```text
created
waiting_user_signature
queued
running
submitted
certified
finalized
action_needed
failed
cancelled
complete
```

Every status payload must include:

- `run_id`;
- `route`;
- `current_step`;
- step list with public/private/disclosed labels;
- last updated timestamp;
- current chain height when known;
- receipt/certificate hashes when known;
- machine-readable error code when failed;
- human-readable message safe for the wallet.

## L1/RPC Requirements

The wallet-facing adapter needs L1/RPC surfaces that do not assume local key
files.

Required read APIs:

- account state and PFTL gas balance;
- issued asset balances for `pfUSDC` and `a651`;
- NAV proof status by asset ID;
- reserve packet hash, NAV epoch, freshness deadline, and supply;
- retained Orchard anchors;
- encrypted Asset-Orchard outputs for local note scanning;
- nullifier set or note status lookup;
- transaction receipts and block certificates;
- bridge status and settlement receipts.
- account escrows by owner/recipient/state;
- escrow info by escrow id;
- atomic settlement template construction;
- registered PFTL-to-Ethereum NAVCoin bridge routes, destination token
  mappings, verifier mode, route caps, and route pause status.

Required prepare APIs:

- prepare transparent NAV primary mint/subscription action;
- prepare public NAV exit action;
- prepare burn-to-redeem action;
- prepare bridge withdrawal/settlement action;
- prepare Asset-Orchard ingress action;
- prepare Asset-Orchard swap action;
- prepare Asset-Orchard private-egress action.
- prepare PFTL atomic escrow-create and escrow-finish actions from
  `atomic_settlement_template`;
- prepare PFTL NAVCoin bridge lock/debit packet for Uniswap handoff;
- prepare safe refund transaction for an expired unconsumed handoff packet.

Required submit APIs:

- submit signed account asset operations;
- submit signed shielded actions or shielded batches;
- relay peer-certified batch rounds;
- submit bridge-out/resume artifacts;
- query and stream finality.
- submit signed escrow transactions;
- submit handoff packets to the Ethereum verifier/bridge controller;
- query Ethereum handoff consume/burn events and PFTL inbound return status.

All prepare APIs must return canonical unsigned bytes plus display-safe decoded
fields. All submit APIs must accept signatures/proofs from the wallet. No
prepare or submit API may require a user owner key file path.

## Wallet State Model

Add a wallet-side NAVSwap state module with these entities:

```text
NavSwapQuote
NavSwapRun
NavSwapStep
WalletActionRequest
WalletActionApproval
NavSwapReceipt
OrchardNote
OrchardSpendLock
```

The note store must track:

- note commitment;
- encrypted output reference;
- asset tag/code after local viewing;
- amount after local viewing;
- owner/view metadata;
- retained anchor;
- spendability;
- pending spend lock;
- nullified/spent state;
- egressed state;
- receipt correlation.

The pending spend lock is mandatory. It prevents the wallet from creating a
second order against the same local note while the first signed action is
pending. This is the wallet-side equivalent of avoiding the "locked by a
different order" failure seen in owned-object FastPay flows.

## Shielded Route Details

The shielded route requires more than a single user's input note. A private
`pfUSDC -> a651` swap consumes private typed notes and emits replacement notes.
Production liquidity must therefore come from one of:

- a market maker's pre-authorized private liquidity note;
- a pool-managed note set controlled by a service key under explicit pool
  policy;
- a bilateral RFQ where both sides sign/prove compatible spends;
- an operator route kept behind an "operator demo" label.

The live wallet must not pretend the user's wallet alone can execute arbitrary
shielded swaps without liquidity and counterparty authorization. The adapter's
quote must identify the liquidity source, policy hash, expiry, and failure mode
without revealing private note openings.

### Shielded Step Sequence

Minimum v1 wallet sequence:

```text
1. Verify wallet unlock and chain capability.
2. Fetch NAV proof snapshot.
3. Fetch quote and liquidity availability.
4. Prepare public pfUSDC ingress.
5. Wallet signs ingress.
6. Adapter relays ingress and returns receipt.
7. Wallet scans encrypted output and confirms private pfUSDC note.
8. Local prover prepares private swap proof using wallet note witness.
9. Wallet signs spend authorization.
10. Adapter relays shielded swap batch and returns certificate.
11. Wallet scans replacement output and confirms private a651 note.
12. User chooses hold-private, direct private egress, or public bridge_out.
13. If egress: local prover creates private-egress proof.
14. Wallet signs/approves public exit destination and disclosure hash.
15. Adapter relays private egress and returns receipt.
16. If bridge_out: adapter prepares public bridge leg from private-egress
    public exit receipt; wallet approves any user-owned public spend.
17. Wallet verifies final receipts and NAV after snapshot.
```

### Privacy Labels

The wallet must label every step:

| Step | Label | Required wording |
|---|---|---|
| Public pfUSDC funding | public | Reveals asset, amount, source, destination, timing |
| Asset-Orchard ingress | public boundary | Burns public asset and creates private note |
| Asset-Orchard swap | private middle | Hides note owner, raw asset IDs, amounts, recipients, and price from the public action |
| Direct private egress | private egress | Hides spent note opening; reveals public destination, asset, amount, fee, nullifier, anchor, proof material, and timing |
| Bridge out | public | Public NAV exit, redeem, withdraw, and settlement |
| Disclosed fallback | disclosed egress | Reveals note facts; use only as explicit fallback |

## Transparent Route Details

The transparent route is the first integration milestone because it can be made
trustless without solving browser private-note proving.

Minimum v1 wallet sequence:

```text
1. Fetch balances for PFT, pfUSDC, and a651.
2. Fetch NAV proof snapshot and freshness deadline.
3. Request quote for pfUSDC -> a651.
4. Adapter prepares the primary mint/subscription actions.
5. Wallet verifies canonical bytes, asset IDs, amount, destination, fees, NAV
   proof hash, and expiry.
6. Wallet signs locally.
7. Adapter relays to PFTL validators and returns receipts.
8. Wallet verifies a651 balance and NAV money-in receipt.
9. If user requests reverse, adapter prepares public NAV exit and bridge-out.
10. Wallet signs any user-owned public spend and verifies final settlement.
```

The current wallet transparent handler must be replaced. Sending an issued
asset payment to the user's own address is not a NAVSwap.

## UI Requirements

The live wallet `Swap` screen should keep one primary swap surface, but route
selection must be tied to actual capabilities.

Required UI states:

- route selector: `Transparent NAVSwap`, `Shielded NAVSwap`, `OTC/RFQ`;
- asset pair selector restricted to supported route pairs at first:
  `pfUSDC -> a651` and later `a651 -> pfUSDC`;
- amount input with decimals and atom-safe validation;
- quote card with NAV, output amount, fees, reserve packet hash, and expiry;
- capability status for prover, RPC, adapter feed, and validator quorum;
- disclosure card that changes by route;
- per-step rail driven by run-scoped status, not global dashboard status;
- receipt drawer with public receipt IDs, certificate hashes, and privacy scan
  result;
- clear action-needed prompts for wallet signatures;
- recover/resume flow by `run_id`.

Do not show:

- fake "about 30s" shielded timing unless measured for the actual route;
- "private" as a single blanket label across public ingress and bridge-out;
- command previews in the normal wallet path;
- operator demo key-file errors to end users.

## Backend Adapter Strategy

Implement the adapter in stages.

### Stage 1: Compatibility Wrapper

Add a wallet-proxy or companion service wrapper that maps:

```text
/api/navswap/nav-proof      -> StakeHub nav-check/snapshot logic
/api/navswap/capabilities   -> derived from chain capabilities and route config
/api/navswap/quotes         -> deterministic quote builder
/api/navswap/runs           -> run-scoped state store
/api/navswap/runs/*         -> adapter-managed status, receipts, and events
```

This stage may call StakeHub internals for read-only evidence, but must not
call demo-wallet mutating actions for production wallet users.

### Stage 2: Transparent Route

Replace the wallet's current transparent swap handler with the adapter-backed
transparent NAVSwap route.

Required changes:

- add prepare APIs that return canonical unsigned PFTL operations;
- make wallet sign locally;
- relay signed operations through adapter;
- stream status via SSE/WebSocket;
- verify receipts in wallet after completion.

### Stage 2A: PFTL Atomic Settlement Route

Wire the existing ESCROW-009 atomic swap template into the wallet for supported
same-chain pairs.

Required changes:

- expose `POST /api/navswap/atomic-templates`;
- call PFTL `atomic_settlement_template` / `build_atomic_swap_template(...)`;
- show both reciprocal escrow legs before either wallet signs;
- make each wallet sign only its own `escrow_create`;
- track both escrow ids in one `settlement_id`;
- stream accepted/rejected status for both create legs;
- finish both escrows with the shared fulfillment only after both create legs
  are accepted;
- support cancel/recovery UX after `cancel_after`;
- block `pfUSDC <-> a651` issued-to-issued atomic swaps until the L1 template
  is extended beyond exactly-one-PFT validation or the route explicitly uses a
  PFT intermediary.

Acceptance:

- wallet can build, display, sign, submit, finish, and cancel a supported
  PFT/issued-asset atomic settlement;
- mismatched condition hash, wrong recipient, wrong amount, unsupported pair,
  stale sequence, and expired cancel height all fail before signing;
- tests reference the existing ESCROW-009 behavior and Python helper contract.

### Stage 2B: Bridge-Aware Uniswap Venue Redesign

Do not reuse the legacy Ethereum `a651/USDC` pool for trustless PFTL handoff.
Redeploy the Uniswap venue around a new bridge-aware wrapped NAVCoin
representation.

Required contract and deployment work:

- deploy a new wrapped NAVCoin ERC-20 whose mint/release path is controlled by
  a PFTL packet verifier or an explicitly labeled controlled-stage verifier;
- deploy an Ethereum handoff controller that consumes finalized PFTL packets,
  enforces packet replay protection, route caps, deadlines, and pause state,
  and mints/releases only the new wrapped token;
- create a new Uniswap pool for `wrapped NAVCoin / USDC`;
- mark the legacy standalone `a651/USDC` pool as unsupported for the trustless
  wallet handoff route;
- bind pool id, token addresses, controller address, verifier mode, route caps,
  and allocation policy hash into a published route config;
- add a return path where burning wrapped NAVCoin on Ethereum can be verified
  by PFTL before native NAVCoin is released;
- include timeout/refund semantics so a PFTL source debit cannot both settle on
  Ethereum and refund on PFTL.

Wallet route behavior:

```text
PFTL NAVCoin -> handoff packet -> verified wrapped NAVCoin mint
PFTL NAVCoin -> handoff packet -> wrapped NAVCoin mint -> bound Uniswap swap
wrapped NAVCoin burn -> verified Ethereum event -> PFTL NAVCoin return
```

The one-transaction Ethereum leg may be atomic internally:

```text
consume packet
mint wrapped NAVCoin to settlement adapter
execute bound Uniswap swap
require amount_out >= min_amount_out
send output to recipient
```

If the Uniswap swap reverts, the consume and mint must revert with it, leaving
the PFTL packet unconsumed. If a claimable-token fallback is desired, it must be
explicitly implemented, surfaced in the quote, and tested.

Acceptance:

- local/fork deployment proves the new wrapped token cannot mint without a
  valid packet or controlled-stage verifier acceptance;
- packet replay, wrong chain, wrong asset, wrong destination, expired packet,
  cap overflow, and wrong pool/path are rejected;
- a live-wallet quote cannot target the old standalone `a651/USDC` pool under
  the trustless handoff label;
- the wallet can verify whether the route is `trustless`, `optimistic`, or
  `threshold-controlled` and labels it accordingly.

### Stage 3: Wallet Note Service

Add wallet-owned Asset-Orchard note support:

- local encrypted note store;
- encrypted output scanning;
- retained anchor selection;
- note spend locks;
- note status reconciliation after refresh;
- receipt correlation.

### Stage 4: Local Shielded Prover

Wire a local prover runner for:

- Asset-Orchard ingress proof where needed;
- private swap proof;
- private-egress proof.

The long-lived runner must expose machine-readable readiness:

```json
{
  "pool_id": "asset-orchard-v1",
  "circuit_id": "...",
  "k": 15,
  "params_hash": "...",
  "vk_hash": "...",
  "pk_cache_status": "warm",
  "ready_at": "..."
}
```

### Stage 5: Shielded Route

Enable shielded `pfUSDC -> a651` with explicit liquidity source and local
wallet proofs.

Required:

- quote binds liquidity source and policy hash;
- wallet confirms public ingress receipt before private swap;
- wallet confirms replacement private note before private egress;
- private-egress receipt passes forbidden-field scan;
- bridge_out remains separate and public.

### Stage 6: Production Hardening

Add:

- route resume by `run_id`;
- idempotency keys on every mutating call;
- cancel/retry semantics for pre-submit and post-submit phases;
- adapter event persistence;
- receipt indexer integration;
- explorer links for public steps;
- warning and fallback paths for stale NAV or bridge outage.

## Security Requirements

### Canonical Verification Before Signing

The wallet must reject a prepared action if any value differs from user intent
or quote:

- chain ID;
- protocol version;
- route;
- from asset ID;
- to asset ID;
- input atoms;
- minimum output atoms;
- destination;
- fee;
- reserve packet hash;
- NAV proof hash;
- expiry height;
- policy hash;
- disclosure hash;
- bridge destination ref.

### Stale NAV and Replay Protection

Every quote and prepared action must include:

- NAV epoch;
- reserve packet hash;
- current height at quote;
- expiry height;
- quote hash;
- wallet address;
- idempotency key;
- route-specific policy hash.

The wallet must reject expired quotes or mismatched reserve packet hashes.

### Private Data Exclusion

Public payloads, adapter logs, receipts, and explorer artifacts must not contain:

- seed, mnemonic, passphrase, or key-file path;
- account private key;
- owner signing key;
- Asset-Orchard viewing/spending keys;
- note opening;
- `nk`;
- `rivk`;
- `rho`;
- `psi`;
- `rcm`;
- spent note output commitment;
- spend randomizer;
- unencrypted local note file contents.

Keep StakeHub's private-egress forbidden-field scan and extend it to the
wallet-facing adapter.

### Server Tamper Resistance

The adapter may lie, but the wallet must be able to catch lies before loss of
funds:

- wrong asset ID fails wallet-side quote/action comparison;
- wrong amount fails wallet-side comparison;
- wrong destination fails wallet-side comparison;
- stale NAV fails wallet-side freshness verification;
- missing liquidity fails route readiness;
- missing or invalid receipt fails completion;
- invalid certificate fails finality verification;
- privacy-scan failure blocks private completion labels.

### Run Isolation

StakeHub's old global runner status is not enough for wallet UX. All status
must be keyed by `run_id`. Duplicate actions must be idempotent, and a failed
run must not corrupt the next run's UI state.

## Testing Plan

### Unit Tests

Wallet:

- route reducer and step status transitions;
- quote validation and expiry;
- canonical action request verification;
- atom/decimal parsing;
- route disclosure labels;
- receipt verification mapping;
- note spend lock behavior.

Adapter:

- capabilities response;
- StakeHub read-only wrapper mapping;
- quote hash stability;
- idempotency handling;
- run-scoped status;
- forbidden-field scan;
- refusal to accept key-file based wallet actions in production mode.

L1/RPC:

- prepare APIs produce canonical bytes matching CLI behavior;
- submit APIs accept wallet signatures without key files;
- stale anchor and duplicate nullifier refusal;
- NAV proof freshness refusal;
- certificate verification.

### Integration Tests

1. Mock NAVSwap adapter with wallet UI:
   - quote -> prepare -> sign -> submit -> status stream -> receipt drawer.
2. Transparent devnet route with real validators:
   - `pfUSDC -> a651` completes from wallet-local signatures;
   - wallet balance updates without refresh;
   - NAV before/after snapshot is shown.
3. Shielded local harness:
   - wallet scans ingress note;
   - local prover creates swap proof;
   - replacement note is detected;
   - private egress proof and receipt pass privacy scan.
4. Failure cases:
   - stale NAV;
   - wrong asset ID in prepared action;
   - adapter tries destination substitution;
   - missing liquidity;
   - proof runner cold/unavailable;
   - duplicate note spend;
   - bridge_out public leg unavailable.

### Live Acceptance Tests

Transparent NAVSwap is wallet-ready when:

- the live wallet can execute `pfUSDC -> a651` through the NAV route;
- no operator key file is used for user-owned funds;
- the wallet sees balance and run status updates through a feed;
- receipts prove NAV money-in and final balance changes;
- refresh is not required for the UI to notice completion.

Shielded NAVSwap is wallet-ready when:

- the wallet locally owns and scans the relevant Asset-Orchard notes;
- private witness data never leaves the wallet/local prover boundary;
- the route can execute ingress, private swap, private egress, and optional
  bridge_out from wallet-approved actions;
- the wallet displays the correct public/private/disclosed labels;
- private-egress public artifacts pass forbidden-field scans;
- the final receipt set is verifiable after browser refresh or wallet restart.

## Migration From Current Code

### Wallet

Replace:

- `Swap.jsx` transparent handler that sends a self-directed issued payment;
- hard-coded private step execution against generic `/api/swap/action`;
- global polling through `swapServer.getStatus()`;
- static route timing copy.

Add:

- `NavSwapClient` with `/api/navswap/*`;
- quote and run state hooks;
- SSE/WebSocket subscription for run status;
- action approval modal backed by `WalletActionRequest`;
- receipt verification drawer;
- local note store module for shielded route.

### Swap Server Client

Replace the generic `SwapServer` path contract:

```text
GET  /api/swap/status
GET  /api/swap/balances
GET  /api/swap/nav
POST /api/swap/action
```

with:

```text
GET  /api/navswap/capabilities
GET  /api/navswap/nav-proof
POST /api/navswap/quotes
POST /api/navswap/runs
GET  /api/navswap/runs/{run_id}
GET  /api/navswap/runs/{run_id}/events
GET  /api/navswap/runs/{run_id}/stream
POST /api/navswap/runs/{run_id}/steps/{step_id}/prepare
POST /api/navswap/runs/{run_id}/steps/{step_id}/submit
GET  /api/navswap/runs/{run_id}/receipts
```

### StakeHub

Keep StakeHub's operator UX and scripts as implementation evidence and
orchestration references. Do not expose its demo-wallet mutating actions as the
wallet's production signing path.

Refactor reusable parts into library/service functions:

- NAV snapshot builder;
- transparent route planner;
- private route planner;
- receipt scanner;
- privacy forbidden-field scanner;
- run status normalizer.

## Definition of Done

The integration is done only when all of the following are true:

1. Transparent NAVSwap executes from the live wallet using wallet-local
   signatures, not StakeHub demo key files.
2. The wallet receives live run updates over SSE or WebSocket and no refresh is
   required to see balance changes.
3. The wallet verifies NAV proof freshness and refuses stale quotes.
4. The wallet verifies canonical action requests before signing.
5. The adapter cannot change amount, asset, destination, or policy after user
   approval.
6. The transparent route has live devnet E2E evidence with receipts and
   before/after balances.
7. Shielded route stays disabled or "operator demo" until local note custody,
   local proving, and wallet-side spend authorization are implemented.
8. Shielded route completion requires private-egress receipt verification and
   correct public/private/disclosed labels.
9. Public artifacts and logs pass forbidden-field scans.
10. Tests cover quote validation, action verification, status streaming,
    idempotency, privacy labeling, and receipt verification.
11. PFTL atomic settlement uses the existing ESCROW-009 reciprocal escrow
    template and blocks unsupported issued-to-issued swaps until the L1 template
    is extended or an explicit PFT intermediary is selected.
12. The Uniswap handoff route cannot be enabled under a trustless label until a
    new bridge-aware wrapped NAVCoin token, handoff controller, and new Uniswap
    pool are deployed and verified.
13. The wallet labels the legacy Ethereum `a651/USDC` pool as legacy secondary
    liquidity, not as the trustless PFTL-to-Uniswap route.
14. `mint_and_swap_uniswap` quotes bind pool/path, router, token in, token out,
    amount in, minimum output, recipient, deadline, and failure behavior before
    the PFTL source debit is signed.
15. Primary NAV issuance tests cover fractional purchases and at least one
    large subscription, for example 100,000 USDC-equivalent, proving that the
    fill uses the finalized pre-inflow NAV checkpoint and that reserves,
    authorized supply, user balance, accepted settlement, and any refund are
    updated in one deterministic transition.
16. The wallet exposes the route type before signing: primary issuance creates
    new native NAVCoin supply, bridge export moves that issued supply to the
    Ethereum wrapped token, and Uniswap trades buy or sell existing wrapped
    supply at AMM price and slippage.

## Open Questions

1. Should the first wallet route support only `pfUSDC -> a651`, or should it
   include `a651 -> pfUSDC` in the same release?
2. Which component owns the local shielded prover: browser WASM, wallet proxy,
   or a separate local daemon?
3. What liquidity model is production-approved for shielded `pfUSDC <-> a651`:
   market-maker note, pool note, bilateral RFQ, or operator-only demo?
4. Should direct private egress support partial-note spends in v1, or require
   whole-note/fixed-denomination notes until change handling is audited?
5. What explorer URL scheme should the wallet use for public NAV receipts,
   private-egress public exit receipts, and bridge settlement receipts?
6. Should ESCROW-009 be extended to issued-asset-to-issued-asset atomic
   settlement for `pfUSDC <-> a651`, or should v1 force a PFT intermediary?
7. What symbol/name should the new bridge-aware Ethereum wrapped NAVCoin use so
   users do not confuse it with legacy standalone `a651`?
8. Which verifier stage is acceptable for the first Uniswap handoff release:
   threshold-controlled, optimistic, succinct PFTL finality proof, or direct
   PFTL light-client verification?
9. Should failed Ethereum `mint_and_swap_uniswap` leave the packet unconsumed
   and refundable only, or consume the packet and credit claimable wrapped
   NAVCoin to the user?
10. Who owns the initial LP position for the new bridge-aware Uniswap pool after
    the seed wrapped NAVCoin has been created by canonical primary issuance plus
    bridge export, and how is that LP position represented in NAV/reserve
    accounting?

## Immediate Next Implementation Task

Build Stage 1 and Stage 2:

1. Add `/api/navswap/capabilities`, `/api/navswap/nav-proof`,
   `/api/navswap/quotes`, `/api/navswap/runs`, run status, and run events.
2. Replace the wallet transparent swap placeholder with a real transparent
   NAVSwap run.
3. Make the adapter prepare canonical unsigned PFTL actions and make the wallet
   sign them locally.
4. Stream status to the wallet so balances and step state update without
   refresh.
5. Add a live devnet E2E test that sends `pfUSDC -> a651` from the wallet and
   verifies receipts.
6. Add `POST /api/navswap/atomic-templates` and a wallet test that builds a
   supported PFT/issued-asset reciprocal escrow swap from ESCROW-009, signs both
   create legs from separate wallets, finishes both with the shared fulfillment,
   and verifies both escrow receipts.
7. Add a route-config gate that disables `uniswap_atomic_handoff` until the new
   bridge-aware wrapped NAVCoin token, handoff controller, verifier mode, and
   new Uniswap pool are configured. The gate must explicitly reject the legacy
   `a651/USDC` pool for the trustless handoff route.

Only after that passes should the shielded wallet route be enabled beyond an
operator-demo label.

2026-06-29 checkpoint:

- Stage 1 transparent adapter endpoints, wallet prepared-action signing, and
  asset finality submission are implemented on `main`.
- The WAN devnet is running release binary SHA3-384
  `93b459dcd16ac332832b517ddc2621214325d4cd6ead0c65170a0ccf6568a9f36a48aeaf320bcc340f8db853e46b6fc1`
  with `mempool_submit_signed_asset_transaction_finality` enabled.
- Wallet-proxy is configured with the operator issuer key for transparent
  completion. The browser still owns user spends; the proxy signs only the
  issuer/operator `nav_mint_at_nav` leg after verifying the wallet-submitted
  allocation.
- A funded wallet-equivalent local-key smoke run completed `pfUSDC -> a651`
  live on the WAN devnet: allocation tx
  `df30ac04ec820d1531d02629900fcb62686db9a7c814f43fb6c5f5ea5428a69bae127cc2bf656e2c3e2df009b4f50e41`,
  operator mint tx
  `0ee8d8a34fd9fee4bd9ec15174aa0afab43f681200ccb17e643226b48cb3967ae2e946d0fe5b9465284e971f28d9100a`,
  run id `navswap-mqz4ht16-cec0e265`.
- A current-chain wallet-code smoke run also completed through the wallet-web
  WASM signer, `TxBuilder`, wallet-proxy prepared actions, the async operator
  completion run, and the live `wallet_update` asset feed. Wallet
  `pfac0562296948fbf35fec6d18d47498b412850a8c` received guarded pfUSDC
  funding, submitted wallet action txs
  `8ca921806dd0c0a56b5009fb54e690943db372d0d1851d2e1c89c8b35eec1d4be1bb3331964a379169bae9f29d03f3f0`
  and
  `5ba2ae2175f2b9e36d45fc7061c1e4b6443e54bd4a484bc959e9eed1d47c6f390cb9e3122144938a467e79d6d7241245`,
  then completed operator mint tx
  `095610fec2230ca371af160927d05bfdc68c16e3010b7deb4d8666bf6317e5001c6de145abce0692d9174e53721eac48`.
  The live feed observed pfUSDC movement from `10000000` to `3041630` atoms
  and a651 movement from `0` to `1`.
- The run spent `6958370` pfUSDC atoms (`6.958370` pfUSDC) and minted `1`
  a651. All six validators converged afterward at height `1369` with identical
  state root
  `74b07a4b9f4aebfb9002d9be572b0822071eac915517fb5bd1364e3f459bd384ad2473857ce46e222acd1804c1d6b94f`
  and empty mempools.
- Remaining transparent-route validation is a manual browser UI click-through
  from the target user wallet and explicit observation that the run
  stream/balance refresh updates the wallet UI without a page refresh. The
  currently requested
  `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` wallet no longer has a
  wallet-side trustline prerequisite in the NAVSwap flow. On `2026-06-29`,
  after redeploying the validator execution rule described below, guarded
  devnet funding submitted tx
  `2f7970f89b52559fb96999cc69234035e5c0fcddecd0c31fac7f4235bbfb0aaacbe891402d4a7efc93fc17744156bc64`
  for the exact `6958370` pfUSDC atom settlement shortfall. The latest
  readiness response for `Mint 1 a651` is `ready_to_submit_wallet_actions`
  with prepared stage `nav_subscription_allocate`; the remaining browser step
  is the target wallet's signed action submit and operator completion
  observation.
- The wallet WebSocket feed now has an `include_assets:true` mode that includes
  live `account_assets` snapshots. The app opts into that feed, `Swap.jsx`
  consumes it for pfUSDC/a651 balances, and the transparent NAVSwap flow also
  refreshes issued-asset balances after wallet-signed action receipts and
  terminal run events. A live feed probe against the funded smoke-test wallet
  returned pfUSDC and a651 balances with `assets_error=null`.
- After the asset feed has loaded, missing canonical pfUSDC/a651 is treated as
  zero balance. The Swap screen blocks transparent NAVSwap submission before
  signing if the wallet lacks the required pfUSDC settlement amount.
- `scripts/navswap-wallet-live-smoke.mjs` is now the committed live smoke
  harness for the transparent wallet route. It has a no-funds dry-run mode for
  readiness checks by wallet address and an explicit `--execute` mode that
  requires a WalletBackupFile, signs through the wallet-web WASM/TxBuilder path,
  starts proxy operator completion, polls run status, and requires the live
  `wallet_update` asset feed to show pfUSDC/a651 movement. Earlier dry-run
  evidence showed `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` could prepare
  the route but had `0` canonical pfUSDC, so it could not execute until direct
  settlement funding landed. Live readiness after the guarded funding tx now
  shows the target wallet is ready to submit the prepared wallet-owned action.
  Later readiness-endpoint evidence under
  `/tmp/navswap-pf124-readiness-20260629T1215-readiness-endpoint` and current
  evidence under `/tmp/navswap-pf124-readiness-current-20260629T135039Z`
  confirmed route capability `quote_ready`, `can_run=true`, quote status
  `prepared_actions_ready`, required settlement `6958370` pfUSDC atoms, live
  pfUSDC `0`, live a651 `0`, and guarded funding configured for the exact
  shortfall.
- `scripts/navswap-fund-pfusdc.mjs` is the committed guarded devnet funding
  helper for that final browser-readiness gap. The NAVSwap wallet/proxy route
  must not require a user trustline before requesting this funding. The helper
  remains capped and issuer-key guarded so the browser flow can fund the exact
  settlement shortfall, execute the `pfUSDC -> a651` route, and verify
  live-feed balance movement under the live-transaction budget.
- The execution layer now treats recipient issued-asset balance records as
  implicit for incoming `issued_payment`. If the recipient account exists and
  does not yet have a balance record for that issued asset, the validator
  creates one with no recipient fee or opt-in requirement before crediting the
  payment. This removes the protocol-level recipient trustline gate from
  issuer funding and normal incoming issued payments while preserving the
  existing internal balance storage record.
- The wallet adapter also exposes `POST /api/navswap/devnet-fund-pfusdc` as a
  guarded devnet-only version of that top-up step. It is disabled unless
  `NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING=true`, requires an issuer key matching
  canonical pfUSDC, funds only the current readiness shortfall, and enforces
  both a per-request cap and an in-memory per-recipient window cap before
  signing. This endpoint is an overnight/devnet convenience for sub-budget live
  testing; it is not part of the trustless production NAVSwap custody model.
- The earlier trustline-centered browser flow is superseded. The Swap screen no
  longer shows output/settlement trustline controls, no longer asks the user to
  sign `trust_set`, and no longer treats `trust_set` as a transparent NAVSwap
  prepared action. The primary browser path is now guarded pfUSDC funding when
  available, then wallet-owned NAVSwap action submission.
- The Swap screen also debounces a transparent-readiness refresh when the live
  wallet asset feed reports changed canonical pfUSDC/a651 balances while a
  quote is open. This closes the remaining refresh gap for externally observed
  funding/balance changes during the browser flow.
- NAVSwap run creation and guarded devnet pfUSDC funding now support
  `idempotency_key`. The proxy replays same-key/same-body responses, rejects
  same-key/different-body conflicts, and shares in-flight duplicate requests so
  browser retries cannot double-submit operator completion or funding. Completed
  idempotent responses are persisted in a local proxy JSONL store and reloaded
  on restart, so same-key retries after a proxy restart still replay the
  original completed response. The wallet HTTP client attaches keys to those
  mutating calls automatically.
- Transparent completion receipts now include
  `postfiat-navswap-receipt-verification-v1`. The verification binds the
  prepared wallet allocation action, wallet-submitted allocation receipt, live
  `nav_subscription` allocation, operator `nav_mint_at_nav` operation, fee
  quote, and operator tx id. The Swap completion card surfaces the verification
  result, allocation id, and operator tx id.
- Wallet-proxy now exposes wallet-scoped NAVSwap run recovery at
  `GET /api/navswap/runs?wallet_address=...`. The endpoint returns active
  nonterminal runs by default and requires an explicit terminal-history flag for
  completed runs. The Swap screen queries it on load so a browser refresh can
  reattach to the latest transparent run and resume status streaming.
- NAVSwap capabilities now include `postfiat-navswap-route-privacy-v1`
  metadata for every route. Transparent wallet signing is labeled public,
  shielded stays an operator-demo route with explicit disclosed operator-demo
  fields, ESCROW-009 and Uniswap handoff labels declare their public disclosure
  surfaces, and the Swap screen renders adapter-provided visibility/disclosure
  labels.
- `scripts/navswap-redaction-check.mjs` now provides the NAVSwap-specific
  forbidden-field scanner. It scans the NAVSwap docs and public-readable
  `/tmp/navswap-*` evidence by default, skips private evidence paths unless
  `--include-private` is requested, and fails on secret-bearing values such as
  seed fields, private-key fields, backup JSON, PEM private keys, or absolute
  wallet/key artifact paths. The current public-artifact scan passes with no
  findings.
- Auto-planned transparent prepared actions now carry quote freshness metadata:
  packet-fresh flags, market-ops status/epoch, NAV epoch, reserve packet hash,
  `quote_generated_at_ms`, and `quote_expires_at_ms` (default five-minute TTL).
  The wallet-local action verifier refuses stale packet flags, stale proof
  status, invalid expiry ordering, or expired quotes before signing.
- The Swap screen consumes the same freshness metadata, displays the prepared
  quote freshness, switches expired prepared batches back to readiness refresh,
  attempts one automatic refresh per expired quote, and refreshes again just
  before signing if a quote expires while the panel is open.
- Source now includes the missing certified finality submit path for signed
  ESCROW-009 transactions:
  `mempool_submit_signed_escrow_transaction_finality`. Wallet-web prefers that
  method for escrow submits, wallet-proxy forwards it as a finality method, and
  Python atomic-settlement tooling can opt into it. This closes the previously
  observed blocker where remote validators allowed template/quote reads but
  rejected raw `mempool_submit_signed_escrow_transaction` writes on read-only
  RPC endpoints.
- `scripts/navswap-atomic-settlement-live-smoke.py` is the guarded smoke
  harness for the ESCROW-009 route. It defaults to dry-run, caps live atom
  amounts, uses the wallet proxy WebSocket RPC by default, and in `--execute`
  mode submits both escrow-create legs through escrow finality, waits for both
  escrows to become open, then reveals the shared fulfillment and verifies both
  terminal escrow states.
- The escrow-finality build with transient parent-readiness retry is deployed
  to all six WAN devnet validators as SHA-256
  `327ff19ca4111c6c2756a840f015a76e42a49e4ecad585ac729b76880b5871ad`.
  A fresh guarded live `PFT <-> a651` ESCROW-009 smoke passed through the
  wallet proxy after deployment. Evidence:
  `/tmp/navswap-atomic-settlement-live-smoke-exec-patched-20260629T134215Z`.
  Both escrows reached `finished`; tx ids were left create
  `f2a0278c54ba7d4a21e353ace1df92337feb316ef5fcd0903de8f54bbd4517e6c87b21e7a81c9f560614ae2993571711`,
  right create
  `934bc3406c980e24949ba1895cc873cc613c7e3c78371523c9733f55cba4d9ee4c12e0b8d7b6caa9a0b495b0dcd70cb8`,
  left finish
  `11fbcf4eeafea3583b56ba2edccbfc8dcb2a84ca2a20475bb8710d6c533f1a71e71393ca15446057b1a6a147b5317fb4`,
  and right finish
  `ece675859ee33af73303eed434b00cffb87f70ea78cabe8a44ecfce2788ed0e36395e70db98a2b3121d3c71ced341d48`.
  All validators converged afterward at height `1384`, state root
  `8ee163ee1981720eb86e9577378f554652fc6091f8aea743ed4aa115acf002a3f6c08fae4851c123d3fbc168d5470b03`,
  with empty mempools.
- Current morning-handoff evidence is summarized in
  `docs/status/navswap-morning-handoff-2026-06-29.md`. It records the
  2026-06-29T15:17Z read-only refresh: target wallet readiness under
  `/tmp/navswap-pf124-readiness-current-20260629T151716Z`, full custody and
  legacy-pool inventory under
  `/tmp/navswap-custody-inventory-current-20260629T144618Z`, and terminal
  run-stream recovery for `navswap-mqz62mp7-184175bd` under
  `/tmp/navswap-run-stream-current-20260629T152628`. That stream smoke now
  records both `run_status_terminal=true` and `stream_terminal=true`, proving
  the polling status and browser-consumed SSE lifecycle signals agree. The
  live transparent route capability now exposes the supported `pfUSDC->a651`
  pair and the
  current amount/settlement asset semantics consumed by the wallet controls,
  with unit-tested fallback parsing in `wallet-web/src/lib/navswap-flow.js`.
  The Swap screen auto-loads transparent readiness for that current pair once
  wallet address, adapter, and amount are available, so the target browser flow
  should now land directly on the prepared action batch submit instead of
  requiring a manual quote click first. The auto-readiness eligibility gate is
  covered in `wallet-web/src/lib/navswap-flow.test.js`. The browser run watcher uses a
  shared tested NAVSwap terminal-state helper and the adapter stream's explicit
  terminal flag before stopping polling/streaming. Terminal snapshots now also
  release the wallet's active-run latch while preserving the terminal status
  and receipts for the completion card. After successful transparent
  completion, that card exposes a tested `Get new quote` action so repeated
  browser smoke runs can move directly from receipt review to a fresh
  readiness quote without manually dismissing/resetting the route first. On
  browser refresh, the Swap screen first reattaches active transparent runs and
  then, if idle, can recover the latest successful terminal transparent run via
  `GET /api/navswap/runs?include_terminal=true`; dismissed terminal run ids are
  remembered client-side so old receipts do not keep reappearing. The live
  terminal-history probe for `pfac0562296948fbf35fec6d18d47498b412850a8c`
  returned `navswap-mqz62mp7-184175bd` with `terminal=true`, `ok=true`, and an
  attached quote. The proxy run status response exposes the same `terminal`
  boolean for polling fallback.

## Overnight Job: No-New-Pool Wiring Pass

Scope: useful live-wallet and StakeHub work that can run overnight without
blocking on a new bridge-aware Uniswap pool deployment and without requiring a
large liquidity transaction.

Live transaction budget:

- live transactions are allowed;
- no single live action may intentionally exceed USD 100 equivalent in value or
  expected gas cost;
- do not seed a new Uniswap pool;
- do not create a large LP position;
- do not move all custody inventory;
- do not deploy a contract if gas estimation suggests the action may exceed the
  USD 100 cap;
- every live transaction must record chain, tx hash, amount, gas estimate,
  actual cost, source address, destination/contract, and purpose.

### 1. Inventory Discovery

Find what can be used without new pool seeding.

Checklist:

- find old Ethereum a651 custody balances;
- find Ethereum USDC balances in StakeHub/operator custody;
- find any existing a651 Uniswap LP/NFT positions controlled by custody;
- find PFTL `a651`, `pfUSDC`, and PFT balances;
- classify each balance as spendable, locked, LP-positioned, bridge-controlled,
  or unknown;
- write a custody report with chain, address, asset, amount, source, and
  spendability.

Acceptance:

- a reader can see whether there is enough old a651 or USDC custody to do
  sub-USD-100 smoke tests;
- no funds are moved during inventory discovery except read-only RPC calls.

Implementation note:

- `scripts/navswap-custody-inventory.mjs` is the read-only inventory command.
  It queries the local wallet proxy for PFTL PFT/pfUSDC/a651 balances, Ethereum
  mainnet for the legacy a651/USDC venue, and Arbitrum for the old pfUSDC vault
  and operator USDC/gas balances. It writes `inventory.json` plus
  `inventory.md`, classifies balances as spendable, LP-positioned,
  bridge-controlled/drained, empty, or unknown, and records that no live
  transaction is required.

### 2. Wallet NAVSwap Readiness

Replace uncertainty with a concrete wallet wiring map.

Checklist:

- audit `wallet-web/src/components/Swap.jsx` against this spec;
- audit `wallet-web/src/lib/swap-server.js`;
- identify the minimum adapter methods needed to stop using the fake
  transparent swap handler;
- add or stub `GET /api/navswap/capabilities`;
- add or stub `GET /api/navswap/nav-proof`;
- add or stub `POST /api/navswap/quotes`;
- add or stub `POST /api/navswap/runs`;
- make route disabled states explicit, especially for
  `uniswap_atomic_handoff`;
- ensure the legacy `a651/USDC` pool is labeled legacy secondary liquidity, not
  the trustless handoff route.

Implementation note:

- `transparent_navswap` is reserved for the future browser-wallet-signed
  transparent route and must remain disabled until that adapter exists.
- The wallet/proxy prepared-action contract is
  `postfiat-navswap-wallet-action-request-v1`. For the transparent route, the
  browser wallet may sign only wallet-owned asset actions:
  `vault_bridge_nav_subscription_allocate` and `nav_redeem_at_nav`.
  `trust_set` is not a transparent NAVSwap route action. The wallet must reject
  issuer/operator actions such as `nav_mint_at_nav` and `nav_redeem_settle`,
  reject any embedded key-file or private-material fields, and re-check source,
  issuer/operator, assets, settlement amount caps, NAV epoch, and
  reserve-packet hash before signing.
- `POST /api/navswap/actions/prepare` is the adapter endpoint for prepared
  wallet actions. Transparent quote/readiness preparation accepts
  `nav_subscription_allocate` and `nav_redeem_at_nav`; planner-fed
  `trust_set` actions are rejected for transparent NAVSwap.
  Each NAVSwap stage resolves the issued NAV asset issuer via `asset_info` and
  returns a canonical unsigned wallet-owned action for browser-local
  verification/signing.
  `nav_subscription_allocate` requires planner-selected settlement bucket,
  settlement receipt, supply allocation IDs, and settlement amount atoms.
  `nav_redeem_at_nav` requires planner-selected NAV amount, finalized NAV
  epoch, and reserve packet hash. The route planner still must select those
  inputs from real state before `transparent_navswap` can be enabled.
- `POST /api/navswap/actions/prepare-batch` is the planner handoff endpoint
  for ordered wallet action sets. It accepts planner-fed stage input objects,
  calls the same canonical per-stage builders, and returns an ordered
  `actions[]` array only if every item prepares successfully. On failure it
  returns the failed index/stage and the wallet must not sign the partial set.
- `POST /api/navswap/planner-inputs` is the first automatic planner discovery
  endpoint. It reads live `vault_bridge_status` and `market_ops_status` RPC
  data, selects an active counted settlement receipt plus a live
  `vault_bridge_supply` allocation with enough remaining capacity for
  `pfUSDC -> a651`, and emits the stage inputs that can feed
  `/api/navswap/actions/prepare-batch`. It accepts both symbolic
  `from_asset`/`to_asset` values and explicit `from_asset_id`/`to_asset_id`
  values. It is read-only and does not sign or submit anything. On the live WAN
  devnet, this endpoint now reaches the validator RPC methods, computes the
  required pfUSDC settlement spend from the live NAV packet, and returns the
  wallet-owned `nav_subscription_allocate` action plan when live
  receipt/allocation capacity is available.
- `POST /api/navswap/readiness` is the read-only browser readiness endpoint for
  the transparent route. It returns the live auto-planned quote, route
  capability state, required settlement atoms, wallet settlement balance,
  prepared action stages, and ordered next steps.
  It does not sign, submit, or fund. The Swap screen uses it for transparent
  quotes so the user sees whether the remaining blocker is adapter config,
  quote planning, pfUSDC funding, or ready-to-submit wallet-owned actions. The
  Swap screen refreshes this readiness after guarded funding and prepared-action
  submits and exposes an explicit refresh control, so the browser flow can
  advance from funding to action submission without a page refresh or a fresh
  quote dance.
  Readiness also preflights native PFT fees/reserves for each prepared
  wallet-owned action through `asset_fee_quote`, exposes the result under
  `wallet_pft`, and refuses the ready state when the wallet lacks enough native
  PFT to pay for the allocation actions. The Swap screen
  now renders that `wallet_pft` readiness as PFT balance plus fee-preflight
  status, so a browser user can distinguish "fund pfUSDC" from "add PFT gas"
  before signing.
  `scripts/navswap-wallet-live-smoke.mjs` records the same readiness payload as
  `adapter-readiness.json`.
- `POST /api/navswap/devnet-fund-pfusdc` is the optional guarded devnet funding
  endpoint surfaced through readiness. The wallet shows its button only when
  the quote is prepared, canonical pfUSDC is the settlement asset, the
  shortfall is within the configured caps, and the proxy's configured issuer
  key matches the live canonical pfUSDC issuer. Successful funding is submitted
  as a normal issued-asset transaction through finality RPC. Validators no
  longer require the recipient to pre-open a user trustline for that incoming
  issued payment; an existing account receives an implicit issued-balance
  record when the payment applies.
- The Swap screen treats transparent NAVSwap as a wallet-owned action
  state-machine, not as a generic server-run route. Even when
  `transparent_navswap.can_run=true`, the bottom action starts with
  quote/readiness; the prepared route panel then exposes a single primary next
  action in order: guarded pfUSDC funding, readiness refresh, or wallet action
  batch submission.
- `uniswap_atomic_handoff` rejects the legacy Ethereum `a651/USDC` pool from
  both request bodies and proxy env configuration. If
  `NAVSWAP_WRAPPED_NAVCOIN_TOKEN` points at the legacy a651 token, or
  `NAVSWAP_UNISWAP_POOL_ID`/`NAVSWAP_UNISWAP_POOL_PATH` points at the legacy
  pool id, capabilities return `disabled_legacy_pool_rejected` and quotes fail
  with `legacy_pool_rejected`.
- When a non-legacy bridge-aware handoff config is present, the route still
  keeps `can_run=false`, but quotes now fail closed until recipient, minimum
  output, and deadline are supplied. A successful configured quote returns a
  hash-bound `postfiat-navswap-mint-and-swap-uniswap-quote-v1` object binding
  pool/path, router, token in, token out, amount in, minimum output, recipient,
  deadline, and failure behavior before any future PFTL source debit can be
  signed.
- `scripts/navswap-wallet-live-smoke.mjs --execute` mirrors that state-machine
  for non-browser evidence. Given a wallet backup, it can request exact guarded
  pfUSDC funding from the proxy, wait for the live wallet feed to observe the
  funded pre-swap balance, submit the prepared wallet-owned batch, start
  operator completion, and verify final pfUSDC/a651 balance movement from the
  pre-swap balance.
- The smoke harness also supports `--stream-run-id` to record
  `/api/navswap/runs/{run_id}/stream` for existing adapter runs without wallet
  seed material or live transactions. This provides repeatable evidence that
  the same SSE stream consumed by the browser reaches a terminal snapshot and
  includes receipts.
- `POST /api/navswap/quotes` also accepts planner-fed transparent action input
  sets and returns a `prepared_actions_ready` quote containing the prepared
  action batch. Plain transparent quotes without planner inputs must fail
  closed with `transparent_navswap_planner_inputs_required`; they must never
  fall back to a placeholder transfer. Quotes may opt into automatic planner
  discovery with `auto_plan: true`, but that path must still fail closed if live
  vault/market status is unavailable or no source allocation has enough
  remaining capacity. The live wallet's transparent quote path now requests
  this automatic planner discovery by default.
- `POST /api/navswap/runs` now accepts transparent NAVSwap completion requests
  after the browser has submitted the wallet-owned prepared batch. The request
  carries the reviewed quote plus the wallet submit result. The adapter verifies
  that the submitted `nav_subscription_allocate` operation matches the prepared
  action, reads public `vault_bridge_status` until a matching live
  `nav_subscription` allocation is visible, fills `settlement_allocation_id`,
  and builds the operator-owned `nav_mint_at_nav` operation. If
  `NAVSWAP_OPERATOR_ISSUER_KEY_FILE` is configured, the proxy quotes, signs via
  `postfiat-node wallet-sign-asset-transaction --key-file`, and submits the
  signed operator mint with canonical
  `mempool_submit_signed_asset_transaction_finality.signed_asset_transaction_json`.
  If the key is not configured, the run stops explicitly at
  `awaiting_operator_signature` after allocation verification. Async transparent
  runs use the same `/api/navswap/runs/{run_id}/stream` SSE feed as StakeHub
  runs.
- The node RPC now supports
  `mempool_submit_signed_asset_transaction_finality` for externally signed
  issued-asset transactions. The finality path admits
  `signed_asset_transaction_json` into the certified mempool round, so
  browser-signed wallet actions can reach certified finality without raw
  mempool submit being enabled on public RPC and without proxy custody of the
  wallet key. Wallet-web uses this method for `TxBuilder.sendAssetTransfer`,
  and wallet-proxy uses it for the operator-owned `nav_mint_at_nav` completion
  leg.
- The live wallet now uses this contract for transparent NAVSwap actions: from
  the Swap screen, the transparent route can fetch, wallet-verify, wallet-sign,
  submit, and display receipt state for prepared wallet-owned actions. The
  wallet library verifies the full ordered action set before producing any
  signature, then signs and submits wallet-owned actions sequentially with
  partial-result reporting on submit failure, including non-`Error` signer
  failures. Reviewed-operation matching canonicalizes known integer fields so
  proxy-prepared actions and node quotes compare correctly across
  string/number JSON representations. Wallet WASM signed-wrapper serialization
  now preserves flattened unsigned transaction fields needed by browser-signed
  asset actions. After a successful batch submit,
  the Swap screen starts an async transparent completion run and subscribes to
  the run stream. The asset-finality build has been deployed to the WAN devnet,
  the proxy has been configured with the issuer key for the operator leg, and a
  current-chain wallet-code E2E run has produced certified receipts for the
  wallet-owned allocation and operator mint. A manual browser UI click-through
  from the target user wallet remains required before marking the product flow
  fully done.
  The wallet feed now includes issued-asset balances, so that browser run should
  show pfUSDC/a651 balance movement through the live feed without a page
  refresh. A committed guarded pfUSDC funding helper now covers the devnet
  top-up step for the exact settlement shortfall.
- The existing StakeHub transparent no-Orchard runner is exposed separately as
  `stakehub_transparent_roundtrip`; it is an operator-backed smoke route, not
  the trustless wallet-signed route.
- `stakehub_transparent_roundtrip` quotes require `NAVSWAP_STAKEHUB_BASE_URL`.
  Live forwarding requires the additional
  `NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS=true` gate and keeps the default
  whole-a651 smoke amount cap unless `NAVSWAP_STAKEHUB_MAX_A651_AMOUNT` is
  deliberately changed.
- With `NAVSWAP_STAKEHUB_BASE_URL` configured, `/api/navswap/nav-proof` reads
  StakeHub `/api/navcoin` and `/api/navcoin/status`; without it, the endpoint
  must report proof unavailable rather than invent freshness.
- StakeHub transparent quotes and runs must reject missing/stale NAV proof
  before forwarding to the operator route. Adapter-managed runs expose
  `/api/navswap/runs/{run_id}`, `/events`, `/stream`, and `/receipts` so the
  wallet can subscribe to stable route state instead of relying on one-shot
  action responses.
- `POST /api/navswap/runs` supports an async mode (`async=true`,
  `async_run=true`, or `mode=async`) for the StakeHub transparent route. Async
  mode returns a `run_id` immediately, records the StakeHub forward in the
  adapter run journal, and lets the wallet consume the `/stream` feed while the
  live StakeHub action is still in flight. The JSON status/events/receipts
  endpoints remain available as fallback and for CLI smoke tools.
- StakeHub transparent quotes also read `/api/shielded-nav-swap/balances` and
  `/api/shielded-nav-swap/status`; missing balance preflight blocks the quote
  before any live action is attempted.
- StakeHub transparent status must expose PFTL finality recovery state for the
  operator runner. If `/api/shielded-nav-swap/status` reports
  `transparent_roundtrip.finality_recovery_required=true` or
  `transparent_roundtrip.status=needs_timeout_certificate`, wallet-proxy
  capabilities, quotes, and runs must fail before forwarding with a specific
  `stakehub_transparent_finality_recovery_required` error. The wallet must be
  able to disable the route from `/api/navswap/capabilities`, before the user
  can launch another vault-supply attempt while the local PFTL node is locked
  on the next height/view and no timeout certificate is configured.
- Finality recovery gating must be view-aware. If the local node has a
  proposal-vote lock at next height `H` view `V`, the configured timeout
  certificate must cover the same height and view before StakeHub re-enables
  the route. A certificate for `H/view 0` is not sufficient after a retry has
  created a `H/view 1` lock. The wallet must surface the locked view and
  required timeout-certificate view.
- StakeHub transparent status must also expose recent incomplete certified
  transport attempts as
  `transparent_roundtrip.status=transport_recovery_required` and
  `transport_recovery_required=true`. Wallet-proxy capabilities, quotes, and
  runs must fail before forwarding with
  `stakehub_transparent_transport_recovery_required` while this time-bound
  guard is active. Operators can tune transparent certified-operation transport
  behavior with `STAKEHUB_NAV_TRANSPARENT_TRANSPORT_TIMEOUT_MS`,
  `STAKEHUB_NAV_TRANSPARENT_TRANSPORT_SEND_RETRIES`,
  `STAKEHUB_NAV_TRANSPARENT_TRANSPORT_RETRY_BACKOFF_MS`, and the incomplete-run
  cooldown with `STAKEHUB_NAV_TRANSPARENT_INCOMPLETE_RUN_COOLDOWN_SECS`.
- StakeHub transparent status must include durable recovery diagnostics for
  incomplete certified transport attempts. At minimum, the latest attempt
  should expose `latest_incomplete_run.latest_transport_round.status`,
  proposal height/view, whether `block-certificate.json` exists, whether the
  peer-certified report exists, and a human diagnosis. The wallet must surface
  this in the route preflight panel so the operator can distinguish "cooldown
  after remote vote collection stopped" from a generic unavailable route.
- Transparent roundtrip early failures must write `roundtrip-failure.json`
  under the run directory so HTTP responses are not the only copy of the
  failure evidence.

Acceptance:

- the wallet can display which NAVSwap routes are available, disabled, or
  operator-demo only;
- the wallet refreshes NAVSwap capabilities while open and polls async run
  status/events without requiring a manual page refresh;
- the disabled Uniswap handoff route explains that a bridge-aware wrapped token
  and new pool are required;
- no route silently falls back to a self-transfer placeholder.

### 3. Existing PFTL Atomic Settlement Wiring

Use ESCROW-009. Do not invent a new atomic-swap primitive overnight.

Checklist:

- expose or exercise `atomic_settlement_template` through the adapter;
- build a supported `PFT <-> issued asset` template;
- verify the response schema is `postfiat-atomic-settlement-template-v1`;
- verify both legs share the same `condition_hash`;
- verify the `settlement_id` is stable and symmetric;
- verify both escrow ids are distinct;
- make each side sign only its own `escrow_create`;
- submit both create legs on a tiny live or local amount;
- finish both legs with the shared fulfillment only after both create legs are
  accepted;
- test cancel/recovery path locally if live cancel timing would be annoying.

Current committed status:

- The wallet proxy exposes `/api/navswap/atomic-templates`, calls
  `atomic_settlement_template` twice, and rejects responses that fail schema,
  condition-hash, stable symmetric settlement-id, reciprocal-leg, PFT/issued
  pairing, or distinct-escrow-id checks.
- `scripts/navswap-atomic-template-smoke.mjs` is a read-only smoke for the
  live proxy/RPC path. It writes request/response/summary artifacts and fails if
  the adapter stops returning the expected ESCROW-009 template contract.
- The browser wallet has reviewed-template escrow create/finish/cancel signing
  wired through `TxBuilder`; it signs only the unlocked wallet's leg and rejects
  fee-quote operation or sequence substitution before signing.
- Python `postfiat_rpc.execute_atomic_settlement` is the tested two-wallet
  executor. It submits both create legs and reveals the shared fulfillment only
  after both creates are accepted.
- Current proof: guarded live `PFT <-> a651` ESCROW-009 smoke evidence under
  `/tmp/navswap-atomic-settlement-live-smoke-exec-patched-20260629T134215Z`
  created both escrow legs through escrow finality, waited for both escrows to
  become open, finished both with the shared fulfillment, and verified both
  terminal escrow states.

Acceptance:

- one tiny atomic settlement completes or a local equivalent proves the full
  flow;
- unsupported `pfUSDC <-> a651` issued-to-issued atomic swaps are blocked with
  a clear reason unless a PFT intermediary is explicitly selected;
- receipts and account history show both escrow legs and shared condition hash.

### 4. Transparent PFTL NAV Route Smoke

Use existing PFTL NAV mint/exit surfaces if balances allow.

Checklist:

- verify wallet/account has enough PFT gas;
- verify `pfUSDC` and `a651` balances;
- quote a tiny `pfUSDC -> a651` route;
- prepare the canonical unsigned action;
- sign locally where the wallet path supports it;
- submit and verify receipts;
- refresh balances through the live feed or polling path;
- keep live value under USD 100.

Acceptance:

- before/after balances are captured;
- receipt ids are captured;
- NAV proof freshness is recorded;
- any failure is reported with exact stage, RPC response, and missing
  prerequisite.

### 5. Legacy a651 Uniswap Pool Inspection

Inspect the old pool without pretending it is the final trustless path.

Checklist:

- read old a651 token metadata and custody balance;
- read old `a651/USDC` pool id/config;
- read current price/liquidity if RPC access is available;
- check whether custody can spend old a651;
- optionally perform a tiny quote or sub-USD-10 smoke swap only if gas and
  amount remain far below the USD 100 cap;
- do not seed liquidity;
- do not create or modify LP positions.

Acceptance:

- report states whether old a651 liquidity is usable as legacy liquidity;
- report states that this pool is not bridge-aware and not the trustless PFTL
  handoff route;
- any live swap has tx hash, gas cost, amount in/out, and reason.

### 6. Bridge-Aware Uniswap Handoff Prep

Prepare the future route without deploying it.

Checklist:

- draft route config fields for the future wrapped NAVCoin token;
- draft handoff controller fields;
- draft verifier mode field: `threshold-controlled`, `optimistic`,
  `succinct-proof`, or `direct-light-client`;
- draft new Uniswap pool id/path fields;
- define wallet disabled-state copy;
- define packet fields needed before a PFTL source debit is signed;
- define exact failure behavior for `mint_and_swap_uniswap`.

Acceptance:

- the wallet/proxy has a config-shaped place to put the future token,
  controller, verifier, and pool;
- until those fields are populated, `uniswap_atomic_handoff` is disabled and
  cannot target the legacy pool.

### 7. Morning Deliverable

The overnight run should leave a concise report with:

- custody inventory;
- wallet routes available/disabled;
- adapter endpoints added or stubbed;
- atomic settlement result;
- transparent NAV route smoke result;
- guarded pfUSDC funding readiness result for the target browser wallet;
- legacy Uniswap pool inspection result;
- every live tx hash and cost;
- exact next blockers for deploying the new bridge-aware wrapped token,
  handoff controller, verifier, and Uniswap pool.

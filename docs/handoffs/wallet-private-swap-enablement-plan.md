# Wallet Private Swap Enablement Plan

Date: 2026-07-02
Status: foundation Steps 1-4 implemented on branch `wallet-private-swap-foundation`; stopped before Step 5 value movement
Scope: enable user-facing wallet private swaps after the merged
Asset-Orchard privacy remediation.

## Current State

The protocol gate that blocked wallet private swaps is cleared. The merged
`main` commit `b985d2660a02285b30d2198b0c2c490351375868` was verified by
`$ORC_DIRECTIVES_ROOT/orchardmanager_status_1254.md`: the real
Asset-Orchard swap proof is generated, verified, applied, rejects a corrupted
proof, hides asset ids and amounts in captured serialized
`AssetOrchardSwapAction` JSON, and rejects non-conserving accounting.

The existing wallet NAVSwap flow is not a private wallet swap. The wallet
`Swap` screen in `wallet-web/src/components/Swap.jsx` has route concepts for
`transparent_navswap`, `shielded_navswap`, StakeHub transparent smoke routes,
PFTL atomic settlement, and the controlled PFTL-Uniswap beta route. The
displayed wallet routes are currently limited to transparent NAVSwap and
PFTL-Uniswap beta by `DISPLAYED_SWAP_ROUTES`. The `shielded_navswap` entry is
present but labeled as an operator demo because wallet-safe custody wiring is
not implemented.

The wallet server client in `wallet-web/src/lib/swap-server.js` already has
generic swap and NAVSwap methods:

- `/api/navswap/capabilities`
- `/api/navswap/readiness`
- `/api/navswap/quotes`
- `/api/navswap/actions/prepare-batch`
- `/api/navswap/runs`
- legacy generic `/api/swap/*`

It does not expose a wallet-safe Asset-Orchard note API, local shielded note
store, local private prover, or `/api/shielded-nav-swap/*` adapter contract.
The wallet signing path is browser-local through `wallet-web/src/App.jsx`,
`wallet-web/src/lib/tx-builder.js`, and wallet WASM. That custody boundary is
also stated in `docs/specs/web-wallet.md`: the server must never receive the
user's seed, passphrase, private key, decrypted backup, or authority to sign
for the user.

The PFTL CLI surfaces exist in the node and are the protocol/tooling basis for
the private route:

- `asset-orchard-ingress-create`
- `asset-orchard-note-status`
- `asset-orchard-swap-create`
- `asset-orchard-swap-live-round`
- `asset-orchard-private-egress-create`
- `shield-batch-asset-orchard-ingress`
- `shield-batch-swap`
- `shield-batch-asset-orchard-private-egress`
- `apply-shield-batch`

These are listed in `crates/node/src/main_parts/runtime_helpers.rs` and wired
in `crates/node/src/main_parts/cli_dispatch_parts/group_05.rs`.

StakeHub has a shielded NAV swap operator flow, but it is not wallet-safe
custody. Relevant files:

- `$STAKEHUB_REPO/docs/current-sprint/shielded-nav-swap-agent-handoff.md`
- `$STAKEHUB_REPO/docs/status/shielded-nav-swap-audit.md`
- `$STAKEHUB_REPO/scripts/shielded-nav-swap-e2e-live.py`
- `$STAKEHUB_REPO/scripts/shielded-navswap-ux-live.py`
- `$STAKEHUB_REPO/stakehub/dashboard_server.py`

StakeHub exposes `/api/shielded-nav-swap/*` and actions such as
`pftl_wallet_create`, `bridge_in`, `shield_ingress`, `prewarm`,
`shield_swap`, `private_egress`, and `bridge_out`. That path uses StakeHub
operator/demo wallet state and local files, and the audit documents global
runner state, synchronous long actions, duplicate-click hazards, and bridge
blocking. It is valuable implementation evidence, not the final browser-wallet
custody boundary.

The previous `pftl-uniswap-wallet-e2e` work was public and operator-attested.
It exercised transparent PFTL/NAVSwap and controlled PFTL-Uniswap paths where
wallet-owned public actions are signed locally and operator-owned legs complete
afterward. Evidence under
`docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/` marks
`shielded_navswap` as `operator_demo`, `enabled: false`, and explicitly says
not to label it wallet-private until local note custody, local proving, and
wallet-side spend authorization are implemented.

## Target Definition

The target route is a wallet-initiated shielded `a651 <-> a652` swap through
Asset-Orchard, with asset and amount hidden on-chain for the internal swap.
`a651` and `a652` are both NAV assets represented by transparent issued asset
ids at the ingress/egress boundaries and by private `AssetTag`/value witnesses
inside the Asset-Orchard pool.

Enabled means all of the following are true:

1. The wallet can show a "Private NAVSwap" route for `a651 -> a652` and
   `a652 -> a651` using real route capability state, not hardcoded demo state.
2. The wallet can create or import local Asset-Orchard viewing/spending
   material without exposing seed, backup JSON, note openings, spend keys, or
   private note files to a remote service.
3. The wallet can discover or recover spendable private notes from encrypted
   chain outputs plus local wallet keys, not only from one-off temporary files.
4. The route can quote a real liquidity source for the opposite private note.
   Asset-Orchard swap v1 preserves the exact two input `(asset_tag, value)`
   pairs; it does not mint, price, or AMM-convert assets by itself. A
   `a651 -> a652` swap therefore needs a real counterparty, RFQ, pool-managed
   note, or issuer/liquidity service note containing `a652`.
5. The wallet verifies quote terms, liquidity policy, chain id, pool id,
   retained anchor, expiry, route privacy class, and expected output note
   ownership before authorizing a private spend.
6. Proving happens in a wallet-controlled local prover/daemon, or in a future
   protocol that does not reveal note openings/spend authority to a remote
   prover. Remote operator proof generation with user note material is not
   wallet-safe.
7. The wallet submits or relays the resulting shielded batch and verifies final
   receipt/certificate.
8. The wallet shows local private note balances after the swap and survives a
   reload/rescan.
9. The captured on-chain `AssetOrchardSwapAction` for the wallet run contains
   no cleartext asset ids, asset tags, amounts, owner, recipient, or price; its
   accounting records expose only commitments.
10. Conservation and duplicate-nullifier rejection remain enforced by consensus.

Acceptance test:

```text
Given a wallet with a spendable private a651 note and access to an approved
liquidity source with a spendable private a652 note,
when the user requests a651 -> a652 from the wallet,
then the wallet verifies route capability, quote, anchor, liquidity policy,
and local custody; creates/proves/signs the Asset-Orchard swap action without
sending private material to a remote service; submits the shielded batch;
observes finality; rescans; displays a spendable private a652 note; and writes
evidence proving the serialized on-chain action hides asset and amount.

The same acceptance must pass in reverse for a652 -> a651.
```

## Gap Analysis

### Already Wired

- Consensus accepts real `AssetOrchardSwapAction` proofs and rejects invalid
  proofs/non-conserving accounting, per the 12:54 UTC verification status.
- The wallet already has public asset balance display for issued assets through
  `account_assets` and live wallet feeds.
- The wallet already has local public transaction signing for transparent
  wallet-owned actions.
- The wallet already has NAVSwap route capability/readiness machinery for
  public routes.
- The PFTL CLI can create ingress, swap, note status, private egress, and
  shielded batches.
- StakeHub demonstrates an operator flow that can orchestrate shield ingress,
  warm proving, shield swap, private egress, and public bridge-out.

### Missing For Wallet Enablement

- The wallet does not call the Asset-Orchard path. `Swap.jsx` does not expose
  `shielded_navswap` as a selectable route, and the submit path is built around
  public NAVSwap prepared asset actions.
- There is no wallet-local Asset-Orchard note vault: no local view/spend keys,
  note index, note locks, retained-anchor choice, encrypted output scan, or
  recovery across reload.
- The wallet has no `a652` asset registry entry or configurable NAV asset
  selection beyond the currently hardcoded `pfUSDC` and `a651` constants in
  `wallet-web/src/lib/utils.js`.
- There is no wallet-safe private-swap API contract. StakeHub's
  `/api/shielded-nav-swap/action` is an operator dashboard API, not a
  browser-wallet API.
- Server-side proving is a custody problem unless the server never receives
  user note openings or spend authority. The existing CLI shape takes private
  note files as inputs; sending those files to a remote prover would be key/note
  handoff for non-operator users.
- P9 note encryption/scanning remains a real prerequisite for multi-party
  private swaps. The first local single-wallet slice can write output-note files
  directly, but a real maker/taker wallet route must let recipients discover
  outputs from encrypted chain data.
- StakeHub shielded-swap E2E must be rerun against the fixed Pedersen-accounting
  main before using it as current wallet evidence. The older audit predates the
  merged remediation and records partial flow/status issues.
- The route needs liquidity semantics. A v1 Asset-Orchard swap is a two-note
  private settlement, not a public AMM. The wallet must know whether liquidity
  is bilateral RFQ, operator inventory, pool-managed notes, or issuer-reserve
  flow, and must label that trust class.
- Private egress and bridge-out are separate public-boundary choices. They
  should not be hidden inside the phrase "private swap".

## Step-By-Step Plan

### Step 1: Wallet Capability And UX Contract

Build a read-only/disabled "Private NAVSwap" wallet route contract before any
value movement.

Work:

- Add `shielded_navswap` to the wallet route selector as a disabled or
  preflight-only route.
- Display live capability fields from the swap server/adapter:
  `enabled`, `can_quote`, `can_run`, `custody_boundary`,
  `requires_local_prover`, `requires_note_scan`, `supported_pairs`,
  `liquidity_mode`, `privacy_label`, and `disabled_reason`.
- Add adapter client methods for read endpoints only:
  shielded status, balances, note capability, prover readiness, and route
  capability.
- Add forbidden-field request tests proving the wallet does not send seed,
  backup JSON, private keys, note openings, note files, or spend keys to the
  shielded route endpoints.

Acceptance:

- The wallet visibly explains why the private route is not executable yet.
- No mutating shielded endpoint is called.
- Tests fail if a shielded request body contains private material fields.
- Existing public swap routes still behave as before.

This is the first concrete buildable increment on operator go.

### Step 2: Asset Registry Generalization

Work:

- Replace the hardcoded `pfUSDC/a651` private-swap assumptions with a route-fed
  asset registry that can include `a652`.
- Require every private route quote to name transparent asset ids,
  display symbols, precision, issuer, NAV source, and route policy hash.
- Keep public transparent NAVSwap and PFTL-Uniswap routes pinned to their
  existing assets unless operator explicitly approves broader route exposure.

Acceptance:

- The wallet can render `a651` and `a652` as private-route assets from
  capability data.
- Unknown assets are display-only until the adapter marks them supported.
- Asset id/symbol mismatches fail closed.

### Step 3: Wallet-Local Asset-Orchard Note Store

Work:

- Define browser/local-daemon storage for Asset-Orchard keys and notes.
- Track notes as `pending`, `spendable`, `locked_for_swap`, `spent`,
  `egressed`, or `unknown`.
- Store note metadata encrypted at rest and never send openings/spend authority
  to remote endpoints.
- Implement note status reconciliation using PFTL status/receipts and
  encrypted output scans.

Acceptance:

- A wallet reload can recover spendable note state from local keys plus chain
  data.
- Duplicate/nullified notes cannot be selected for a new swap.
- P9 status is explicit: full multi-party recovery/scanning is blocked until
  encrypted output handling is complete.

### Step 4: Local Prover Boundary

Work:

- Define a local-only prover process/API that accepts local note references,
  route quote commitments, and output recipient material, then returns a
  signed/proved `AssetOrchardSwapAction`.
- Prewarm K=15 proving/verifying keys and expose readiness with hashes:
  `pool_id`, `circuit_id`, `k`, `params_hash`, `vk_hash`,
  `pk_cache_status`, and `ready_at`.
- Make the wallet verify action JSON before relay: chain id, genesis, pool,
  anchor, nullifier count, output count, proof system id, and no forbidden
  cleartext fields.

Acceptance:

- The local prover can build a swap action from local notes without a remote
  service seeing note openings.
- Cold/hot timing is reported.
- The wallet blocks submission if privacy scan or action verification fails.

### Foundation Implementation Status: 2026-07-02

- [x] Step 1: Wallet capability and UX contract.
  Evidence: `wallet-web/src/components/Swap.jsx` exposes `shielded_navswap`
  as a selectable preflight-only route, displays `enabled`, `can_quote`,
  `can_run`, custody boundary, note scan, local prover, liquidity mode,
  privacy label, disabled reason, assets, pairs, and P9 status. The route is
  non-executable and existing public routes remain on their existing paths.
- [x] Step 1 forbidden-field custody tests.
  Evidence: `wallet-web/src/lib/shielded-navswap.test.js` and
  `wallet-web/src/lib/swap-server.test.js` reject seed, backup JSON, private
  keys, note openings, note files, and spend keys before any shielded endpoint
  submission.
- [x] Step 2: Asset registry generalization.
  Evidence: `wallet-web/src/lib/shielded-navswap.js` normalizes route-fed
  private asset registry entries with asset id, symbol, precision, issuer,
  NAV source, and policy hash. It can render `a652` from capability data, but
  does not hardcode or register `a652` as tradeable.
- [x] Step 3: Wallet-local Asset-Orchard note store.
  Evidence: `wallet-web/src/lib/shielded-navswap.js` adds encrypted local
  note-vault primitives, note states, reconciliation, spendable-note filtering,
  nullifier handling, and explicit P9 disabled status. Tests prove reload
  recovery from encrypted storage and that nullified notes are not reselected.
- [x] Step 4: Local prover boundary.
  Evidence: `wallet-web/src/lib/shielded-navswap.js` adds a loopback-only
  local prover client, K=15 readiness normalization, request forbidden-field
  checks, and returned `AssetOrchardSwapAction` JSON verification for chain,
  genesis, pool, anchor, counts, and forbidden cleartext. Tests prove remote
  prover URLs are rejected and action JSON with cleartext assets/openings is
  blocked.
- [ ] Step 5: Public ingress to private notes.
  Gate: not started. No public burn, ingress, egress, bridge, live swap,
  reserved flag, or `can_run: true` was enabled in this foundation slice.

### Step 5: Public Ingress To Private Notes

Work:

- For a public `a651` or `a652` balance, prepare an
  `AssetOrchardIngressV1` burn and shielded batch.
- Browser signs any public asset burn locally.
- Adapter relays/certifies and returns the ingress receipt.
- Wallet scans or records the new private note.

Acceptance:

- Public ingress labels asset, amount, source, and timing as public.
- The resulting private note is visible in the local note store.
- Duplicate ingress retries are idempotent and cannot burn twice without a new
  explicit user signature.

### Step 6: Quote And Liquidity Binding

Work:

- Define quote schema for `a651 <-> a652` that identifies liquidity mode:
  bilateral RFQ, operator inventory, pool-managed note, or issuer/reserve
  source.
- Bind quote expiry, policy hash, expected assets, expected values, output
  recipients, and failure mode into the wallet verification checklist.
- For first live beta, prefer a controlled pool-managed liquidity note with
  explicit trust copy over an ambiguous "private AMM" label.

Acceptance:

- Wallet refuses to prove a swap without a live liquidity commitment.
- Wallet copy says whether the counterparty/liquidity source is controlled,
  bilateral, or trustless.
- Quote replay after expiry fails before proving.

### Step 7: Private Swap Submit And Receipt Verification

Work:

- Submit the locally built `AssetOrchardSwapAction` through
  `shield-batch-swap`/certified batch relay or a wallet-safe RPC equivalent.
- Verify final receipt/certificate and update note state.
- Capture the serialized action and run the same privacy assertions used in
  the 12:54 UTC verification gate.

Acceptance:

- `a651 -> a652` completes and wallet shows private `a652` note.
- Captured action has no cleartext asset id, tag, amount, owner, recipient, or
  price.
- Conservation and duplicate-nullifier failure cases are covered in tests.

### Step 8: Reverse Direction

Work:

- Repeat the route for `a652 -> a651`.
- Verify note locks release after failure and after finality.
- Confirm balances/notes after reload.

Acceptance:

- Reverse private swap completes with the same privacy scan.
- Failed reverse attempt leaves no note in a false spendable state.

### Step 9: Egress And Bridge Boundary

Work:

- Keep "private swap" separate from "exit".
- Offer hold-private first.
- Offer direct private egress only with explicit copy: spent note opening
  hidden, public destination/asset/amount/timing visible.
- Offer bridge-out only after egress/public exit receipts exist.

Acceptance:

- User can stop after receiving private `a652`.
- Any public exit clearly labels what becomes public.
- Bridge-out is not auto-started by a private swap success.

### Step 10: End-To-End Evidence Gate

Work:

- Rerun StakeHub shielded NAV swap E2E on fixed main as operator-demo evidence.
- Run wallet-local private swap E2E twice:
  `a651 -> a652`, then `a652 -> a651`.
- Package evidence: action JSON, privacy scan, note state before/after,
  final receipts, no-private-material request logs, and reload/rescan proof.

Acceptance:

- Two runs pass with identical pass/fail summary fields.
- Evidence proves wallet did not send private material to remote services.
- Operator explicitly approves enabling `can_run` for live wallet route.

## Reserved Flags

These require explicit operator approval and must not be auto-built or enabled:

- Any live-money transaction, bridge, public burn, ingress, egress, or
  bridge-out.
- Enabling `can_run: true` for `shielded_navswap` in the production wallet.
- Any server/API design where a remote service receives user note openings,
  spend keys, private note files, decrypted backups, seeds, or passphrases.
- Any deployment of a public route claiming trustless/private status before the
  custody, P9 note encryption/scanning, privacy evidence, and route trust class
  gates are satisfied.
- Registering or exposing `a652` as tradeable without a verified asset id,
  issuer/NAV source, precision, liquidity source, and policy hash.
- Reusing StakeHub operator demo wallet keys, local key files, or global runner
  state as if they were user wallet custody.
- Changing verifier/prover parameters, circuit ids, or pinned hashes.
- Mainnet or public beta claims; current work is controlled pre-testnet.

P9 note encryption/scanning is not required for the first read-only wallet
capability increment. It is required before real multi-party private swaps,
reload-safe recipient discovery, or any route that claims the recipient can
recover output notes from chain data without local output-note file handoff.

## Implemented Foundation Increment

The approved foundation slice built Steps 1-4 only:

```text
Wallet Private NAVSwap capability/readiness slice:
show shielded_navswap as disabled/preflight-only, fetch route status, expose
why it is blocked, show required local-prover/note-scan/liquidity gates, and
add request-body tests that forbid private material.

Wallet-local foundation slice:
normalize route-fed private assets, including display-only a652 capability
entries; define encrypted local note-vault state/reconciliation; and define a
loopback-only local prover client that verifies Asset-Orchard swap action JSON
before any future relay.
```

This increment is useful immediately because it turns the current invisible
operator-demo state into explicit wallet route state without touching money,
signing, proving, deployment, or live private notes. Step 5 remains the next
review gate.

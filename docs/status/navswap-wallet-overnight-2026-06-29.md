# NAVSwap Wallet Overnight Progress - 2026-06-29

Scope: progress against `docs/plans/trustless-navswap-wallet-integration-spec.md`
`Overnight Job: No-New-Pool Wiring Pass`.

Current concise handoff: `docs/status/navswap-morning-handoff-2026-06-29.md`.
It refreshes target-wallet readiness, custody inventory, legacy pool
inspection, and completed-run stream evidence at `2026-06-29T14:46Z`.

## Summary

The wallet now has a concrete NAVSwap adapter surface instead of a fake
transparent swap. The proxy exposes `/api/navswap/*` route capability endpoints,
the browser wallet discovers those capabilities by default through the wallet
proxy, and the Swap screen shows route states explicitly.

The ESCROW-009 PFTL atomic-settlement path is now materially closer to a live
wallet route: the proxy can build a positive live `PFT <-> a651` template, the
node CLI can sign escrow fee quotes from custody `.key.json` files, and Python
escrow tooling now uses that key-file signer when available. The browser
wallet also has reviewed-leg escrow create/finish/cancel signing wired through
`TxBuilder`; live two-party completion still requires both counterparties to
submit their own legs.

The trustless transparent NAV route is now past the raw asset-submit blocker:
browser-signed issued-asset transactions can use
`mempool_submit_signed_asset_transaction_finality`, so wallet-owned
`trust_set` and `nav_subscription_allocate` actions can target certified
finality without giving the proxy wallet key material. The route has now been
deployed to the WAN devnet and smoke-tested end to end with the wallet-web
WASM signer, `TxBuilder`, the wallet proxy, the operator completion run, and
the live wallet asset feed.

Live funds were moved under the overnight budget: the smoke wallets were funded
with pfUSDC and each successful NAVSwap spent `6.958370` pfUSDC for `1` a651
through the transparent route. The specific browser wallet address
`pf124071fd53a12ca4556b7aa1f5ec98b585e73468` still needs a usable pfUSDC
trustline/balance before it can execute the same route interactively.

## Live E2E Evidence

Source commits now on `main`:

- `0e85502a` adds certified finality submission for externally signed asset
  transactions and wires wallet-web/wallet-proxy NAVSwap submission to it.
- `18de47e8` routes NAVSwap operator finality submits to the deterministic
  proposer instead of assuming validator-0.
- `46854765` skips redundant NAVSwap `trust_set` actions when the wallet
  already has a usable trustline, fixing the live `reserve_mismatch` failure.
- `b3bf06a6` combines the browser settlement-trustline and guarded pfUSDC
  funding steps into one explicit primary action once readiness says funding is
  configured and only blocked on the settlement trustline.
- The follow-up Swap screen change debounces transparent-readiness refreshes
  from live pfUSDC/a651 asset-feed changes while a quote is open, so externally
  observed funding or balance changes advance the readiness panel without a
  page refresh.
- NAVSwap run creation and guarded devnet pfUSDC funding now carry
  `idempotency_key` from the wallet client. The proxy replays same-key/same-body
  responses, rejects same-key/different-body conflicts, and shares in-flight
  duplicate requests, preventing browser retries from double-submitting the
  operator completion or funding legs.
- Completed NAVSwap idempotency responses are now also persisted to an
  append-only local proxy store and reloaded on restart, so same-key retries
  after a proxy restart still replay the original completed response instead of
  starting a duplicate run or funding request.
- Transparent completion receipts now include a
  `postfiat-navswap-receipt-verification-v1` object that binds the prepared
  wallet allocation action, wallet-submitted allocation receipt, live
  `nav_subscription` allocation, operator `nav_mint_at_nav` operation, fee
  quote, and operator tx id. The Swap result card shows the verification status,
  allocation id, and operator tx id.
- Auto-planned transparent prepared actions now carry quote freshness metadata:
  packet-fresh flags, market-ops status/epoch, NAV epoch, reserve packet hash,
  `quote_generated_at_ms`, and `quote_expires_at_ms`. The wallet refuses stale
  packet flags, stale proof status, invalid expiry ordering, or expired quotes
  before signing.

Deployment:

- Release binary SHA3-384
  `93b459dcd16ac332832b517ddc2621214325d4cd6ead0c65170a0ccf6568a9f36a48aeaf320bcc340f8db853e46b6fc1`
  was deployed to all six WAN devnet validators.
- The wallet proxy was restarted with `NAVSWAP_OPERATOR_ISSUER_KEY_FILE`
  configured, so transparent completion can sign the issuer-owned
  `nav_mint_at_nav` operator leg.
- After the live smoke run, all six validators converged at height `1369`,
  state root
  `74b07a4b9f4aebfb9002d9be572b0822071eac915517fb5bd1364e3f459bd384ad2473857ce46e222acd1804c1d6b94f`,
  block tip
  `6dcc919ea815259421f8bc45b73b4cf7205e0a7df4ca136dd9d35e463a1323f9c24d6510141f6d31b52d5636eb4a1802`,
  and empty mempools.

Live transactions:

- Test buyer:
  `pf07381735ddb7de134e8be8402b465c9cd8ec7546`.
- Issuer funding tx:
  `62d6f46166e0cc28b6a16c714af3fcf9e2a4c6d8c4150d15ffc28cf0b42fd4eb8ab9a9f76ce21ac2af694369f4b7760f`,
  funding the buyer with `10` pfUSDC through
  `mempool_submit_signed_asset_transaction_finality`.
- NAVSwap quote: `pfUSDC -> a651`, amount `1`, settlement
  `6958370` pfUSDC atoms (`6.958370` pfUSDC), mint amount `1` a651.
- Wallet-owned allocation tx:
  `df30ac04ec820d1531d02629900fcb62686db9a7c814f43fb6c5f5ea5428a69bae127cc2bf656e2c3e2df009b4f50e41`,
  accepted through certified finality.
- Operator mint tx:
  `0ee8d8a34fd9fee4bd9ec15174aa0afab43f681200ccb17e643226b48cb3967ae2e946d0fe5b9465284e971f28d9100a`,
  run id `navswap-mqz4ht16-cec0e265`, final status
  `operator_mint_submitted`.
- Buyer before the NAVSwap had `895` a651; after the run it has `896` a651.
  Its pfUSDC-WAN-v2 balance after funding and settlement is `6.123994`
  pfUSDC, matching the `13.082364 - 6.958370` spend.
- The wallet feed now supports `include_assets:true` and pushes `account_assets`
  snapshots over the existing `wallet_update` WebSocket path. A live probe
  against the funded test buyer returned pfUSDC and a651 balances in the
  wallet feed with `assets_error=null`.
- Current-chain wallet-code execution also passed through the wallet-web
  WASM/TxBuilder path. Wallet
  `pfac0562296948fbf35fec6d18d47498b412850a8c` was funded with PFT tx
  `d39f06352d3452b3e72bea8ab52078d9c257108a9521494985abad3b9e189929a239f0fcb18843385d39a3e2e22e70b4`,
  opened the canonical pfUSDC trustline with tx
  `565c3524eebc4e166e8675065cc8fe1871260e250228fb387295d54c4cf67fd46b97064c32648e410aa3a3c5b9eb198e`,
  received pfUSDC funding tx
  `20a09e42d573e71218246e018baf150296130828916ce5109f8a7c46877adce2d1ba408eddfd9576ce61d6c203632241`,
  then executed NAVSwap with wallet action txs
  `8ca921806dd0c0a56b5009fb54e690943db372d0d1851d2e1c89c8b35eec1d4be1bb3331964a379169bae9f29d03f3f0`
  and
  `5ba2ae2175f2b9e36d45fc7061c1e4b6443e54bd4a484bc959e9eed1d47c6f390cb9e3122144938a467e79d6d7241245`,
  operator tx
  `095610fec2230ca371af160927d05bfdc68c16e3010b7deb4d8666bf6317e5001c6de145abce0692d9174e53721eac48`,
  run id `navswap-mqz62mp7-184175bd`, and live-feed balance movement from
  `10000000` to `3041630` pfUSDC atoms plus `0` to `1` a651.

## Latest Checkpoint

Additional source work in the latest source commits:

- `vault_bridge_status` now exposes `nav_per_unit` and
  `circulating_supply` directly in the public RPC report. The transparent
  NAVSwap wallet planner needs those fields to compute the settlement spend
  for a requested NAV mint amount without reading validator data files.
- The wallet-proxy transparent subscribe planner now treats bare `amount` as
  the requested NAV/a651 mint amount, matching the Rust runner/protocol tests.
  Explicit `settlement_amount_atoms` or `amount_atoms` still force the raw
  settlement spend and must match the NAV-derived required settlement. The
  planner returns both `mint_amount_atoms` and `settlement_amount_atoms`.
- The proxy now builds the wallet-owned batch plus an operator completion
  template for the next leg, `nav_mint_at_nav`. The browser must not sign this
  leg; it is returned as `operator_completion` with an allocation lookup for
  the operator process to fill after the wallet's `nav_subscription_allocate`
  action lands.
- The proxy now has the transparent operator-completion run path:
  `POST /api/navswap/runs` accepts the reviewed transparent quote plus the
  wallet-owned batch submit result, verifies the submitted
  `nav_subscription_allocate` operation against the prepared quote, confirms a
  matching live `nav_subscription` allocation in public `vault_bridge_status`,
  fills `settlement_allocation_id`, and builds the issuer/operator-owned
  `nav_mint_at_nav` operation. With `NAVSWAP_OPERATOR_ISSUER_KEY_FILE`
  configured it quotes, signs through `postfiat-node
  wallet-sign-asset-transaction --key-file`, and submits the canonical
  `mempool_submit_signed_asset_transaction_finality` RPC request. Without that
  key it stops explicitly at `awaiting_operator_signature` after allocation
  verification.
- The wallet Swap route panel now shows `Required settlement`, `Wallet spend`,
  and `Operator leg` for transparent NAVSwap quotes. Before signing a prepared
  batch, it checks the loaded settlement asset balance and refuses the submit
  locally if the wallet does not have enough pfUSDC.
- The wallet now requests transparent auto-planning by default, submits the
  wallet-owned prepared batch, then starts an async transparent completion run
  and subscribes to the existing NAVSwap run SSE/polling feed for the operator
  leg status.
- Mutating NAVSwap adapter calls from the browser now include idempotency keys
  for run creation and guarded devnet pfUSDC funding. Duplicate requests with
  the same body replay the original response; conflicting reuse is rejected.
- Transparent run receipts now carry machine-checkable receipt verification for
  the wallet allocation, live allocation lookup, and operator mint. The wallet
  completion panel displays that proof state instead of only showing a raw
  receipt count.
- Transparent prepared actions now embed quote freshness/expiry metadata from
  the auto-planner. Wallet-local verification rejects stale or expired prepared
  actions before any signature is produced.
- The Swap screen now reads the prepared-action freshness metadata, shows quote
  freshness beside the prepared batch, switches the primary action to
  `Refresh readiness` after expiry, attempts one automatic readiness refresh for
  each expired quote, and refreshes again just before signing if the quote aged
  out while the user had the panel open.
- The global wallet feed now opts into issued-asset snapshots, and the Swap
  screen consumes those snapshots for pfUSDC/a651 balances. The Swap screen
  also refreshes issued-asset balances after wallet-signed NAVSwap action
  receipts and after terminal NAVSwap run events, so the manual browser test no
  longer depends on a page refresh to observe balance movement. When an open
  transparent quote is present, changed live pfUSDC/a651 feed balances now also
  trigger a debounced readiness refresh.
- Missing canonical pfUSDC/a651 after the asset feed loads is now treated as a
  visible zero balance in Swap, and NAVSwap submission is blocked locally before
  signing when the wallet lacks the required pfUSDC settlement amount.
- Added `scripts/navswap-wallet-live-smoke.mjs`, a committed wallet-style
  transparent NAVSwap smoke harness. Dry-run mode requires only a wallet
  address and proves quote readiness plus the live issued-asset feed; execution
  mode requires `--execute --wallet-backup-file ...` and uses the wallet-web
  WASM signer, `TxBuilder`, wallet-proxy `/api/navswap/runs`, and the live
  `wallet_update` asset feed.
- Dry-run evidence for
  `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` was written under
  `/tmp/navswap-pf124-readiness-20260629T113335Z`: quote status
  `prepared_actions_ready`, required settlement `6958370` pfUSDC atoms,
  prepared stages `trust_set, nav_subscription_allocate`, live-feed pfUSDC `0`,
  live-feed a651 `0`, and `settlement_sufficient=false`.
- Added `scripts/navswap-fund-pfusdc.mjs`, a guarded canonical pfUSDC funding
  helper for devnet readiness. It defaults to dry-run, checks the canonical
  pfUSDC issuer, checks the recipient trustline, refuses amounts above a
  configurable safety cap, and requires `--execute` plus the issuer key before
  it can move funds.
- Funding-helper dry-run evidence for
  `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` was written under
  `/tmp/navswap-pf124-fund-readiness-20260629T114210Z`: canonical pfUSDC
  issuer `pff3e396f771a8f490ca330e1720472d473bcfcb6d`, balance `0`, no
  trustline, `trustline_usable=false`. The next manual step for that wallet is
  to open the canonical pfUSDC trustline from the Swap screen, then rerun this
  helper with `--execute` under the overnight funding cap.
- The Swap screen now exposes a settlement-asset trustline action after a
  transparent quote. For `pfUSDC -> a651`, the wallet can open a pfUSDC
  trustline with the quoted required settlement limit before receiving pfUSDC
  funding; this is separate from the output a651 trustline action.
- Added `/api/navswap/readiness`, a read-only transparent-route readiness
  endpoint. It returns the live quote, route capability state, required
  settlement atoms, settlement balance/trustline status, prepared stages, and
  ordered next steps such as opening the canonical pfUSDC trustline, funding
  the wallet, or submitting prepared wallet-owned actions.
- The wallet Swap screen now uses the readiness endpoint for transparent
  quotes and displays compact execution readiness, next step, settlement
  balance, and funding-trustline rows. The signing path still uses the
  readiness response's prepared quote and keeps all wallet-owned signing local.
- After wallet-signed NAVSwap trustline or prepared-action submissions, the
  Swap screen now refreshes transparent readiness automatically and also offers
  a manual `Refresh readiness` control. This prevents stale UI after the target
  wallet opens the pfUSDC trustline; the panel should advance from
  `open the canonical settlement-asset trustline` to the next live blocker,
  typically funding the required pfUSDC.
- `scripts/navswap-wallet-live-smoke.mjs` now records
  `adapter-readiness.json` and includes the adapter readiness status/next
  steps in its dry-run and execution evidence, so CLI evidence and browser UI
  readiness use the same proxy contract.
- Live dry-run evidence for
  `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` using the new readiness
  endpoint was written under
  `/tmp/navswap-pf124-readiness-20260629T1215-readiness-endpoint`: route
  capability `quote_ready`, `can_run=true`, quote status
  `prepared_actions_ready`, required settlement `6958370` pfUSDC atoms, live
  pfUSDC `0`, live a651 `0`, settlement trustline missing, and adapter next
  steps `open the canonical settlement-asset trustline` then `fund the wallet
  with the required settlement asset`.
- Added `POST /api/navswap/devnet-fund-pfusdc`, a disabled-by-default wallet
  adapter funding helper for the transparent devnet route. When
  `NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING=true` and the configured issuer key
  matches canonical pfUSDC, it funds only the current readiness shortfall,
  refuses missing trustlines, enforces a per-request cap plus an in-memory
  per-recipient window cap, and submits through
  `mempool_submit_signed_asset_transaction_finality`. The Swap screen only
  shows the request button when readiness reports this helper as available.
- Fixed the wallet's transparent NAVSwap primary flow so
  `transparent_navswap.can_run=true` no longer makes the bottom action call
  `/api/navswap/runs` directly. Transparent NAVSwap now starts with quote and
  readiness, then shows one primary next action at a time: open/raise canonical
  pfUSDC trustline, request guarded pfUSDC funding, refresh readiness, or submit
  the wallet-owned action batch.
- Fixed transparent readiness trustline math for partially funded wallets:
  issued-asset receive capacity is now compared with the settlement shortfall,
  not the full settlement amount, and readiness reports
  `trustline_healthy` plus `receive_capacity_atoms`.
- Transparent readiness now also preflights native PFT fees/reserves for the
  prepared wallet-owned actions by quoting each action with `asset_fee_quote`.
  The readiness response includes `wallet_pft.balance_atoms`, per-action fee
  diagnostics, and blocks `can_execute` with the next step
  `fund the wallet with PFT for NAVSwap fees/reserves` when the wallet cannot
  afford the prepared trustline/allocation actions.
- `scripts/navswap-wallet-live-smoke.mjs --execute` now follows the same
  readiness-driven route as the browser. With a wallet backup it can open the
  canonical pfUSDC settlement trustline, request exact guarded pfUSDC funding
  through `/api/navswap/devnet-fund-pfusdc`, write funding/trustline evidence,
  submit the wallet-owned NAVSwap action batch, start operator completion, and
  verify live-feed movement from the funded pre-swap balance.
- The same smoke harness now has `--stream-run-id` for no-funds evidence of
  the wallet run stream. It records `/api/navswap/runs/{run_id}/stream`, run
  status, events, and receipts for an existing run. Evidence for
  `navswap-mqz62mp7-184175bd` was written under
  `/tmp/navswap-run-stream-20260629T-mqz62mp7`: terminal stream observed,
  `run_status=operator_mint_submitted`, `stream_event_count=1`, and
  `receipt_count=1`.
- Added `GET /api/navswap/runs?wallet_address=...` for wallet-scoped active
  run recovery. It returns the newest nonterminal runs for the current wallet by
  default, with terminal history available only when explicitly requested. The
  Swap screen uses this endpoint on load to reattach to the latest transparent
  NAVSwap run after a page refresh and resume the existing SSE/polling status
  path.
- NAVSwap capabilities now carry explicit route privacy metadata:
  `postfiat-navswap-route-privacy-v1` labels for transparent public wallet
  signing, disabled shielded operator-demo routing, operator-backed StakeHub
  smoke routing, public ESCROW-009 templates, disabled public Uniswap handoff,
  and legacy-pool inspection. The Swap screen displays the adapter-provided
  visibility and disclosure label instead of relying only on local route text.
- Added `scripts/navswap-redaction-check.mjs` plus a fixture test for NAVSwap
  public artifact hygiene. The checker scans NAVSwap docs and public-readable
  `/tmp/navswap-*` evidence for secret-bearing values, skips private evidence
  paths by default, and has `--include-private` for explicit private audits.
  Current public-artifact scan passed with `187` files scanned, `30` private
  paths skipped, and `0` findings after old scratch files containing local
  key-file paths were made private-readable.
- Dry-run evidence for the target wallet after this harness update was written
  under `/tmp/navswap-pf124-readiness-20260629T-smoke-autofund-dryrun`: quote
  status `prepared_actions_ready`, required settlement `6958370` pfUSDC atoms,
  funding endpoint enabled with exact amount `6958370`, and current blocker
  `settlement_trustline_not_usable`.
- Current target-wallet dry-run evidence was written under
  `/tmp/navswap-pf124-readiness-current-20260629T135039Z`. It confirms wallet
  `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` still has live pfUSDC `0`, live
  a651 `0`, no canonical pfUSDC settlement trustline, quote status
  `prepared_actions_ready`, required settlement `6958370` pfUSDC atoms, guarded
  funding enabled/configured for the exact shortfall, and unavailable reason
  `settlement_trustline_not_usable`. The next target-wallet action is the
  unlocked browser signing the canonical pfUSDC trustline; the wallet UI now
  offers that as `Open and fund pfUSDC`, then refreshes readiness and requests
  guarded funding if it becomes available.
- Added `scripts/navswap-custody-inventory.mjs`, a read-only custody inventory
  command for the overnight no-new-pool pass. It writes `inventory.json` and
  `inventory.md`, covers selected PFTL PFT/pfUSDC/a651 balances, Ethereum
  mainnet legacy a651/USDC/operator/pool balances, Arbitrum operator USDC/gas,
  and the old pfUSDC vault, and classifies each row by spendability. Live
  evidence was written under `/tmp/navswap-custody-inventory-full-check-v2`:
  the target wallet has `6.999955` PFT, `0` pfUSDC, and `0` a651; the operator
  has `3815.876054831038397894` legacy Ethereum a651, `5805.752289` Ethereum
  USDC, and `130.000099` Arbitrum USDC; the old Arbitrum pfUSDC vault has `0`
  USDC; the legacy Ethereum a651/USDC pool reports StateView liquidity `0`.
- Tightened the `uniswap_atomic_handoff` proxy gate so the legacy Ethereum
  `a651/USDC` pool is rejected even if it is accidentally configured through
  environment variables instead of passed in a request body. Capabilities now
  report `disabled_legacy_pool_rejected` for legacy-token/pool config and
  quotes fail with `legacy_pool_rejected`.
- Configured-but-disabled `uniswap_atomic_handoff` quotes now fail closed until
  recipient, minimum output, and deadline are supplied. Successful configured
  quotes include a hash-bound
  `postfiat-navswap-mint-and-swap-uniswap-quote-v1` object binding the pool or
  path, router, token in, token out, amount in, minimum output, recipient,
  deadline, and failure behavior while keeping `can_run=false` until verifier
  and receipt checks are implemented.
- Dry-run evidence for the funded smoke-test backup was written under
  `/tmp/navswap-buyer-readiness-20260629T113349Z`: the harness derived
  `pf07381735ddb7de134e8be8402b465c9cd8ec7546`, loaded the wallet WASM backup
  path, proved quote readiness, and reported canonical pfUSDC `6123994`, which
  is short of the `6958370` atoms required for another 1-a651 execution.
- Funding-helper dry-run evidence for the same smoke-test backup was written
  under `/tmp/navswap-buyer-fund-readiness-20260629T114210Z`: the canonical
  pfUSDC trustline exists, is authorized/unfrozen, has a `1000000000000` atom
  limit, and is usable for a guarded top-up.
- `/api/navswap/capabilities` now reports the remaining transparent-route work
  as `manual browser UI click-through from the target user wallet`, rather than
  the stale live-E2E evidence blocker.
- Fixed wallet-web submit error handling so non-`Error` signer failures still
  preserve partial submit results and failed-action metadata.
- Fixed reviewed-action matching for issued-asset and escrow operations by
  canonicalizing known integer operation fields, so node quotes using numeric
  strings compare equal to browser-reviewed numeric fields.
- Fixed wallet WASM serialization for signed transaction wrappers by returning
  JSON-parsed values. This preserves flattened nested fields such as
  `unsigned.chain_id`, which `serde_wasm_bindgen::to_value` was dropping from
  wallet-signed asset actions.
- NAVSwap `trust_set` prepared actions now include the canonical default
  fields `authorized:false`, `frozen:false`, and `reserve_paid:10`, matching
  the live transaction builder.
- Fixed the browser RPC client to submit signed issued-asset transactions with
  the node's canonical `signed_asset_transaction_json` parameter. The
  transparent wallet batch depends on this path for both `trust_set` and
  `nav_subscription_allocate`.
- Added certified/finality submission for externally signed issued-asset
  transactions: native RPC now accepts
  `mempool_submit_signed_asset_transaction_finality`, the certified mempool
  round can admit `signed_asset_transaction_json`, wallet-proxy routes that
  method as a finality submit, and wallet-web uses it for
  `TxBuilder.sendAssetTransfer(...)`. NAVSwap operator completion now submits
  the issuer-owned `nav_mint_at_nav` leg through the same finality method.
- Added the corresponding certified finality path for externally signed
  ESCROW-009 transactions:
  `mempool_submit_signed_escrow_transaction_finality`. The node certified
  mempool round can now admit `signed_escrow_transaction_json`, wallet-proxy
  forwards the method as finality, wallet-web prefers it for escrow submits,
  and Python atomic settlement can opt into it. The Python atomic settlement
  executor now waits for both escrow-create legs to be visibly open before
  revealing the shared fulfillment for finish legs.
- Added `scripts/navswap-atomic-settlement-live-smoke.py`, a guarded
  `PFT <-> a651` ESCROW-009 smoke harness. It defaults to dry-run, uses the
  wallet proxy WebSocket RPC by default, caps live amounts, and writes
  before/template/execution/receipt/escrow artifacts. Live `--execute` mode now
  targets the deployed validator/proxy build that contains
  `mempool_submit_signed_escrow_transaction_finality`.
- Hardened the finality parent-readiness wait used by proxy finality submits.
  If the proposer briefly cannot parse local status while an
  `ordered_commit_journal.json` is being created or removed, the wait now
  retries until the existing readiness timeout instead of aborting a valid
  multi-leg flow between certified blocks.

Validator redeployment for this checkpoint:

- Built and deployed release binary SHA3-384
  `93b459dcd16ac332832b517ddc2621214325d4cd6ead0c65170a0ccf6568a9f36a48aeaf320bcc340f8db853e46b6fc1`
  to all six WAN devnet validators.
- The validator hosts are currently running direct root-owned
  `postfiat-node` transport and RPC processes rather than active systemd
  services. Both process types were restarted with
  `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1`.
- All six validators were verified running the deployed hash, with matching
  height `1369`, state root
  `74b07a4b9f4aebfb9002d9be572b0822071eac915517fb5bd1364e3f459bd384ad2473857ce46e222acd1804c1d6b94f`,
  block tip
  `6dcc919ea815259421f8bc45b73b4cf7205e0a7df4ca136dd9d35e463a1323f9c24d6510141f6d31b52d5636eb4a1802`,
  and empty mempools after the live NAVSwap smoke run.

Escrow-finality redeployment for this checkpoint:

- Built and deployed release binary SHA-256
  `327ff19ca4111c6c2756a840f015a76e42a49e4ecad585ac729b76880b5871ad`
  to all six WAN devnet validators as `/usr/local/bin/postfiat-node`.
- Restarted the same manual root-owned transport/RPC command lines on each
  host with `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1`; each host retained a
  timestamped backup of the previous binary.
- Verified `mempool_submit_signed_escrow_transaction_finality` is live on the
  public RPC path: an empty-payload probe now reaches the method and returns
  `rpc_protocol_error: signed_escrow_transaction_json must be nonempty`, not
  `rpc_method_not_allowed`.
- Restarted wallet-proxy on PID `1687832` with
  `NAVSWAP_OPERATOR_ISSUER_KEY_FILE` and
  `NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING=true`.
- A first live execute run exposed a transient parent-readiness status error
  on the fourth leg after both create legs and the left finish had already
  applied. The signed right-finish leg was resubmitted once through escrow
  finality and accepted; evidence was written under
  `/tmp/navswap-atomic-settlement-live-smoke-exec-after-escrow-finality-20260629T133402Z`.
- After the retry hardening redeploy, a fresh guarded live
  `PFT <-> a651` ESCROW-009 smoke passed end to end through the wallet proxy:
  `/tmp/navswap-atomic-settlement-live-smoke-exec-patched-20260629T134215Z`.
  Both escrows reached `finished`, with accepted receipts for all four legs:
  left create
  `f2a0278c54ba7d4a21e353ace1df92337feb316ef5fcd0903de8f54bbd4517e6c87b21e7a81c9f560614ae2993571711`,
  right create
  `934bc3406c980e24949ba1895cc873cc613c7e3c78371523c9733f55cba4d9ee4c12e0b8d7b6caa9a0b495b0dcd70cb8`,
  left finish
  `11fbcf4eeafea3583b56ba2edccbfc8dcb2a84ca2a20475bb8710d6c533f1a71e71393ca15446057b1a6a147b5317fb4`,
  and right finish
  `ece675859ee33af73303eed434b00cffb87f70ea78cabe8a44ecfce2788ed0e36395e70db98a2b3121d3c71ced341d48`.
- Post-smoke all six validators converged at height `1384`, state root
  `8ee163ee1981720eb86e9577378f554652fc6091f8aea743ed4aa115acf002a3f6c08fae4851c123d3fbc168d5470b03`,
  block tip
  `7c73c18558f4d2da560c07d85ed62f3676919756465235ae275f7e517c4007fb7ca935a07df15cad2aa39e4562ed5d0f`,
  and empty mempools.

Live proxy quote after redeploy:

- `POST /api/navswap/quotes`, route `transparent_navswap`, `pfUSDC -> a651`,
  wallet `pf124071fd53a12ca4556b7aa1f5ec98b585e73468`, amount `1`, and
  `auto_plan:true` returned `prepared_actions_ready`.
- At live NAV epoch `38`, quote amount `1` a651 requires `6958370` pfUSDC
  atoms (`6.958370` pfUSDC) and returns `mint_amount_atoms=1`,
  `settlement_amount_atoms=6958370`, `nav_per_unit=695836990`, and reserve
  packet hash
  `cd51081cd9a88feb87910ef885a83f12d660e6c715953bd451593cb678dead0619dda085dee4510097c54e9c0a30fd1b`.
- The prepared wallet batch contains `trust_set` and
  `nav_subscription_allocate`; the returned `operator_completion` is the
  issuer/operator-owned `nav_mint_at_nav` template. The browser still must not
  sign that operator leg. For wallets that already have a usable a651 trustline,
  the planner now skips the redundant `trust_set` and prepares only
  `nav_subscription_allocate`.

Verification run:

- `node wallet-proxy/test_navswap_adapter.js`
- `node --check wallet-proxy/server.js`
- `cargo check -p postfiat-node`
- `npm test` in `wallet-web`
- `npm run build` in `wallet-web`
- `cargo build --release -p postfiat-node`
- `git diff --check`

New focused adapter coverage:

- transparent completion fails closed until the matching
  `nav_subscription` allocation is visible;
- transparent completion verifies the allocation and stops at
  `awaiting_operator_signature` when no issuer key is configured;
- configured operator completion signs through a key-file signer and submits
  the canonical `signed_asset_transaction_json` RPC parameter through
  `mempool_submit_signed_asset_transaction_finality`.
- browser RPC client coverage now asserts
  `submitSignedAssetTransaction(...)` uses `signed_asset_transaction_json`.
- browser RPC client coverage also asserts
  `submitSignedAssetTransactionFinality(...)` uses
  `mempool_submit_signed_asset_transaction_finality`, and tx-builder coverage
  proves asset transfers prefer the finality path and consume inline finality
  receipts without polling.

Committed PFTL source work:

- `eacb6281` adds the signed `market_ops_policy_register` operation, so a NAV
  issuer can register the market-operations policy needed before finalizing a
  live market-ops envelope.
- `44474919` adds `postfiat-node market-ops-operation-bundle`, which builds a
  replayable policy-register plus market-ops-finalize operation bundle from
  live NAV asset state and policy inputs.

Validator deployment:

- Built and deployed release binary
  `ff974f7c478ae8efc044067ae3cc39f972f04fdd878746c24f2ff1e0eb87e3a5` to all
  six WAN devnet validators.
- Restarted validators with `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1`, because
  the transport/RPC listeners intentionally reject public plaintext binds
  unless the controlled-testnet override is explicit.
- All six validators were verified running the deployed hash, with matching
  state at height `1365`, state root
  `61d04ee99a79cbe58bed1fa31bb00240883678e133691d6fe11aa0c54237219ccaef961c712c5a2ab33eb22cee5cec2d`,
  and block tip
  `45abcef38596ec4e377ff615f5e1730ce1f99df2fef4fa56ceb9fe7581cd1589da36580b9c1ea7242a2b1ff094aadfac`.

Verification run:

- `cargo fmt --check`
- `cargo test -p postfiat-execution market_ops_policy_register_allows_later_finalize`
- `cargo test -p postfiat-types`
- `cargo test -p postfiat-node market_ops_`

Live market-ops envelope submitted:

- Generated a live a651 market-ops operation bundle under
  `/tmp/navswap-market-ops-20260629T095612Z` on validator-0.
- Submitted issuer-signed `market_ops_policy_register` and
  `market_ops_finalize` asset operations in one certified batch. The CLI
  returned a final local-apply error because the batch was already applied, but
  all six validators converged to height `1366`, state root
  `0688de59f03f3d733f3d70efd9dc079e9c5b7fd41de21c86176c5a4429bd075f3b897d774ee997abc7118c30ac876bf8`,
  and empty mempools.
- Policy-register tx:
  `c1000adaa218faf0c81e30af29a5818e2b531fa559faf754953113578b45bb0f98b63caf07a7847f6194e74200eae876`,
  sequence `206`, fee `23`.
- Market-finalize tx:
  `fe838da9db9cae5d01b6b328e6a3608eba30c6a0329487bc0a7c5025296f4e0f9a3c830694ff1fde3b84c494f162834d`,
  sequence `207`, fee `30`.
- Finalized envelope hash:
  `e80af987d6c7874fca13939d6bd18bb5b36c9d86997f7e2ad024aaf9a8244c36d08f11ad65289b3e27898b6b20b435db`.
- Live `market_ops_status` is now `active` on all six validators with
  `current_mint_cap_atoms=100`,
  `current_reserve_deploy_cap_usd_e8=25000000000`,
  `nav_floor_usd_e8=661045140`, and envelope epoch `38`.
- Wallet-proxy `/api/navswap/planner-inputs` for `pfUSDC -> a651`, amount `1`,
  now returns a `trust_set` plus `nav_subscription_allocate` action plan. The
  selected live supply allocation had `13041630` atoms remaining while the
  backing receipt had `0` unallocated atoms, which exposed and fixed a planner
  selector bug: the consume-supply path must select by remaining
  `vault_bridge_supply` allocation, not by receipt unallocated capacity.
- Wallet-proxy `/api/navswap/quotes` with `auto_plan:true` now returns
  `prepared_actions_ready` and a two-action
  `postfiat-navswap-wallet-action-request-v1` batch for browser-local
  verification/signing.
- This checkpoint still does not deploy a new Uniswap pool, does not move EVM
  custody inventory, and does not claim the trustless Uniswap handoff route is
  live. The next blocker is wallet-side execution against an account that
  actually holds pfUSDC; the current tested wallet has no issued-asset balance.

## Implemented

- Added wallet-proxy NAVSwap adapter endpoints:
  - `GET /api/navswap/capabilities`
  - `GET /api/navswap/nav-proof`
  - `POST /api/navswap/quotes`
  - `POST /api/navswap/runs`
  - `POST /api/navswap/atomic-templates`
- Wired `GET /api/navswap/nav-proof` to read StakeHub `/api/navcoin` and
  `/api/navcoin/status` when `NAVSWAP_STAKEHUB_BASE_URL` is configured. Without
  that env it returns an explicit unavailable snapshot instead of fake proof
  freshness.
- `POST /api/navswap/quotes` now rejects the StakeHub transparent route when
  configured StakeHub NAV proof is missing or stale; successful quotes carry
  the normalized NAV proof summary used for the route decision.
- Successful StakeHub transparent quotes now also require and return a
  read-only preflight snapshot from StakeHub `/api/shielded-nav-swap/balances`
  and `/api/shielded-nav-swap/status`. Balance read errors block the quote
  instead of letting a live run fail later with missing prerequisites.
- StakeHub `/api/shielded-nav-swap/status` now exposes
  `transparent_roundtrip.finality_recovery_required` when the local PFTL node
  has a next-height proposal-vote lock and no configured timeout certificate.
  Wallet-proxy capabilities and quotes block this case with
  `stakehub_transparent_finality_recovery_required` before another live
  `vault_supply` attempt is forwarded, so the wallet can show the route as
  unavailable before the user presses the swap action.
- The finality recovery gate is now view-aware. A timeout certificate must
  match the latest next-height proposal-vote lock height and view. The route no
  longer re-enables merely because a view-0 timeout certificate exists after a
  later retry has produced a view-1 lock.
- StakeHub transparent status now includes artifact-derived recovery evidence
  for incomplete certified transport runs. The latest incomplete run reports
  its latest peer-certified transport round, including whether a block proposal
  was written, whether a block certificate was written, whether the round
  report exists, and the proposal height/view when available.
- Transparent roundtrip early failures now persist `roundtrip-failure.json` in
  the run directory, so a failed live action leaves durable evidence even if
  the HTTP caller disconnects or only sees a generic request failure.
- `POST /api/navswap/runs` now records StakeHub transparent runs in an adapter
  run journal and returns stable status, event, and receipt endpoints:
  - `GET /api/navswap/runs/{run_id}`
  - `GET /api/navswap/runs/{run_id}/events`
  - `GET /api/navswap/runs/{run_id}/receipts`
- The StakeHub transparent run endpoint now supports async execution for the
  live wallet. The wallet submits `async:true`, receives a `run_id`
  immediately, and consumes the run-scoped SSE stream while the proxy forwards
  the live action to StakeHub in the background. The original blocking run mode
  and JSON status/events/receipts polling remain available for deterministic
  adapter tests, fallback clients, and one-shot smoke tools.
- Adapter-managed runs now expose `GET /api/navswap/runs/{run_id}/stream`.
  The feed emits `navswap_run_snapshot`, `navswap_run_update`, and terminal
  `navswap_run_done` events. The wallet subscribes with `EventSource` and falls
  back to polling if streaming is unavailable.
- Added route gates for:
  - `transparent_navswap`: capabilities now publish the wallet prepared-action
    schema `postfiat-navswap-wallet-action-request-v1`, the allowed
    wallet-owned transparent NAV actions (`trust_set`,
    `vault_bridge_nav_subscription_allocate`, `nav_redeem_at_nav`), and the
    issuer/operator-owned actions the wallet must not sign
    (`nav_mint_at_nav`, `nav_redeem_settle`). With the current deployed finality
    build and configured operator key, the route reports `quote_ready` and can
    run after the browser wallet completes its readiness steps; no self-transfer
    fallback is allowed.
    The proxy also exposes `POST /api/navswap/actions/prepare` for wallet-owned
    transparent actions: preparing canonical `trust_set`,
    `nav_subscription_allocate`, and `nav_redeem_at_nav` requests after
    resolving the NAV asset issuer with `asset_info`. The live planner now
    selects the `pfUSDC -> a651` allocation inputs from public
    `vault_bridge_status` and `market_ops_status`; redeem planning remains a
    future extension.
  - `shielded_navswap`: operator-demo only.
  - `stakehub_transparent_roundtrip`: exposes the existing StakeHub transparent
    no-Orchard PFTL roundtrip as a separate operator-backed smoke route. It is
    not treated as the trustless browser-signed route. Quotes require
    `NAVSWAP_STAKEHUB_BASE_URL`; live runs additionally require
    `NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS=true`.
  - `pftl_atomic_settlement`: ESCROW-009 template route is exposed; run
    execution still requires each wallet to sign its own escrow-create leg.
  - `uniswap_atomic_handoff`: disabled until a bridge-aware wrapped NAVCoin
    token, handoff controller, verifier mode, router, and new Uniswap pool are
    configured.
  - `legacy_a651_uniswap`: inspection-only secondary liquidity; explicitly not
    the trustless PFTL-to-Uniswap route.
- Changed the wallet's default swap adapter URL to the wallet proxy:
  - local HTTP Vite: `http://<host>:8080`
  - HTTPS Vite: same-origin, with Vite proxying `/api/navswap` to `:8080`
- Removed the Swap screen's previous transparent handler that submitted an
  issued-asset self-transfer to the user's own address.
- Added the `StakeHub transparent` route to the wallet Swap screen and changed
  runnable NAVSwap routes to call `/api/navswap/runs` instead of silently
  falling back to quote-only behavior.
- The wallet Swap screen now refreshes NAVSwap capabilities every 10 seconds
  while open, shows StakeHub transparent preflight gates from
  `/api/navswap/capabilities`, and streams async run progress from
  `/api/navswap/runs/{run_id}/stream`. This removes the manual-refresh gap for
  route state and run progress.
- When StakeHub reports `transport_recovery_required`, the wallet preflight
  panel now shows the latest certified transport round status, proposal
  height/view, and retry-gate countdown.
- When StakeHub reports `needs_timeout_certificate`, the wallet preflight
  panel now shows the latest proposal-vote lock height/view and the required
  timeout-certificate view.
- Added browser client methods for the NAVSwap adapter.
- Added `wallet-web/src/lib/navswap-actions.js`, a wallet-local verifier for
  `postfiat-navswap-wallet-action-request-v1`. It rejects action requests that
  contain key-file/private-material fields, refuses source/wallet changes, and
  binds user-approved issuer/operator, assets, settlement amount caps, NAV
  epoch, and reserve packet hash before calling the asset transaction signer.
- Added `SwapServer.prepareNavswapAction(...)` for
  `/api/navswap/actions/prepare`, giving the browser a typed path to fetch
  prepared transparent NAV wallet actions. The proxy currently prepares
  `trust_set`, planner-fed `nav_subscription_allocate`, and planner-fed
  `nav_redeem_at_nav` actions.
- Added `POST /api/navswap/actions/prepare-batch` and
  `SwapServer.prepareNavswapActionBatch(...)` so the transparent route planner
  can hand the wallet one ordered set of stage inputs and receive one ordered
  set of canonical wallet-owned actions. Batch preparation is all-or-nothing:
  the proxy returns the failed index and no caller should sign the partial set.
- `POST /api/navswap/quotes` now supports `transparent_navswap` only when the
  request includes planner-fed action inputs. It returns a
  `prepared_actions_ready` quote with the prepared action batch. Without those
  inputs it still fails closed with `transparent_navswap_planner_inputs_required`
  and no self-transfer fallback.
- Added read-only RPC exposure for `market_ops_status` and
  `vault_bridge_status`, and extended `vault_bridge_status.allocations[]` with
  `released_atoms` and `remaining_atoms`. This gives the wallet proxy enough
  public state to evaluate live receipt/allocation capacity without reading
  validator data files.
- Added `POST /api/navswap/planner-inputs`. It discovers transparent planner
  inputs from live vault/market status, selecting an active counted settlement
  receipt plus a live `vault_bridge_supply` allocation with enough remaining
  capacity. `POST /api/navswap/quotes` can now opt into this discovery with
  `auto_plan: true`; plain transparent quotes still fail closed unless planner
  inputs are supplied.
- The Swap screen now has a transparent-route prepared-action submit path for
  the receiving trustline. It fetches a canonical `trust_set` action from the
  proxy, verifies it with the wallet-local NAVSwap verifier, signs it locally
  with `TxBuilder.sendAssetTransfer(...)`, submits it to PFTL, and displays the
  tx/receipt state.
- Added a generic wallet-local prepared-action batch submitter. It verifies the
  entire action set before producing any signature, then signs/submits
  wallet-owned actions sequentially and returns partial results if a later
  submit fails. This is the wallet primitive needed for planner-fed allocation
  and redeem action sets.
- The Swap screen can now consume a `prepared_actions_ready` transparent quote,
  show the ordered quote batch, wallet-verify the full set, sign locally, submit
  the wallet-owned actions sequentially, and display batch progress, submitted
  tx count, last tx, failed stage, and partial results. The remaining target
  blocker is not batch UI orchestration; it is the target browser wallet opening
  the canonical pfUSDC trustline, receiving guarded pfUSDC funding, and then
  completing the visible click-through while the run/feed streams update.
- Hardened `TxBuilder.sendAssetTransfer(...)` so asset fee quotes must return
  the same source and operation the user reviewed. The wallet now refuses
  source, amount, destination, or policy substitution before asset signing, the
  same way the escrow path already did.
- Fixed SDK escrow quote validation so omitted zero `finish_after` /
  `cancel_after` fields are accepted consistently with the serialized escrow
  transaction schema.
- Added `postfiat-node wallet-sign-escrow-transaction --key-file ...` for
  escrow fee quotes. This matches existing key-file signing for asset and offer
  transactions and works with runbook custody `.key.json` files.
- Updated Python `submit_escrow_transaction(...)` to prefer the node key-file
  escrow signer when `wallet.key_file` exists, while retaining the SDK backup
  fallback.
- Added Python `load_wallet(...)` backup chain-id validation so stale backups
  fail fast instead of surfacing later as signer errors.
- Added browser WASM exports for `wallet_sign_escrow_transaction` and
  `wallet_sign_escrow_transaction_fields`, regenerated the live wallet WASM
  artifact, and added `TxBuilder.sendEscrowTransaction(...)` plus RPC client
  methods for `escrow_fee_quote` and
  `mempool_submit_signed_escrow_transaction`. The browser now has a local
  signer/submission primitive for the ESCROW-009 create/finish/cancel legs;
  the Swap UI still needs the approval/leg-execution flow wired on top.
- Wired the wallet Swap screen's `PFTL atomic` result card to identify the
  escrow-create leg owned by the current wallet and submit that reviewed leg
  through `TxBuilder.sendEscrowTransaction(...)`. The signer refuses quote
  source, sequence, or operation substitution before signing. This moves the
  route from template-only to browser-local submission of the current wallet's
  own create leg. Counterparty create-leg coordination, shared-fulfillment
  finish, and cancel/recovery UX remain pending.
- Added wallet-side construction for ESCROW-009 finish and cancel operations
  from the reviewed template. The Swap screen now offers `Finish incoming
  escrow` and `Cancel my escrow` actions. Before signing a finish, the wallet
  reads `escrow_info` for both the wallet-owned escrow and the incoming
  counterparty escrow and refuses to reveal the fulfillment unless both are on
  ledger and open. This still lacks cross-wallet coordination and durable
  settlement-state persistence, but the browser now has local create, guarded
  finish, and cancel transaction paths for the atomic route.

## Custody Inventory

Read-only calls only. No funds were moved.

### Ethereum mainnet legacy a651 venue

Source: `https://ethereum.publicnode.com`, `cast call` / `cast balance`.

| Address | Asset / object | Raw balance | Human balance | Classification |
| --- | --- | ---: | ---: | --- |
| `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | ETH | `303860214726445495` wei | `0.303860214726445495 ETH` | spendable if StakeHub EOA key is available |
| `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | a651 `0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e` | `3815876054831038397894` | `3815.876054831038397894 a651` | spendable if StakeHub EOA key is available |
| `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | USDC `0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48` | `5805752289` | `5805.752289 USDC` | spendable if StakeHub EOA key is available |
| `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | Uniswap v4 PositionManager NFT balance | `1` | `1 position` | LP-positioned; token id not resolved because `tokenOfOwnerByIndex` reverted |

The legacy pool id remains
`0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84`.
It is usable only as historical/secondary liquidity inspection. It is not the
trustless PFTL-to-Uniswap handoff route.

Current read-only Uniswap v4 inspection:

- StateView `0x7ffe42c4a5deea5b0fec41c94c136cf115597227`, per official Uniswap
  v4 Ethereum deployment docs.
- `getSlot0(pool_id)` returned `sqrtPriceX96 =
  264840598407943562605399`, `tick = -252187`, `protocolFee = 0`,
  `lpFee = 500`.
- `getLiquidity(pool_id)` returned `0`.
- a651 token metadata: name `a651`, symbol `a651`, decimals `18`, total supply
  `4000000000000000000000`.
- PoolManager aggregate a651 ERC-20 balance:
  `93936271343217656` raw (`0.093936271343217656 a651`).
- The PoolManager USDC ERC-20 balance is aggregate across all v4 pools, so it
  is not used as a pool-specific reserve read.

Conclusion: the legacy Ethereum a651/USDC pool is currently inactive as active
swap liquidity. It should remain visible as historical secondary liquidity, but
wallet `uniswap_atomic_handoff` must stay disabled and must not route to this
pool.

### Arbitrum bridge/roundtrip custody

Source: `https://arb1.arbitrum.io/rpc`, `cast call` / `cast balance`.

| Address | Asset | Raw balance | Human balance | Classification |
| --- | --- | ---: | ---: | --- |
| `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | ETH | `8943968515780000` wei | `0.00894396851578 ETH` | gas inventory, spendable if StakeHub EOA key is available |
| `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | USDC `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | `130000099` | `130.000099 USDC` | enough for sub-USD-100 smoke tests |
| `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9` | ETH | `0` | `0 ETH` | empty bridge vault |
| `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9` | USDC `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | `0` | `0 USDC` | empty bridge vault |

### PFTL WAN devnet accounts

Source: public WAN RPC `192.0.2.10:27650`, read-only `account` and
`account_assets`.

| Account | PFT atoms | pfUSDC raw | a651 raw | Classification |
| --- | ---: | ---: | ---: | --- |
| `pf07381735ddb7de134e8be8402b465c9cd8ec7546` | `3639` | `3082364` | `895` | runbook buyer account; has pfUSDC/a651 inventory |
| `pf65c9783ceafc0f519a74195e78cc7909f92429c3` | `5251622` | `0` | `1100` | runbook holder account; has a651 inventory |
| `pff3e396f771a8f490ca330e1720472d473bcfcb6d` | `9999568` | `0` | `0` | runbook issuer account; gas only in this read |
| `pfa95c2c765a41b24867b23703ac688d9eaa8a9264` | `69976857` | `60003476` | `3` | PFTL custody-like account from StakeHub status docs; has pfUSDC |
| `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` | `6999955` | `0` | `0` | current tested wallet account; no issued assets |

## Route And Adapter Checks

Local proxy smoke on alternate port `18080`:

- `GET /api/navswap/capabilities` returned
  `postfiat-navswap-capabilities-v1`.
- `POST /api/navswap/quotes` for `uniswap_atomic_handoff` against legacy pool
  `0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84`
  returned `legacy_pool_rejected`.
- `POST /api/navswap/quotes` for `pftl_atomic_settlement`, `PFT -> a651`,
  returned `template_ready` with next endpoint
  `/api/navswap/atomic-templates`.
- `stakehub_transparent_roundtrip` is now visible in capabilities:
  - without `NAVSWAP_STAKEHUB_BASE_URL`, status is
    `operator_not_configured`;
  - with `NAVSWAP_STAKEHUB_BASE_URL`, status is `operator_quote_only`;
  - live forwarding remains blocked unless
    `NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS=true`;
  - fractional amounts are rejected because the current StakeHub
    `transparent_roundtrip` action accepts whole a651 smoke amounts only.
- `GET /api/navswap/nav-proof?asset_id=a651` is now a configured passthrough:
  - without `NAVSWAP_STAKEHUB_BASE_URL`, returns `proof_available:false` and
    the missing proof-source reason;
  - with `NAVSWAP_STAKEHUB_BASE_URL`, normalizes `a651` to the PFTL asset id,
    reads StakeHub `/api/navcoin` plus `/api/navcoin/status`, and returns chain
    id, PFTL height, NAV epoch, reserve packet hash, freshness deadline height,
    NAV/unit, supply, proof status, staleness, and source receipt hashes.
- StakeHub transparent quotes/runs now require that same proof to be available
  and non-stale before forwarding to the StakeHub action endpoint.
- StakeHub transparent quotes now include the operator preflight snapshot:
  PFTL address, pfUSDC balance, a651 balance, and current swap runner status.
- Adapter-managed run status is now available for the StakeHub transparent
  route. The run journal records proof check, StakeHub forward start, final
  completion/failure event, and a StakeHub result receipt when the runner
  returns one.
- Async adapter run status is covered by a fake StakeHub test that deliberately
  holds the action response open. The proxy returns `status: running` and a
  usable `run_id` before the fake StakeHub response is released, then updates
  the same run journal to `transparent_complete` when the background action
  completes.
- Live async smoke after the polling work:
  - `POST /api/navswap/runs`, route `stakehub_transparent_roundtrip`, amount
    `1`, `async:true` returned immediately with
    `run_id=navswap-mqyrwe6b-8ab86e14`.
  - The wallet/proxy event journal recorded `run_started`,
    `nav_proof_checked`, `async_run_accepted`, `stakehub_forward_started`, and
    then `run_failed`.
  - Failure: `peer certified batch round local vote failed: conflicting block
    proposal vote already recorded for validator validator-0 at height 473
    view 1`.
  - StakeHub wrote durable evidence at
    `$STAKEHUB_STATE/shielded-nav-swap/transparent-no-orchard/stakehub-transparent-20260629T052421Z-93d4c92e/roundtrip-failure.json`.
  - Resulting required next recovery artifact: a timeout certificate for height
    `473`, view `1`; the existing height `473`, view `0` timeout certificate is
    no longer sufficient.

Read-only local StakeHub smoke:

- Endpoint: `http://127.0.0.1:8787`
- `GET /api/navcoin` returned a fresh PFTL NAV reserve packet:
  - proof status: `fresh`
  - stale: `false`
  - epoch: `35`
  - current height: `472`
  - freshness deadline height: `100469`
  - NAV/unit: `6.95315504`
  - supply: `4000`
  - reserve packet hash:
    `1f5a34c58e7a49dc65f5e0e6ab63403f6a51d663334e379e9b9f46333790faebde3931dd02594c2a1197e02bb84c4f37`
- `GET /api/shielded-nav-swap/nav-check?phase=before` returned
  `ok:true`, `fast_path:true`, `proof_status:fresh`, matching the same reserve
  packet hash.
- `GET /api/navcoin/status?asset_id=<a651>` still returned
  `market_operations_status: unavailable` because no finalized market-ops
  envelope is present. The proxy quote gate accepts the `/api/navcoin` proof
  freshness, but this remains a prerequisite to resolve for production market
  operations policy checks.
- Read-only proxy helper quote with
  `NAVSWAP_STAKEHUB_BASE_URL=http://127.0.0.1:8787` succeeded for
  `stakehub_transparent_roundtrip`, `pfUSDC -> a651`, amount `1`, returning
  `operator_quote_only` with the fresh proof summary. No funds were moved.
- After preflight wiring, the same read-only quote returned:
  - operator address: `pfa95c2c765a41b24867b23703ac688d9eaa8a9264`
  - pfUSDC balance atoms: `60003476`
  - a651 balance atoms: `3`
  - swap runner status: `idle`

Live StakeHub transparent run attempt:

- Route: `stakehub_transparent_roundtrip`
- Amount: `1` whole a651, under the USD 100 live-action cap
- Proxy run id: `navswap-mqyp8wku-f6a905cc`
- Endpoint: `http://127.0.0.1:8787/api/shielded-nav-swap/action`
- Result: failed before transparent roundtrip completion
- Stage: `vault_supply`
- Error:
  `transport block vote request to validator-1 failed after 1 attempts:
  response read failed: Resource temporarily unavailable (os error 11)`
- Run directory:
  `$STAKEHUB_STATE/shielded-nav-swap/transparent-no-orchard/stakehub-transparent-20260629T041006Z-62176b3e`
- Artifacts present: vault-supply prepared request files and synthetic deposit
  receipt evidence.
- Artifacts absent: no `pftl-only-summary.json`, no bridge-out resume file, no
  primary mint receipt, no NAV exit redemption id.
- The proxy accepted fresh NAV proof and recorded run events correctly; the
  failure is in the StakeHub/PFTL vault-supply certification step. The proxy
  now surfaces StakeHub `error` text directly for failed runs instead of
  returning the generic "response received" message.
- Retry:
  - proxy run id: `navswap-mqypc3gu-3e940873`
  - run directory:
    `$STAKEHUB_STATE/shielded-nav-swap/transparent-no-orchard/stakehub-transparent-20260629T041235Z-2582d47b`
  - result: failed again at `vault_supply`
  - error:
    `peer certified batch round local vote failed: conflicting block proposal
    vote already recorded for validator validator-0 at height 473 view 0`
  - no `pftl-only-summary.json`, no bridge-out resume file, no primary mint
    receipt, and no NAV exit redemption id were produced.
  - stopped after this retry to avoid repeated live attempts against a
    certification-height conflict.
- StakeHub code fix:
  - commit `fab2624` in `$STAKEHUB_REPO`
  - transparent certified-asset-ops now preserves `--height`, `--view`, and
    `--timeout-certificate-file` from base args.
  - it will retry a conflicting proposal vote at the next view only when a
    timeout certificate is configured; without one, it returns the original
    conflict plus `nonzero-view retry requires --timeout-certificate-file`.
  - focused tests passed:
    `PYTHONPATH=. python3 -m pytest tests/test_dashboard_server.py -k
    'transparent_roundtrip_certified_ops_requires_timeout_cert_for_view_retry
    or transparent_roundtrip_certified_ops_retries_next_view_with_timeout_cert
    or transparent_roundtrip_vault_ops_split_separates_challenge_window_phases
    or transparent_roundtrip_runs_pftl_only_without_orchard
    or transparent_roundtrip_pfusdc_preflight_issues_public_balance'`
- Post-restart smoke:
  - detached StakeHub dashboard is running on `127.0.0.1:8787`.
  - read-only `stakehub_transparent_roundtrip` quote still succeeds with
    `proof_status:fresh` and `preflight_ok:true`.
  - one 1-a651 live run with the loaded retry code produced proxy run id
    `navswap-mqypn1z4-234714c0` and failed at `vault_supply` with
    `nonzero-view proposal requires timeout certificate evidence`.
  - no summary, bridge-out resume, primary mint receipt, or NAV exit redemption
    id was produced. This confirms the remaining blocker is a PFTL timeout
    certificate/finality recovery requirement for height `473`, not missing
    wallet/proxy route plumbing.
- Follow-up finality gate:
  - StakeHub commit `b4c33d6` adds a read-only transparent-roundtrip readiness
    summary to `/api/shielded-nav-swap/status`.
  - The readiness check reads the configured PFTL data dir, local
    `block_proposal_vote_locks`, current node status, and configured
    `--timeout-certificate-file`.
  - On the live devnet state it identifies finalized height `472` plus a
    validator-0 proposal-vote lock at height `473`, view `0`, and reports
    `needs_timeout_certificate` unless a timeout certificate file is configured.
  - Wallet-proxy dynamic capabilities and quote preflight now treat that status
    as a hard blocker with code
    `stakehub_transparent_finality_recovery_required`.
  - Tests added:
    `test_transparent_roundtrip_status_reports_timeout_cert_required`,
    `test_transparent_roundtrip_status_accepts_configured_timeout_cert`,
    `testStakehubTransparentQuoteBlocksFinalityRecovery`, and
    `testStakehubTransparentCapabilitiesBlockFinalityRecovery`.
- Timeout-certificate recovery:
  - Local artifact:
    `$STAKEHUB_STATE/navswap-finality-recovery/height-473-view-0/timeout-certificate.json`
  - Derived base args:
    `$STAKEHUB_STATE/navswap-finality-recovery/height-473-view-0/phase1-base-args-with-timeout-certificate.json`
  - Certificate: height `473`, view `0`, quorum `5/6`, votes from
    `validator-0` through `validator-4`, high-QC id
    `b6231c5451577ff1f2400f95cfcfa22b68878c1c89653799d1e24eb6f7a1dbf524d0bb2b4416137cd1964f2455dd1f85`.
  - Certificate id:
    `a7dcb9b15bfd7ea3dbdadd7beae7c8c303241d66e8f0b743159a07861c30d8238a950dba2c3fac940594482c5a63b302`.
  - With StakeHub restarted against the derived base args,
    `/api/shielded-nav-swap/status` reports
    `transparent_roundtrip.status=ready`, and wallet-proxy
    `/api/navswap/capabilities` returns `can_quote:true`, `can_run:true` for
    `stakehub_transparent_roundtrip`.
  - Wallet-proxy quote for 1 a651 returned HTTP `200`, `proof_status:fresh`,
    and `preflight_ok:true`.
- Additional live smoke results:
  - First timeout-certificate run, proxy run id
    `navswap-mqyqdviu-c863bb6c`, failed at `vault_supply` because the
    timeout certificate was incorrectly attached to the initial view-0
    proposal: `view 0 proposal must not include timeout certificate evidence`.
    StakeHub commit `9e8aac7` fixed this by attaching timeout evidence only on
    nonzero-view retry.
  - Second run, proxy run id `navswap-mqyqf6u5-da162362`, reached view `1`
    with the timeout certificate and then failed at `vault_supply` on remote
    transport to `validator-1`: `Resource temporarily unavailable (os error
    11)`. StakeHub commit `58e1e18` adds resilient retry flags and direct
    view-1 recovery when a timeout certificate is configured.
  - Follow-up direct-view-1 attempts reached the expected command shape
    (`--view 1 --timeout-certificate-file ...`) but were manually stopped after
    the remote transport call remained in-flight. No `pftl-only-summary.json`,
    bridge-out resume, primary mint receipt, or NAV exit redemption id was
    produced.
  - Current remaining blocker is no longer wallet/proxy route plumbing or
    missing timeout-certificate evidence. It is WAN validator transport
    responsiveness during the view-1 vault-supply certified-asset-ops round.
  - Follow-up hardening added a time-bound `transport_recovery_required` status
    for the StakeHub transparent route. If the latest transparent run has
    certified transport artifacts but no `pftl-only-summary.json`, StakeHub
    reports `transparent_roundtrip.transport_recovery_required=true`, and
    wallet-proxy disables `stakehub_transparent_roundtrip` capabilities, quotes,
    and runs with `stakehub_transparent_transport_recovery_required` instead of
    stacking another live validator transport attempt.
  - The transparent certified-operation transport knobs are now explicit:
    `STAKEHUB_NAV_TRANSPARENT_TRANSPORT_TIMEOUT_MS`,
    `STAKEHUB_NAV_TRANSPARENT_TRANSPORT_SEND_RETRIES`,
    `STAKEHUB_NAV_TRANSPARENT_TRANSPORT_RETRY_BACKOFF_MS`, and
    `STAKEHUB_NAV_TRANSPARENT_INCOMPLETE_RUN_COOLDOWN_SECS`.

Existing StakeHub route mapped for wallet adapter use:

- Action endpoint: `/api/shielded-nav-swap/action`
- Action body: `{ "action": "transparent_roundtrip", "amount": "<whole a651>" }`
- Underlying StakeHub runner: `nav-roundtrip-live-demo --pftl-only`
- Custody boundary: `stakehub-operator-wallet`, not browser-local signing
- Current wallet proxy env:
  - `NAVSWAP_STAKEHUB_BASE_URL`
  - `NAVSWAP_STAKEHUB_ACTION_PATH`
  - `NAVSWAP_STAKEHUB_NAVCOIN_PATH`
  - `NAVSWAP_STAKEHUB_NAVCOIN_STATUS_PATH`
  - `NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS`
  - `NAVSWAP_STAKEHUB_MAX_A651_AMOUNT`
  - `NAVSWAP_STAKEHUB_TIMEOUT_MS`
  - `NAVSWAP_STAKEHUB_READ_TIMEOUT_MS`

Wallet client methods added for run tracking:

- `swapServer.getNavswapRun(runId)`
- `swapServer.getNavswapRunEvents(runId)`
- `swapServer.getNavswapRunReceipts(runId)`

Live RPC read-only template reachability:

- `atomic_settlement_template` is reachable through the WAN RPC.
- Dummy-account attempt failed with source account not found.
- Known-wallet same-owner/same-recipient attempt failed with
  `escrow_create.owner must differ from recipient`.
- Positive live template proof with two runbook accounts succeeded:
  - buyer: `pf07381735ddb7de134e8be8402b465c9cd8ec7546`
  - holder: `pf65c9783ceafc0f519a74195e78cc7909f92429c3`
  - pair: `PFT <-> a651`
  - schema: `postfiat-atomic-settlement-template-v1`
  - left escrow id:
    `1ebce027f33e91c01bb33b5198f080ce2771fe275e5d367b4be75385825fdc8527de25daa088c549681fc4d314005549`
  - right escrow id:
    `7ebbca949b48494b93c2272f496f7676f0f11279500fd05b2ee0069d66721b44c993ae0d24200b36c83116d7021c5531`

## Atomic Settlement Signing And Smoke

Live one-atom attempt:

- Endpoint: `192.0.2.10:27650`
- Start height: `1338`
- Cancel height: `1458`
- Value: `1` PFT atom for `1` raw a651 unit
- Condition hash:
  `c8f2fb6010385bbf851e1aa4da636c46c1c3126b354c1de8a3f336312da32f81dbe54ffae2277cd7e9603fd45fae2ac7`
- Settlement id:
  `2031558c0b6ee62585e9fa07b7252168fea3044520ff2b6fd2316bdb153a80fe7dc2a3bd8596612d44a5c5d1e7707461`
- Result: template built and the buyer create leg signed locally from
  `buyer.key.json`; submit stopped before state mutation because the WAN RPC
  returned `rpc_method_not_allowed` for
  `mempool_submit_signed_escrow_transaction`.

Local-equivalent proof:

- `cargo test -p postfiat-node
  atomic_settlement_template_builds_pft_issued_swap_through_escrow_rails`
  passes. This test builds the PFT/issued-asset template, submits both escrow
  create legs locally, finishes both legs with the shared fulfillment, and
  verifies the resulting receipts/account history path.

## Live Wallet Adapter Success

Later 2026-06-29 update: the live wallet adapter path completed a
`stakehub_transparent_roundtrip` run end to end.

- Wallet-proxy run id: `navswap-mqyvb4gv-a971ebe5`
- Route: `stakehub_transparent_roundtrip`
- Pair: `pfUSDC -> a651`
- Amount: `1`
- Final adapter status: `transparent_complete`
- Final message:
  `Transparent no-Orchard PFTL roundtrip completed: public pfUSDC, public a651 mint, public NAV exit.`
- StakeHub run dir:
  `$STAKEHUB_STATE/shielded-nav-swap/transparent-no-orchard/stakehub-transparent-20260629T065947Z-3cce5f6c`
- Summary:
  `$STAKEHUB_STATE/shielded-nav-swap/transparent-no-orchard/stakehub-transparent-20260629T065947Z-3cce5f6c/pftl-only-summary.json`
- Summary status: `final_summary_ok=true`, `failure_reasons=[]`,
  `completion_status=on_pftl_complete_bridge_out_deferred`.
- Bridge-out resume:
  `$STAKEHUB_STATE/shielded-nav-swap/transparent-no-orchard/stakehub-transparent-20260629T065947Z-3cce5f6c/bridge-out-resume.json`
- NAV exit redemption id:
  `a6e7a8420deebf11509cded2c9755bf9dfacc91d541a635ae6e865f6c451b244ae149f79b28d4dbc1b4cc70fa61e8aed`
- Primary mint settlement receipt id:
  `1f5d9a0536edac979a253d394b5376e1cf3de53f9472820634e491f9c3bf5708af0565231538740e17b3d9d843a63d3c`
- Final public fleet state after manual validator-0 state sync: validators
  `0` through `5` all report height `1365` and state root
  `61d04ee99a79cbe58bed1fa31bb00240883678e133691d6fe11aa0c54237219ccaef961c712c5a2ab33eb22cee5cec2d`.
- StakeHub `/api/shielded-nav-swap/status` reports
  `transparent_roundtrip.status=ready`.
- Wallet-proxy `/api/navswap/capabilities` reports
  `stakehub_transparent_roundtrip.status=operator_run_enabled`,
  `can_quote=true`, and `can_run=true`.

Fixes required to reach this state:

- `postfiatl1v2` commit `61bad782`: accepts the operator-local validator state
  as quorum evidence when the duplicate public self endpoint lags, reports
  transport validator rejections instead of masking them as batch-ack parse
  errors, and treats quorum-early unresolved vote targets as strict-mode safe
  when local apply and full certified sends are verified.
- `StakeHub` commit `405e2b9`: forces transparent NAVSwap runs through the
  transparent transport normalizer so `--quorum-early-full-propagation`,
  `--local-apply-before-certified-send`, and `--allow-existing-mempool` are
  present even when older base args omit them.

## Python Atomic Settlement Orchestrator

Later 2026-06-29 update: the Python tooling now has a standard ESCROW-009
orchestrator for `PFT <-> issued asset` settlement.

- Helper: `postfiat_rpc.wallet.execute_atomic_settlement`.
- It builds the canonical `atomic_settlement_template`, submits the left
  wallet's template `escrow_create` with the left wallet, submits the right
  wallet's template `escrow_create` with the right wallet, and only then
  finishes both escrows with the shared fulfillment.
- The helper uses the template leg sequences and escrow ids, so the create
  transactions match the settlement template instead of being reconstructed
  ad hoc.
- Rejected create legs stop execution before the fulfillment is revealed.
- Public export: `postfiat_rpc.execute_atomic_settlement`.

## Wallet Atomic Template Builder

Later 2026-06-29 update: the wallet `PFTL atomic` route now calls the
wallet-proxy `/api/navswap/atomic-templates` adapter.

- The wallet route collects counterparty, counter amount, cancel height, and
  condition alongside the existing from/to asset selectors.
- It builds a `PFT <-> issued asset` ESCROW-009 template through the adapter
  and displays the settlement id, condition hash, and both escrow ids.
- Browser-side escrow signing is wired for the wallet-owned create leg, the
  incoming finish leg, and the wallet-owned cancel path. The wallet only signs
  operations from the reviewed template and `TxBuilder` rejects fee-quote
  operation or sequence substitution before signing.
- Full browser settlement is still a two-party workflow: the user can submit
  only the leg owned by the unlocked wallet, and finish requires both create
  escrows to be open.
- The tested two-wallet execution path remains the Python
  `execute_atomic_settlement` helper, which submits both create legs and reveals
  the fulfillment only after both creates are accepted.
- Atomic template amounts are validated as positive whole raw units before the
  request is sent to the proxy.
- Read-only live smoke helper:
  `node scripts/navswap-atomic-template-smoke.mjs --out-dir /tmp/navswap-atomic-template-smoke-...`.
  It proves the live proxy/RPC template schema, condition hash, symmetric
  settlement id, and distinct escrow ids without signing or moving funds.

## Live Planner RPC Deployment

Later 2026-06-29 update: the WAN devnet validators now run the node binary
that exposes the transparent NAVSwap planner read methods.

- Built `target/release/postfiat-node` from the current `postfiatl1v2` source
  and deployed it to all six all-Vultr validator hosts as
  `/usr/local/bin/postfiat-node`.
- Installed binary SHA-256 on validators `0` through `5`:
  `3c14dfa9eb4db172fb28da02db931e364caf69c38b9a93ea8b0323c857a130fd`.
- Each host retained a timestamped backup of the previous binary with SHA-256
  `57e1fa74241b74650b41536b84f8e5ddc9eb5ddbd0c5f0bccbe8bf63211df6da`.
- Restarted the existing validator transport and RPC command lines on each
  host. The running services remain manual root-owned processes, matching the
  pre-deploy posture; returning them to the enabled systemd units should be a
  separate operator hygiene task because some state files are currently
  root-owned.
- Post-deploy public RPC status: validators `0` through `5` all report height
  `1365`, state root
  `61d04ee99a79cbe58bed1fa31bb00240883678e133691d6fe11aa0c54237219ccaef961c712c5a2ab33eb22cee5cec2d`,
  and empty mempool.
- `vault_bridge_status` is now enabled and returns live status on all six
  public RPC endpoints.
- Superseding update: after the market-ops operation bundle work and live
  issuer-signed submission, `market_ops_status` is active for the a651 asset on
  all six endpoints at height `1366`.
- Wallet-proxy `/api/navswap/planner-inputs` accepts explicit
  `from_asset_id`/`to_asset_id` fields as well as symbolic `from_asset` /
  `to_asset`. The live endpoint now returns planner actions for `pfUSDC ->
  a651` instead of failing on
  `transparent_navswap_market_ops_envelope_missing`.

## Verification

- `node wallet-proxy/test_navswap_adapter.js`
- `node --check wallet-proxy/server.js`
- `cargo test -p postfiat-node rpc_serve_allows_navswap_planner_read_methods`
- `node wallet-proxy/test_proposer_routing.js`
- `node wallet-proxy/test_fastpay_quorum.js`
- `npm test` in `wallet-web`
- `npm run build` in `wallet-web`
- `node --check wallet-proxy/server.js`
- `cargo test -p postfiat-rpc-sdk
  wallet_flow_summary_helpers_decode_validated_responses`
- `cargo test -p postfiat-rpc-sdk
  wallet_sdk_creates_identity_and_signs_quoted_transfer_without_key_file`
- `cargo test -p postfiat-node
  wallet_sign_escrow_transaction_signs_escrow_fee_quote`
- `cargo test -p postfiat-node
  atomic_settlement_template_builds_pft_issued_swap_through_escrow_rails`
- `PYTHONPATH=python python3 -m pytest python/tests/test_wallet.py -k
  'load_wallet or submit_escrow_transaction_prefers_key_file_signer or
  escrow_wallet_helpers'`
- `PYTHONPATH=python python3 -m pytest python/tests/test_wallet.py -k
  'atomic_settlement'`
- `PYTHONPATH=python python3 -m pytest python/tests/test_wallet.py`
- `python3 -m py_compile python/postfiat_rpc/wallet.py
  python/postfiat_rpc/__init__.py python/tests/test_wallet.py`
- `cargo test -p postfiat-node nav_roundtrip_fleet_preflight_`
- `cargo test -p postfiat-node nav_roundtrip_strict_round_report_`
- `cargo test -p postfiat-node transport_batch_ack_reports_validator_rejection`
- `cargo build -p postfiat-node --release`
- Earlier live WAN devnet RPC check: `vault_bridge_status` enabled on all six
  public validators; `market_ops_status` enabled on all six and returning the
  then-current missing-a651-envelope state.
- Earlier live wallet-proxy `/api/navswap/planner-inputs` check with
  `from_asset_id=pfUSDC` and `to_asset_id=a651`, returning
  `transparent_navswap_market_ops_envelope_missing`.
- Superseding live wallet-proxy `/api/navswap/planner-inputs` check with the
  finalized market-ops envelope active: `pfUSDC -> a651`, amount `1`, returned
  `ok:true` with `trust_set` and `nav_subscription_allocate`.
- Superseding live wallet-proxy `/api/navswap/quotes` check with
  `auto_plan:true`: `pfUSDC -> a651`, amount `1`, returned
  `prepared_actions_ready` with a two-action wallet-local signing batch.
- `PYTHONPATH=. python3 -m pytest tests/test_dashboard_server.py -k
  'transparent_roundtrip_runs_pftl_only_without_orchard or
  transparent_roundtrip_transport_args_use_env'`
- Ethereum mainnet read-only `cast call` checks against a651 token,
  Uniswap v4 PoolManager, PositionManager, and StateView.
- `git diff --check`

## Remaining Overnight Items

- The remaining transparent-wallet acceptance gap is a manual browser
  click-through from the target wallet
  `pf124071fd53a12ca4556b7aa1f5ec98b585e73468`. Current live evidence shows the
  route is quote-ready and funding is configured, but the wallet has no
  canonical pfUSDC trustline. Because no target-wallet backup/private key was
  found locally, the trustline must be signed in the unlocked browser wallet.
  The current primary action is `Open and fund pfUSDC`; after the trustline
  receipt lands, the wallet refreshes readiness and requests guarded devnet
  pfUSDC funding for the exact shortfall.
- After the target wallet is funded, the next acceptance step is to submit the
  prepared `pfUSDC -> a651` wallet-owned action batch from the browser, start
  the operator completion run, and confirm the run stream plus issued-asset feed
  update the visible pfUSDC/a651 balances without a page refresh.
- ESCROW-009 is past the previous writable-RPC blocker. A guarded live
  `PFT <-> a651` smoke using escrow finality completed through the wallet proxy
  under `/tmp/navswap-atomic-settlement-live-smoke-exec-patched-20260629T134215Z`.
  Browser-side atomic settlement still needs durable two-party coordination
  state before it is a polished wallet UX, but the protocol path and finality
  submit path have live evidence.
- Custody inventory is complete enough for the overnight no-new-pool decision:
  it found spendable legacy a651/USDC inventory but StateView liquidity for the
  legacy Ethereum `a651/USDC` pool is `0`. LP NFT token-id resolution and deeper
  Uniswap v4 position internals remain optional inspection work, not blockers
  for the current wallet-side PFTL NAV route.
- Bridge-aware Uniswap handoff remains correctly disabled until the new wrapped
  NAVCoin token, handoff controller, verifier mode, router, and new pool are
  deployed/configured. The proxy rejects the legacy token/pool for the trustless
  handoff route.

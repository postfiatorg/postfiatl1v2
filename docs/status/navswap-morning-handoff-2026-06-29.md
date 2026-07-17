# NAVSwap Morning Handoff - 2026-06-29

Generated from current live/read-only checks at `2026-06-29T15:17Z`.
Updated after removing the transparent NAVSwap trustline gate and redeploying
the validator execution rule at `2026-06-29T18:54Z`.

Scope: trustless NAVSwap wallet wiring under the no-new-pool overnight budget.
The initial refresh was read-only. The later trustline-removal validation
submitted one guarded devnet pfUSDC funding transaction under the approved
sub-$100 live budget; it did not approve, bridge, or seed liquidity.

## Current Result

Transparent wallet NAVSwap is wired through the live wallet/proxy path, with
wallet-local signatures for wallet-owned PFTL actions and an operator issuer
key only for the issuer-owned `nav_mint_at_nav` completion leg.

The remaining target-wallet blocker is the final browser-signed NAVSwap action
batch and operator-completion observation. Wallet
`pf124071fd53a12ca4556b7aa1f5ec98b585e73468` no longer has a wallet-side
trustline prerequisite in the transparent NAVSwap flow, and the exact pfUSDC
settlement shortfall has now been funded.

## Fresh Evidence

- Target wallet dry-run:
  `/tmp/navswap-pf124-readiness-current-20260629T151716Z`
- Target wallet live readiness spot-check after proxy restart:
  direct `POST /api/navswap/readiness` at `2026-06-29T18:34Z`
- Validator redeploy verification at `2026-06-29T18:52Z`: all six
  transport/RPC services active and running rebuilt binary hash
  `23590104474a76e286acd664b5befe47db57132e85a0d808712b046d4daaf64a`.
- Guarded target-wallet pfUSDC funding submitted at `2026-06-29T18:53Z`:
  tx
  `2f7970f89b52559fb96999cc69234035e5c0fcddecd0c31fac7f4235bbfb0aaacbe891402d4a7efc93fc17744156bc64`,
  amount `6958370` pfUSDC atoms. This is the live proof that validators no
  longer reject the recipient for `missing_trustline`.
- Target wallet live readiness after funding:
  `/tmp/navswap-readiness-after-funding.json`
- Full read-only custody inventory:
  `/tmp/navswap-custody-inventory-current-20260629T144618Z`
- Existing transparent run stream evidence:
  `/tmp/navswap-run-stream-current-20260629T152628`
  (`run_status_terminal=true`, `stream_terminal=true`)
- Public redaction scan:
  `node scripts/navswap-redaction-check.mjs`
  passed with `238` files scanned and `30` private paths skipped.

## Target Wallet Readiness

Wallet: `pf124071fd53a12ca4556b7aa1f5ec98b585e73468`

Current live balances from the dry-run/feed:

| Asset | Balance |
| --- | ---: |
| PFT | `6.999955` |
| pfUSDC | `6.958370` |
| a651 | `0` |

Transparent quote state:

| Field | Value |
| --- | --- |
| Route | `pfUSDC -> a651` |
| Requested mint | `1` a651 atom |
| Quote status | `prepared_actions_ready` |
| Required settlement | `6958370` pfUSDC atoms (`6.958370` pfUSDC) |
| Prepared stages | `nav_subscription_allocate` |
| Adapter readiness | `ready_to_submit_wallet_actions` |
| Next steps | `submit the prepared wallet-owned actions` |

Prepared action PFT fee preflight:

| Field | Value |
| --- | --- |
| PFT balance | `6999955` atoms (`6.999955` PFT) |
| Prepared action count | `1` |
| Total minimum fee | `24` atoms |
| Status | `fee_preflight_ready` |
| Sufficient for prepared actions | `true` |

Funding helper state:

| Field | Value |
| --- | --- |
| Endpoint | `/api/navswap/devnet-fund-pfusdc` |
| Enabled | `true` |
| Issuer signing configured | `true` |
| Issuer key matches canonical pfUSDC | `true` |
| Funding amount | `0` atoms |
| Per-request cap | `10000000` atoms |
| Recipient window used | `6958370` atoms |
| Recipient window remaining | `3041630` atoms |
| Current unavailable reason | `settlement_already_sufficient` |

Expected browser action path:

1. Unlock the target wallet.
2. Open Swap, keep route `Transparent NAVSwap`. The screen now pre-fills
   `Mint 1 a651` and shows the settlement side as `Settle with pfUSDC`; the
   transparent route locks those asset controls to the currently supported live
   pair.
3. The transparent readiness request should auto-load; no manual `Get route
   quote` click is required before the prepared action batch appears.
4. Primary action should be `Submit NAVSwap actions`.
5. After wallet action submit, the async run stream should report operator
   completion and the wallet feed should show pfUSDC down/a651 up without page
   refresh.

## Route Matrix

Current `/api/navswap/capabilities` state:

| Route | Status | Enabled | Notes |
| --- | --- | --- | --- |
| `transparent_navswap` | `quote_ready` | yes | Quotes wallet-owned actions and can complete operator mint after wallet allocation lands. Remaining required next is target browser click-through. |
| `shielded_navswap` | operator demo | no production route | Must stay disabled beyond demo until local note custody/proving/spend authorization exist. |
| `pftl_atomic_settlement` | template route | yes for supported PFT/issued pairs | Uses ESCROW-009 reciprocal escrow template; issued-to-issued remains blocked unless a PFT intermediary is explicit. |
| `uniswap_atomic_handoff` | `disabled_missing_bridge_aware_pool` | no | Requires bridge-aware wrapped NAVCoin token, handoff controller, verifier mode, router, and new pool. Legacy pool is rejected. |
| `legacy_a651_uniswap` | `inspection_only` | no | Historical Ethereum a651/USDC venue only; not a trustless PFTL handoff route. |

The transparent route capability now also exposes
`supported_pairs=["pfUSDC->a651"]` and `current_pair.amount_asset="a651"`,
`current_pair.settlement_asset="pfUSDC"`. The Swap screen consumes this
`current_pair` metadata directly, falling back to the live `pfUSDC->a651`
pair only if the adapter response is unavailable or malformed. The Swap screen
also auto-loads transparent readiness for the current pair once the wallet
address, adapter, and amount are available. Because the target wallet has now
received the required pfUSDC, the first actionable browser state should be the
prepared action batch submit.
The browser run watcher now uses the same tested NAVSwap terminal-state helper
as the flow logic and honors the adapter stream's explicit terminal flag before
stopping run polling/streaming. The proxy run status response also exposes the
same `terminal` boolean, so polling fallback and SSE stream handling use the
same lifecycle signal. Terminal snapshots release the wallet's active-run latch
while keeping the terminal status and receipts visible. Successful transparent
completion cards now expose `Get new quote`, so repeated browser smoke runs can
move directly from receipt review to a fresh readiness quote. On browser
refresh, the Swap screen reattaches active transparent runs first and then, if
idle, can recover the latest successful terminal transparent run from
`GET /api/navswap/runs?include_terminal=true`. Dismissed terminal run ids are
remembered client-side so old receipts do not keep reappearing. The live
terminal-history probe for `pfac0562296948fbf35fec6d18d47498b412850a8c`
returned `navswap-mqz62mp7-184175bd` with `terminal=true`, `ok=true`, and an
attached quote.

## Live Transparent Run Evidence

Existing completed wallet-code run:

| Field | Value |
| --- | --- |
| Wallet | `pfac0562296948fbf35fec6d18d47498b412850a8c` |
| Run id | `navswap-mqz62mp7-184175bd` |
| Wallet action txs | `8ca921806dd0c0a56b5009fb54e690943db372d0d1851d2e1c89c8b35eec1d4be1bb3331964a379169bae9f29d03f3f0`, `5ba2ae2175f2b9e36d45fc7061c1e4b6443e54bd4a484bc959e9eed1d47c6f390cb9e3122144938a467e79d6d7241245` |
| Operator tx | `095610fec2230ca371af160927d05bfdc68c16e3010b7deb4d8666bf6317e5001c6de145abce0692d9174e53721eac48` |
| Run status | `operator_mint_submitted` |
| Stream evidence | terminal SSE snapshot observed, `stream_event_count=1`, `receipt_count=1` |
| Feed movement | pfUSDC `10000000 -> 3041630`, a651 `0 -> 1` |

The latest no-seed stream probe against that run confirms the browser-consumed
SSE path can recover a terminal snapshot and receipt without wallet key
material, and that the status polling fallback reports the same terminal
lifecycle signal.

## Custody And Legacy Pool Inventory

Read-only inventory: `/tmp/navswap-custody-inventory-current-20260629T144618Z`.

Relevant balances:

| Chain | Address | Asset | Balance | Classification |
| --- | --- | --- | ---: | --- |
| PFTL WAN devnet | `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` | PFT | `6.999955` | spendable fee balance |
| PFTL WAN devnet | `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` | pfUSDC | `0` | empty |
| PFTL WAN devnet | `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` | a651 | `0` | empty |
| PFTL WAN devnet | `pf07381735ddb7de134e8be8402b465c9cd8ec7546` | pfUSDC | `16.123994` | spendable if key available |
| PFTL WAN devnet | `pf07381735ddb7de134e8be8402b465c9cd8ec7546` | a651 | `898` | spendable if key available |
| PFTL WAN devnet | `pfac0562296948fbf35fec6d18d47498b412850a8c` | pfUSDC | `3.041630` | spendable if key available |
| PFTL WAN devnet | `pfac0562296948fbf35fec6d18d47498b412850a8c` | a651 | `1` | spendable if key available |
| Ethereum mainnet | `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | a651 | `3815.876054831038397894` | legacy ERC-20 custody if key unlocked |
| Ethereum mainnet | `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | USDC | `5805.752289` | ERC-20 custody if key unlocked |
| Arbitrum One | `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | USDC | `130.000099` | ERC-20 custody if key unlocked |
| Arbitrum One | `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9` | USDC | `0` | drained old vault |

Legacy Ethereum a651/USDC pool:

| Field | Value |
| --- | --- |
| Pool id | `0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84` |
| StateView liquidity | `0` |
| Status | `legacy_pool_inactive_zero_stateview_liquidity` |
| Tick | `-252187` |
| LP fee | `500` |

Conclusion: there is legacy a651/USDC custody, but the legacy pool is not
usable active trustless routing and must remain labeled secondary/inspection
only. It is not the bridge-aware PFTL-to-Uniswap handoff route.

## ESCROW-009 Atomic Settlement

The existing PFTL atomic route uses ESCROW-009, not a new swap primitive.
Recent guarded live smoke evidence is under
`/tmp/navswap-atomic-settlement-live-smoke-exec-patched-20260629T134215Z`.

Accepted live tx ids:

- left create:
  `f2a0278c54ba7d4a21e353ace1df92337feb316ef5fcd0903de8f54bbd4517e6c87b21e7a81c9f560614ae2993571711`
- right create:
  `934bc3406c980e24949ba1895cc873cc613c7e3c78371523c9733f55cba4d9ee4c12e0b8d7b6caa9a0b495b0dcd70cb8`
- left finish:
  `11fbcf4eeafea3583b56ba2edccbfc8dcb2a84ca2a20475bb8710d6c533f1a71e71393ca15446057b1a6a147b5317fb4`
- right finish:
  `ece675859ee33af73303eed434b00cffb87f70ea78cabe8a44ecfce2788ed0e36395e70db98a2b3121d3c71ced341d48`

Both escrows reached `finished`; validators converged afterward at height
`1384` with empty mempools.

## Current Verification Commands

Fresh commands run for this handoff:

- `node scripts/navswap-wallet-live-smoke.mjs --wallet-address pf124071fd53a12ca4556b7aa1f5ec98b585e73468 --amount 1 --timeout-ms 30000`
- `node scripts/navswap-custody-inventory.mjs`
- `node scripts/navswap-wallet-live-smoke.mjs --stream-run-id navswap-mqz62mp7-184175bd --timeout-ms 15000`
- `node scripts/navswap-redaction-check.mjs`

Latest source verification after the trustline-gate removal:

- `cargo test -p postfiat-execution`: `75/75` passed
- `node --test wallet-web/src/lib/navswap-flow.test.js`: `19/19` passed
- `node wallet-proxy/test_navswap_adapter.js`: passed
- `npm --prefix wallet-web test`: `117/117` passed
- `npm --prefix wallet-web run build`: passed
- `node scripts/navswap-redaction-check.mjs`: passed with `241` files scanned
- `git diff --check`: passed

## Exact Remaining Work

For the target browser wallet:

1. The user must unlock wallet
   `pf124071fd53a12ca4556b7aa1f5ec98b585e73468` in the live browser wallet.
2. The browser must sign and submit the prepared wallet-owned action batch.
3. The wallet must observe the async operator run and live-feed balance movement
   without refresh.

For the future Uniswap handoff:

1. Deploy/select a new bridge-aware wrapped NAVCoin token distinct from legacy
   Ethereum `a651`.
2. Deploy/select the handoff controller.
3. Select verifier mode: threshold-controlled, optimistic, succinct proof, or
   direct light-client.
4. Deploy/select the new Uniswap pool/path and seed only under an approved
   liquidity budget.
5. Keep `uniswap_atomic_handoff` disabled until those fields are configured and
   quote bindings plus receipt verification are complete.

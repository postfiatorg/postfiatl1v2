# Z3 cycle-1 review packet — 2026-11-11

For the product owner's review under gate G4's first line of the [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md#gate-g4-operator-authorizes-one-integrated-testnet-cycle): the G2 compatibility result, the G3 tests, the dry-run amounts, the stop conditions and the wallet cap, in one place. **No cycle has run. G4 is not authorized, and this packet does not ask for it:** two signed actions of the other lane come first ([section 4](#4-g2-result)).

Everything here was read or run on 2026-11-11 between 10:00 and 10:13Z on main `69c6c2e3`, read-only. Nothing was signed, submitted, restarted or deployed; no keystore was opened.

## 1. What one cycle does, in order

One funded test wallet owns the whole cycle. The [dry run](#6-g3-tests-and-dry-run-today) prints 39 commands, each behind its own stop; 14 sign on PFTL, 2 on Arc ([command sheet](../runbooks/z3-cycle-dry-run.md), [plan, controlling scope](../plans/active/z3-navcoin-roundtrip-plan.md#controlling-scope)).

| Stage | Steps | What happens |
| --- | --- | --- |
| Preflight | 1 | Six-validator, route, NAV, balance and contract read-backs checked against the frozen checkpoint |
| Deposit on Arc | 2 | `depositV2` of 2,000,000 USDC atoms into vault `0x1603…fc250f`, route binding `0xd9e0cd40…` |
| Mint | 3–11 | Ingress capture and Groth16 proof, then propose, finalize and claim: 2,000,000 atoms of the Arc pfUSDC series `3923511d…` |
| Primary subscription | 12–16 | Reserve with the Arc series selected, `pftl_uniswap_primary_subscribe_v2`, release of the unused export entitlement, verification |
| Fresh NAV and route epoch | 17–25 | New reserve packet, NAV finalize, route pause, route-epoch advance, resume |
| Redemption | 26–28 | `pftl_uniswap_primary_redeem` into the same Arc series |
| Burn | 29–31 | `vault_bridge_burn_to_redeem` of the redeemed pfUSDC |
| Release on Arc | 32–37 | Egress witness and proof, `withdrawWithProof` on Arc, replay check (must revert), settlement on PFTL |
| Seal | 38–39 | Build and verify the ten-section cycle packet (`python/postfiat_rpc/z3_cycle.py`) |

## 2. Amounts and cap

| Item | Value | Record |
| --- | --- | --- |
| Per-cycle cap | 2,000,000 source atoms (2 testnet USDC) | [G1](../plans/active/z3-navcoin-roundtrip-plan.md#gate-g1-operator-authorizes-the-bounded-shadow-envelope) |
| Arc deposit and pfUSDC mint | 2,000,000 atoms (dry-run `amount_atoms`) | today's dry run, step 2 |
| A666 mint | **not quoted**: `mint_amount_atoms` is null until fresh NAV (row 14) | [inputs, row 14](../status/z3-cycle1-inputs-20260922.md#the-17-formerly-unknown-fields) |
| A666 ceiling within the cap at today's NAV | 1,704,729 atoms: base 1,990,048, settlement 1,999,999, spread 9,951 (NAV epoch 9, 116,736,912 USD_1E8, issue 10,050 bps) | `derive_issue_amounts` in `scripts/a666-pfusdc-reserve-demo.py`; was 1,922,312 at NAV epoch 8 |
| Redemption of that amount at the same NAV | output 1,989,052, spread 996 (redeem 9,995 bps) | `derive_redeem_amounts`, same script; the real cycle redeems at the fresh NAV |
| Test wallet | `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB`: 20,000,000 USDC atoms, allowance to the vault **0** | [fixture](../status/z3-g2-selected-state-20261111.json), `arc_testnet.test_wallet` |

The allowance must be approved, separately authorized, before step 2 ([inputs, blocker 8](../status/z3-cycle1-inputs-20260922.md#re-read-after-the-2026-09-25-deployment)).

## 3. Stop conditions and latency bounds

Stop before any submission on a stale, missing or inconsistent identity, height, root, route epoch, NAV packet, proof key, code hash, source domain, balance, capacity, nonce or queue; stop after any submitted step that lacks both finality and an accepted receipt ([plan, stop conditions](../plans/active/z3-navcoin-roundtrip-plan.md#stop-conditions)). A clean cycle and what makes an attempt unclean are defined in the [plan](../plans/active/z3-navcoin-roundtrip-plan.md#meaning-of-succeed-repeatedly).

The nine declared bounds, adopted 2026-09-23 ([G5](../review/z3-g5-failure-rehearsal-20260917.md#latency-bounds-and-pause-thresholds)); exceeding one pauses the cycle:

| Preflight | Deposit | Ingress proof and claim | Subscription | Entitlement release | NAV and route epoch | Redemption | Burn and egress | Final convergence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 300 s | 180 s | 7200 s | 300 s | 180 s | 7200 s | 300 s | 7200 s | 300 s |

The wrapper enforces a per-command timeout only; the aggregate stage timers are a monitoring duty ([inputs](../status/z3-cycle1-inputs-20260922.md#predecessors-read-backs-and-monitoring-still-required)).

## 4. G2 result

### A. The frozen read

[`docs/status/z3-g2-selected-state-20261111.json`](../status/z3-g2-selected-state-20261111.json): all six validators at **height 1177**, tip `661a1ec2b9e99c3f…`, state root `c6dfd0ebea8c2bdb…`, build `ae70d66d` (binary `87cfe797…`, release `vote-records-20261030`), read 10:00:58–10:01:18Z. Every validator returned identical results for the same 13 reads (one normalized digest, `ca3b587b…`) and the height did not move. Arc testnet was read at block 71,859,953 and Ethereum at block 26,390,966.

### B. Offline replay

`crates/bridge/tests/z3_g2_selected_state_replay.rs` takes the fixture as input and recomputes with the repository's own functions: **5 passed**. Changing one input (the Arc route epoch, the policy start height) turns two tests red.

| Identity | Function | Result |
| --- | --- | --- |
| Arc source series `3923511d…` | `pfusdc_source_series_id` | equals `asset_info` and the bucket's series |
| Arc bucket `fcc20960…`, domain `erc20_bridge_vault:5042002:0x1603…:0x3600…` | `vault_bridge_bucket_id` | equals the asset's `source_bucket_id` and `vault_bridge_status` |
| Route config digest `12ed00ca…` | `pftl_uniswap_route_config_digest` over `deployments/a666-mainnet-20260727/09-production-route-config.json` | equals the live route |
| Route policy `32c970e9…` (epoch 11, valid 1110–10000, pricing NAV epoch 9, packet `15628936…`) | `PftlUniswapPrimaryMarketPolicyV2::computed_hash` | equals the live route |
| A666 NAV profile `a94a3929…`; pfUSDC family NAV profile `e0bc0d58…` | `NavProofProfile::new` (+ route-policy binding for the family) | both equal their read |
| Route epoch and NAV packet | — | route pricing epoch 9 and packet equal the finalized A666 NAV epoch 9 |

Arc and Ethereum: anchor `0x3782655d…`, vault `0x8d15bb9d…`, verifier `0xf6606628…` and token `0xc9987bd3…` code hashes, binding `0xd9e0cd40…`, egress key `0x0036cbe7…`, route epoch 9, finalized height 948, vault not paused. On Ethereum, wA666 `totalSupply` 31,418,438,511 equals the route's wrapped exposure.

### Changed since the September and October records

| Identity | Then | 2026-11-11 | Why |
| --- | --- | --- | --- |
| Release / height | `a666-source-route-20260907`, 1020 (09-22); `combined-fastpay-20260925`, 1050 (09-28) | `vote-records-20261030`, 1177 | later releases; the fleet has run `vote-records-20261030` since 2026-10-30 ([Current State](../status/chain-state-current.md)) |
| Route policy epoch / hash / valid from | 10 / `fffecee5…` / 1012 (09-28) | 11 / `32c970e9…` / 1110 | route-epoch advance at block 1110, 2026-10-04 |
| Route pricing NAV epoch / packet | 8 / `f670ad00…` | 9 / `15628936…` | A666 NAV epoch 9 submitted at 1108, finalized at 1109 |
| A666 NAV per unit | 103,523,693 | 116,736,912 | NAV epoch 9 |
| pfUSDC family NAV profile / epoch | `f644d1c0…` (Arc source class) / 52, height-1001 snapshot ([G2 record](z3-g2-route-compatibility-20260917.md#identities-read-back)) | `e0bc0d58…` (Ethereum vault `0xe7a4…` source class) / 54 | bound to the family's active governed route `ethereum-mainnet-usdc-v1` epoch 10, active since height 1008 |

**Unchanged:** route id and family, config digest `12ed00ca…`, settlement family `02c46a36…`, A666 asset and issuer `pffcb93…`, multipliers 10,050 / 9,995, A666 NAV profile `a94a3929…`, key `0x00580ee8…` and valuation policy `076c071e…`, the single Ethereum custody row `2bae082a…` (principal 12,419, spread 54,721), the Arc series, bucket, source policy `f7ce6d3c…` and domain, and every Arc contract read of 2026-09-22 and 09-28 including wallet 20,000,000, vault 1,000,000 and allowance 0.

### C. Verdict on the G2 line: **open**

> Read back at height 1177 and replayed offline from the frozen fixture: the A666 route (epoch 11, config digest, policy hash), the A666 and Arc-series assets, the Arc source domain, bucket and pair, the NAV epoch-9 packet and the current A666 NAV profile `a94a3929…`; the proof-profile the cycle needs, an `sp1-nav-reserve-v1` profile with its public-values schema and source-manifest hash, does not exist on chain and cannot be read back until the A666 issuer registers and binds it (asked 2026-09-22), after which the route epoch, policy and NAV identities change again with the advance that adds the Arc custody row (asked 2026-09-28).

How the read shows each gap:

- **Arc custody row (2026-09-28):** `a666_primary_route.source_settlement_custody` holds one row, `2bae082a…`; none names `3923511d…`. Adding it takes a `pftl_uniswap_route_epoch_advance` with `settlement_source_asset_ids`, signed by `pffcb93…`.
- **Governed A666 NAV profile (2026-09-22):** `a666_nav.active_profile` is `sp1-groth16` with no `public_values_schema` and no `source_manifest_hash`; the epoch-9 packet has `proof_verified_net_assets` 0 and `source_count` 0. The driver's `validate_nav_binding` requires both bindings; BMNRC's `sp1-nav-reserve-v1` profile `4ff2afa5…` is the reference shape ([BMNRC check](bmnrc-protocol-enforcement-check-20261002.md)).
- **Arc ingress route (our decision, inputs blocker 5):** `vault_bridge_route` for the Arc series still returns "no active governed profile".

The fixture must be frozen again after those signed actions; this one then serves as the before state.

## 5. The 17 input rows today

**9 resolved, 8 waiting**, unchanged in count since 2026-09-28. Today's dry run lists exactly the 8 waiting rows as unresolved.

| # | Field | Status today |
| --- | --- | --- |
| 1 | anchor code hash | resolved; re-read today, unchanged |
| 2 | primary-route policy hash | resolved; full hash now read: `32c970e9…` (epoch 11) |
| 3, 9 | NAV proof key | resolved; `0x00580ee8…` unchanged; changes with the governed profile |
| 5, 6, 7 | proposer, finalizer, settler | resolved, `pf23d883…`; custody stays with the other lane |
| 10 | remote runner | resolved |
| 15 | reservation TTL | resolved, 128 blocks |

Waiting rows, all owned by the other lane (answers of 2026-10-20 in the [inputs](../status/z3-cycle1-inputs-20260922.md#answers-of-2026-10-20-and-the-order-of-events)):

| # | Field | Waits on | First asked |
| --- | --- | --- | --- |
| 4 | cycle owner account | PR #49 | 2026-09-22 |
| 8 | NAV source-manifest hash | governed NAV profile | 2026-09-22 |
| 11 | proposer host map file | PR #49 | 2026-09-22 |
| 12 | opening NAV manifest | governed NAV profile | 2026-09-22 |
| 13 | fresh post-subscription packet | governed NAV profile | 2026-09-22 |
| 14 | A666 mint quote | rows 4, 12, 13 and the Arc custody row | 2026-09-28 |
| 16 | deposit nonce | PR #49 | 2026-09-22 |
| 17 | reservation recipient | PR #49 | 2026-09-22 |

Also waiting: the PFTL signer keys, which stay on the other lane's host where cycle 1 signs (2026-09-22, answered 2026-10-20).

## 6. G3 tests and dry run today

| Command (main `69c6c2e3`) | Result |
| --- | --- |
| `PYTHONPATH=python python3 -m pytest -q python/tests/test_z3_cycle.py python/tests/test_z3_composition.py` | 41 passed |
| `python3 scripts/test-a666-pfusdc-reserve-demo.py` | 27 passed |
| `cargo test -p postfiat-bridge --test z3_g2_selected_state_replay` | 5 passed |
| The seven focused Rust commands of the [G2 record](z3-g2-route-compatibility-20260917.md#tests-executed) (`postfiat-execution`: source round trip, overlay, Arc Groth16 fixture, ar05, ar07; `postfiat-types`: `pftl_source_`, source-series identity) | 8 passed (5 + 3) |
| `scripts/z3-cycle-dry-run.py run … --dry-run` | exit 0, `DRY_RUN`, **39 steps, 0 submissions**, 8 unresolved inputs |

Resolution of the 39 printed commands: 30 Python rows exist and every printed flag is in their `--help`; 3 `cast` rows resolve; the 3 prover rows (`arc-ingress-capture`, `arc-ingress`, `egress`) and the 3 node rows (5, 29, 32, in the `vote-records-20261030` binary's usage) carry every printed flag. **39 resolved, 0 flag mismatches.**

What no longer resolved from the September seed, all inputs rather than tooling: the seed's campaign window (2026-09-18 to 09-25) made the wrapper stop with "cycle outside operator window"; the run used a placeholder window of 2026-11-11 to 11-18, which decides nothing. The seed's node, prover and egress-ELF paths were replaced by the running release binary, the 2026-10-22 prover build and the restored ELF. No tooling change was needed.

## 7. Prover, guest program and node provenance

| Component | Identity | Record |
| --- | --- | --- |
| Node | release `vote-records-20261030`, source `ae70d66d`, binary SHA-256 `87cfe797…`, equal on all six validators and to the local release build the dry run named | fixture `fleet.binary_sha256` |
| Host prover | rebuilt 2026-10-22 from main `4bd15ec0`, SHA-256 `ee1387c9…`, build receipt | [prover provenance](../status/z3-cycle1-inputs-20260922.md#prover-provenance) |
| Arc egress guest | `arc-v2-20260902`, ELF SHA-256 `8b0f266a…`, restored from commit `65df4263`; key `0x0036cbe7…` equals the Arc verifier's `programVKey` today | same, and fixture `arc_testnet.verifier` |
| Arc ingress guest | ELF `7660fcc5…`, key `0x0050a8b0…`, embedded in the host prover | same |
| NAV prover | none qualified for A666: waits on the governed profile | same |

## 8. Read-backs that must all agree on the day

Each is in today's fixture except where marked new; on the day they must agree on all six validators at one height and within the 300 s preflight bound ([plan, cycle packet item 2](../plans/active/z3-navcoin-roundtrip-plan.md#meaning-of-succeed-repeatedly)).

1. Height, tip, state root, build and binary on all six; empty mempool.
2. Route: id, epoch, config digest, policy hash and validity, live and not paused, **an Arc custody row enabled for issue**, no active reservation or export entitlement.
3. A666 NAV: profile id, kind `sp1-nav-reserve-v1`, schema and source-manifest hash, finalized epoch and packet within `max_nav_age_blocks`.
4. Assets: A666, pfUSDC family, Arc series and bucket (active, counted, impairment 10,000 bps).
5. Owner balances, issue and redeem capacity, source principal (new: owner from row 4).
6. Proof keys: ingress, egress, NAV.
7. Arc: code hashes of anchor, vault, verifier and token; binding; pause; verifier route epoch, finalized height and checkpoint; wallet USDC, approved allowance and gas; `depositSeen` for the cycle's deposit ID (new: nonce from row 16).

## What this packet does not claim

It does not authorize or schedule a cycle, approve the allowance, qualify the NAV prover, or decide the Arc ingress route. It does not tick G2 or any G4 line. It gives no dates for the other lane's actions. Z3 stays OPEN.

# Wallet app private path built end to end and permit tool added

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-10-29 UTC

## BLUF

The wallet lane's [handoff of 2026-10-29](2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md) asked the L1 side for four things. All four were answered today on main:

1. **Ingress permit.** `wallet-proxy/ingress-permit-activate.js` switches the permit and its driver pin together, checks readiness afresh and rolls back on failure ([runbook](../runbooks/ingress-permit-activation.md), `3b62e8df`). The permit's content validator and the stuck deposit's admission record exist only on the operator host. The tool therefore takes the validator command as `--validator`, and the deposit waits for the D15b plan.
2. **W-2.** The per-install native session, `POST /api/native-session/v1/open` (`e037829f`, `74992eb5`).
3. **W-1.** The [specification](../specs/user-proven-shielded-actions-spec-20261029.md) and [milestone](../plans/active/user-proven-shielded-actions-milestone.md) (`2bf89150`, `9c36a3d1`, `3475c5f0`, `987eddb6`), with segments S1–S7 built today:
   - contract fixtures and vectors (`346c79e4`);
   - gateway routes `/api/private/v1/` with durable once-only admission (`db9af4f2`, `f8b9a838`, `d9095797`, `ea2213d0`);
   - the validator-owned admission store, round service and receipt read behind `rpc-serve --allow-private-action-admission` (`968da908`, `d7de5ec7`, `657ce706`);
   - the finalized reads `private_pool_snapshot`, `_page`, `_account_lines`, `_receipts` and `_market` (`683a5b32`, `0c396fc8`);
   - the pricing claim and egress binding reads (`93454729`, `500cdb07`);
   - the three at-NAV pricing rules in consensus, behind a governed activation (`ed89c916`);
   - the governed egress policy registry (`899b26f8`) and its [disclosure schema document](../specs/asset-orchard-private-egress-disclosure-schema-v1.md) (`1e02d103`);
   - the readiness read `private_readiness` (`556cac38`);
   - six-validator test suites, including two release-only modules with real swap and egress proofs (`e37d77fd`, `e5f8f5b8`, `14c8e43e`);
   - the gateway-to-node integration test with the real node and proxy (`3747a2e7`, `c308b7af`, `46c5a956`);
   - the read-only page `python/postfiat_rpc/private_path_ui.py` (`403af19d`);
   - retained finalized snapshot points, so a snapshot token outlives new blocks (`110997f1`, `a4346f94`).
4. **Asset-Orchard secret lifetimes.** Filed as [issue #181](https://github.com/postfiatorg/postfiatl1v2/issues/181). Points 1, 2 and 4 are fixed (`548d4918`); points 3 and 5 need upstream changes, analysed on the issue.

Also today:

- [PR #180](https://github.com/postfiatorg/postfiatl1v2/pull/180) was reviewed and merged (`29589ab2`), with the SCT-08 note `2f38e608`.
- Bot PRs [#177](https://github.com/postfiatorg/postfiatl1v2/pull/177) (IPv6 brackets in `pft_token`'s redacted RPC URL; `3592c25b`) and [#179](https://github.com/postfiatorg/postfiatl1v2/pull/179) (`nav_reserve_packet_id` computed like the chain; `8db13571`) were merged after both bugs were reproduced on main.
- The ERC-20 milestone's P1.10 records that the other lane sent the exchange guidance page to Coinbase on 2026-10-28; it now waits for Coinbase's reply (`cfb5960e`).
- Vite in the web wallet was updated to clear the open dependency alerts (`ca337838`).
- The proof public-input inventory hashes were updated after the privacy change (`d23b14d1`).

## Current state

- **W-1 user-proven private path.** The milestone records ticked and open items.
  - **Ticked:** S1.1, S1.2, S1.4, S2.1–S2.3, S2.5, S3.1, S3.4, S4.1–S4.3, S5, S5b.1–S5b.4, S6.1–S6.3, S7.2, W-2, M2 and M3.
  - **S6.2 complete.** `e5f8f5b8` accepts a real swap proof and a real private-egress proof on six validators and refuses an invalid proof, a wrong recipient and a spent nullifier. `14c8e43e` adds a second release-only module (`crates/node/src/main_parts/tests/private_action_pricing_six_validator_tests.rs`) for mode mismatch, pair not governed, stale anchor and stale NAV: each is a real swap proof that breaks one rule, refused on all six validators with no state change (355 s in release). The "wrong issuer" case is an asset/issuer binding mismatch, because no A666/pfUSDC issuer profile exists on the ingress path yet.
  - **Open, wallet team:** S1.5, S2.4, S4.4 and the app part of S6.4.
  - **Open, S1.3:** the atomic-batch activation height; `manifest.json` holds `null` until validator-0 runs a release that serves `private_readiness`.
  - **Open, S3.2:** height ownership, quorum and catch-up across six validators.
  - **Open, S3.3:** the operator switch `WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION` is in (default `on`; `4bf643aa`, `cd4dab07`). Removing the legacy paths waits until the app is on `/api/private/v1/`.
  - **Open, M1:** the spec scored 85.13, then 84.20, then 85.40 (GPT-6 Astra Pro 86.20, Claude Fable 5.1 82.40, GLM 5.3 87.60) after its AGENTS.md step-4 pass: one direct call to `openai/gpt-5.6-sol-pro` with the second run's critiques, the rewrite corrected against the code. Below 86, so it is not locked (`60424350`).
  - **Open, others:** S7.1 (publication from a deployed release), S8 (the approved ce22 run) and M4.
- **Nothing of W-1 runs on ce22.**
  - The admission switch is off by default.
  - The pricing enforcement and the egress policy are governed activations, and neither is scheduled.
  - The devnet runs `index-v3-20261027` (`d728f895`), which has none of today's code ([Current State](../status/chain-state-current.md)).
- **Finalized reads and snapshot tokens.** Since `110997f1` each node keeps the last 16 finalized snapshot points (`POSTFIAT_PRIVATE_POOL_SNAPSHOT_POINTS`, 1–1024) in `private_pool_snapshot_points.json` in its data directory, a node-local file rewritten atomically after each durable commit and never consensus state, so the ring survives a restart. An older retained token still serves its pages, asset rows and receipts; account lines and the market need the current tip and refuse it with `snapshot_not_current`; a token 16 or more blocks behind the tip gets `snapshot_expired`, and a wrong hash or root stays `snapshot_unavailable`. On the gateway side, `a4346f94` reads the account lines right after taking the snapshot and treats all three codes as a stale token: one retry from a fresh token, then 503.
- **Interface facts for the wallet team:**
  - The intent id covers only `chain_id`, `genesis_hash` and `protocol_version`. The address is bound to the native session principal at the gateway.
  - The gateway sends the identical reserved bytes to every configured validator, because a node holds a record until it is the proposer.
  - A snapshot token is valid for the node's last 16 finalized points (account lines and the market only at the tip).
  - The egress `disclosure_hash` must equal the governed schema hash. The schema document gives that hash and the proposed policy ids.
- **W-2 native session.**
  - Documented in [the web wallet page](../wallets/web-wallet.md) under "Native app sessions" and "Private actions".
  - The proxy gained the pinned dependency `@noble/post-quantum` 0.7.1 for ML-DSA-65 verification.
  - A Rust-signed vector (`crates/crypto_provider/examples/native_session_vector.rs` produces `wallet-proxy/fixtures/native-session-rust-vector.json`) verifies in the proxy.
  - `docker-compose.wallet-public.yml` passes every new variable unset, so the supported deployment behaves as before.
- **Ingress permit.**
  - The tests cover every refusal in `--check` and `--activate`, a simulated death between the two renames, and rollback. The runbook includes a no-money rehearsal.
  - The proxy's routes file pins the driver config, and the tool rewrites that pin too.
  - An unfinished bridge job keeps its saved config copy and would stall after an activation, so the tool refuses while such jobs exist unless `--allow-inflight-jobs` is given.
- **Consensus changes,** all behind governance and inactive on the devnet:
  - **Pricing rules.** Keys `asset_orchard_pricing_enforcement_activation_height` and `asset_orchard_pricing_max_nav_age_blocks`. Codes `asset_orchard_pricing_nav_stale`, `_pair_not_governed` and `_mode_mismatch`. The governed pair comes from `pftl_uniswap_routes`, so the A666/pfUSDC route must be registered before activation.
  - **Egress policy registry.** Amendment kind `asset_orchard_egress_policy_v1:<asset_id>:<disclosure_schema_hash>:<policy_id>`. Global switch `asset_orchard_egress_policy_activation_height`. Records are ratified with the `ratify-asset-orchard-egress-policy` commands.
- **CI.** At the time of writing, `gh run list` shows today's product-security-ci green on every completed run from `d23b14d1` on except `8db13571` (an anvil port collision in the evm-contracts job), after the superseded red runs from `548d4918` to `346c79e4` (the proof-inventory hash drift), and 16 runs of both workflows still in progress. rust-ci is green up to `c308b7af` and red on all 11 completed runs from `0c396fc8` on, each time in `transport_batch_payload_tests::activated_consensus_v2_transport_survives_failed_view_zero_proposer_n4` (the transport ack read times out after three attempts, and the poisoned test lock then fails six more tests in that module; the `ca337838` run also hit the same socket read timeout in `rpc_serve_readiness_reports_its_own_admission_switch`); the cause is not yet diagnosed, and no commit after `c308b7af` qualifies for a release until it is.
- **Devnet.** Nothing was changed. The only live reads were read-only RPC calls to validator-0 for the contract manifest and asset profile (`server_info`, `asset_info` and `nav_reserve_proof_status`, at finalized height 1161). Everything above is merged but not deployed.
- **Task Node.**
  - Nineteen tasks were rewarded today, 50.68 PFT in total:
    - `task_aca4932b…`, 1.4 (PR #180);
    - `task_1805d127…`, 2.5 (pricing rules);
    - `task_00b2f692…`, 2 (spec and milestone);
    - `task_1c539ef9…`, 3 (admission service);
    - `task_130eb87c…`, 2.8 (readiness);
    - `task_41fe36d4…`, 2.8 (six-validator suite);
    - `task_dbc3fa94…`, 3.5 (egress registry);
    - `task_a42cc686…`, 2 (read-only page);
    - `task_87449420…`, 3.5 (receipt and market reads);
    - `task_a4e564ae…`, 3.5 (secret inspection);
    - `task_bdf551ff…`, 3.5 (operations follow-ups);
    - `task_35f46503…`, 2.98 (proof cases);
    - `task_bcdb68c4…`, 2 (gateway crash cases);
    - `task_722b0b60…`, 2 (spec content pass);
    - `task_54999557…`, 2 (disclosure schema);
    - `task_5dc3515c…`, 3.2 (legacy switch);
    - `task_bca136c3…`, 3 (the S6.2 cases);
    - `task_a5a692aa…`, 2 (the specification's improvement pass and third score);
    - `task_7443dd94…`, 3 (snapshot retention).
  - Task generation failed and was skipped for the permit tool, the secret fix, the pricing reads, W-2, the contract fixtures, the gateway routes, the pool reads and the integration test. The backend errors were `taskgen_readiness_invalid`, `inference_http_503` and `inference_timeout`.

## Next decision or action

My next steps, in order:

1. Cut and roll out the next validator release: the VR-01 fix `5fc43961` plus #174 and #175, from a commit with both workflows green (first diagnose the rust-ci transport test failure above). Today's W-1 code stays out unless it is qualified separately.
2. Remove the S3.3 legacy paths once the app has moved to `/api/private/v1/`.
3. With the wallet team, in order:
   - S1.5 (their helper reproduces the vectors);
   - S2.4 (the app on the new routes);
   - agreeing the egress disclosure schema;
   - the governance records and activations on the devnet;
   - S8.
4. M1: the spec's next improvement pass through the harness.
5. The Phase 2 genesis lane and release path of the ERC-20 milestone, once the mainnet genesis plan exists. The biweekly page update is due on 2026-11-10.

Only the other lanes can provide these (date first asked):

- the permit content validator command on the operator host, to wire into the activation tool and rehearse (2026-10-29);
- the D15b recovery plan for the stuck deposit `0x6998…884e` (2026-10-29);
- the ERC-20 slice size, the launch allocation, the issuing entity, the treasury Safe's signers and threshold, the PFT Ledger treatment, and the audit budget and vendor (2026-10-26; the supply was answered 2026-10-28);
- the two issuer signatures per the issuer signing sheet, A666 first and then BMNRC (2026-09-25 and 2026-10-01; also W-3 in the app spec);
- the on-chain registration of the reserve-proof successor (2026-09-24);
- the `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28);
- the StakeHub acceptance file and binaries (2026-10-21);
- SCT-08 items 1–3 (2026-10-23);
- the production genesis plan and the website go-live (2026-10-23).

## References

- [Task Node wallet findings for the L1](2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md)
- [User-proven shielded actions milestone](../plans/active/user-proven-shielded-actions-milestone.md) and [specification](../specs/user-proven-shielded-actions-spec-20261029.md)
- [Private egress disclosure schema v1](../specs/asset-orchard-private-egress-disclosure-schema-v1.md)
- Contract fixtures: `crates/node/tests/fixtures/private-actions-v1/`
- [Ingress permit activation runbook](../runbooks/ingress-permit-activation.md) and [public RPC operator policy](../runbooks/public-rpc-operator-policy.md)
- [Web wallet](../wallets/web-wallet.md)
- [Issue #181](https://github.com/postfiatorg/postfiatl1v2/issues/181) and [PR #180](https://github.com/postfiatorg/postfiatl1v2/pull/180)
- [PFT as an ERC-20 on Base milestone](../plans/active/pft-erc20-base-listing-milestone.md)
- [PR #177](https://github.com/postfiatorg/postfiatl1v2/pull/177) and [PR #179](https://github.com/postfiatorg/postfiatl1v2/pull/179)
- Previous handoff: [Base Sepolia token live, lock contract merged and exchange page corrected](2026-10-28___dravlic__base_sepolia_token_live_lock_contract_merged_and_exchange_page_corrected.md)
- [Current State](../status/chain-state-current.md)


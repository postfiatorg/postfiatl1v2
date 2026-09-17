# Z3 G2 route compatibility — 2026-09-17

The candidate can use the existing A666 primary route with an explicitly governed Arc pfUSDC source series. It needs no additional consensus code, transaction kind, NRRS facility, bridge contract, or settlement-price format. The unmodified Python driver cannot build that source-preserving cycle. The operator has not selected a Z3 pair or qualified lineage; those decisions remain open.

## Scope and sources

This is an offline source review under the [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md), applying [NRRS section 2.2](../deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md#22-binding-monday-demonstration-profile). Section 2.2 takes precedence over the generic facility and settlement-price proposals.

- Main was pulled first at `b83c3a3c260755108f070d0fbe1a795fbafc4a20`.
- Read-only candidate: `15126ac3408f3afe58cf4e4ed4c31d00efa04366` on `origin/release/combined-devnet-20260915`, draft PR #41; supplied merge base `faff0e53533888bb229add117fcda3ed4094aed4`.
- Disposable detached worktree: `/tmp/z3-g2-20260917`, created after `git fetch origin`. Builds used `/tmp/z3-g2-20260917-target`.
- Qualified node source recorded by the [release-repair plan][repair]: `1c435f4fb482ea7830bee5c7018d370a10a34bfd`. It is an ancestor of the candidate; `git diff 1c435f4f HEAD -- crates scripts python programs` is empty. This verifies code presence at both commits, without choosing the operator's G1 lineage.
- No candidate source edit, candidate commit/push/rebase, existing release-checkout access, Task Node action, fleet call, chain RPC, or live transaction was used. Only the verdict and G2 annotations are published on main.

Source files read for the compatibility trace:

- `crates/types/src/core_chain.rs`, `transactions_mempool_receipts.rs`, `market_nav_asset_types.rs`, `account_owned_asset_types.rs`, `transactions_validation_helpers.rs`, and `tests.rs`.
- `crates/execution/src/nav_vault_asset_execution.rs`, `pftl_source_settlement.rs`, `issued_asset_ledger_helpers.rs`, `nft_escrow_asset_execution.rs`, `nav_sp1_verifier.rs`, `pftl_uniswap_ethereum_verification.rs`, `market_nav_execution_tests.rs`, and `core_asset_execution_tests.rs`.
- `crates/node/src/vault_bridge_workflows.rs`, `state_commitment.rs`, `market_bridge.rs`, and `execution_actions.rs`.
- `scripts/a666-pfusdc-reserve-demo.py`, `scripts/test-a666-pfusdc-reserve-demo.py`, `scripts/a666-build-live-nav-mark-ops.py`, `scripts/a666-build-route-epoch-advance.py`, and `python/postfiat_rpc/testnet_path.py`.
- `docs/specs/pfusdc-arc-mvp-testnet-spec-20260828.md` and `docs/specs/pfusdc-arc-tier4-spec-20260828.md`; the frozen files below and the release-repair proof identity record.

All candidate links pin the full candidate commit. Archived readbacks are evidence of their recorded state, not current state or Z3 authorization.

## Identities read back

A666 configuration sources are `deployments/a666-mainnet-20260727/09-production-route-config.json`, `09-production-route-config-digest.json`, `10-production-route-activate.ops.json`, and `opening-nav-proof-manifest.json`. The later snapshot is `deployments/a666-source-route-20260907/canary-rpc.json` at height 1001. Their epochs must not be mixed into a synthetic current configuration.

| Identity | Exact value | Frozen source |
| --- | --- | --- |
| Primary route / family | `pftl-a666-ethereum-wA666-usdc-v1 / primary_pftl_mint` | [route][route] |
| Route config digest | `12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933` | [digest][digest] |
| A666 native asset ID | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` | [route][route] |
| pfUSDC settlement **family** ID | `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b` | [route][route]; both Arc profiles |
| A666 issuer / route operator | `pffcb93d9f87a843a8aa34e1adf241f5d58143e81b` | [activation][activation] |
| Existing wA666 / controller | `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5 / 0x9A0262C0572fb4DB08765408eB225E207F40c3d9` | [route][route] |
| July route epoch / policy epoch | `2 / 2` | [activation][activation] |
| July primary policy hash | `77e8d7d8bd242fcfa3c87af6cd5f89aedc47e21a8c9de2cdd1afae3dace9e774979635b7eb176f4d9cd42dd02d69dbdb` | [activation][activation] |
| Issue / redeem multipliers (bps) | `10050 / 9995` | [activation][activation] |
| July pricing NAV epoch / NAV (USD_1E8) | `1 / 100000000` | [activation][activation], [NAV][nav] |
| July pricing reserve packet | `c8bbb35b7b0eb4a567f04945eb977b3fe5dc539cd7d845f1266d39959eb301ffbe263ebec9ceb985ef09370f314e5b3e` | [activation][activation], [NAV][nav] |
| July A666 NAV proof profile | `8c0244fe0cfb216fb5ab471d0c9e060a5c8ba052b5a29952d6e7aad76b24523af2b7e0ed82885c11d2c6308ddfcc9118` | [NAV][nav] |
| July reserve-proof policy | `a13553ba6f1a48dbe02dbc34de4d8faed1afa962dc2d2b29ff6f0c6b7ac6fd5c` | [NAV][nav] |
| September 7 A666 NAV epoch / NAV | `8 / 103523693` | [snapshot][snapshot] |
| September 7 A666 NAV profile | `a94a392926967722b2cceb88b19c4108726381f5b184007185a27e91f3e27c63dcc713a28f6cc4cf765826564e2b5f52` | [snapshot][snapshot] |
| September 7 A666 source class | `stakehub-six-leg-reserves-v3` | [snapshot][snapshot] |
| September 7 A666 valuation policy | `076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7` | [snapshot][snapshot] |
| September 7 A666 reserve packet | `f670ad00bfdcf838a5d551988a2f750bd118deae03ea8042a2ff3879bc2fd7d3664c22a87ff40e8aab2260b17685ac96` | [snapshot][snapshot] |
| September 7 pfUSDC NAV epoch / NAV | `52 / 1000000` | [snapshot][snapshot] |
| September 7 pfUSDC NAV proof profile | `f644d1c01b1bd0516122f78ffc6150493f9b87b3a84589ba8031b3df368c7833795f6fa926ef55d335059d502c5e5b09` | [snapshot][snapshot] |
| September 7 pfUSDC reserve packet | `863c61951090cffc4f7ffc3a581d3c73056a04eb0a170b94c8b1bd0b421d6a26c7573080b8a739daa5b3e1c807ee67c2` | [snapshot][snapshot] |

The July NAV registration's initial `USDC` valuation unit is corrected to `USD_1E8` by `deployments/a666-mainnet-20260727/11a-nav-valuation-unit-fix.ops.json`. The September 7 pfUSDC NAV source class is `vault_bridge:erc20_bridge_vault:5042002:0x160307f3efead79b6a3629c4b8d90e8301fc250f:0x3600000000000000000000000000000000000000`; its verifier is `sp1-arc-finality-v1` and valuation policy is `1c61349713b41cc15b3ec0863605b1ed5ef65ee43faa862ff17211c194023b26`.

For comparison, the frozen Ethereum successor profile is [`deployments/pfusdc-eth-mainnet-20260809-epoch6-successor/route-profile.mainnet-epoch6.json`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/pfusdc-eth-mainnet-20260809-epoch6-successor/route-profile.mainnet-epoch6.json). It binds the same pfUSDC family, not either Arc series. This is an archived source identity, not a current deployment selection.

| Ethereum comparator identity | Exact value |
| --- | --- |
| Bridge route / source chain / epoch | `ethereum-mainnet-usdc-v1 / 1 / 6` |
| Source domain | `erc20_bridge_vault:1:0x4939a45caa85da31fb26d7dbe6477b45f7f08688:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48` |
| Route-profile / deposit policy hash | `f088876e4bc7f611fdf7199237f241a1bb91ffc1850a8b65cd50a4852cab2ec40a2fae18c6dbf0ee5dd4934b22107f1a` |
| EVM route binding | `0a8b4b4184ca85b6b3f4e54d8bd581c747da3984df13b85d3610a57613d7228c` |
| Verifier kind / policy | `sp1-groth16 / 389a52d374b1ec5cb897aadaaf0d4db97df66a2f88b930c5fc783ccba19e95b6` |

Both Arc profiles use the pfUSDC family ID above, chain `5042002`, token `0x3600000000000000000000000000000000000000`, six decimals, verifier `sp1-arc-finality-v1`, evidence tier `receipt-proven`, and encoding `groth16`. The token runtime code hash is `0xc9987bd3af6b26a030951faa7eacc017b68343aeedf3ce5fe68f821c4b93939d`.

Arc source paths, relative to `docs/evidence/arc-mvp-20260828/`:

- August: [`route-profile.json`][arc7], [`route-profile-info.json`][arc7info], [`deployments.md`][deploy], and [`program-info.json`][program7].
- September 2: [`devnet-20260902/ingress/route-profile.v2.json`][arc9], [`route-profile-info.v2.json`][arc9info], [`factory.env`][factory], [`deposit.receipt.json`][deposit], [`devnet-20260902/devnet/relay-bundle.report.json`][relay], and [`program-info.current-v2-docker-20260902.json`][program9].

| Identity | August epoch-7 pair | September 2 current-v2 pair |
| --- | --- | --- |
| Route ID | `pfusdc-arc-testnet-tier4-epoch7` | `pfusdc-arc-testnet-tier4-epoch9` |
| Route epoch | `7` | `9` |
| Anchor | `0x661D558a818A07002C7D5da4A3179c4672FEf124` | `0x92390D3a2102CB74E4746c05B4d91F61093475D0` |
| Vault | `0xe88fb9ab4890f513261f0aca4ff13bfba3e14862` | `0x160307f3efead79b6a3629c4b8d90e8301fc250f` |
| Factory | `0xcc8D866C40eBf78185B3f0ca8e540c3cc1411953` | `0xC4932b2d2E2E7A97c90c1595a13A5913C70cab81` |
| Vault runtime code hash | `0x91e90b52152fe81576d7813f1f3f5c13bd35ec97cbae98991ce8f000c0b39de4` | `0x8d15bb9dc20c416db72c724eeeb725b8b395cb850a889cde130645dcc752764e` |
| Source domain | `erc20_bridge_vault:5042002:0xe88fb9ab4890f513261f0aca4ff13bfba3e14862:0x3600000000000000000000000000000000000000` | `erc20_bridge_vault:5042002:0x160307f3efead79b6a3629c4b8d90e8301fc250f:0x3600000000000000000000000000000000000000` |
| Route-profile hash / deposit policy_hash | `2f86a3ddfcd93a053346e9098fead505cf17731ac574f00662a8ec6e21503d6b6df0e165d16f0f2b633f830ddd38d80b` | `f7ce6d3cce3bd058a218db6bd829b01be13c576a2270aed362052d12654fc7a911a8423ee2d961ca45dbf72c08df6ae2` |
| EVM route binding | `6edfa31c57cfeec8955572fd9cdb81b22222beb1dbac432ff7c7f0fc7ad9c520` | `d9e0cd409c5d1e118d65c78ee059adcbba937616353e9675e350d52ee8d498b2` |
| Verifier policy hash | `14a86beca4947bda9e69a362b474d74f7bfe3e110e562c341c5c89e36ead68ee` | `1c61349713b41cc15b3ec0863605b1ed5ef65ee43faa862ff17211c194023b26` |
| Ingress program vkey | `0x0010bf08f18ba7b60a7c6d10ccdad3d29e2fdfeecec8426a12be127d44710929` | `0x0050a8b0daed2fa75f44d4102b42204c34668a03d311cb727cc6ca3f8df5cf16` |
| Egress program vkey | `0x00c8d744e19bc828d1b3fb19709d36863d8c5aba14af0ca939eb85fc806f868f` | `0x0036cbe7d36bbfe1118a3c544eeba74f3791d2a19bf5ec59b972f72d36416852` |
| Egress ELF SHA-256 | `8e2464227d7428d9928871c4a655fd73f6a87879c2e8eae6c0228a5db367f7bd` | `8b0f266a035a432ef3c0e4233672d8a15b913cefd4c85ff07f91f008601bb744` |

The September deposit receipt identifies the full vault and anchor log emitters. [`devnet-20260902/devnet/arc-finalize-claim.report.json`][claim] records finalize/claim in height 947; [`devnet-20260902/egress/egress-witness.json`][egress] pins the epoch-9 route/profile and height-948 burn; [`withdraw.receipt.json`][release] records the release. These are the plan's existing bridge-cycle evidence, not an A666 cycle. The stale deployment-status heading does not supersede the dated September packet. The August pair remains pinned to its August egress key. Neither pair is selected here.

**Derived identities, not live asset readbacks.** Calling the candidate's `pfusdc_source_series_id` and `vault_bridge_bucket_id` offline with PFTL chain `postfiat-wan-devnet-2`, the shared family ID, and each complete profile tuple gives:

| Profile tuple | Exact source-series asset ID | Exact bucket ID |
| --- | --- | --- |
| August epoch 7 | `eae855625e28c65fc9a10358abedfbbc4c08d862dccf710d4b48d3de5c0ebd59c170196bc5e35f5d34368e9072da75fe` | `25a4fb3e4d6779e0801aaedcda9696a5ee74ce1b5102459cf89e469a617b3594858e85dde85b133052618ab09e5e657c` |
| September epoch 9 | `3923511d5be0557a61051e099b606d3decc11a5ba274c7d551168735accad8ed18d89c9200efc4bfbbbab6e85d4c173f` | `fcc209605f8cfda895acbf78047f83f97b0bc1cee3927582fb262efb46e7d136b098183d5f83e19a82d34c218bab67a7` |

The September bucket matches the archived withdrawal packet exactly. Derivation does not prove an extant balance, source-series activation, or an enabled primary-route custody row. A route-profile hash, its verifier policy hash, and the family's NAV proof-profile ID are different identities.

## Compatibility trace

### G2.1 — PASS: candidate source pinned and required code present

The [existing kinds](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/types/src/core_chain.rs#L44) include `vault_bridge_deposit_claim`, `vault_bridge_burn_to_redeem`, `pftl_uniswap_order_reserve`, `pftl_uniswap_primary_subscribe_v2`, `pftl_uniswap_order_release`, `pftl_uniswap_primary_redeem`, and `pftl_uniswap_route_epoch_advance`.

[Arc proof admission](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L7703) verifies bounded Groth16 bytes and matches chain, vault, token, route binding, deposit ID, amount, recipient, nonce, and block hash. [Claim execution](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L2410) binds the governed route epoch and creates/credits the exact source series when source-series activation applies. [Burn execution](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L6934) selects that series through its bucket; [the node bundle](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/node/src/vault_bridge_workflows.rs#L1524) makes the same selection.

Test evidence: `arc_ingress_real_groth16_fixture_verifies_and_rejects_mutations` passes. It uses the separate corrected epoch-8 fixture, so it proves the verifier implementation rather than selecting either Z3 pair. The [retained proof-identity record][proofids] also retains the September current-v2 ingress/egress identities; its historical CI was read, not rerun here. The existing genesis/governance source-series activation height is resolved in `crates/node/src/execution_actions.rs:372`; a future activation amendment is an existing operation, not a code addition. Its selected-chain state must be pinned before qualification. G1 deployment/lineage selection remains the operator's decision.

### G2.2 — NEEDS OPERATOR DECISION: archived identities read; selected tuple remains open

The tables read back both pairs and distinguish the route family, source series, profile, policy, and NAV epochs. No frozen packet cited here establishes an operator-selected Z3 pair plus an enabled A666 source-custody row and fresh NAV/policy on the selected lineage. This checkbox remains unticked. In particular, the July activation fixture contains no `settlement_source_asset_ids` selection.

Test evidence: `pfusdc_source_series_identity_binds_every_governed_source_field` passes; the canonical-function derivation above agrees with an independent SHA3-384 calculation and the September bucket readback. These checks establish identity semantics, not operator approval.

### G2.3 — PASS: existing operations can settle the governed Arc source

The route's `settlement_asset_id` remains the pfUSDC **family** ID. Replacing it with an Arc series ID is not the compatibility mechanism.

1. [Route epoch advance](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L5527) accepts `settlement_source_asset_ids` through the existing issuer-authorized operation. [`pftl_source_govern`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/pftl_source_settlement.rs#L90) enables custody rows on that same route. It creates no second route or NRRS facility. The asset must already exist as a six-decimal source series of the route family, with an active, fully counted bucket and `impairment_factor_bps == 10000`.
2. [Order reserve](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L4435) takes signed `settlement_source_asset_id`; `pftl_source_reserve` checks issuer enablement and escrows that asset from the subscriber. Omitting the selector uses pooled family pfUSDC.
3. [Subscribe v2](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L4699) still requires the family ID in `settlement_asset_id`. It consumes the reservation's exact source escrow, credits newly issued A666, increases aggregate and source principal by the base amount, and records spread separately. It cannot choose a different funding series at subscription.
4. [Entitlement release](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L4608) removes unused export capacity without changing minted supply or reserve principal. Fresh NAV and route maintenance then use their existing operations.
5. [Primary redeem](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L5345) takes signed `settlement_source_asset_id`; `pftl_source_redeem` debits that source's principal and credits that same series to the recipient. Pooled redemption cannot consume source principal, and a selected source cannot draw another source's reserve.

The route's Ethereum chain/controller fields remain its existing export/return bindings. [Live route-reference verification](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/pftl_uniswap_ethereum_verification.rs#L94) does not equate those fields with the settlement source chain. This transparent cycle releases its export entitlement and performs no A666 export.

Test evidence: `pftl_source_settlement_roundtrip_preserves_source_and_pooled_custody` passes, including governance enablement, signed reserve/subscription/redemption, refund, failed-debit rollback, disabled-source issue rejection, pooled-drain rejection, and redemption replay rejection. Its source is an Ethereum-shaped synthetic series and it bypasses external route-reference verification; it is not a frozen Arc end-to-end replay. The chain-independent family/source checks above establish applicability to the Arc tuples, subject to G1 state selection.

**Driver result:** all five existing Python tests pass. The hard-coded default subscriber is `pfab9b9228942e5c529633a13aa271d5297bec6353`; the reservation recipient is `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0`. Route and A666 asset constants are the exact primary identities tabulated above. An additional offline invocation of the unchanged `cmd_build_issue`, using each derived Arc identity in temporary fixtures, confirms that adding `settlement_source_asset_id` to route status emits no source selector. Substituting the fixture's `settlement_asset_id` emits a subscription with the wrong family binding while reserve still omits the selector. Such generated JSON would not fund the selected source on the existing route. No valid Arc cycle can be run through this driver without tooling changes.

### G2.4 — PASS: reserve counted once; settlement identities cannot be substituted

For issue base `B` and spread `S`, source escrow becomes source principal `B` plus source spread `S`; aggregate route reserve increases by `B`, not `B + S`. These are two views of the same custody.

[`issued_asset_supply_with_non_nav_spread`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/issued_asset_ledger_helpers.rs#L711) subtracts source principal/spread from family route custody, then counts them once under their exact series, including outstanding source escrows. `issued_asset_family_supply` sums the family and its distinct series. The source-roundtrip test asserts unchanged family supply and pooled balance across reserve, refund, subscription, and redemption.

[`nav_subscription_reserve_overlay`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L390) includes the aggregate primary reserve once, capped by active bucket backing. It does not add the source-custody principal again; spread and unconsumed reservation escrow are excluded from NAV. [Reserve submission](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nft_escrow_asset_execution.rs#L1304) subtracts this overlay from submitted net assets before verifying the base proof and binds the composite source root. A fresh packet therefore contains base proof value plus the overlay once. `a666_nav_overlay_counts_only_vault_backed_primary_market_reserve` and `ar07_wrong_overlay_fails_closed` pass.

The family mismatch check in subscribe, source-family/bucket checks, exact reservation custody, and source-specific redemption capacity enforce the identity binding. [Signed operation encodings](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/types/src/transactions_mempool_receipts.rs#L2745) bind reserve/redeem selectors and the governance source list. `pftl_source_selections_are_signed` and `pftl_source_governance_signs_unchanged_disabled_and_selected_states` pass. Epoch advance changes policy/configuration and source enablement, not the route's family ID. [Ledger validation](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/types/src/market_nav_asset_types.rs#L4940) enforces custody family/amount consistency; [replicated state commitment](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/node/src/state_commitment.rs#L1530) commits the source rows. Source rows are outside the exported route witness, preserving its existing format.

Limits: the executed tests prove the component accounting and rejection rules. No existing test run here combines either exact Arc tuple with same-cycle claim, fresh A666 proof, route advance, redemption, and Arc release. G3 must cover that composition and its source-specific artifact reconciliation. The issuer source enablement must be established as route maintenance, not substituted for each customer's signed trade.

### G2.5 — PASS: prohibited-expansion stop checked

No new consensus implementation, transaction kind, generic NRRS facility, bridge contract, or settlement-price format is required by this compatibility path. [`pftl_uniswap_v2_base_value`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/execution/src/nav_vault_asset_execution.rs#L4393) uses the existing finalized NAV and valuation-unit conversion; `pftl_source_asset_check` restricts admitted source series to full par backing. The deployed primary multiplier/rounding model is the section 2.2 demonstration profile.

### Tests executed

From the detached candidate, with Rust `1.95.0`. Rust commands used `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/z3-g2-20260917-target CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_DEBUG=0`.

| Command after that Rust environment prefix | Result |
| --- | --- |
| `cargo test -p postfiat-execution tests::pftl_source_settlement_roundtrip_preserves_source_and_pooled_custody --locked -- --exact --nocapture` | 1 passed |
| `cargo test -p postfiat-execution a666_nav_overlay_counts_only_vault_backed_primary_market_reserve --locked -- --nocapture` | 1 passed |
| `cargo test -p postfiat-execution tests::arc_ingress_real_groth16_fixture_verifies_and_rejects_mutations --locked -- --exact --nocapture` | 1 passed |
| `cargo test -p postfiat-execution ar05_active_export_entitlement_blocks_route_epoch_advance_until_closed --locked -- --nocapture` | 1 passed |
| `cargo test -p postfiat-execution ar07_wrong_overlay_fails_closed --locked -- --nocapture` | 1 passed |
| `cargo test -p postfiat-types pftl_source_ --locked -- --nocapture` | 2 passed |
| `cargo test -p postfiat-types pfusdc_source_series_identity_binds_every_governed_source_field --locked -- --nocapture` | 1 passed |

`PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-a666-pfusdc-reserve-demo.py -v`: **5 passed**. Temporary fixture diagnostics used a dummy signer-file object and did not sign or submit. An initial Rust `--exact` filter without the `tests::` prefix selected zero tests; the corrected one-test result above is the evidence. No workspace, Orchard/Halo2, fleet, or archived-chain campaign was run.

Publication gates on main: `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links` (406 files), and `scripts/public-secret-scan` (tracked tree) all passed. The link checker used `.venv-docs/bin` on `PATH`. MkDocs excludes `docs/review/` from the public site by existing configuration; the verdict is published in Git.

## Stop verdict

**No technical expansion stop on the reviewed candidate.** Existing source selection and issuer route maintenance can express the Arc-backed primary cycle. Do not replace the route family ID, add a new facility, or deploy another bridge to work around the driver's missing fields.

**Hold for the operator:** choose the qualified lineage and exact pair, then pin source-series activation, profile, policy, source enablement, NAV, and available source custody. The G2.2 checkbox and every G1 checkbox remain open. This verdict does not qualify the August pair for current-v2 use or authorize a transaction. If the operator's selected state cannot meet the existing checks, stop; this verdict authorizes no consensus repair.

## What remains for G3

- Require explicit route ID, native asset ID, settlement **family** ID, exact source-series ID/bucket/profile, PFTL chain, subscriber/owner, and reservation recipient. Remove qualification defaults: `ROUTE_ID`, `A666_ASSET_ID`, `DEFAULT_SUBSCRIBER`, and `DEFAULT_ETHEREUM_RECIPIENT`; `validate_route` also fixes Ethereum chain `1` and the trust classes. Keep the existing route's export identity distinct from Arc's source chain.
- Emit the existing signed `settlement_source_asset_id` in reserve and redeem. Keep subscribe's `settlement_asset_id` as the family ID; bind manifests and balance verification to the exact series. Support `settlement_source_asset_ids` in the route-maintenance builder: absent means unchanged, an empty list disables issue, and a populated list selects enabled series. Verify existing source enablement before user issuance.
- Reconcile the NAV manifest interfaces. The driver requires `postfiat.a666.live_nav_mark.v1`, `nav_per_unit_usd_1e8`, `verified_net_assets_usd_1e8`, and historical boolean fields; the current NAV builder emits `postfiat.a666.provider_neutral_nav_mark.v1`, `nav_per_unit`, and `verified_net_assets`. The route-advance builder also pins successor profile `f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91`, which differs from the September 7 snapshot. Accept and validate the operator-selected proof/profile/valuation identities explicitly; do not merely rename an incompatible packet.
- Consume the existing `source_settlement_custody` rows emitted by `crates/node/src/market_bridge.rs:1690`; no new consensus readback format is needed. Bound redemption by the same-cycle base reserve **and the selected source's remaining principal**, policy capacity, wallet A666, and fresh NAV. The status field `available_redeem_atoms` is aggregate, not source-specific. Check exact source balance, aggregate reserve, source principal/spread/escrow, family supply, entitlement release, and composite NAV overlay. Bind source rows to finalized state evidence rather than relying only on the route's `ledger_hash`. Reconcile reserve reduction to redemption base; output plus spread equals that base.
- Compose existing Arc claim/burn/proof commands around the driver with explicit pair/proof-release selection and stops before any future submission. Preserve fail-on-overwrite behavior and separate signer-local requests from redaction-safe evidence.
- Add the planned manifest/verifier and focused success, stale-proof, wrong-route/source/family, duplicate/replay, active-entitlement, insufficient-source-capacity, and partial-artifact cases. Include the exact selected Arc tuple and NAV refresh in the offline composition. Keep the public testnet status `Z3 = OPEN`.

[route]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/a666-mainnet-20260727/09-production-route-config.json "deployments/a666-mainnet-20260727/09-production-route-config.json"
[digest]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/a666-mainnet-20260727/09-production-route-config-digest.json "deployments/a666-mainnet-20260727/09-production-route-config-digest.json"
[activation]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/a666-mainnet-20260727/10-production-route-activate.ops.json "deployments/a666-mainnet-20260727/10-production-route-activate.ops.json"
[nav]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/a666-mainnet-20260727/opening-nav-proof-manifest.json "deployments/a666-mainnet-20260727/opening-nav-proof-manifest.json"
[snapshot]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/a666-source-route-20260907/canary-rpc.json "deployments/a666-source-route-20260907/canary-rpc.json"
[arc7]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/route-profile.json "docs/evidence/arc-mvp-20260828/route-profile.json"
[arc7info]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/route-profile-info.json "docs/evidence/arc-mvp-20260828/route-profile-info.json"
[arc9]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/route-profile.v2.json "docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/route-profile.v2.json"
[arc9info]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/route-profile-info.v2.json "docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/route-profile-info.v2.json"
[deploy]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/deployments.md "docs/evidence/arc-mvp-20260828/deployments.md"
[factory]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/factory.env "docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/factory.env"
[deposit]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/deposit.receipt.json "docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/deposit.receipt.json"
[relay]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/relay-bundle.report.json "docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/relay-bundle.report.json"
[claim]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/arc-finalize-claim.report.json "docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/arc-finalize-claim.report.json"
[egress]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/egress/egress-witness.json "docs/evidence/arc-mvp-20260828/devnet-20260902/egress/egress-witness.json"
[release]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/devnet-20260902/egress/withdraw.receipt.json "docs/evidence/arc-mvp-20260828/devnet-20260902/egress/withdraw.receipt.json"
[program7]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/program-info.json "docs/evidence/arc-mvp-20260828/program-info.json"
[program9]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/evidence/arc-mvp-20260828/program-info.current-v2-docker-20260902.json "docs/evidence/arc-mvp-20260828/program-info.current-v2-docker-20260902.json"
[proofids]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/deployments/release-repair-20260916/proof-identities.json "deployments/release-repair-20260916/proof-identities.json"
[repair]: https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/docs/plans/completed/release-blocker-repairs-20260916.md "docs/plans/completed/release-blocker-repairs-20260916.md"

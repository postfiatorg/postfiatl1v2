# Finding — legacy Arbitrum One vault-bridge route is an active, SP1-bound route

Author: Ghash [orc], dispatch #86, 2026-07-25 UTC. Read-only record.
No fleet host or RPC was accessed for this finding; no deployment, governance
action, recommendation, or mutation outside this one artifact was performed.
Shared-tree rule observed: one owner per artifact, no destructive git commands,
foreign-path anomalies are reports.

Source of truth for every governance value below — the already-copied campaign
snapshot, not a new read:

- path: `docs/evidence/pfusdc-eth-campaign-20260725/lane-b/conservation-current/scratch/run-5ed3c8d275504797a16c4130844c504d/snapshot/governance.json`
- SHA-256: `a8a8e668e784abfdc9ef9fa3faddcea3f9ab2bbc56078868848e6e64e134f5d4`
- located with `rg -l 'pfusdc-tier4-arbitrum-one-v1' docs/evidence/pfusdc-eth-campaign-20260725`
  (the same values also appear in the sibling `imported-data/governance.json` of
  that run)
- `vault_bridge_route_profiles` contains exactly **one** record (`profile_count = 1`)

<a id="verifier-classification"></a>

## verifier-classification

Classification: **SP1 program-vkey (receipt-proven) profile**. It is *not* an
observer/attestation-quorum route and *not* a mock route.

| Field path in the snapshot | Value |
| --- | --- |
| `.vault_bridge_route_profiles[0].profile.route_id` | `pfusdc-tier4-arbitrum-one-v1` |
| `.vault_bridge_route_profiles[0].profile.verifier_kind` | `sp1-arbitrum-finality-v1` |
| `.vault_bridge_route_profiles[0].profile.verifier_program_vkey` | `0x0033bd140207b97fb2442eb279cc2ce55714be6fbcd66beb325fe7c3786d4dfc` |
| `.vault_bridge_route_profiles[0].profile.verifier_policy_hash` | `6e44608381a3ec9ea38e171547daed2a16e6ee6f3fce1dc1e18df4dc39ceb4cc` |
| `.vault_bridge_route_profiles[0].profile.evidence_tier` | `receipt-proven` |
| `.vault_bridge_route_profiles[0].profile.verifier_proof_encoding` | `groth16` |
| `.vault_bridge_route_profiles[0].profile_hash` | `d71eb53441627b41049e12481ac0f8f90f469b016648f85e626773fa8c2e6c057d181fbe42ff9f177dd5681260ba7332` |
| `.vault_bridge_route_profiles[0].profile.activation_height` | `64` |
| `.vault_bridge_route_profiles[0].authorized_height` | `66` |
| `.vault_bridge_route_profiles[0].governance_amendment_id` | `219306e62688976275b0c98499a4f43e2d69ced8e2d2ee12c35ca486896ed2f4fbc92fcfa5c997e1614d69ac20ad39d9` |
| `.vault_bridge_route_profiles[0].profile.expires_at_height` | `100000` |
| `.vault_bridge_route_profiles[0].profile.source_chain_id` | `42161` |
| `.vault_bridge_route_profiles[0].profile.vault_address` | `0x850e4ceea147f3551c68c2251129e5945d0afb58` |
| `.vault_bridge_route_profiles[0].profile.token_address` | `0xaf88d065e77c8cc2239327c5edb3a432268e5831` |
| `.vault_bridge_route_profiles[0].profile.vault_runtime_code_hash` | `0x12b566433c726126ba812e5f21510420d6374b137181523e4030028012816fe2` |
| `.vault_bridge_route_profiles[0].profile.token_runtime_code_hash` | `0xad30d819dbc47814b7e6cb837fd7cc57fcb591479a38596ee93de4fc52e8c435` |
| `.vault_bridge_route_profiles[0].profile.min_attestations` | `0` |
| `.vault_bridge_route_profiles[0].profile.minimum_confirmations` | `0` |
| `.vault_bridge_route_profiles[0].profile.asset_id` | `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b` |
| `.vault_bridge_route_profiles[0].profile.route_epoch` | `1` |
| `.vault_bridge_route_profiles[0].schema` | `postfiat.vault_bridge.route_profile_record.v1` |

**No-fallback invariant verdict: PASS.** The tiers are mutually exclusive
branches of one validator, so a proof-verified profile cannot silently degrade to
observer evidence: `crates/types/src/account_owned_asset_types.rs:199-210` forces
the `independently-observed` tier to carry *no* proof-verifier fields, while
`:211-238` forces the SP1 branch (`sp1-groth16`, `sp1-arbitrum-finality-v1`,
`sp1-arbitrum-bonded-v1`) to reject any `min_attestations`/
`minimum_confirmations` above zero and to require a 32-byte
`verifier_policy_hash`, a `0x`-prefixed 32-byte `verifier_program_vkey`, and
`groth16` encoding; `:240-245` then rejects any profile whose `evidence_tier`
disagrees with its `verifier_kind`, and `:105-106` defines the only two tier
constants. The snapshot values satisfy exactly the SP1 branch, and there is no
third "mock" tier in the type at all. Route/proof-kind pairing is additionally
pinned per chain at `crates/types/src/market_nav_asset_types.rs:3669-3689`
(Arbitrum One `42161` accepts only `sp1-arbitrum-finality-v1` /
`sp1-arbitrum-bonded-v1`), and Ethereum routes are held to
`sp1-ethereum-finality-v1` at
`crates/execution/src/nav_vault_asset_execution.rs:4971-4981`.

<a id="coexistence-mechanics"></a>

## coexistence-mechanics

All citations are code paths, not tests.

1. **Active-route selection before expiry** —
   `crates/types/src/shielded_bridge_governance.rs:961-1003`
   (`active_vault_bridge_route_profile`): requires the route authority to be
   activated and the height to be at or above it (`:966-971`); skips records whose
   `asset_id` differs or whose `activation_height > current_height` (`:974-977`);
   picks the maximum `(route_epoch, activation_height)` and **fails closed on a
   tie with differing `profile_hash`** ("vault bridge route governance is
   ambiguous at the latest epoch", `:979-995`); errors if no governed profile
   applies (`:1000-1001`). Authorization consistency, including a `paused`
   authorizing amendment, is enforced at `:898-921`
   (`active_vault_bridge_route_policy_hash`).
2. **Can ingress transact now?** Yes, on this route, subject to proof material.
   New ingress is bound to the *active* profile: `crates/node/src/execution_actions.rs:131`
   resolves `active_vault_bridge_route_profile(&target.asset_id, block_height)`
   whenever `require_active` is set, and `:135-146` rejects the transaction unless
   the operation's `policy_hash` equals the active `profile_hash`.
   `require_active: true` is set for `VaultBridgeDepositPropose` (`:235-243`) and
   `VaultBridgeReceiptSubmit` (`:246-256`). With `activation_height 64`,
   `authorized_height 66` and `expires_at_height 100000`, any height in
   `[66, 100000)` resolves this profile as active, so at the campaign's recorded
   ce22 tip the Arbitrum One route is the live ingress route for asset
   `02c46a36…05d7b`, and Arbitrum proof material is the only kind it accepts
   (`crates/types/src/market_nav_asset_types.rs:3679-3686`).
3. **Can egress transact now?** Yes, and it stays possible after any future
   rebinding, because egress resolves the *pinned* profile rather than the active
   one: `crates/node/src/execution_actions.rs:132-134` uses
   `authorized_vault_bridge_route_profile(&target.asset_id, &target.policy_hash)`
   when `require_active` is false (bucket-scoped operations, `:214`, `:218-231`),
   and the redemption/egress paths do the same —
   `crates/node/src/pfusdc_tier4.rs:104-107` (Tier-4 egress witness) and
   `crates/node/src/batch_snapshot.rs:136`. The intent is stated in the code
   comment at `crates/types/src/shielded_bridge_governance.rs:922-925`: "new
   ingress must use the active route, while an already-pinned deposit or
   redemption must remain finishable after a later route becomes active".
   Conservation reporting follows the same split at
   `crates/node/src/vault_bridge_conservation.rs:124` (active) and `:153`
   (authorized/pinned).
4. **Natural expiry** — `crates/types/src/shielded_bridge_governance.rs:998-1001`:
   once `current_height >= profile.expires_at_height` the resolver returns "latest
   vault bridge route profile is expired", after which no `require_active`
   operation can bind the route. For this profile that boundary is height
   `100000`. The profile validator also requires
   `expires_at_height > activation_height` and a non-zero activation
   (`crates/types/src/account_owned_asset_types.rs:254-258`), so expiry cannot be
   set to never.
5. **Explicit governance deactivation** — **mechanism absent.** No transaction,
   governance operation, or CLI command in the searched crates deactivates,
   revokes, retires, or disables a vault-bridge route profile. The only related
   levers found are: (a) the authorizing amendment's `paused` flag
   (`crates/types/src/shielded_bridge_governance.rs:440`), which is *checked* at
   `:915-920` and `:949-954` and makes both active and pinned resolution fail
   closed — but no code path in the searched crates sets `paused = true` for a
   vault-bridge route amendment; and (b) `bridge-pause` /
   `bridge_pause(BridgePauseOptions)`
   (`crates/node/src/main_parts/cli_dispatch_parts/group_05.rs:2239-2252`,
   `crates/bridge/src/lib.rs:674`), which pauses a *legacy bridge domain*, a
   different object from a vault-bridge route profile. Route removal by editing
   `vault_bridge_route_profiles` is not a governance action and is not asserted
   here.
6. **Supersession / rebinding when pfUSDC is bound to Ethereum** — supersession is
   the additive path, not deletion. A new authorized
   `VaultBridgeRouteProfileRecordV1` for the **same `asset_id`** with a strictly
   greater `(route_epoch, activation_height)` becomes the active route at its
   activation height by the ordering rule at
   `crates/types/src/shielded_bridge_governance.rs:979-995`, while the Arbitrum
   record remains in governance and remains resolvable by
   `authorized_vault_bridge_route_profile` (`:922-955`) so pinned Arbitrum
   deposits/redemptions stay finishable. Each profile's authorizing amendment kind
   is bound to its own profile hash
   (`vault_bridge_route_amendment_kind`, `:1059-1068`, prefix constant
   `GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1` `vault_bridge_route_v1` at
   `:11`), which is why the snapshot's
   `.amendment_supersession_records` carry `kind`
   `vault_bridge_route_v1:<asset_id>:<profile_hash>`. Two same-epoch,
   same-activation profiles with different hashes are rejected rather than
   silently coexisting (`:986-993`). Consequence for a pfUSDC-to-Ethereum
   rebinding: if the Ethereum profile is authorized under the *same* asset id at a
   higher epoch, new ingress moves to Ethereum automatically and Arbitrum ingress
   stops being bindable; if it is authorized under a *different* asset id, the
   Arbitrum route remains the active route for `02c46a36…05d7b` until its own
   expiry at height `100000`.

<a id="legacy-lineage-figures"></a>

## legacy-lineage-figures

Canonical figures for this legacy lineage. The two balance/supply rows are
**independently verified campaign inputs supplied to me, not new reads by me**;
the governance rows are read from the snapshot cited at the top of this record.

| Figure | Value | Provenance |
| --- | --- | --- |
| Route id | `pfusdc-tier4-arbitrum-one-v1` | snapshot `.vault_bridge_route_profiles[0].profile.route_id` |
| Source chain | `42161` (Arbitrum One) | snapshot `.profile.source_chain_id` |
| Vault address | `0x850e4ceea147f3551c68c2251129e5945d0afb58` | snapshot `.profile.vault_address` |
| Activation height | `64` | snapshot `.profile.activation_height` |
| Expiry height | `100000` | snapshot `.profile.expires_at_height` |
| Native USDC backing | `5,999,000` atoms | campaign input, independently verified; not re-read here |
| USDC.e backing | `0` | campaign input, independently verified; not re-read here |
| Circulating pfUSDC | `1,000,010` atoms | campaign input, independently verified; not re-read here |

Cross-reference note for Krimp: cite this record's anchors
(`#verifier-classification`, `#coexistence-mechanics`,
`#legacy-lineage-figures`) from the checker and runbook instead of duplicating any
of these facts.

## Scope limits of this record

- Classification and mechanics only. No recommendation, no deactivation plan, no
  activation or governance action is proposed or taken here.
- Nothing in this record was derived from tests; every mechanic is cited to
  non-test source lines, and where the code supplies no mechanism the record says
  "mechanism absent".
- No fleet host, RPC endpoint, or credential was accessed.

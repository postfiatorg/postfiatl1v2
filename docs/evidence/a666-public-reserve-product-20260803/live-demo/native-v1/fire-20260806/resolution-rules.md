# Fire 2026-08-06 staged resolution rules

## Immutable source and output discipline

The 16 HELD source packets and `authorization-binding-native-v1.json` in the parent `native-v1/` directory are immutable lineage inputs. `resolve.py` copies only the selected source bytes into `packets-<stage>/`, adds each copy's `source_packet_sha256` and `resolved_fields` diff list, and regenerates `binding-<stage>.json`. Any source-packet or resolved-packet change requires a new stage binding. Any deviation anywhere is STOP-no-retry.

Resolver input is JSON with `stage`, an `active_packets` array, and `resolved_values` mapping packet filenames to JSON-pointer/value replacements. It accepts only existing field pointers, serializes sorted JSON with fixed separators, and self-checks the same input twice for identical SHA maps.

## Stage S1, t0

Resolve and bind legs 0, 1, 2a, 2b, 3a, 3b0, 3b, 3c, 3d, and 3e. Leg 3e amount-in is exactly `11027135` atoms, chained to the required leg-2b mint output. Leg 3e min-out is `floor(0.97 × fresh simulated fill)`. The S1 binding is authorization preparation only: every command still requires packet hash confirmation, receipt gating, and STOP-no-retry enforcement.

FIRE-1 first reported a STOP-quality simulation-evidence defect: `/tmp/krimp-fire1/fork-sims.txt` recorded a stale quoted block and failed two-RPC agreement. This rejection is retained as audit history. FIRE-1B resolved it with `/tmp/krimp-fire1b/fork-sims-agreement.txt` and `/tmp/krimp-fire1b/fork-sims-values.txt`: publicnode and drpc agree at block `25696286`, block hash `0x05c48df05c746fb065e495887df17b99a5f65575f8375acedc155a3f2b5e0ae8`, delta zero; the S1 leg-3e fill is `8057858`, its min-out is `7816122`, and fire must occur no later than block `25696414` (128-block freshness window).

## Stage S2, after finalized leg 3e

Resolve and bind legs 3f, 3g, and 3h only after the leg-3e journal receipt is finalized. Both approvals are exactly the actual leg-3e USDC output atoms. Leg 3h amount-in is that same actual output. Leg 3h min-out is `floor(0.97 × fresh re-simulated fill)`, with a fresh two-RPC-agreed block no more than 128 blocks behind fire. FIRE-1B is reference evidence only: at its block, `8057858` USDC simulated to `11013374` wA666 and the reference min-out was `10682972`; it does not override the receipt-chain or re-simulation requirement.

## Stage S3, after finalized leg 3h

Resolve and bind leg 4 only after the leg-3h journal receipt is finalized. Leg 4 `amount_atoms` is exactly the actual leg-3h wA666 output atoms. The two-phase burn/import gate remains mandatory: phase 1 status-1 burn receipt injects its transaction hash into phase 2; a missing or mismatched receipt is STOP-no-retry.

## Stage S4, after finalized leg-4 import

Resolve and bind legs 5a and 5b only after the leg-4 import receipt is finalized. Leg 5a `nav_amount_atoms` is the actual imported amount. Its minimum settlement is `floor(ceil(amount × 90234207 / 100000000) × 9995 / 10000)`. Leg 5b amount is exactly the actual leg-5a pfUSDC payout. No receipt-chained amount may be substituted with a reference simulation value.

## Field rules

- **Leg 2a expiry.** Bind `expires_at_height = current fleet binding height + 1000`. At the FIRE-1 snapshot this is `776 + 1000 = 1776`. This is a declared deviation from accepted precedent: the five recorded order-reserves in `/tmp/ghash-u28/precedent-bodies.json` at batches 658, 665, 672, 679, and 689 expire 99 blocks after their reserve batch (757, 764, 771, 778, and 788 respectively). Ruling rationale: an inter-leg expiry is the worse failure; a 1000-block reservation auto-releases on abort, with no fund loss, and gives roughly a 90-minute worst-case lock window at seconds-scale fleet blocks.
- **Nonce rule.** Derive `subscription_nonce` (2b), `export_nonce` (3a), and `redemption_nonce` (5a) as the SHA-256 digest of `native-v1|pftl-a666-ethereum-wA666-usdc-v1|<leg>|776|20260806`, with `<leg>` respectively `leg2b-subscribe`, `leg3a-export`, and `leg5a-redeem`. Nonce encoding follows accepted on-chain wire format (64-char lowercase hex, no `0x`), digest construction unchanged. This is code-mandated by the lower-hex 64 validation and matches the accepted U28 bodies and opening-export precedent. The replay sweep is CLEAN; any future collision causes a new stage binding, never nonce reuse.
- **Leg 3a refund delay.** Bind `refund_delay_blocks = 100`, the accepted precedent in `deployments/a666-mainnet-20260727/11-opening-inventory-export.ops.json:26`.
- **Gas ceilings.** Each gas leg receives a fresh ceiling computed as current base fee × gas units × 1.25 and converted using the current ETH/USD quote. The binding must prove `prior_actual + fresh_quote <= 530.000000` or STOP-no-retry. FIRE-1 reference quotes total `0.9220` USDC: projection `501.024845 + 10.000000 + 0.9220 = 511.946845 <= 530.000000`, headroom `18.053155`. The fire-time binding must re-verify this arithmetic.
- **Lineage.** Every resolved copy includes `source_packet_sha256` and the exact `resolved_fields` JSON-pointer diff list. The stage binding contains only hashes of its resolved copies, never a mutable source hash.

## Freshness gate

Freshness gate: before any swap leg executes, if current Ethereum block > sim_block + 128, the sim is re-run and the stage binding regenerated (S1b refresh for 3e; S2 already covers 3h). Expired freshness = STOP-no-retry.

## Stage authorization rule

A completed stage binding is not an execution command. The executor must first verify its exact packet hash against the stage binding, complete all listed preconditions, and commit the required final receipt before the next stage exists. Any stale simulation, unresolved ceiling, replay hit, state disagreement, or source/receipt mismatch is STOP-no-retry.

## FIRE-10 v2 rulings

- **Leg 1 policy correction and proof gate.** The deposit relay uses vault-bridge claim policy `5025bdfe92669e3d8f81ce7e739fd132063261b92ef7e7ee7db19b2762e88b736bd40cd4826375e041584533f4137158`, not primary-market policy `db6be8d0…`; the latter remains limited to legs 2a, 2b, and 5a. Leg 1 is three ordered stages: deposit, `native_prover_leaf.py`, then relay/claim. The relay accepts source proof kind `sp1-ethereum-finality-v1`; source proof and public-values hashes are stage-2 receipt outputs. Proposer/finalizer are the same issuer identity, and no attestor flag is allowed because the policy has `min_attestations=0`.
- **Leg 3a optional fields.** `ethereum_packet_digest` and `ethereum_packet_schema_version` are omitted from the export-debit op. FIRE-10 code-truth determined both fields optional; no derived digest is supplied.
- **U57 custody leaves.** Legs 3b through 3h invoke `scripts/native_evm_contract_leaf.py` with the exact U57 argv contract. Calldata that depends on a prior receipt remains stage-bound; calldata generated from already-fixed inputs is recorded with its encoding command and SHA in the resolution input.
- **Staged-fields convention.** Every remaining `PENDING-FIRE-TIME` value is recorded in the stage binding with its named producing receipt or prerequisite stage. An unresolved value is an execution gate, never a default. For legs 2b through 5b, receipt-chained values must come only from the specified prior finalized receipt. For leg 1, `beacon-endpoint` is BOUND to `https://ethereum-beacon-api.publicnode.com` from prover stand-up evidence `/tmp/krimp-gpu/instance.txt` and rearm files; `prover-ssh-target` is BOUND to `pftl-fire-prover`, which resolves through `/home/postfiat/.ssh/config` to Vast instance `47003476` at `ssh4.vast.ai:13476`. For EVM owner nonces and gas ceilings, the fire-time read/quote is the only source.
- **U64/FIRE-15 depositV2 ruling.** Leg 1 stage-1 uses the patched depositV2 client tool (commit `c33b3b7` on deployed lineage `2246d257`, binary SHA-256 `a982f8d27a42daad39e6a7d2ad1aff69a97064b7890da654fc9aae8f47f58f95`, path `/tmp/fire-20260806-bin/postfiat-node-client-depositv2`); fleet deployed binary pin `05330fb20a40b8a4536000ec57da1862d879bcdc4a21bc8c0657f5c56aa8e0f5`/`2246d257` remains unchanged and distinct; `--route-binding` is a BOUND constant for this route; legacy `deposit(uint256,string,bytes32)` is prohibited on this route.

## 2026-08-06 FIRE-16 two-phase relay addendum

- **Nazgûl ruling.** The dead local `nav-roundtrip-live-demo --deposit-relay-only` path is prohibited: it requires validator-local topology, NodeStore, and validator key access and cannot execute through forwarded RPC. Control host builds and signs the propose then finalize/claim batches with issuer/holder operation keys using `--batch-only`; only signed public `mempool-batch.json` artifacts transfer to validator-0. Validator-0 runs `transport-certified-batch-round` sequentially with its own validator key directory. Before batch-only, validator-0 height must remain below 1776 and clone issuer/holder account sequences/state roots must equal validator-0 and validator-1; only one re-clone/re-check is allowed before any submission.
- **Placeholder-key tripwire.** Control-host batch-only commands bind `/nonexistent-placeholder-validator-key-must-never-be-dereferenced.json` as syntactically required `--key-file`. Any dereference failure is STOP-no-retry. Issuer and holder keys never leave the control host; validator keys never leave validator-0.

## 2026-08-06 FIRE-17 live-deposit reprove addendum

- **Stale-descriptor root cause and quarantine.** FIRE-16's prover leaf never generated `deployment.json`; its capture therefore used an h390 remnant. That proof set is quarantined at `/tmp/krimp-exec-fire20260806/leg-1/stale-fire16/` and cannot be relayed.
- **Hardened reprove.** U69, U72, and U73 hardened the leaf in commits `f77392d`, `1257ed2`, and `01d105d`. The live-deposit reprove uses the descriptor, proof, and public values under `/tmp/krimp-exec-fire20260806/leg-1-reprove/`; the prover leaf is commit `01d105d`.
- **Descriptor-before-capture gate.** Capture is permitted only after the deployment descriptor SHA-256 has been generated and matches the bound descriptor hash. Missing, stale, or mismatched descriptor material is STOP-no-retry.

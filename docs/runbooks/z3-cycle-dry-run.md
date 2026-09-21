# Z3 cycle dry-run command sheet

**2026-09-17 — offline G3 tooling; Z3 remains OPEN.**

This sheet implements the [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md).
It authorizes no RPC contact, signing, fleet work, or transaction. During G3 use
only the dry-run and offline verification commands. Each future submission
requires the operator's G4 authorization and a separate named confirmation.

The Arc commands exist on the candidate lineage reviewed by
[G2](../review/z3-g2-route-compatibility-20260917.md), not on main. Run Arc steps
only with an operator-qualified checkout, node binary, prover and retained ELF.
The wrapper refuses execution on main, checks the checkout's full source commit
and the node SHA-256, and never selects a pair from a shortened address.
The protected release checkout is not a preparation workspace.

## Operator inputs

Keep four directories separate: TOOLING (this main checkout), LINEAGE (the
operator-qualified source checkout), WORK (signer-local requests and results),
and PACKET (public evidence). WORK and PACKET cannot contain one another.

The funded Arc wallet address is
**0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB**.
Its keystore path is **~/.foundry/keystores/arc-testnet-server**.
Signer arguments below are paths only. Supply the password-file path locally
through SIGNER_PASSWORD_FILE; do not display, read into a shell variable, copy,
or publish either file's contents. PFTL owner and maintenance accounts are
separate explicit public inputs. Evidence contains account addresses, never
signer paths or credential material.

Arc parameters are RPC **https://rpc.testnet.arc.network**, chain **5042002**,
and USDC **0x3600000000000000000000000000000000000000** (six decimals, also
the gas asset). Retain exact native gas charges in wei; do not round gas into
six-decimal token atoms.

The two archived pairs below are alternatives for operator review, not choices
made by this sheet:

| Pair | Anchor | Vault |
| --- | --- | --- |
| August epoch 7 | 0x661D558a818A07002C7D5da4A3179c4672FEf124 | 0xe88fb9ab4890f513261f0aca4ff13bfba3e14862 |
| September epoch 9 | 0x92390D3a2102CB74E4746c05B4d91F61093475D0 | 0x160307f3efead79b6a3629c4b8d90e8301fc250f |

The candidate's explicit egress release selector is arc-v2. Its pinned egress
key is 0x0036cbe7d36bbfe1118a3c544eeba74f3791d2a19bf5ec59b972f72d36416852;
the ingress key is
0x0050a8b0daed2fa75f44d4102b42204c34668a03d311cb727cc6ca3f8df5cf16.
An older pair's key cannot be substituted. The wrapper stops if the supplied
keys do not match this command set; matching older tooling would need separate
qualification. This does not authorize choosing the September pair.

Create a public INPUTS JSON with these four top-level objects. Every value must
come from the operator's envelope or retained readbacks; there are no
qualification identity defaults.

| Object | Required contents |
| --- | --- |
| manifest | cycle_number, utc_start, utc_end (null in the skeleton), release_id, full 40-character source_commit, binary_sha256, amount_atoms; identities, policy_hashes, proof_keys, accounts below |
| reserve_identities | The complete postfiat.reserve_demo_identities.v1 object documented in the reserve driver's docstring; includes primary route, native asset, settlement family, exact source series/bucket/profile, export chain and trust classes, NAV profile/source/policy/vkey/schema/unit |
| paths | node_bin, prover_bin, egress_elf, data_dir, work_dir, packet_dir, remote_runner, proposer_hosts_file, remote_binary, remote_topology, opening_nav_manifest, fresh_packet_operation |
| parameters | cap_atoms, mint_amount_atoms, window_start, window_end, reservation_ttl_blocks, latency_bound_seconds, full route_binding, full deposit_nonce, full reservation_recipient, egress_release |

The manifest identities object contains pftl_chain_id, source_chain_id,
source_vault_address, source_anchor_address, source_token_address, source_route_id,
source_route_epoch, source_profile_hash, source_bucket_id, route_id,
native_nav_asset_id, settlement_asset_id, settlement_source_asset_id,
protocol_version, vault_code_hash and anchor_code_hash. Preserve the primary
route's export identity separately from its Arc funding source.

The policy_hashes object contains primary_route, source_profile, verifier_policy
and nav_valuation. The proof_keys object contains ingress, egress and nav.
The accounts object contains arc_wallet, owner, proposer, finalizer,
route_operator, reserve_operator and bridge_settler. The A666 NAV builder's
existing issuer and reserve authority are checked explicitly. The owner signs
subscription and redemption; maintenance authorities do not replace that owner.

The requested amount is USDC atoms, bounded by cap_atoms. The native mint amount
is a separate input. Compute its base value and issue spread from current NAV;
do not assume one USDC produces one A666. The reserve driver bounds redemption
by same-cycle reserve, selected-source principal, policy capacity and wallet
balance. Retained native units and reserve are reported, not hidden.

## Generate the offline sheet

Set the following shell variables to operator inputs. This block contains no
submission. INPUTS, WORK, PACKET, LINEAGE, and signer-path variables must be
explicitly set before use.

~~~bash
set -u
umask 077
export PYTHONPATH="$TOOLING/python"
KEYSTORE="$HOME/.foundry/keystores/arc-testnet-server"
COMMON=(
  --inputs "$INPUTS" --checkout "$LINEAGE"
  --keystore "$KEYSTORE" --password-file "$SIGNER_PASSWORD_FILE"
  --holder "$PFTL_HOLDER_SIGNER_PATH"
  --proposer "$PFTL_PROPOSER_SIGNER_PATH"
  --finalizer "$PFTL_FINALIZER_SIGNER_PATH"
  --issuer "$PFTL_ISSUER_SIGNER_PATH"
  --reserve "$PFTL_RESERVE_SIGNER_PATH"
  --settler "$PFTL_SETTLER_SIGNER_PATH"
)
python3 "$TOOLING/scripts/z3-cycle-dry-run.py" run "${COMMON[@]}" --dry-run
~~~

This prints all 39 commands in order and creates only
PACKET/cycle.skeleton.json. It does not run a command, inspect a signer, contact
a node, or generate a proof. A second attempt to write that skeleton fails.
The skeleton has null artifact hashes and cannot receive a PASS verdict.
Cycle `0` is allowed only with `--dry-run`; the skeleton explicitly records
`dry_run: true` and `counts_as_cycle: false`. Live cycles still begin at `1`.
Unknown live accounts, anchor/primary-policy/NAV bindings, quote/nonce/recipient/TTL,
and runner/host/NAV-input paths may be explicit `null` values for this diagnostic.
They remain null in the skeleton, print as named `@FIELD@` placeholders in commands,
and appear in the final `unresolved_inputs` list. Known values are still validated;
these omissions block every live confirmation. A printed command resolves an
interface, not its missing live inputs or predecessor artifacts.

Future command values are explicit entries in a public VALUES JSON:
DEPOSIT_TX, INGRESS_EXPIRES_HEIGHT, ISSUE_HEIGHT,
ROUTE_HEIGHT, REDEEM_HEIGHT, REDEEM_OUTPUT_ATOMS, WITHDRAWAL_ID,
PRIOR_CHECKPOINT, EGRESS_PUBLIC_VALUES_HEX, EGRESS_PROOF_HEX and
SETTLEMENT_OPERATION. Fill each only from that cycle's verified predecessor.
The two egress hex values are the exact public proof files, not signer data.
INPUTS and CHECKPOINT are filled by the CLI. Unresolved @NAME@ placeholders
block execution. Keep INPUTS, CHECKPOINT and signer pathnames stable across
the cycle so completed command hashes remain reproducible.

## Readbacks and the stop before every step

After G4 authorization, obtain fresh readbacks before each step. Freeze the
originals under unique packet-relative names. The checkpoint used by the
wrapper is a public projection of those originals. Its current pathname may
stay fixed; retain every prior version separately. A checkpoint expires within
five minutes and must bind the SHA-256 of the immutable INPUTS.

For each of the six validator snapshots, use the qualified node's existing
read commands on the operator-provided data source. The same finalized height,
block ID and state root must bind all associated route, NAV and asset views.
These are future readback commands, not actions performed during G3:

~~~bash
"$NODE" status --data-dir "$VALIDATOR_DATA_DIR"
"$NODE" navcoin-bridge-supply-status --data-dir "$VALIDATOR_DATA_DIR" --route-id "$ROUTE"
"$NODE" asset-info --data-dir "$VALIDATOR_DATA_DIR" --asset-id "$NATIVE_ASSET"
"$NODE" asset-info --data-dir "$VALIDATOR_DATA_DIR" --asset-id "$SOURCE_SERIES"
"$NODE" account-assets --data-dir "$VALIDATOR_DATA_DIR" --account "$PFTL_OWNER"
"$NODE" vault-bridge-status --data-dir "$VALIDATOR_DATA_DIR" --asset-id "$ASSET_FAMILY"
cast chain-id --rpc-url "$ARC_RPC"
cast code "$VAULT" --rpc-url "$ARC_RPC"
cast code "$ANCHOR" --rpc-url "$ARC_RPC"
cast call "$USDC" 'balanceOf(address)(uint256)' "$ARC_WALLET" --rpc-url "$ARC_RPC"
cast call "$USDC" 'balanceOf(address)(uint256)' "$VAULT" --rpc-url "$ARC_RPC"
cast call "$USDC" 'allowance(address,address)(uint256)' "$ARC_WALLET" "$VAULT" --rpc-url "$ARC_RPC"
cast balance "$ARC_WALLET" --rpc-url "$ARC_RPC"
~~~

Use the same pinned Arc block for the contract/balance readbacks by adding
--block "$ARC_BLOCK" to the applicable cast commands. Retain contract code
hashes and deployed proof-key/route-binding readbacks using the selected
contract ABI. An unexpected ABI or key is a stop, not permission to substitute
a getter, contract, or proof release.

An existing allowance must cover the deposit before the cycle starts. If it
does not, stop. Any separately authorized pre-cycle approval is another
submission with its own hard stop:

~~~bash
cast send "$USDC" 'approve(address,uint256)' "$VAULT" "$CAP_ATOMS" \
  --rpc-url "$ARC_RPC" --chain 5042002 --from "$ARC_WALLET" \
  --keystore "$KEYSTORE" --password-file "$SIGNER_PASSWORD_FILE" --json
~~~

Require its status-1 receipt, read back allowance, and take a new preflight
baseline including its gas cost. The cycle wrapper never automatically approves.

A checkpoint contains ready_for; captured_utc/expires_utc; inputs_sha256;
identities/accounts; finalized (height, block_id, state_root); and six distinct
validators with those same finalized fields, route_state_hash, nav_state_hash,
asset_state_hash and queues. It also contains proof_valid_from/proof_valid_until;
contract_keys_match, source_enabled, nonce_unused, balances_confirmed,
signing_recoverable, quote_current and allowance_sufficient; entitlement_atoms;
required_capacity_atoms/available_capacity_atoms; and the ordered completed list.
For deposit also retain arc_usdc_balance_atoms, allowance_atoms,
arc_required_gas_wei and arc_wallet_wei. Before a PFTL submission,
next_operation_sha256 is SHA-256 of the public operation object encoded as
sorted-key, compact JSON. It excludes the signer-local request envelope.

Each completed entry names step, command_sha256, and nonempty artifacts
(path and sha256). Submitted steps additionally require an accepted, finalized
receipt and its unique tx_id. Keep the source receipt and finality certificate
among those artifacts. A process exit code never marks a step complete.
The full cycle verifier separately checks receipt/certificate block and
transaction membership. A replay entry must record the actual expected replay
rejection and unchanged state; a timeout or connection failure is not replay
evidence. Checkpoint assertions must come from verified readbacks, never from
manually declaring a failed check true.

After the operator reviews one exact next command, use the following functions
for **one row at a time**. Do not put the rows in a loop or script. Every
confirmation returns STOP; another fresh checkpoint and confirmation are needed.

~~~bash
next_step() {
  python3 "$TOOLING/scripts/z3-cycle-dry-run.py" run "${COMMON[@]}" \
    --checkpoint "$CHECKPOINT" --values "$VALUES"
}
confirm_step() {
  python3 "$TOOLING/scripts/z3-cycle-dry-run.py" run "${COMMON[@]}" \
    --checkpoint "$CHECKPOINT" --values "$VALUES" --confirm-step "$1"
}
next_step
~~~

The table lists every command for one cycle. **SUBMIT** means stop before any
value or governance submission, review its exact argv from next_step, then
individually authorize only that row. Preparation also uses a separate
confirmation. No confirmation can execute a second row.

| Order / command | Existing command composed | Readback required before proceeding |
| --- | --- | --- |
| 1. confirm_step preflight | check-preflight | Six validators agree; empty queues, entitlement and pending withdrawal; full identities, cap/window, balances and recoverable signer |
| 2. confirm_step arc-deposit **SUBMIT** | cast send depositV2(uint256,string,bytes32,bytes32) | Exact pair/binding, owner recipient, unused nonce, allowance, amount plus gas |
| 3. confirm_step ingress-capture | pfusdc-tier4-prover arc-ingress-capture --rpc --deposit-tx --route-id --vault --token --output | Status-1 deposit; block/global log position and deposit ID; exact wallet/vault deltas; Arc finality |
| 4. confirm_step ingress-proof | pfusdc-tier4-prover arc-ingress --witness --output-dir --prove | Exact authenticated registry witness and ingress key; no stale proof |
| 5. confirm_step ingress-bundle | postfiat-node vault-bridge-deposit-relay-bundle | Same deposit receipt, family/profile/epoch, proof-calldata.bin and public-values.bin |
| 6. confirm_step prepare-propose | wrap-operation | Public propose.operation.json; explicit proposer and signer-local path |
| 7. confirm_step ingress-propose **SUBMIT** | a666-ce22-remote-finality-op.py | One reviewed propose operation; current proof and expiry |
| 8. confirm_step prepare-finalize | wrap-operation | Accepted propose receipt and matching finality |
| 9. confirm_step ingress-finalize **SUBMIT** | a666-ce22-remote-finality-op.py | Same deposit; governed finalization eligibility |
| 10. confirm_step prepare-claim | wrap-operation | Accepted finalize receipt and matching finality |
| 11. confirm_step ingress-claim **SUBMIT** | a666-ce22-remote-finality-op.py | Ordinary owner's claim; same source series; no duplicate claim |
| 12. confirm_step prepare-subscription | a666-pfusdc-reserve-demo.py build-issue | Exact minted source balance; enabled custody row, current NAV, quote, capacity and owner |
| 13. confirm_step reserve **SUBMIT** | a666-ce22-remote-finality-op.py | One reservation, bounded amount, source series, route/policy, nonce and expiry |
| 14. confirm_step subscribe **SUBMIT** | a666-ce22-remote-finality-op.py | Accepted reservation; exactly one expected reservation; family and quoted spend agree |
| 15. confirm_step entitlement-release **SUBMIT** | a666-ce22-remote-finality-op.py | Accepted subscription; exact new supply, source debit, reserve/spread; expected entitlement amount |
| 16. confirm_step verify-subscription | a666-pfusdc-reserve-demo.py verify-issue | Zero entitlement; release changed no supply, balance or reserve |
| 17. confirm_step prepare-nav | a666-build-live-nav-mark-ops.py --packet-operation | Fresh packet for post-subscription supply, explicit active proof profile, reserve counted once |
| 18. confirm_step nav-submit **SUBMIT** | a666-ce22-remote-finality-op.py | Exactly one nav_reserve_submit; fresh proof and composite overlay |
| 19. confirm_step nav-finalize **SUBMIT** | a666-ce22-remote-finality-op.py | Accepted reserve-submit; finalized fresh packet/epoch eligible |
| 20. confirm_step prepare-route-pause | route-switch --paused true | Empty reservation/entitlement state |
| 21. confirm_step route-pause **SUBMIT** | a666-ce22-remote-finality-op.py | Current route is unpaused; explicit route authority |
| 22. confirm_step prepare-route-epoch | a666-build-route-epoch-advance.py --identities --operator | Paused route, finalized fresh NAV, exact selected profile; record before/after policy hashes |
| 23. confirm_step route-advance **SUBMIT** | a666-ce22-remote-finality-op.py | One epoch increment; unchanged source enablement; bounded remaining policy capacities |
| 24. confirm_step prepare-route-resume | route-switch --paused false | Accepted route advance and matching finality |
| 25. confirm_step route-resume **SUBMIT** | a666-ce22-remote-finality-op.py | Same advanced route/NAV; live-value policy permits resume |
| 26. confirm_step prepare-redemption | a666-pfusdc-reserve-demo.py build-redeem | Unpaused fresh route; same-cycle reserve and source principal; wallet/capacity; no entitlement |
| 27. confirm_step redeem **SUBMIT** | a666-ce22-remote-finality-op.py | Owner-signed operation; conservative output and exact source identity |
| 28. confirm_step verify-redemption | a666-pfusdc-reserve-demo.py verify-redeem | Accepted receipt; exact native retirement/source credit; reserve decrease = output + spread |
| 29. confirm_step burn-bundle | postfiat-node vault-bridge-burn-to-redeem-bundle | REDEEM_OUTPUT_ATOMS equals the retained redeem manifest; exact bucket and Arc recipient |
| 30. confirm_step prepare-burn | wrap-operation | Reviewed burn operation and owner's signer-local path |
| 31. confirm_step burn **SUBMIT** | a666-ce22-remote-finality-op.py | Exactly the redeemed pfUSDC amount; no duplicate burn |
| 32. confirm_step egress-witness | postfiat-node pfusdc-egress-witness --withdrawal-id --prior-checkpoint | Accepted burn and certificate; withdrawal ID, bucket, recipient and expected pending egress |
| 33. confirm_step egress-proof | pfusdc-tier4-prover egress --egress-release arc-v2 --elf --witness --output-dir --prove | Exact retained ELF/vkey; finalized ancestry and withdrawal packet/nullifier |
| 34. confirm_step arc-release **SUBMIT** | cast send withdrawWithProof(bytes,bytes) | Proof/public values equal retained files; pair key, recipient, output and gas agree |
| 35. confirm_step release-replay-check | cast call withdrawWithProof(bytes,bytes) | Status-1 release, exact wallet/vault deltas; same nullifier must reject without a transaction |
| 36. confirm_step prepare-egress-settlement | wrap-operation | SETTLEMENT_OPERATION is the existing vault_bridge_redeem_settle operation with qualified withdrawal observations and release receipt hash |
| 37. confirm_step egress-settle **SUBMIT** | a666-ce22-remote-finality-op.py | Existing issuer/redemption authority; exact settled amount; no replacement proof or authority |
| 38. confirm_step seal-packet | python -m postfiat_rpc.z3_cycle build --layout --output | Complete public layout, final UTC end, every original artifact present |
| 39. confirm_step final-convergence | python -m postfiat_rpc.z3_cycle verify | Final six-validator snapshot; all accepted receipts; zero pending state; exact full-cycle conservation |

The wrapper records WORK/attempts/STEP.json exclusively before executing.
Failure or interruption leaves the marker and stops the campaign. Do not
delete it to retry. Record the failed attempt, reconcile its exact request
identity, and follow the plan's recovery/authorization boundary. Markers now
also retain step kind and the SHA-256 of an unsigned PFTL request envelope
(`null` for Arc commands, whose exact arguments are command-hashed).
A repeated confirmation explicitly refuses replay; retained receipts and
readbacks establish the original terminal result, not the marker alone.
The reserve driver's issue/redeem builders refuse existing output before
generating another nonce.

For an incomplete attempt, the offline Python API
`z3_cycle.build_attempt_manifest(meta, root, output, attempt, evidence)`
seals a separate `postfiat.z3.attempt.v1` packet. Supply the public cycle
metadata with end time; step, reason, submission_started and prior_consecutive;
and packet-relative observed-failure/checkpoint artifacts, plus retained marker,
terminal response and state readbacks where available. The wrapper does not
automatically assemble this projection. Never copy signer-local request
envelopes into public evidence; retain their digests and public operations.

`z3_cycle verify` audits the artifact hashes and outcome consistency. An
unclean attempt reports FAIL, pause/reset required, and zero consecutive count
after correction. An environmental interruption before any submission reports
NOT_A_CYCLE and preserves the prior count. Both exit nonzero because neither
is a clean-cycle PASS. Contradictory predecessor or submission-marker evidence
blocks the environmental exception. Record uncertain publication as started;
never remove a marker to claim otherwise.

The [G5 rehearsal record](../review/z3-g5-failure-rehearsal-20260917.md) contains
the failure/recovery tables, invariant scope and proposed stage thresholds.
Those aggregate thresholds need operator confirmation and monitoring; the
wrapper's single timeout setting still applies per command.

## Packet and verifier contract

The ten named sections are manifest, preflight, deposit, ingress, subscription,
entitlement_release, nav_route_epoch, redemption, egress and final_convergence.
The manifest carries the public metadata above. In layout.json every other
section maps named artifact roles to packet-relative filenames; the builder
replaces these paths with path/SHA-256 references. It refuses overwrite,
directory escape, symlinks and signer/secret fields.

Each section has a record JSON: explicit identities, accounts and policy_hashes;
a finalized height/block/state root; and a state of integer accounting values.
Receipts, quotes and proof reports are referenced from their retained originals
with JSON pointers, for example:

~~~json
{
  "receipts": {
    "claim_receipt": {
      "receipt": {"$ref": "claim_receipt", "pointer": ""},
      "finality": {"$ref": "finality", "pointer": "/claim_receipt"}
    }
  }
}
~~~

This is an excerpt, not a complete record. The exhaustive role inventory and
executable synthetic packet example are in
[z3_cycle.py](https://github.com/postfiatorg/postfiatl1v2/blob/f23f77b9/python/postfiat_rpc/z3_cycle.py)
and
[test_z3_cycle.py](https://github.com/postfiatorg/postfiatl1v2/blob/f23f77b9/python/tests/test_z3_cycle.py).
Public projections must retain their original source artifacts and the
qualified verifier's reports. The normalized contract is not permission to
invent missing fields or treat a synthetic packet as chain evidence.

The state fields are native_supply, native_wallet, external_native_supply,
family_supply, series_supply, pfusdc_wallet, settlement_reserve, source_principal,
source_spread, source_escrow, non_nav_spread, entitlement_atoms,
entitlement_count, reservations, pending_orders, pending_egress, source_vault,
arc_wallet_wei, issued_total, counted_total, redeemed_total,
uncredited_deposits and released_unsettled. Source counters refer to the exact
selected source; family_supply retains the family aggregate. issued_total and
counted_total are cumulative minted/credited source totals, redeemed_total is
cumulative source burn, and series_supply = counted_total - redeemed_total.
Keep these distinct from the node's outstanding receipt balances.

The verifier recomputes every artifact SHA-256; requires all named accepted
PFTL receipts and their verified finality reports; matches certificate
block/height and transaction membership; rejects duplicate transaction IDs;
requires Arc status-1 receipts and finality; and checks proof/public-values
hashes, program keys and freshness. It verifies six distinct converged
validators at preflight and finish.

Its integer checks cover each stage's exact supply, wallet, reserve,
principal/spread/escrow changes; ceiling/floor quote arithmetic; unchanged
economic state during entitlement release and NAV/route maintenance; zero
remaining entitlement; fresh NAV/route epochs; and the composite NAV overlay
counting the source reserve once. Egress includes separate after_burn and
after_release states, then settlement. It checks:

~~~text
source vault = source supply + uncredited deposits
               + burned unsettled - released unsettled
issue spend = issue base reserve + issue spread
redemption base reserve = pfUSDC output + redemption spread
net native supply = minted native - retired native
net reserve = issue base reserve - redemption base reserve
net source supply = Arc deposit - Arc release
net Arc wallet wei = (release atoms - deposit atoms) * 10^12 - gas fees
~~~

Final independent conservation readbacks must agree, including retained native
units and reserve. Replay rejection is mandatory. Missing data or any mismatch
prints a JSON FAIL verdict and exits nonzero. Verification is an offline audit
of retained evidence, not a fresh cryptographic verifier, live-state query,
operator authorization, or qualification result.

Offline sealing and independent verification can also be run directly:

~~~bash
PYTHONPATH="$TOOLING/python" python3 -m postfiat_rpc.z3_cycle build \
  --layout "$PACKET/layout.json" --output "$PACKET/cycle.json"
PYTHONPATH="$TOOLING/python" python3 -m postfiat_rpc.z3_cycle verify "$PACKET/cycle.json"
~~~

## Stop conditions and G4 handoff

Stop before submission if any validator identity, height, state root, route
epoch, NAV packet, proof key, contract code hash, source-domain identity,
balance, capacity, nonce, or queue state is stale, missing, or inconsistent.
Stop if the exact amount and conservative output have not been calculated from
current governed state. Stop if the wallet lacks confirmed test USDC, gas,
PFTL authority, or a recoverable signing path.

Stop if any step needs a consensus upgrade, emergency deployment, manual ledger
edit, hidden transfer, new facility, new bridge, or NRRS activation. Stop after
a submitted step lacking both finality and an accepted receipt. An RPC response,
proof generation, block inclusion or source-chain receipt alone is insufficient.

A rejection, timeout beyond the declared bound, proof or reconciliation
mismatch, validator disagreement, manual intervention after first submission,
or missing artifact makes the attempt unclean. Retain it, pause, and reset the
consecutive count after recording root cause and corrective work. An
environmental interruption before the first submission is recorded but does
not count or reset the count. Retry only with the exact published request
identity and its original terminal result or an explicit replay rejection.

G0's baseline freeze/redaction tasks, G1's operator-selected lineage/pair,
source configuration, wallet/signing controls, cap/window and bounded testnet
authorization, and G2's selected-state readbacks remain open. Before G4 the
operator must review these tooling results and explicitly authorize one cycle.
Fresh proofs, verified settlement observations, actual signer use and a complete
integrated packet have not been produced here. One future clean G4 cycle does
not establish the ten-cycle/seven-day G6 window or close Z3.

## Offline validation

Tooling commits: f680d78a (manifest/verifier), f23f77b9 (composition and focused
tests). On 2026-09-17:

| Command | Result |
| --- | --- |
| PYTHONPATH=python python3 -m pytest python/tests -q | 567 passed, 3 skipped, 103 subtests passed |
| PYTHONPATH=python python3 -m pytest python/tests/test_z3_cycle.py python/tests/test_z3_composition.py -q | 32 passed after final tooling edits |
| PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-a666-pfusdc-reserve-demo.py -v | 25 passed |
| .venv-docs/bin/mkdocs build --strict | Passed before each tooling commit |
| PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links | Passed before each tooling commit |
| scripts/public-secret-scan | Passed on the staged/tracked tree before each tooling commit |

Focused tests include complete success, missing/hash-mismatched artifacts,
rejected receipts, conservation failure, overwrite, stale proof, wrong
route/source asset, duplicate/replay, active entitlement, insufficient capacity,
partial evidence, absent Arc commands, and single-confirmation/failed-attempt
stops. No Rust code changed; the prior affected Rust results are in G2.
No workspace or Orchard/Halo2 run was needed.

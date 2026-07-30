# A666/pfUSDC Monday demo command sheet

**Demo date:** 2026-08-03

**Scope:** ordinary user, canonical Ethereum-mainnet USDC -> pfUSDC -> newly
issued A666 -> fresh reserve-aware NAV/route -> partial A666 redemption ->
pfUSDC.

**Do not add on Monday:** private execution, A666 bridge-out, a Uniswap trade,
Uniswap liquidity changes, generic NRRS code, another settlement asset, or a
validator upgrade.

The qualifying live rehearsal and exact transaction record are in
[`../evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/README.md`](../evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/README.md).

## Presentation plan

Run the reserve witness/proof and pfUSDC funding before the audience-facing
segment. The controlled rehearsal took:

- about 20 minutes from Ethereum deposit start to the pfUSDC claim artifact;
- additional time to build the fresh six-leg proof; and
- 8 minutes 24 seconds from the pre-issue route being ready through final fleet
  reconciliation.

The stage segment begins only after the pre-issue NAV/route gate passes. Show
the already-finalized Ethereum deposit and pfUSDC claim as the funding leg,
then execute issue, NAV refresh, route advance, and a partial redemption live.

## Fixed production anchors

```bash
REPO=/home/postfiat/repos/a666-eth-fast-lane-combined-20260724
HOSTS=docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json
NODE=target/release/postfiat-node
REMOTE_RUNNER=scripts/a666-remote-sync-round.py
REMOTE_NODE=/opt/postfiat/releases/resident-local-commit-777faa0/postfiat-node
REMOTE_TOPOLOGY=/etc/postfiat/releases/resident-local-commit-777faa0/topology.json
HOLDER_KEY=/home/postfiat/tmp/pfusdc-closed-roundtrip-20260720/keys/holder.json
LANE_MANIFEST=docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/pfusdc-egress/recovery-epoch5/deploy/manifest.postdeploy-enriched.json
LANE_SHA=b69417647e6a4bed5a3e7fa5069a0844b80a63f78020ba34f4796e373e92e904
```

Create a new fail-on-overwrite run directory and a unique workflow ID:

```bash
cd "$REPO"
RUN=docs/evidence/a666-pfusdc-reserve-demo-20260803/live-run-01
WORKFLOW=a666-pfusdc-demo-20260803-run01
test ! -e "$RUN"
install -d -m 700 "$RUN/deposit"
```

Never reuse the 2026-07-30 operation packets, nonces, route captures, NAV
packets, or output directories.

## Gate 0 — fleet, funding, and stale-state stop

Before any value moves:

1. Verify all six validators run revision `777faa0e`, agree on height, state
   root, block tip, route state, and have empty mempools.
2. Verify the route is live, unpaused, invariant-valid, and has no active
   reservation or export entitlement attributable to this user/order.
3. Verify the user has sufficient confirmed USDC, ETH gas, and PFTL authority.
4. Verify the exact governed SP1 vkey is
   `0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7`.
5. Stop if any proof, NAV, route, capacity, balance, hash, or fleet value is
   stale or disagrees.

There is no “fix it live” path. A validator upgrade, state edit, manual ledger
mutation, or changed program vkey means cancel the value-moving demo.

## Gate 1 — Ethereum USDC deposit

Pick `DEPOSIT_ATOMS` only after calculating the fresh issue amount. Approve in
one StakeHub session and deposit in a separate session:

```bash
python3 scripts/a666-mainnet-pfusdc-deposit.py \
  --approval-only \
  --amount-atoms "$DEPOSIT_ATOMS" \
  --output "$RUN/deposit/preapproval.json" \
  --deployment-manifest "$LANE_MANIFEST" \
  --expected-manifest-sha256 "$LANE_SHA"

python3 scripts/a666-wait-ethereum-deposit-window.py \
  --output "$RUN/deposit-window.json"

python3 scripts/a666-mainnet-pfusdc-deposit.py \
  --require-preapproved \
  --amount-atoms "$DEPOSIT_ATOMS" \
  --output "$RUN/deposit/deposit-result.json" \
  --deployment-manifest "$LANE_MANIFEST" \
  --expected-manifest-sha256 "$LANE_SHA"
```

Stop unless the deposit result is `PASS`, the emitted amount/recipient/route
binding are exact, and Ethereum finality evidence is present.

## Gate 2 — pfUSDC proof and PFTL claim

Capture/prove the exact deposit and finalize its PFTL claim:

```bash
bash scripts/a666-mainnet-pfusdc-claim-after-deposit.sh \
  --run-dir "$RUN" \
  --workflow-id "$WORKFLOW" \
  --expected-pftl-height "$PFTL_HEIGHT_BEFORE_CLAIM" \
  --expected-holder-atoms "$EXPECTED_PFUSDC_AFTER"
```

Stop unless `"$RUN/pfusdc-claim-summary.json"` is `PASS`, the exact deposit ID
and amount match the Ethereum event, and all six validators converge.

## Gate 3 — fresh governed reserve proof and pre-issue NAV

Generate the six-leg StakeHub witness and Groth16 proof before the
audience-facing segment. Copy it to a new `"$RUN/por-preissue"` and validate:

- exact governed ELF SHA-256 and vkey;
- execute, preview, and proved public values are byte-identical;
- all six current reserve legs reconcile;
- the PFTL reserve overlay is freshly captured;
- no Uniswap price is used; and
- the NAV manifest binds current supply, reserve, proof, policy, and packet.

Build the NAV operations:

```bash
python3 scripts/a666-build-live-nav-mark-ops.py \
  --proof-dir "$RUN/por-preissue" \
  --pftl-status "$RUN/por-preissue/pftl-status.json" \
  --route-status "$RUN/por-preissue/route-status.json" \
  --vault-status "$RUN/por-preissue/vault-status.json" \
  --profile-manifest "$RUN/por-preissue/profile-manifest.json" \
  --output-dir "$RUN/por-preissue/nav"
```

Submit `nav_reserve_submit`, wait for finality, then submit
`nav_epoch_finalize`. Capture all six validators after each round. Build and
finalize a route epoch advance that pins this exact NAV packet:

```bash
python3 scripts/a666-build-route-epoch-advance.py \
  --route-status "$RUN/preissue-route-before.json" \
  --nav-manifest "$RUN/por-preissue/nav/live-nav-mark-manifest.json" \
  --valid-from-height "$NEXT_HEIGHT" \
  --output-dir "$RUN/preissue-route-advance"
```

Stop unless NAV and route finalization are unanimous and current.

## Gate 4 — audience-facing issue

Capture fresh before-state files. Build new issue packets:

```bash
python3 scripts/a666-pfusdc-reserve-demo.py build-issue \
  --route-status "$RUN/issue/route-before.json" \
  --nav-manifest "$RUN/por-preissue/nav/live-nav-mark-manifest.json" \
  --holder-key-file "$HOLDER_KEY" \
  --output-dir "$RUN/issue/ops" \
  --mint-amount-atoms "$MINT_ATOMS" \
  --current-height "$CURRENT_HEIGHT"
```

Read the manifest aloud: A666 amount, base reserve, total pfUSDC, spread,
route epoch, NAV epoch, policy hash, and reservation expiry. Then submit each
packet as a separate finalized round:

1. `01-reserve.ops.json`
2. `02-subscribe.ops.json`
3. `03-release-entitlement.ops.json`

Use `scripts/a666-ce22-remote-finality-op.py` with the fixed node, runner,
hosts, remote binary, and topology anchors above. Stop after any round that
does not finalize cleanly across all six validators.

Run `verify-issue`. It must prove exact supply, balance, base-reserve, spread,
and entitlement-release deltas and `creates_ethereum_export=false`.

## Gate 5 — post-issue NAV and route

Capture the new PFTL route/vault/supply state. If no StakeHub portfolio
transaction occurred since Gate 3 and the proof remains fresh, reuse the exact
governed proof public values but build a new NAV manifest with the new
settlement-reserve overlay and circulating supply. Otherwise recapture and
reprove all six legs.

Finalize the new NAV reserve packet and NAV epoch, then finalize a new route
epoch pinning that exact packet. Stop unless:

```text
verified_assets_after - verified_assets_before
  == base_pfUSDC_added × pfUSDC_price

supply_after - supply_before
  == newly_issued_A666
```

The NAV should remain stable apart from deterministic rounding when reserve
and supply increase proportionally. Any unexplained movement is a stop.

## Gate 6 — partial redemption

Use a visibly partial amount so most same-run reserve remains in the facility.
Build the operation only from fresh post-issue route and NAV files:

```bash
python3 scripts/a666-pfusdc-reserve-demo.py build-redeem \
  --route-status "$RUN/redeem/route-before.json" \
  --nav-manifest "$RUN/postissue-nav/live-nav-mark-manifest.json" \
  --issue-manifest "$RUN/issue/ops/issue-manifest.json" \
  --holder-key-file "$HOLDER_KEY" \
  --output-dir "$RUN/redeem/ops" \
  --current-height "$CURRENT_HEIGHT" \
  --nav-amount-atoms "$REDEEM_ATOMS"
```

Read and confirm the maximum same-run redeem amount, output, spread, retained
A666, and retained same-run reserve. Finalize the operation with
`scripts/a666-ce22-remote-finality-op.py`, then run `verify-redeem`.

## Gate 7 — final evidence and verdict

Capture all six status and route responses. PASS only if:

- all validators agree on revision, height, state root, block tip, NAV, and
  route;
- mempools are empty;
- route states are byte-identical;
- active reservation count and export entitlement count are zero;
- supply and reserve deltas are exact;
- route invariant is true; and
- the machine summary contains every Ethereum and PFTL transaction ID.

Copy the 2026-07-30 `summary.json` schema, update it from authoritative new
artifacts, and independently recompute every delta. Do not copy prior values.

## What to say in the demo

“This is primary issuance, not an AMM purchase. The buyer's pfUSDC increased
verified reserve and the protocol created new A666 at governed NAV. The
operator did not sell existing A666. We then refreshed NAV, redeemed a partial
amount against the posted pfUSDC reserve, and left most of the buyer-funded
reserve backing the remaining A666.”

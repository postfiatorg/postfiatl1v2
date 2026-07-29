# A666 Variable-Size NAV Round-Trip — Mainnet Result

**Execution date:** 2026-07-28

**Terminal PFTL height:** 440

**Business-flow result:** PASS

**Release result:** FAIL

**Unresolved recovery:** no

This campaign completed the requested live-value sequence:

1. transparent issuance of `1.000000 A666`;
2. private-middle issuance of `100.000000 A666`;
3. a new proof-backed NAV mark from the real StakeHub six-leg source path;
4. transparent redemption of `1.000000 A666`; and
5. private-middle redemption of `100.000000 A666`.

Both acquisitions created new A666 supply and delivered standard ERC-20 wA666
on Ethereum without trading against the Uniswap pool. Both redemptions retired
that supply and released proof-gated Ethereum USDC. The private claim is
limited to the PFTL middle: Ethereum deposits, wA666 burns, and final USDC
withdrawals remain public.

## Exact economics

The new StakeHub mark finalized A666 NAV epoch 2 at:

```text
NAV = 90,103,113 USD_E8 = $0.90103113 per A666
```

The proof-backed net assets were `$28,259.75143580`; the counted primary
reserve overlay was `$204.00000000`; total value used for the NAV numerator
was `$28,463.75143580`. Uniswap price and issue spread were excluded.

| Leg | A666 | Base NAV value | User Ethereum USDC |
|---|---:|---:|---:|
| Transparent redemption | `1.000000` | `0.901032` | `0.900581` |
| Private-middle redemption | `100.000000` | `90.103113` | `90.058061` |

The transparent quote builder originally rounded the base value down and
requested `0.900580`. Consensus correctly credited `0.900581`, leaving one
pfUSDC atom on PFTL. Reconciliation detected it; the remaining `0.000001`
USDC was released through a separate proof-gated withdrawal. The builder now
uses the protocol's required ceiling rule.

## Terminal reconciliation

All calculated terminal values match live readbacks:

| Invariant | Baseline | Expected final | Observed final |
|---|---:|---:|---:|
| Valid A666 supply | `31,489.197455` | `31,489.197455` | `31,489.197455` |
| wA666 total supply / PFTL claims | `31,489.197455` | `31,489.197455` | `31,489.197455` |
| Joe wA666 | `103.000000` | `103.000000` | `103.000000` |
| Primary reserve | `103.000000` | `112.995855` | `112.995855` |
| Non-NAV spread | `0.531500` | `1.082003` | `1.082003` |
| Joe residual pfUSDC | `0.800000` | `0.800000` | `0.800000` |
| Uniswap active liquidity | `3,000,000,000` | unchanged | `3,000,000,000` |

The Ethereum epoch-5 vault held `10.557358 USDC`, exactly equal to its
obligations. All six validators finalized height 440 with state root
`a07bb418b4031b7bde1368683104e9e6890cabb10ce63d52f54af3a15aa28d00f904245632b7477a697fc063b99b0da3`
and empty mempools. There are no active reservations, export entitlements, or
pending return imports.

## Timing and release verdict

The functional flow passes, but the release gate does not:

| Measured leg | Duration | 25-minute gate |
|---|---:|---:|
| Historical small issue | `6,660s` | not a fresh-start measurement |
| Fresh 100 A666 private issue | `3,948s` | FAIL |
| 1 A666 transparent redemption, main payout | `828s` | PASS |
| 1 A666 transparent redemption, exact completion after one-atom recovery | `3,240s` | FAIL |
| 100 A666 private redemption | `1,776s` | FAIL |

Private Orchard proof creation was CPU-bound: the private-primary redemption
proof and private-egress proof were generated serially on validator 2. The
private redemption missed the target by `276s`. The large issue also included
proof-worker crash recovery and serialized proof/finality work.

`intervention_free_after_p_large_funding` is false. The run included an
ingress-finality hardening change, A100 proof-worker recovery, and the
one-atom payout recovery described above. These facts are intentionally
reflected in the top-level `FAIL` verdict even though custody and accounting
are terminal and correct.

## Primary evidence

- `acceptance-summary.json` — machine verdict
- `final/reconciliation.json` — expected versus observed accounting
- `final/timing.json` — Ethereum block-timestamp timing
- `final/ethereum.json` — final wallet, vault, wrapper, and pool readback
- `final/fleet-status.json` — six-validator convergence
- `stakehub-nav-mark/nav-epoch-2/live-nav-mark-manifest.json` — NAV inputs,
  proof hashes, reserve overlay, and integer result
- `transparent-withdrawal-1-a666/` — transparent return and payout lineage
- `private-withdrawal-100-a666/` — private-middle return and payout lineage
- `artifact-sha256.txt` — content hashes for this evidence tree

Private note files, spending keys, note seeds, and openings remain only in the
validator's mode-0600 private workspace and are not included here.

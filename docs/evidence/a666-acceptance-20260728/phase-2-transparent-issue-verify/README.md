# Phase 2 — Clean Transparent Primary-Issue Verification

**Functional verdict:** PASS

**25-minute SLO verdict:** FAIL (`1,812 seconds`, or `30.2 minutes`)

This was the clean rerun after fixing the Phase-1 operator defects. Joe
deposited exactly `1.005000 USDC`; PFTL finalized exactly `1.005000 pfUSDC`;
the primary market spent that settlement to create exactly `1.000000 A666`;
and the finalized export minted exactly `1.000000 wA666` to Joe on Ethereum.
There were no rejected transactions, manual governance changes, validator
upgrades, reconciliation steps, or retries.

## Finalized transitions

| Transition | Result |
|---|---|
| USDC deposit | block `25,630,098`, tx `0x84d69a16dbe3ce687c45bf2e1a9658abf0094f6acf82f57ea771135ef769153f` |
| Ethereum finality witness | finalized execution block `25,630,125` |
| pfUSDC propose / claim | PFTL heights `364` / `365` |
| A666 reserve / subscribe / export | PFTL heights `366` / `367` / `368` |
| Export-proof acceptance | block `25,630,248`, tx `0x608bd168cf4a7cdd67bc4c8ba8172c1b72e5ce041c97e26787dfc948f6161500` |
| One-time wA666 mint | block `25,630,249`, tx `0x9c5810c87cc6d1426bebad61490f5eae79d9de8356ff88474574da891c99cda0` |

The PFTL export packet is
`2cddf3126cc59eb258adf9845b4ef7b2115fd112f243ffff4ed08737c2f614c2cb812f85882f9f329de2ffbd56aaa84c`.
Its Ethereum packet digest is
`0x05909622c89b3d9c245aa2233e92efa8baebc5e362d70b5eed558504c301f2d8`.

## Exact conservation readback

- Joe pfUSDC after subscription: `0.800000`, equal to his pre-run balance.
- Joe A666 after subscription: `1.000000`.
- Joe A666 after export: `0`.
- Joe wA666: `101.000000` → `102.000000`.
- Authorized / wrapped A666 supply: `31,487.197455` → `31,488.197455`.
- PFTL settlement principal reserve: `101.000000` → `102.000000 pfUSDC`.
- Accumulated non-NAV spread: `0.505000` → `0.510000 pfUSDC`.
- A651 migration reserve: unchanged at `27,306.423797 A666`.
- The mint packet is consumed exactly once.
- The PFTL supply invariant holds and the route remains unpaused.

## Fix verification

The optimized ingress prover was built with the SP1 CUDA feature and required
both `SP1_PROVER=cuda` and `--require-prover cuda`. Its proof report records:

- `prover_backend: "cuda"`
- `host_execute_skipped: true`
- `execute_ms: 0`
- `setup_and_groth16_ms: 142854`
- the exact governed vkey
  `0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e`

The Ethereum executor consumed the finalized receipt witness directly,
verified its nonzero receipt hash/root and packet consistency, never changed
the governed pause state, and produced the exact one-token balance and supply
deltas.

## SLO finding

The clean run removed the operator delays but still missed the 25-minute
target. The deposit landed just after an Ethereum finality checkpoint and
therefore waited nearly three epochs before a finalized execution block
covered it. This is a protocol timing-budget failure, not a functional or
operator failure. A hard 25-minute confirmation-to-mint guarantee cannot rely
on worst-case Ethereum finalized-head timing plus the current two proof legs
and five PFTL consensus heights without further latency reduction or a
different governed finality assumption.

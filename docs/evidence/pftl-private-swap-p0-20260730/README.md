# PFTL private-swap P0 qualification evidence

Date: 2026-07-30 UTC

Scope: optimize only the resident `pftl_swapd` finality observer, preserve
consensus and validator execution, deploy only that daemon, and qualify one
controlled private issue/redeem roundtrip before any ten-cycle campaign.

## Source and test evidence

- Starting source: `f1c11769df7b919e5b1b956b5b71ef33a2f168b3`
- Mount identity-map preservation commit:
  `42c3cf6` (`Harden resident prover mount identity mapping`)
- P0 source commit:
  `8c73735ff63a1a51d3b4d777a8fef90049a5f213`
- Rust: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`

The P0 diff is restricted to the resident service and its durable timing
journal:

- `pftl_swapd` passes the committed proposal view to
  `read_consensus_v2_qc_graph_for_view`;
- view zero therefore has no historical QC dependency;
- nonzero views retain the existing dependency loader and full cryptographic
  commit verification;
- all certificate-domain and exact archived-batch bindings remain fail-closed;
- `processed_finality_verify_ns` is durably recorded before the terminal
  `Published -> Committed` transition and is idempotent across recovery;
- no consensus rule, validator state transition, receipt, or state-root code is
  changed.

Commands run:

```text
cargo test -p postfiat-node --bin pftl_swapd processed_finality -- --nocapture
# 2 passed

cargo test -p postfiat-node --bin pftl_swapd -- --nocapture
# 10 passed

cargo test -p postfiat-node pftl_swap_service::tests -- --nocapture
# 8 passed

cargo test -p postfiat-node four_nodes_require_prepare_and_precommit_qcs_for_exact_block -- --nocapture
# 1 passed; includes valid nonzero-view recovery

cargo check -p postfiat-node --bin pftl_swapd
# passed

cargo clippy -p postfiat-node --bin pftl_swapd -- -D warnings
# passed

cargo fmt --all -- --check
# passed (stable rustfmt emitted warnings for nightly-only repository options)

git diff --check
# passed

cargo test -p postfiat-node --lib
# interrupted after approximately 31 minutes while the last unrelated
# full-circuit test remained CPU-bound; all preceding tests passed, but this
# command is deliberately not claimed as a complete pass

cargo clippy -p postfiat-node --lib --bin pftl_swapd -- -D warnings
# passed in 11.58 seconds
```

The resident binary tests use a real six-validator consensus-v2 fixture with
signed proposal, prepare QC, and precommit QC. Adversarial cases cover:

- 256 corrupt irrelevant historical QC files at view zero;
- nonzero-view dependency loading;
- certificate schema, chain, genesis, and protocol mutations;
- height, view, proposer, block hash, proposal payload, and parent mutations;
- proposal, prepare-vote, and precommit-vote signature tampering;
- insufficient quorum;
- archived payload mutation.

The targeted deterministic replay, crash-prefix state recovery,
consensus-v2 nonzero-view recovery, receipt replay, and state-root invariant
tests passed. The broader `--lib` invocation was interrupted only during its
last unrelated full-circuit test and is not represented as a completed suite.

## Predeployment live baseline

An independent read-only SSH audit of all six public validator hosts found:

- height: `512` on all six;
- tip:
  `2e501231356267cdc1be6da74fde717cf48401bcbcd506a3cc4545c9a9076fbd69f0636a6b870bf66b4206b560da1203`;
- state root:
  `74081d8b3d4fe9ae84b1db78a6a6af1a24be48d61f9d3c5baa79305d0ffeaedbe92a4471bd1033c3c98c981655c31010`;
- mempool count: `0` on all six;
- validator release revision: `777faa0e`;
- validator binary SHA-256:
  `0d47fc2ce57b8f5cdbcda2db1a406eb90d9d7b2c1bebe974a347de2d8d104291`;
- A666 supply: `31,489,197,455` atoms;
- route invariant: true;
- active reservations: `0`.

The topology-based fleet-preflight utility was also attempted and failed
because this deployment intentionally binds public-host RPC endpoints to
loopback, while that utility tried the private overlay addresses. This was a
preflight/topology mismatch, not chain divergence. The failed evidence is
preserved on validator 2 at:

```text
/var/lib/postfiat/validator-2/pftl-swapd/qualification/p0-view-aware-20260730/predeploy-fleet
```

The direct six-host audit above is the independent convergence check.

## Resident-service baseline and rollback

Before deployment:

- active since: `2026-07-30 02:24:07 UTC`;
- binary:
  `/opt/postfiat/services/releases/pftl-high-core-prover-83b515c/pftl_swapd`;
- binary SHA-256:
  `1210ce0edf8b08d02080117002346ebfce9192d88496f3dc55dac146b782e7e1`;
- unit SHA-256:
  `53b6962186fc7390087e5c6bf65abf8d4e76be5befe453446afa3cdd0ffc1ec5`;
- environment SHA-256:
  `d3c734b72ec6982d42d7d6cba7e3bf0e0ec4ef4cc8bb7ca01c84f306338ee226`;
- private-egress circuit: `asset-orchard-private-egress-v1`, `k=15`;
- params hash:
  `bcd57a07fc6729861fa7524a16825722d7c96e1703990f673d95e7c28c77db2da7844a4a9f981dd54b4499edddd3d555`;
- VK hash:
  `a8e020b877a45f9691a266990e3466b52b1518518d3a5264f9725be201b9884db80d4c9cb457c29020e65cfefa85d6be`;
- readiness: green, zero active swaps, five authenticated remote peers,
  persistent vote streams, remote proposer routing, local apply before
  certified sends, next height 513.

The current binary, unit, environment, and journal were copied with restrictive
permissions to:

```text
/var/lib/postfiat/validator-2/pftl-swapd/qualification/p0-view-finality-20260730/predeploy-backup
```

Backup hashes:

- binary:
  `1210ce0edf8b08d02080117002346ebfce9192d88496f3dc55dac146b782e7e1`;
- unit:
  `53b6962186fc7390087e5c6bf65abf8d4e76be5befe453446afa3cdd0ffc1ec5`;
- environment:
  `d3c734b72ec6982d42d7d6cba7e3bf0e0ec4ef4cc8bb7ca01c84f306338ee226`;
- journal:
  `0df0aedffc81bea1d12057fca4ec820930f6680a790b3fa06cd7bb9b5c7bb81c`.

Rollback is limited to the observer service: restore the backed-up unit, stop
the service, restore the backed-up binary to its original release path, run
`systemctl daemon-reload`, start `postfiat-pftl-swapd`, and require green
`/v1/ready`. No validator restart or rollback is involved.

## Controlled pfUSDC recovery plan

At height 512, the controlled transparent wallet held `750,430` pfUSDC atoms.
Its private note index held three spendable pfUSDC notes of `900,581` atoms
each. The one-A666 issue quote requires `905,538` pfUSDC atoms.

One indexed note will be restored through the already-proven, signed
`asset-orchard.private_egress.v2` action and an ordinary certified PFTL round.
That yields `1,651,011` transparent pfUSDC atoms before the issue. There will be
no direct ledger, consensus, journal, or wallet mutation. The private redeem
output becomes the input to the next cycle through the same on-chain egress
path. The initial working balance is enough to cover the issue/redeem spread
for ten cycles without acquiring additional USDC.

## Deployment and qualification results

### Pinned deployment

- release binary:
  `/opt/postfiat/services/releases/pftl-finality-view0-8c73735/pftl_swapd`;
- binary SHA-256:
  `fd4d59ef50be6aa1ed62b83470204fcc4cd7cf82e95f8418754ea45e3a55a31f`;
- binary size: `28,027,984` bytes;
- reviewed source:
  `8c73735ff63a1a51d3b4d777a8fef90049a5f213`;
- live unit SHA-256:
  `a6c83095157eb6c5790e4a7675612cb4aacff262ba6a2eedc9a5f23c6e5f0ba4`.

Only `pftl_swapd` was deployed. All six validator binaries remained on
revision `777faa0e`. Two controlled service restarts at height 512 returned to
green readiness before live qualification.

### Controlled private roundtrip

The input was restored using the signed private-egress path and an ordinary
certified PFTL round; no ledger, consensus, wallet, or note-index file was
manually edited.

| Height | Operation | Result |
|---:|---|---|
| 513 | private pfUSDC egress/restoration | committed and finalized |
| 514 | private pfUSDC -> A666 issue with restart after `PUBLISHED` | committed exactly once after observer recovery |
| 515 | private A666 -> pfUSDC redeem | committed; private output indexed exactly once |

The height-514 restart was intentionally injected after durable publication.
The graceful stop itself consumed approximately 65 seconds and therefore is
not a latency sample. Excluding that injected wait, the measured operational
stages were approximately 48.8 seconds.

The redeem completed in 38.429 seconds. After it:

- A666 supply returned exactly to `31,489,197,455` atoms;
- settlement reserve returned exactly to `112,995,855` atoms;
- the route invariant passed;
- active reservations and all six mempools were zero;
- the publication outbox was empty;
- all six validators agreed at height 515.

The new finality observer measured 359.707 ms for the restart-recovered issue
and 342.086 ms for the redeem. This is a reduction of approximately
2.85–3.34 seconds from the historical 3.211–3.686 second estimate. The
remaining roughly 0.3–0.4 seconds is the intended exact certificate, archive,
signature, quorum, proposal, block, and payload verification.

One setup failure is preserved rather than hidden: the first restoration
outbox file was installed as root mode `0600`, so the `postfiat` round driver
could not read it. No block was proposed. Ownership was corrected before the
successful certified round.

### Partial ten-cycle gate and NO-GO

Four complete issue/redeem cycles committed exactly once. The runner used a
third ordinary round per cycle to egress the returned private pfUSDC into the
controlled transparent balance; it did not edit the private-note index or
consensus state.

Accepted-to-commit timings from the four complete samples:

| Operation | Samples (seconds) | Four-sample p95 |
|---|---|---:|
| private issue | 46.118, 47.956, 49.508, 50.365 | 50.365 s |
| private redeem | 32.650, 37.235, 34.913, 38.862 | 38.862 s |

The private-primary proof DAG remained bounded:

- issue maximum: 12.496 seconds;
- redeem maximum: 12.526 seconds.

The finality-observer measurements ranged from 292.794 ms to 407.842 ms.

Cycle 5 stopped before publication. The durable journal state is
`FAILED_PREPUBLISH`, the generated pending output was marked `discarded`, and
the outbox remained empty. Re-running the exact prepared batch through
`shield-batch-simulate` produced:

```text
atomic_batch_aborted
stale_pftl_uniswap_pricing:
PFTL-Uniswap finalized NAV pricing is older than the consensus freshness window
```

This is a correct fail-closed consensus result. It also proves the current
campaign runner is not unattended: it must refresh the governed StakeHub NAV,
wait for six-validator convergence, and acquire a new quote before the
freshness boundary.

The explicit decision is **NO-GO for 100/100** because:

1. only four complete cycles ran;
2. the runner did not automate NAV refresh;
3. transient prover-mirror readiness races required explained retries; and
4. issue p95 was 50.365 seconds, above the 42-second gate.

### Final live audit

After stopping the campaign safely, a read-only audit on all six validators
found identical:

- height: `528`;
- tip:
  `dc4aba8b9a43e8fcae3bd92cf7b16fb622bdcdb5dd356a3b918b454c286d437c1464d8fba9e0b090cad8bb71e4456d4d`;
- state root:
  `83a836dd56e8ed359fea2ca67a26fa8ef4da7fab78e7da02e526d7416315bcb7761b5851b56b4206a527baf05bb53916`;
- A666 supply: `31,489,197,455` atoms;
- settlement reserve: `112,995,855` atoms;
- route invariant: true;
- active reservations: `0`;
- mempool pending: `0`.

The resident service remains active and ready on the reviewed P0 binary.
The private-note index contains no pending cycle-5 output; that failed output
is durably `discarded`. The service is improved but remains
limited-availability and is not qualified for 100/100.

Live private records, note material, proofs, and full journal contents remain
only in the restricted validator-2 qualification directory:

```text
/var/lib/postfiat/validator-2/pftl-swapd/qualification/p0-view-finality-20260730
```

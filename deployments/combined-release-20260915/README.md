# Combined devnet release qualification

**Decision: HOLD — integration complete; release qualification fails.**

Candidate source: `87cf9dd136e79e041b395229b9b734ac05e0d8e2` on
`release/combined-devnet-20260915`. It combines main `faff0e53`, PR 39
`f2e749a1`, and the remaining PR 37 commits through `c87e7dc6`. All seven
queued consensus fixes are ancestors: `bbb291ce`, `f9f13ead`, `e95efbdf`,
`c9a61fcd`, `6ec35092`, `0a1216c3`, and `33c8ce34`.

The candidate preserves the source-settlement, pfETH, Arc, Cobalt and RPC
work while restoring archived-proof checks lost during the merge. The
integration also fixes documentation links, pins two retained artifacts and
updates rustls from 0.23.43 to 0.23.45. No new consensus rule was introduced
by the integration commits.

## Deployment blockers

1. **FastPay historical compatibility.** State verification, block replay and
   finalized-checkpoint import reject the deployed recovery committee with
   `FastPay recovery committee is invalid`. Commit `0a1216c3` changes the
   committee-root preimage and retained-certificate commitments under existing
   v1 formats. The deployed committee uses epoch 1, admission heights 11–10000
   and the earlier root. A versioned compatibility and activation rule must
   preserve authenticated historical bytes while retaining the repaired
   admission and certificate bindings for new operations. Source:
   `crates/types/src/fastpay_recovery_types.rs`.
2. **Baseline history verification.** The exact deployed binary independently
   rejects block 1011 during full snapshot export and state verification:
   issued-asset family supply is 313700595, while the finalized NAV supply
   check expects at most 304700595. This is a replay failure in the baseline;
   it is distinct from the candidate's FastPay failure. Reconcile the
   historical source-series accounting check with the recorded transactions
   and reproduce the certified roots. Source:
   `crates/node/src/state_commitment.rs`.
3. **Egress proof identity.** CI builds the egress guest twice identically,
   but its ELF and verification-key identities differ from the pinned current-v2
   manifest. The retained ELF has a third identity. PR 39 already failed this
   gate; the combined source produces another distinct rebuilt identity.
   Preserve the historical proof identities and establish the intended source,
   artifact and activation mapping before release. The ingress guest reproduces
   its pinned identity successfully. See `proof-identity-observation.json` and
   the retained CI logs. No manifest or verifier identity was changed to turn
   this failed check into a pass.

The six copied nodes report the stored height-1020 tip and start/restart on
loopback successfully. These are service and stored-tip observations. They
are **not** successful state-root recomputation or full replay. A
finalized-checkpoint snapshot exported by the old binary also fails import
under the candidate. Neither a checkpoint shortcut nor omitted history closes
the compatibility gate.

## Evidence boundary

Fleet access consisted of authenticated reads and file downloads. All builds,
imports, path relocation, key staging and service exercises ran in private
local copies. No live deployment, restart, configuration change, signed chain
transaction or remote snapshot export was performed.

The exact rollback binary was retrieved and verified as
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`.
Restoring the original copied data and that binary recovers the recorded tip
on all six local copies. Full rollback verification remains blocked by the
baseline replay failure; new-block finality and an activation/rollback cycle
are unqualified.

Database copies retain their original local integrity keys in private storage.
The local relocation helper authenticates each original generation pointer,
changes only its machine-local path, and reauthenticates it. Logical state is
unchanged. Live database byte hashes changed during acquisition; no atomic
physical-copy claim is made. The finalized-checkpoint export validates the
captured certified tip, and all six live status reads independently agree.

## Qualification results

| Gate | Result | Evidence |
| --- | --- | --- |
| Combined source lineage | PASS | Both PR tips and all seven fix commits are ancestors |
| Two independent clean release builds | PASS | Identical SHA-256 `203995122290895a023dde4bf1cfb8cc4f05b22628822d7a32829ffd185fb314`; [provenance](build-provenance.md) |
| Full Rust workspace suite | PASS | 1,429 passed, 39 ignored, zero failures; default profile, two test threads; later rustls delta covered by four Arc tests |
| Separately required warm latency test | PASS | Explicitly run after the full suite and redundant compiler stopped |
| Focused integration, Python, wallet and offline contracts | PASS | 2 pfETH, 1 source-settlement, 34 RPC, 4 updated Arc; 533 Python passed / 3 skipped; 260 wallet, 36 proxy, 178 contracts |
| Formatting, check, Clippy, docs, current-tree hygiene and supply chain | PASS | Raw output under `logs/`; rustls fixed at 0.23.45 |
| Guest proof identity reproduction | FAIL | Ingress matches; egress rebuild differs from both its pinned manifest and retained ELF |
| Ethereum mainnet contract forks | PASS | Two configured local fork tests pass |
| Pinned testnet contract forks | BLOCKED | Both configured providers lack the required historical state |
| Six-copy startup and restart | PASS, limited scope | [Service receipt](current-tip-service-receipt-final.json); stored-tip reads only |
| Candidate state verification, replay and checkpoint import | FAIL | FastPay committee commitment mismatch; [independent hash comparison](fastpay-root-comparison.json) |
| Exact rollback binary plus original copied data | PASS, limited scope | Six restored copies report the recorded tip; [observations](fleet-and-restore-observations.json) |
| Full rollback verification | FAIL | Baseline supply check fails at block 1011 |
| New-block finality and activation/rollback cycle | BLOCKED | Compatibility failures prevent qualification |
| Reachable-history secret scan | BLOCKED | Seven pre-existing findings; candidate and baseline main reports are byte-identical |

The [machine-readable assessment](qualification.json) identifies each gate's
scope and evidence. [CI observations](ci-source-observation.json) are pinned to
source revision `dd276d70`: Rust CI and the reserve-proof kit remain pending;
the egress identity job fails. The final evidence commit triggers fresh CI. A passing historical P0/P1 closure table does not override
the current replay failures. The three release blockers above require repairs
and a fresh qualification before deployment approval.

Node data, signing keys, raw RPC captures and binary executables stay outside
Git. The packet includes raw test/build output, failure stderr, public status
summaries and artifact identities.

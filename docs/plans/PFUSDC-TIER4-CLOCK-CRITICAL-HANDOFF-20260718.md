# pfUSDC Tier-4 Clock-Critical Closure Plan and Agent Handoff

**Date:** 2026-07-18
**Priority:** P0 / clock critical
**Execution mode:** Tier-4 core boundary complete at 4/4; stopped at the
founder-review boundary before controlled-testnet product hardening.
**Canonical architecture reference:** `docs/plans/PFUSDC-TIER4-IMPLEMENTATION-PLAN-20260717.md`
**Execution incident report:**
`docs/postmortems/pfusdc-tier4-clock-critical-execution-2026-07-18.md`
**Original public tier ladder:**
`https://postfiat.org/research/trustless-wrapped-stablecoins/` (source file:
`postfiatorg/postfiatorg.github.io/content/research/trustless-wrapped-stablecoins.md`).
That writeup defines Tier 1 as independently observed deposits, Tier 2 as
receipt-proven deposits against a governed finalized checkpoint, Tier 3 as
finality-verified/trustless entry, and Tier 4 as a proof-verified PFTL-finality
exit completing the trustless round trip.

### Immutable operator directive — agents may not change plan parameters

No agent is authorized to change, reinterpret, pause, narrow, expand, reorder,
or replace the parameters of this document without an explicit user instruction
that identifies the change. This includes execution status, scope, priority,
funding authorization and cap, gate definitions, proof count, validation limits,
deployment target, security invariants, and the critical-path order.

Agents must continue executing this plan end to end. They may update factual
status, hashes, transaction identifiers, evidence paths, observed blockers, and
completed checklist items as work progresses, but those factual updates must not
silently alter the plan's governing parameters. A user statement that they are
going to sleep, asking for a status update, or asking that information be
recorded is **not** authorization to pause execution or change this plan.

## 1. Mission

Finish the pfUSDC Tier-4 protocol on the existing PFTL and Arbitrum path, then
carry the frozen protocol artifacts into the existing CLI and StakeHub product
path.

Tier 4 means both bridge directions are authorized by cryptographic finality
proofs, not by an observer or withdrawal-signing committee:

1. **USDC -> pfUSDC:** PFTL verifies a proof that a canonical vault deposit
   completed on Arbitrum and is contained in an Arbitrum assertion confirmed
   under finalized Ethereum state.
2. **pfUSDC -> USDC:** the Arbitrum vault verifies a proof that an exact burn
   and withdrawal packet were accepted in a finalized PFTL consensus-v2 block.
3. Relayers transport proofs but have no authority to fabricate, redirect, or
   duplicate value.
4. The Tier-4 route cannot fall back to observer or threshold authorization.

The original public tier ladder defines Tier 4 by the evidence accepted in both
bridge directions. It does not require frontend polish, private swaps, two
fresh-wallet demonstrations, or a broad production battery. This plan therefore
separates:

- **Tier-4 protocol implementation:** Gates 1-4. Both real proof directions work
  on an activated controlled target with no observer, threshold signer, mock
  verifier, or downgrade path.
- **Controlled-testnet product launch:** Gates 5-7. CLI/StakeHub integration,
  broad recovery batteries, rolling deployment, and demonstration evidence.

No percentage completes either boundary. Report core gates passed out of four,
then launch gates passed out of three. Gates 5-7 must not delay work on Gates
1-4.

### 1.1 Plain-English status at the 2026-07-18 sleep handoff

**Authoritative terminal status — 2026-07-19 03:07 UTC: pfUSDC Tier 4 is 4/4
core gates green and is stopped at the founder-review boundary.** The terminal
acceptance record is
`docs/evidence/pfusdc-tier4-core-live-corrected-20260718/ACCEPTANCE.json`
(SHA-256
`76b9a8c943d0ab3c1fd5effbdb52eb9684726a9b3b54c8abfdfd2225d2f76a56`).
It records `result=true`, `core_gates_passed=4`, `core_gates_total=4`, and no
unresolved core findings. Controlled-testnet launch gates remain separately
0/3 and do not invalidate the completed Tier-4 protocol boundary.

The accepted Arbitrum Sepolia `withdrawWithProof` transaction is
`0x23eed687e728055c821d98ca4c45e3fe43a806c9b4c32cc6d2298f03adde6311`,
status 1 in block 288,973,669. Historical state reads at block 288,973,668 and
288,973,669 prove the atomic transition: vault balance 1,000,000 to 0,
recipient balance 0 to 1,000,000, withdrawal commitment false to true, and
proof nullifier false to true. The transaction emitted one exact 1,000,000-atom
USDC transfer from the frozen vault to the proof-bound recipient and one exact
`ProofNativeWithdrawal` event. A no-broadcast replay is rejected. Recovered
egress evidence is converged at
`docs/evidence/pfusdc-tier4-egress-live-corrected/summary.json`; the immutable
block-by-block reconstruction is recorded in the adjacent
`recovery-forensics.json`.

The submit-on-unlock runner broadcast the successful transaction but exited
before persisting its transaction hash. Its subsequent retries did not
rebroadcast because the on-chain withdrawal commitment was already consumed;
they failed closed at the recovery guard instead. The crash-looping unit was
stopped, chain logs recovered the unique transaction hash, and that hash was
written into `withdrawal-state.json`. The normal recovery path then verified
the receipt, exact deltas, both nullifiers, and replay rejection without a new
transaction. The terminal acceptance readback was also corrected to compare
the verifier's mutable latest checkpoint against the height-27 checkpoint
decoded from the accepted egress public values, while retaining exact checks
of all immutable deployment fields and the constructor checkpoint history.
After 4/4 acceptance, redundant billed Vast instances 45264613 (CPU fallback)
and 45275714 (gate RTX 5090) were destroyed. The sibling R&D instance 45269977
was not touched.

The status below is historical chronology and is superseded by the terminal
4/4 status above.

**Historical V2 founder-directive status — 2026-07-18 22:52 UTC: execution
was active and the V2 gate count was 0/4.** The controlling directive is
`/home/postfiat/repos/orc_directives/PFUSDC-TIER4-V2-DIRECTIVE-20260718.md`.
The V1 egress proof is abandoned: it must not be resumed, retried, submitted,
or counted. Its partial proof directories are archived as historical evidence.

The standalone egress guest now carries the required SP1 precompile patches,
including the exact `sha3` 0.10.8 resolution required by the SP1 patch while
`fips204` had requested 0.10.9. Bounded ancestry verification was reduced to
the valid non-nil precommit commit QC, while the terminal target still requires
proposal, prepare QC, and precommit QC. Negative target/ancestry tests and the
bounded 1/2/8/64-block synthetic segment checks pass. The optimized candidate
ELF SHA-256 is
`27e0d07d5563b982ef549a4ad793c0b49d9ef69534aff33ac2c6a224cc9f0099`
and its candidate vkey is
`0x0079327e8673d2d415897390d9ad7c050bb0f74d694e498b62436b568bc0116e`.
They are candidates, not frozen deployment artifacts, until the representative
proof succeeds.

The exact archived 26-block witness fell from 2,030,290,233 V1 cycles to
520,023,827 V2 cycles. A local representative proof nevertheless hit the
mandatory systemd `MemoryMax=60G` ceiling because Groth16 recursion/compression
has a large nonlinear memory floor. That incomplete attempt is archived at
`docs/evidence/pfusdc-tier4-v2-representative-proof-26block-local-cap-20260718T220851Z/`
and cannot count as a receipt. There will be no further proof attempt on the
122 GiB host.

The same single representative proof is now running on rented Vast instance
45264613 under its remote cgroup `memory.max` of 734,528,536,576 bytes with
swap disabled, supervised by persistent local systemd unit
`pfusdc-tier4-v2-remote-benchmark-proof-retry1-20260718.service`. The pinned
prover, witness, optimized ELF, and official SP1 v6.1.0 Groth16 circuit files
all match local SHA-256 values. Execute completed in 520,023,827 cycles; the
proof has measured 135.9 GiB peak RSS so far and has not produced a receipt yet.
The duplicate KVM instance 45265250 was destroyed and is no longer billing.

Remote prover PID 6501 completed SP1 recursion, shrink, and wrap after 2 hours
43 minutes with 135.9 GiB peak RSS, then failed at the final Groth16 conversion
at 01:14:34 UTC on 2026-07-19 because SP1 6.3.1 requires the official gnark
finalizer through a `docker` invocation and the rented container had neither a
Docker client nor daemon. The remote cgroup recorded zero memory-high,
memory-max, OOM, OOM-kill, or pressure events. No `proof.bin` was produced, so
the failed run is not a receipt and cannot count. Its output is preserved at
`/workspace/pfusdc-tier4-v2/representative-proof-26block-failed-no-docker-20260719T011434Z`.

The exact official `ghcr.io/succinctlabs/sp1-gnark:v6.1.0` image was already
available on the local Docker host. Its `/gnark-cli` executable was extracted,
copied to the rented box with SHA-256
`e9e9f32503c5cb7dc5fa87b651318fe0c53d5066a60e55db1f7563d047d5047d`,
and tested there behind a minimal compatibility runner. This changes prover
infrastructure only; it does not change the guest, ELF, vkey, witness, public
statement, proof mode, or gate definitions.

At 01:17 UTC, one corrected representative rerun was launched with the same
pinned inputs and CPU settings. It is fully detached: `/usr/bin/time` PID 12854
has PPID 1 and session ID 12854, and prover PID 12856 is its child. Output and
logs are respectively
`/workspace/pfusdc-tier4-v2/representative-proof-26block-gnark-fixed/` and
`/workspace/pfusdc-tier4-v2/representative-proof-26block-gnark-fixed.log`.
The provider cgroup's 684 GiB memory cap remains enforced. No competing prover
is running. The bounded action is to let this corrected run finish and require
its locally verified `proof.bin` receipt.

At 02:05 UTC on 2026-07-19, an explicit founder instruction added a parallel
GPU gate-proof path without stopping the CPU fallback. Vast instance 45275714
is a standard Docker RTX 5090 instance using the official SP1 6.3.1 CUDA server
and official v6.1.0 Groth16 circuit artifacts. This GPU path is deliberately
pinned to the already-deployed route-epoch-1 egress gate, not the optimized V2
candidate: egress ELF SHA-256
`8d2d5ce451bbd91c28f8fafcbd12f7bc961c6a4be59de12e246b8cb6734f81e8`,
program vkey
`0x00eaaf9372917c3edf9d6fdf70ff64ae08ba25e13cb1e2b2ab7b6e9585d50cd4`,
and height-27 witness SHA-256
`b5645fde0d8c13438be5218fc91b0cb5d275405ab1aba068374ca3e3874a7b46`.
The deployed verifier was read back on Arbitrum Sepolia and reports that exact
program vkey. CUDA prover PID 6768 is detached with PPID 1 and session ID 6768;
the CPU fallback PID 12856 remains alive and untouched. No receipt exists yet,
so this parallelization does not increment any gate. Whichever path first
produces a locally verified receipt matching the deployed vkey is the only
receipt to be submitted to `withdrawWithProof`.

The RTX 5090 path produced and locally verified that Groth16 receipt at 02:16
UTC in 603,256 ms of setup/prove time. The winning Groth16 `proof.bin` SHA-256
is `c7b41506189c76178edff641d6480d9c0f6258c0932d5ebc7ad983883969179f`;
the raw proof-calldata SHA-256 is
`018779094b97ae7474215ee22f7ed0fa973f810e14291d9d07162b0b50ca8c44`;
the public-values SHA-256 is
`dfe21e43169ba1d262abbc941dd66dab8747503414b3c79db7968c813ca8d0e8`.
The canonical evidence copy is
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof/`.

An immediate no-broadcast Arbitrum simulation then exposed a deployment
configuration error: the deployed pfUSDC finality verifier points to Succinct's
v6.1.0 **Plonk** gateway `0x3B6041173B80E77f038f3F2C0f9744f04837185e`,
while the requested proof is Groth16 with selector `0x4388a21c`. The gateway
reverted `RouteNotFound(0x4388a21c)`; the Groth16 gateway
`0x397A5f7f3dBd538f23DE225B51f532c34448dA9B` does have that route. No
transaction was broadcast, the vault still holds exactly 1,000,000 USDC atoms,
the recipient still holds zero, and neither nullifier is consumed. Because the
existing vault has no verifier setter or rescue path and its witness binds its
address, the bounded compatibility fix is a Plonk receipt for the identical
frozen ELF, vkey, witness, and public values. The deployed gateway has the live,
unfrozen v6.1.0 Plonk route selector `0x5a093a2f`.

The RTX 5090 Plonk path completed at 02:47:28 UTC and won the proof race. The
host verifier accepted the receipt against frozen program vkey
`0x00eaaf9372917c3edf9d6fdf70ff64ae08ba25e13cb1e2b2ab7b6e9585d50cd4`.
The verified Plonk `proof.bin` SHA-256 is
`d69238c9313e4fcde242fc90c83225ced1864de12b1027a9839e0af6c8a3d8e2`;
the submitted proof-calldata SHA-256 is
`73ca78617e6e74f5f28f6edb0e38dd65057b78dee6c961d077b09003462d73da`;
the public-values SHA-256 remains
`dfe21e43169ba1d262abbc941dd66dab8747503414b3c79db7968c813ca8d0e8`.
The proof-calldata begins with the deployed Plonk route selector `0x5a093a2f`.
Its measured setup/prove time was 1,215,862 ms. The incompatible but valid
Groth16 receipt is preserved separately at
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof-groth16-verified-route-mismatch-20260719T0216Z/`;
the compatible Plonk receipt is now the canonical `proof/` evidence. CPU
fallback PID 12856 remains alive and untouched as explicitly directed.

A no-broadcast `eth_call` against Arbitrum Sepolia block 288,971,596 accepted
the canonical Plonk proof. It decoded the exact 1,000,000-atom payout to
`0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`; the vault held exactly
1,000,000 atoms, the recipient held zero, and both the withdrawal commitment
and proof nullifier were false before submission. At that preflight moment no
transaction had been broadcast and no nullifier had been consumed.

Historically, the previous wallet-agent process disappeared during the earlier host process
crash and left a stale StakeHub socket. At 23:06 UTC a fresh-USDC acquisition
preflight therefore failed closed before broadcasting any transaction. The
wallet agent has been restarted as persistent user unit
`stakehub-pfusdc-tier4-wallet-agent-20260718.service`; it is currently locked.
This does not block the representative proof, but it must be unlocked before
the already-authorized fresh USDC acquisition or V2 contract deployments can
sign. No additional authorization is required. A single persistent acquisition
waiter, `pfusdc-tier4-v2-canonical-usdc-wait-20260718.service`, is active and
will continue automatically after unlock; it has not broadcast a transaction.

At 02:51 UTC, the locked StakeHub signing vault was the only blocker to the
already-simulated `withdrawWithProof` transaction. The crash-resume state is
prepared with the exact frozen withdrawal commitment and proof nullifier. The
bounded user unit
`pfusdc-tier4-egress-submit-on-unlock-20260719.service` polls only the existing
agent and will submit the verified canonical receipt immediately after unlock,
then require the exact 1.000000-USDC vault/recipient delta, both consumed
nullifiers, and replay rejection before writing converged egress evidence.

At 03:02 UTC the submit-on-unlock unit was stopped after the successful
transaction's missing local hash caused recovery-guard retries. On-chain
forensics and the normal recovery path closed that evidence gap. The bounded
Core Gates 1-4 acceptance passed at 03:07 UTC. No GitHub Actions, workspace
battery, observer/threshold fallback, gate weakening, extra proof, or extra EVM
transaction was used. There is no remaining core blocker; stop for founder
review before launch-gate work.

The historical V1 status below is retained for chronology and is superseded by
this V2 status.

**Authoritative current status — 2026-07-18 20:40 UTC: execution is active,
not paused. Core acceptance is now 2/4. The single ingress SP1 proof is complete,
verified, and was accepted by the six-validator PFTL target; it must not be
regenerated. The exact 1,000,000-atom PFUSDC credit, NAV checkpoint, burn,
height-27 exit root, egress witness, and bounded 20/20 egress mutation audit are
complete. The only unfinished proof is the single egress SP1 proof required by
the deployed Arbitrum vault before it can release the exact 1.000000 USDC.**

pfUSDC is an ordinary issued PFTL asset, not a ZK token. The Tier-4 proofs are
boundary authorization proofs: ingress proves the finalized canonical
Arbitrum deposit to PFTL, and egress proves the finalized PFTL burn/withdrawal
packet to the Arbitrum vault. No ingress proof, chain round, burn, witness, or
mutation audit is being rerun.

The completed ingress proof is preserved at
`docs/evidence/pfusdc-tier4-ingress-live-corrected/proof/`; the accepted PFTL
ingress lifecycle is recorded at
`docs/evidence/pfusdc-tier4-ingress-pftl-live-corrected/summary.json`. The
height-27 egress witness and audit are
`docs/evidence/pfusdc-tier4-egress-live-corrected/witness.json` and
`docs/evidence/pfusdc-tier4-egress-live-corrected/audit.json`.

The first persistent egress proving attempt executed the exact witness in
2,030,290,233 cycles, then was killed by the host OOM killer after 1 hour 8
minutes 55 seconds. Its service peaked at 109.2 GB RAM plus 5.1 GB swap and did
not produce `proof.bin`, `proof-calldata.bin`, or `proof-report.json`; therefore
it did not generate a usable proof and was not submitted anywhere. Its partial
artifacts are preserved at
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof-oom-20260718T2037Z/`.
An earlier process interrupted when the agent execution process crashed is
preserved separately at
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof-interrupted-20260718T1908Z/`.

At 20:40 UTC the same single egress proof was restarted as persistent user
service `pfusdc-tier4-egress-lowmem-20260718.service` with
`RAYON_NUM_THREADS=20`, `TRACE_CHUNK_SLOTS=2`, and CPU affinity `0-19`. These
settings bound prover concurrency and memory only; they do not change the
guest, ELF, vkey, witness, public statement, contracts, route, proof count, or
any other frozen plan parameter. On success the crash-resumable runner submits
the resulting proof to `withdrawWithProof`, verifies exact USDC balance deltas
and both nullifiers, performs a non-broadcast replay rejection check, runs only
the bounded core acceptance check, and updates this document. There is no
operator, funding, StakeHub, GitHub Actions, or authorization blocker.

The 18:22 UTC status below is retained as historical chronology and is
superseded by the authoritative 20:40 UTC status above.

**Authoritative current status — 2026-07-18 18:22 UTC: execution is active,
not paused. Core acceptance remains 1/4 because no SP1 proof has been generated
yet. There is no authorization, funding, StakeHub, implementation, test, or
GitHub Actions blocker. The governed bootstrap, route activation, and holder
provisioning are complete. The sole current wait is the protocol-required next
Ethereum-finalized checkpoint needed to advance the ingress witness beyond the
governed bootstrap.**

The user subsequently gave an explicit instruction to fix the frozen-guest and
StakeHub defects and to continue until completion. That instruction superseded
the historical stop recorded below while leaving the governing scope,
two-proof limit, `$500` cap, gate definitions, and security invariants
unchanged. Agents may not use this factual correction to change any other plan
parameter or pause execution.

The corrected live path is now:

- Corrected ingress ABI decoding is frozen in ELF SHA-256
  `7c581aa42a196bd5df5a1efc2c4569663744d9b597cc1cd2253e839f9ba2f921`
  with SP1 vkey
  `0x0033bd140207b97fb2442eb279cc2ce55714be6fbcd66beb325fe7c3786d4dfc`.
  The unchanged egress ELF/vkey remain
  `8d2d5ce451bbd91c28f8fafcbd12f7bc961c6a4be59de12e246b8cb6734f81e8`
  and
  `0x00eaaf9372917c3edf9d6fdf70ff64ae08ba25e13cb1e2b2ab7b6e9585d50cd4`.
  The corrected ingress guest executed the exact live witness natively in
  34,205,473 cycles. Exactly zero SP1 proofs have been generated so far.
- The corrected manifest SHA-256 is
  `5871fa73bcf5472198c6946095a388bdf7d32bd535429b53c3c45ce8ea408ad4`;
  its route-profile hash is
  `7b93053c2a1a26b918c3bd2cd4737d1e00f3f5cf0f8cb8fba9aff1a1126eac10516881b4a2bb153200f9c08fe8c1b5ef`
  and its governed route binding is
  `0cdf6748abdf669143acea7a4e657066b7e4c049594966b8f5cfde31f7c6d2c5`.
- Corrected contracts are deployed and exact readback passed: Arbitrum
  verifier `0xa17a876dEea3a711591248f726D9Fac420809cfe` in transaction
  `7fbe110610d423033a6a7887bc44accd351ef0a5f1011ababf35e3485f7a83c9`;
  Arbitrum vault `0x2983579e8C60B1e1fF06B3Bdc59805FFb0D4f915` in transaction
  `ca52439e65915ba56e02ccddcf558f3c9481870e58f53b36149b8967c6ac7b9a`;
  and Ethereum ingress anchor
  `0xabE2A1A76fB5c89f00780bb46f9870B7768F523A` in transaction
  `c32fc09dd1ea0b4e8f8b6c61591f4c77880df76278a3fbdbc6c8acfe7a58f5bb`.
- The replacement exact `1.000000` USDC deposit is complete at Arbitrum block
  `288827626`: approval
  `031f8e7e71dc0289704215edfea8754adc6c24a2b65bc301e1ca2a3f8a5f0e47`,
  deposit
  `fc1dd222a9c84f5f37cbcdf8c3572db33ea1741b05c357a14904e296054a0736`,
  and deposit ID
  `0x9d9226a435d55ef91f26c22387b5921b922cae2689704e0fed2f94569ceb7835`.
  Exact balance deltas passed and a non-broadcast replay was rejected. Evidence:
  `docs/evidence/pfusdc-tier4-ingress-live-corrected/deposit-state.json`.
- A clean six-validator target was rebuilt from the certified height-1
  snapshot. Blocks 2-4 committed the corrected profile, PFUSDC definition, and
  NAV binding; height 5 committed the missing route-authority activation
  amendment; heights 6-19 are accepted staging transfers. All six validators
  agree at height 19 on block
  `478e6ee5ff49c31f63f04ae6973a451d52c74bea4e6622e304b7e98e7f0bc2db050c1751814c2c1c39a4dae8458b33b7`
  and state root
  `27fc319096949d5fa911761847e9b245455979887b75fd91eadfbafa8fe25623466f7d850aa0f717d4da6dd34ed6d3d4`.
  Height 20 remains reserved for the exact corrected route plus live finality.
- The corrected governed finality bootstrap is complete at Ethereum finalized
  slot `10722016`, bound to confirmed Arbitrum assertion
  `0xc8fc3aec8312ee745e7bab0a546f2136833ccdceb7a65dc2752d27af560701b3`.
  Evidence is
  `docs/evidence/pfusdc-tier4-finality-live-corrected/bootstrap.json`.
- The exact route activated at height 20 on all six validators with block
  `47b0b53d84000a27a5a658e66c1e9bf490293dc071b187845811363b40cae99ff5b6917208c1cedbf719f94d7d8dd136`
  and state root
  `4d6bb09ecfb6ed10377ece5ae1751ed42af77a24f3efd3de4202b83f57956d7376ba35cb65f255fa8aee064b82b049a7`.
  Holder funding and the exact PFUSDC trustline then converged at height 22.
- On the next finalized checkpoint, continue without another prompt: capture
  and audit one corrected ingress witness; generate the single ingress proof;
  certify the ingress lifecycle and exact PFUSDC credit;
  certify the NAV checkpoint and exact burn; generate the single egress proof;
  submit it to the corrected vault; verify exact USDC deltas and both replay
  protections; then run only the bounded core acceptance check and update this
  document. Do not run GitHub Actions, a workspace-wide proof/test battery, or
  any extra SP1 proof.

**Current result: 1/4 core gates passed; 0/3 launch gates passed. The three
Tier-4 contracts are deployed and the exact PFTL route is active at height 20;
the holder/trustline is ready at height 22.**

- Gate 1 is done: the integrated code, corrected Tier-3 ingress statement, and
  corrected/frozen V3 ingress ELF/vkey exist and their targeted checks are
  green.
- The legacy PFTL snapshot has been archived without mutation. The fresh
  six-validator consensus-v2 target has now finalized through block 19 with
  identical block IDs, state roots, accepted receipts, and replicated files on
  all six nodes. Blocks 2-4 registered the exact Tier-4 NAV proof profile,
  created PFUSDC, and bound PFUSDC to that profile through three separate
  certified rounds. Blocks 5-19 each carried one accepted certified staging
  transfer and stop immediately before the immutable route activation height
  20. The
  deterministic two-chain deployment manifest, exact Tier-4 NAV/route profile,
  governance bootstrap operations, constructors, predicted addresses, and code
  hashes were first frozen at `f4a199b` and were corrected at `7c0019b` after a
  live-style lowercase-address conformance test exposed noncanonical guest
  address rendering. This does **not** yet include the live Ethereum/Arbitrum
  finality-state value, which can only be captured after deployment. No
  deployment transaction, deposit, burn, or proof has been run.
- No live funds have been spent. StakeHub is **not** a signing or authorization
  blocker: its Ethereum-mainnet ETH and USDC are unlocked, and Section 2.4
  authorizes at most $500 aggregate for the minimum required testnet funding.
- The agent is authorized to use the unlocked StakeHub mainnet funds to acquire
  Ethereum-Sepolia gas, Arbitrum-Sepolia gas, and canonical Circle test USDC
  within the aggregate $500 cap. The provider, exact contract/runtime hash,
  target orders, quote checks, and crash-resume driver are pinned at `d14d74a`.
  Its mainnet contract must still be admitted by the StakeHub transaction
  policy before spending. Do not bypass that policy or substitute a mock token.
- **Plan correction:** the existing PFTL chain cannot safely activate
  consensus-v2 in place. `consensus_v2_activation_height` is a genesis field,
  and the full genesis document is committed by the genesis hash. The current
  live genesis omits that field. Changing it necessarily creates a different
  chain identity. One controlled PFTL reset is therefore required; archive the
  old chain and batch all required genesis/profile changes into that one reset.
- **Plan correction:** Tier-4 route governance alone is insufficient. The
  active pfUSDC NAV proof profile is still the old Arbitrum-One observer
  profile. Route activation validates an exact match on source class, verifier
  kind, ingress policy, route hash, SP1 vkey/encoding/bounds, confirmations,
  timing, attestations, and bond fields. The reset/bootstrap must register the
  exact Tier-4 NAV profile and bind pfUSDC to it before route activation.
- There is no honest fixed-hours completion estimate yet. Ethereum/Arbitrum
  finality and the two required SP1 proof runs impose real elapsed time. Report
  the current bounded action and observed duration instead of multiplying an
  early gate count into a 16-hour estimate.
- The documentation-only sleep handoff is preserved at `eacd7d7`. Execution
  resumed from that exact boundary; no proof, deployment, spend, or long test was
  used to create the initial checkpoint.

**External prerequisites:** cleared. StakeHub is unlocked, the Drip.Tools vault
is allowlisted, both testnet gas balances were delivered, and canonical Circle
USDC was acquired and deposited. The current failures are internal frozen-guest
and controlled-target bootstrap defects recorded below; no passphrase, faucet,
API key, GitHub Action, broad test, or provider investigation is required.

**Bounded finality tooling is complete at `7c0019b`:** `finality-bootstrap`
host-verifies the Helios transition, finalized RollupCore storage, canonical
Nitro assertion, and anchor/vault/token code proofs before writing the governed
`EthereumArbitrumFinalityStateV2`. `ingress-capture` now requires that file,
bootstraps Helios from its exact retained root/slot, and proves the resulting
public values can advance the governed state before writing a witness. Current
source contains one Helios capture call; the earlier report of a duplicate call
was incorrect. Seven focused prover tests and seven ingress-library tests pass.
No SP1 proof or workspace battery was run for this tooling closure.

**2026-07-18 correction of prior agent error:** a prior agent incorrectly
interpreted the user's request to record the plan before sleep as an instruction
to pause execution. The user gave no such instruction. Execution remains active.
No funding transaction, deployment, deposit, burn, or SP1 proof had been started
at the correction point. The authorization remains exactly the Section 2.4
authorization: use
the unlocked StakeHub Ethereum-mainnet ETH and/or USDC, through the existing
agent signer, for at most **$500 aggregate** of the minimum required Sepolia
funding path. This authorization is active and is not authorization to guess a
provider, contract, quote, recipient, or constructor value.

**2026-07-18 active-execution update:** operator absence never paused the
sprint. After correcting the pause error, execution advanced the controlled
six-validator target through three additional certified rounds without changing
any plan parameter. The accepted transaction IDs are
`306966aa2bc6e2d696ddb489a18862d268662d006671196be56f93a79c3167664886bbe32ce8e0b4203522f2e98edceb`
(profile registration),
`6525bef33001a7e8b85cf39a995f2f6c7c51dc2ab00b6eee21df8d860043d75a74f4b828bd9da0ce5844d0910717bbe4`
(PFUSDC creation), and
`35e39131a909337080c6766c813eab33b0aade19540849b2cfbf133e057c0fe516d6ec60c82ff028dbe0f159074b869a`
(NAV asset registration). All six validators reached the bootstrap checkpoint
at height 4 with
state root
`56f232665ecdb2c32cb3931965c449da209116323ec916e80247031aeb98530ee038431015e6bcf501c988221b325dcc`.
The read-back ledger contains exactly one asset definition, one NAV proof
profile, and one NAV asset; their asset ID, profile ID, ingress vkey, policy
hash, route-profile hash, and source class exactly match the frozen manifest.

**2026-07-18 activation-slot correction:** source enforcement requires the
route governance batch to be committed at exactly the profile's
`activation_height`; submitting it earlier would produce a rejected receipt.
The controlled target therefore advanced through 15 dependency-free certified
blocks and is stopped at height 19. Every staging receipt is `code=accepted`,
and all six validators agree on block hash
`a60c5c502beb7a5e36051309e6b815406ca647ce2c4c720558d4fa53eb11a04507aa3b4f734080ffe7043befb1e7ab7f`
and state root
`66f9e6d762299ef14515fa3157275465a6b0a2d0ad68933461b1413383cf4f4a9fc6ad32fd541fe95bc0d6a856ceb120`.
Height 20 is reserved for the frozen route profile carrying the matching live
finality bootstrap; its deterministic proposer is `validator-2`. Do not commit
another staging transfer or attempt route governance before the live finality
file exists.

**2026-07-18 active-runner correction:** the overnight runner remains active
and polls the StakeHub policy state every 30 seconds. Its allowlist, gas-delivery,
canonical-USDC, and finality waits do not expire merely because the operator is
absent; factual status reporting and documentation do not stop execution. The
only current external hold is that the pinned Drip.Tools mainnet vault remains
absent from the StakeHub allowlist; the live two-order quote remains approximately `$2.12`
and no funds have been spent. Once admitted, the runner follows the existing
critical-path order without another operator prompt: acquire both Sepolia gas
assets, verify delivery, deploy and read back the three frozen contracts, wait
for canonical Circle test USDC, make the single dust deposit, capture the live
finality state and finalized ingress witness, run the bounded ingress audit
once, activate the route at exact PFTL height 20, generate the single ingress
proof, and submit it through certified six-validator consensus.

The runner uses the release prover, not the debug executable. A targeted local
release build completed after supplying the missing local `protoc` tool, and a
non-proving `program-info` readback reproduced the frozen ingress ELF/vkey
(`9e9278fc725541815fb36a5e6049301a4183e3a950778cb091be2a4bf719c373`,
`0x00cf5150195737400718baa10a8cc8bfe419857a2507d5916bb95e024fa52726`)
and egress ELF/vkey
(`8d2d5ce451bbd91c28f8fafcbd12f7bc961c6a4be59de12e246b8cb6734f81e8`,
`0x00eaaf9372917c3edf9d6fdf70ff64ae08ba25e13cb1e2b2ab7b6e9585d50cd4`).
No SP1 proof was generated by that build/readback. Both live submit paths use
the prover's raw `proof-calldata.bin`; the bincode `proof.bin` is retained only
as an archival SDK artifact and is not sent to PFTL or the EVM verifier.

Pre-spend inspection found two runner defects and corrected them before any
live transaction or proof: the deposit driver read addresses/route binding from
nonexistent manifest fields, and it selected the PFUSDC issuer as the mint
recipient even though consensus requires a non-issuer holder trustline. The
driver now reads the frozen deployment sequence and route-profile binding and
uses dedicated holder
`pfab9b9228942e5c529633a13aa271d5297bec6353`. After exact height-20 route
activation, certified blocks 21 and 22 fund that holder and establish its
PFUSDC trustline; the proof-native proposal then carries the actual Groth16
proof bytes and canonical public values at height 23, followed by
finalize/claim and exact-credit readback at height 24. This operational
provisioning does not change any frozen asset, route, proof, funding, or gate
parameter.

The fresh NAV registration also begins with no finalized reserve epoch, so a
burn at height 25 would be rejected even after the exact ingress credit. The
egress runner therefore uses the existing deterministic vault-bucket accounting
path: certify the custody-derived PFUSDC reserve packet at height 25, finalize
epoch 1 after the one-block challenge window at height 26, and certify the
single exact burn-to-redeem at height 27. It then exports a bounded finality
segment from the constructor-pinned block-1 checkpoint, runs the single egress
proof, submits `withdrawWithProof` to the frozen Arbitrum vault through the
unlocked StakeHub signer, checks the exact 1-USDC balance deltas, and verifies
proof/withdrawal replay rejection without broadcasting a second withdrawal.
No observer attestation, withdrawal signature, mock verifier, or hash-only
proof is used.

**2026-07-18 live deployment update:** the StakeHub policy blocker is cleared.
StakeHub now permits an already-unlocked daemon to persist policy changes
without a second passphrase prompt; the Drip.Tools vault is allowlisted. The
two bounded mainnet source transactions
`e870a6668265cf8758ddd6fe9f162a1ea2d35b3e0620909ca61683b131060f99`
and
`9df1cfba09a3e039add525afb6629c771b97d22c41989b02b0679c36032987a9`
delivered exactly `4.2` test ETH to the frozen wallet on each of Ethereum
Sepolia and Arbitrum Sepolia. The provider quote remained approximately
`$2.13` aggregate, far below the `$500` authorization.

All three frozen contracts are now deployed and exact readback passed:

- Arbitrum-Sepolia finality verifier `0x89eC019B4AA5423b8d96152a502a0DB52CF48164`,
  transaction `1011c98010062ce491738f3c62319900d577efd34c81bfac964c39eb66f2305b`;
- Arbitrum-Sepolia vault `0xa796dc3c9308F9C855a0659153b7AfC2006cF27B`,
  transaction `ff1f4d4e808cb5ae60dcae6d526e45b4a684ab96415401a5160be385132c9032`;
- Ethereum-Sepolia ingress anchor `0x89eC019B4AA5423b8d96152a502a0DB52CF48164`,
  transaction `3e8d77e7e403711887e774871eb34a4c4330a83bd2d764cc53f379fb839c66f3`.

The browser/API-key faucet hold is also bypassed without changing the frozen
canonical token. Transaction
`a3ead60374d2e0fecaaef5b2ec2a6c94022a81ecb63ac3002eb56bb5dbe0301c`
swapped `0.0002` Ethereum-Sepolia test ETH through the live official Uniswap
pool into `3.999644` Circle test USDC at the exact canonical Ethereum-Sepolia
token. Exactly `2.000000` USDC was approved and burned through Circle CCTP V1
in transactions
`67cd4335ca4d96ce7d60302fc5a18870c07959bf98bb1c4435323f8b290f5734`
and
`8d57d8a1762d5cd7ca599e508347f509f197b2c6e0dfdf27fcd9be620de246b3`.
Circle message
`0x5fbc06cd00db030a6ce12514a5fbe54bb5852943dd422965bceb654c34009445`
completed after 82 Ethereum-Sepolia confirmations. Transaction
`a928b82df8b8a9d2743707105f07e8f99affcbda2df80fc777ae2a23d802f8c6`
minted exactly `2.000000` USDC to the unchanged canonical Arbitrum-Sepolia
token `0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d`.

The one authorized live ingress deposit is also complete. Approval transaction
`0978009dce68cb676f202869f831491ef06c22e9cf0c6fcd3672730aa0d31eb3`
and deposit transaction
`6a492ca85953fce48135f06c24bc1cadd8f56f66f32d9b2f50f8d614e1f2e09e`
moved exactly `1.000000` canonical USDC from the wallet into the frozen vault.
The exact event fields match the frozen recipient, nonce, route binding, chain,
vault, and token; wallet and vault deltas are each exactly `1.000000`, and a
non-broadcast exact replay is rejected. Evidence is
`docs/evidence/pfusdc-tier4-ingress-live/deposit-state.json`.

The finality wait is complete. Ethereum-finalized assertion
`bf69500f7ec035e1016974b60425e2327b3cd814ac8ae2d40079c899b5f80009`
covers Arbitrum block `288800186`, past the deposit at `288793284`. The live
witness was written and natively verified at
`docs/evidence/pfusdc-tier4-ingress-live/witness.json`; its bounded audit
rejected all 21 mutations at
`docs/evidence/pfusdc-tier4-ingress-live/audit.json`. No SP1 proof has been
generated.

**2026-07-18 live execution defect record — frozen parameters unchanged:**

1. The exact signed route batch was certified at PFTL height 20. All six
   validators finalized the same block
   `64e6c2cc54fc746c27d60c48857257414e8a89132444252235392545989e30139348f648a76edbb67d7b9960a57b3e03`
   and state root
   `2dde21b1223bf4ad5ab3bb50980a146bf33af6665b06396b1fc4eae5e93ebb090512ac412ed53774a8b92a35b2707400`,
   but the route receipt was rejected with
   `vault_bridge_route_profile_rejected: vault bridge route authority is not active`.
   The controlled-target bootstrap omitted the prerequisite route-authority
   amendment. Because the profile is frozen to activation height 20, the same
   target cannot honestly retry it at height 21. Evidence:
   `docs/evidence/pfusdc-tier4-finality-live/route-activation-failure.json`.
2. Non-proving execution of the exact live witness against frozen ingress ELF
   SHA-256
   `9e9278fc725541815fb36a5e6049301a4183e3a950778cb091be2a4bf719c373`
   returned zero public-value bytes. The ELF was frozen at `7634799` from guest
   source that calls `SolValue::abi_decode` on ten top-level function arguments
   as though they were one dynamic tuple. It therefore expects an outer `0x20`
   offset that canonical Solidity `abi.encodeCall` does not emit. The live
   calldata begins correctly with the deposit ID, and changing it would break
   the Nitro item hash/sendRoot proof. Evidence:
   `docs/evidence/pfusdc-tier4-ingress-live/frozen-elf-execute-failure.json`.
3. **Historical stop, superseded by the user's later explicit instruction to
   fix the defect and continue:** the source decoder and host
   evidence-coordinate construction were corrected, the guest was rebuilt,
   and the dependent manifest, contracts, deposit, and controlled target were
   replaced as recorded in the authoritative status above. This did not alter
   the two-proof limit, funding cap, acceptance gates, or downgrade/security
   invariants.

Before that single egress proof, the release driver runs one bounded native
mutation audit against the exact exported live witness. Its 20 cases cover the
chain/genesis/checkpoint, activation height, route profile, committed exit root,
proposal-only and prepare-only artifacts, four-of-six/under-quorum finality,
duplicate/wrong committee, ML-DSA signature/context binding, rejected receipt,
Merkle path/leaf, packet, recipient, and packet hash/digest boundaries. The
audit command was smoke-verified on a valid committee-rotation fixture with
20 rejected mutations and zero accepted mutations. It generates no SP1 proof;
the live EVM proof/withdrawal replay check remains part of the one egress proof
submission.

The bounded Gate-4 contract prerequisite is also complete without a live
transaction or proof. The exact frozen Arbitrum-Sepolia and Ethereum-Sepolia
fork suite passed 2/2 at pinned blocks `288769892` and `11298882`; three Tier-4
fuzz tests passed 64 runs each (192 generated cases); the immutable-binding
invariant passed 32 runs and 1,024 calls; and the non-exact token-delta test
proved fee-on-transfer/rebasing-like ingress and egress behavior reverts
atomically. A direct pause test also proves pause blocks new work without
rewriting already-consumed deposit or withdrawal replay keys. Evidence is recorded in
`docs/evidence/pfusdc-tier4-gate4-bounded-contract-20260718/`. This closes only
the bounded fork/fuzz/invariant prerequisite. Gate 4 remains open until the
production contracts are deployed/read back, the exact route is activated,
and both live proof artifacts are verified against it.

The corrected ingress guest, deployment-manifest generator, and frozen Sepolia
input/manifest/bootstrap bundle are committed at `7c0019b`. The frozen input
SHA-256 is
`7a507e956198c3f35f4ea1e22e68629ced5118866237e51fa9fd0ca57ddd5bc9` and
the frozen manifest SHA-256 is
`efc94f6f426a89f6e8581af95e6f95e0138a312bf3b06ac7113134ffd0af3ada`.
Two targeted manifest derivation/cross-binding tests passed. This closes the
manifest prerequisite only; it is not Gate 2 evidence. The funding route was
subsequently pinned at `d14d74a`; continue from Section 9.

The shortest correct path from here is:

1. **Complete:** use the initialized controlled target and rollback archive,
   bring up all six validators, and finalize the first consensus-v2 block.
2. **Complete:** use that real finalized block, committee root, new genesis
   hash, state root, and checkpoint—together with the frozen vkeys and
   deterministic EVM deployment inputs—to freeze the pfUSDC asset, Tier-4 NAV
   profile, route, deployment manifest, and activation operations; then apply
   the profile/asset/NAV bootstrap through certified consensus. The controlled
   target is now converged at height 19, immediately before activation height
   20. Route activation remains later because it requires the post-deployment
   live finality-state capture.
3. Acquire only the required test assets under the $500 cap; deploy the
   Ethereum-Sepolia anchor and Arbitrum-Sepolia verifier/vault; verify every
   address, constructor value, storage binding, and runtime code hash.
4. Make one dust canonical test-USDC deposit, wait for real finality, capture
   its witness, and run the bounded native mutation audit once.
5. Generate exactly one ingress proof; activate the pinned PFTL profile and
   encoding; submit it; require `code=accepted`, exact credit, and replay
   rejection. That closes Gate 2.
6. Burn once under the activated encoding, export the finalized PFTL witness,
   generate exactly one egress proof, claim exact USDC through the deployed
   verifier/vault, and prove nullifier/replay rejection. That closes Gate 3.
7. Record the immutable deployment/route/no-downgrade/fork evidence. Deployment
   work begins before Gates 2-3, but Gate 4 closes only after both proof paths
   are bound to the activated route. Then report **4/4 core** before doing the
   separate CLI, StakeHub, recovery, and demonstration launch gates.

### 1.2 Original public tier ladder

The source of truth is the public write-up
`postfiatorg.github.io/content/research/trustless-wrapped-stablecoins.md` at
commit `2735ad7`. Its permitted claims are:

| Tier | Evidence change | Accurate claim |
|---|---|---|
| 1 | Exact deposit facts plus a registered-observer quorum | Independently observed deposits |
| 2 | Receipt-inclusion proof against a governed finalized-header checkpoint | Receipt-proven deposits |
| 3 | Source-chain finality proof replaces the governed checkpoint | Trustless entry |
| 4 | Succinct PFTL-finality proof replaces the threshold exit verifier | Trustless round trip |

This closure plan implements Tier 3 entry and Tier 4 exit together. Product UI,
two demonstration wallets, and Tier-5 Circle canonical alignment are not part
of the definition of the four core Tier-4 protocol gates.

## 2. Non-negotiable execution rules

### 2.1 Stay on the Tier-4 critical path

- Do not work on repository publication, FastSwap, FastPay, Cobalt, frontend
  redesign, a651 epoch work, generalized cleanup, or unrelated documentation.
- Do not start, poll, wait for, or inspect GitHub Actions on the Tier-4 critical
  path. In particular, do not run `gh run view`, create a background CI monitor,
  or treat public-repo CI as a protocol or launch gate. Local commands and
  captured local evidence are authoritative for this closure.
- Do not start an external or independent review gate. External review is not a
  prerequisite for completing and proving the devnet implementation.
- Keep architecture-plan edits limited to correcting stale protocol facts. This
  file is the live closure checklist.
- Do not report completion percentages. Report `gates passed / gates total`,
  the current failing command, and the next bounded action.

### 2.2 No random 45-minute validation loops

- Use targeted tests during implementation. A test must correspond to the
  component just changed and should normally return within five minutes on the
  existing warm build tree.
- Do not run `cargo test --workspace` after every edit or commit.
- Run the full local workspace gate exactly once for an immutable
  controlled-testnet launch candidate, after the four core gates and all
  targeted checks are green. It is launch hardening, not a reason to delay the
  Tier-4 protocol result.
- Run a second full workspace gate only if Rust source changes after that first
  release-candidate run. Documentation, evidence, manifests, or deployment
  records do not justify repeating it.
- Generate exactly one required ingress proof and one required egress proof from
  the frozen release-candidate guests. Do not generate proofs for benchmarking,
  progress theater, repeated validation, or deployment. Regenerate an ELF/proof
  only when that guest's Rust source changes, then record why the old artifact
  was invalidated.
- A long operation is allowed only when it directly produces a required
  artifact: an SP1 build/proof, the one release-candidate workspace battery, an
  Arbitrum fork battery, or a live deployment gate. Record its purpose before
  starting it.
- Do not run open-ended fuzzers or soak tests on the critical path. Property,
  fuzz, and invariant commands must have an explicit case or time bound and
  target the decoder/state transition just changed.
- If a command produces no useful progress for ten minutes, diagnose the
  process instead of blindly waiting. Do not repeatedly poll it from the
  conversational loop.

### 2.3 Safety boundaries

- Never accept block inclusion or six-node convergence as success without the
  literal accepted receipt code.
- Never replace proof verification with a trusted RPC assertion, operator
  signature, mock verifier, hash-only placeholder, or observer attestation.
- Never loosen consensus quorum, proof bounds, route bindings, finality rules,
  replay protection, or conservation checks to make a test pass.
- Never add an automatic fallback from the Tier-4 route to the observer or
  threshold route.
- The current chain's genesis hash commits the absent consensus-v2 activation
  field, so a versioned in-place transition is not available. Perform exactly
  one controlled reset before the real proof gates. Archive the old chain,
  initialize consensus-v2 and the exact Tier-4 NAV/route bootstrap together,
  and do not reset again merely to simplify testing.
- Preserve the existing uncommitted Tier-4 work. Do not reset, checkout, clean,
  or overwrite it.

### 2.4 Authorized testnet funding

The user explicitly authorized, on 2026-07-18, an aggregate maximum of **$500
USD equivalent** from the unlocked StakeHub EVM wallet's live Ethereum-mainnet
ETH and/or USDC solely to acquire the testnet assets and pay the funding-path
fees required for this controlled Tier-4 Sepolia deployment.

- Approved wallet: `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`.
- Observed before authorization: `0.303226550043211924 ETH` and
  `3,462.712789 USDC` on Ethereum mainnet, plus `0.008164051255532 ETH` on
  Arbitrum One. Re-read balances immediately before spending.
- The $500 cap includes provider charges, transfers, swaps, and mainnet gas.
  Spend only the minimum necessary; the cap is not a target.
- Use the existing unlocked StakeHub agent signer. Never extract, print, or
  copy a raw private key.
- Before each real-value transaction, pin the provider/contract, source chain,
  destination chain, recipient, asset, amount, expected delivery, and maximum
  fee. Record the quote, transaction hash, delivered testnet balance, and total
  cumulative USD-equivalent spend.
- This authorization does not permit a mainnet Tier-4 deployment, unrelated
  purchases, or weakening the Sepolia finality/proof acceptance gates.
- Free faucets may be used when immediately available, but CAPTCHA/account
  delays must not become an open-ended critical-path loop. A verified paid
  route is authorized within the cap.

## 3. Exact handoff state

### 3.1 Worktree and branch

```text
worktree: /home/postfiat/repos/postfiatl1v2-public-main-verification-20260717
branch:   pfusdc-tier4-20260717
handoff baseline:           b5b66b7 (corrected Tier-4 activation handoff)
current protocol/artifact:  7c0019b (canonical ingress + governed finality capture)
last protocol code commit:  7c0019b (canonical live-address encoding)
V3 guest source freeze:     7c0019b
base:     cc23185
remote:   https://github.com/postfiatorg/postfiatl1v2.git
public main observed 2026-07-18: 66de35034c46dabe46302e2abbeead23a438d3d0
```

The Tier-4 history preserves both the superseded and corrected V3 ELF/vkey
records. The corrected Nitro-output ingress is
committed at `ce511818eab246d59d3aed66e4628c5f9045d802`; the storage integration
fixture repair is committed at `887d98280bf9ff755966c322e156aaa1aee8794e`.
Public `main` is two CI-only commits ahead of the branch base
(`b4dab41f2de9`, `66de35034c46`). They do not change Tier-4 Rust or Solidity.
No dirty ingress correction remains. Integrate those CI-only commits only at a
normal clean-tree boundary, and do not run their GitHub workflows.

### 3.2 Implemented and committed

- `BridgeExitLeafV1`, canonical encodings, Merkle roots, and proof public values.
- Consensus-v2 binding of accepted bridge exits through `bridge_exit_root`.
- Block/replay/history/state/RPC plumbing for the exit commitment.
- Proof-native ingress operation fields and PFTL execution verification path.
- PFTL finality witness export and pure egress proof verification.
- Bounded checkpoint advancement and PFTL committee-rotation proof handling.
- Dedicated ingress and egress SP1 guest workspaces and generated ELF artifacts.
- Tier-4 proof builder/prover CLI foundations.
- `PFTLFinalityVerifierV1.sol` and `ERC20BridgeVaultV2.sol`.
- Solidity replay, wrong-binding, wrong-version, committee-progression, rejected
  receipt, atomic deposit-send, and exact-payment tests.

### 3.3 Corrected-ingress work — committed and preserved

```text
crates/execution/src/nav_vault_asset_execution.rs
crates/node/src/vault_bridge_workflows.rs
crates/types/src/pfusdc_tier4_types.rs
crates/types/src/tests.rs
programs/pfusdc-ingress/elf/pfusdc-ingress-program
programs/pfusdc-ingress/src/lib.rs
```

Commit `ce511818eab246d59d3aed66e4628c5f9045d802` replaces the initial ingress V1
receipt attachment with ingress V2:

- bind the confirmed Nitro assertion `sendRoot`;
- prove the exact output leaf and Merkle path;
- bind sender, destination, value, calldata, L2/L1 block numbers, timestamp, and
  output index;
- decode the exact canonical `recordDepositV1(...)` payload;
- bind deposit ID, depositor, recipient, amount, nonce, route, vault, and token;
- update canonical public values and the regenerated ingress ELF.

Do not revert this correction merely to recover an earlier green test. Its
Gate-1 ingress V2 ELF SHA-256 was
`04856ece0239146e6f3ce9ca191ef5ff6ce6c1fac42a0dfef719423539876fd7`.
Gate-2 code-proof closure changes the guest statement to V3, so that V2 ELF is
now explicitly invalidated and must not be proved or activated.

### 3.4 Gate-2 V3 closure — committed and frozen

Commit `0b68a5be71c80d1cdc89d12e5c7cfe77b1eb831f` supersedes the
pre-release ingress public values V2 with V3 before any Tier-4 activation:

- Nitro leaf and accumulator rules are pinned to Offchain Labs Nitro commit
  `a618155919315241665356fe60f3cd00d66d5e46`, with fixed item-hash/sendRoot
  vectors and the canonical final `index == 0` rule.
- Zero-sibling single-leaf proofs are accepted; indexes deeper than their proof
  path reject.
- The confirmed assertion block hash authenticates a canonically decoded L2
  header; its state root verifies exact vault and token account/code proofs.
- The Ethereum-finalized state root verifies both Rollup `latestConfirmed` and
  the parent-chain ingress-anchor account/code proof.
- The guest allowlists only the canonical Ethereum/Arbitrum mainnet pair or the
  canonical Sepolia/Arbitrum-Sepolia pair, including genesis root, fork
  schedule, Rollup proxy, and storage slot. Cross-network mixtures reject.
- Arbitrum Sepolia is the clock-critical controlled target. It provides real
  Ethereum/Nitro finality in minutes; a local fork cannot provide a valid
  Ethereum-finality witness, and a new Arbitrum One assertion normally cannot
  close inside this work window.
- `PfUsdcIngressPublicValuesV3` commits the Rollup code hash/slot, asserted L2
  state root, output sender, and ingress-anchor runtime code hash in addition to
  all prior deposit bindings.
- `PfUsdcIngressAnchorV1` is the production parent-chain destination and checks
  active Outbox, proof-derived L2 sender, route fields, recipient hash, and
  deposit replay if the Nitro message is executed.
- The anchor route binding is constructor-set, non-settable storage rather than
  an immutable. Making it an immutable creates a deployment hash cycle
  (`route profile -> verifier policy -> anchor runtime hash -> route binding ->
  route profile`). The bridge, vault, token, and chain ID remain bytecode-level
  immutables. The deployment manifest must record and read back the stored route
  binding; PFTL mint authorization independently verifies that same binding in
  the proof policy and public values.

Conformance record:
`docs/specs/pfusdc-nitro-sendroot-conformance.md`.
Local evidence:
`docs/evidence/pfusdc-tier4-gate2a-20260718T022015Z/`.

Host commit `69a056f5` adds two bounded commands without changing guest code:

- `ingress-capture` decodes the real deposit/ArbSys receipt, bootstraps a
  Helios light-client store from standard Beacon REST data, fetches the
  finalized Ethereum Rollup/anchor proofs, recovers the confirmed BoLD
  assertion, fetches asserted-L2 vault/token proofs, obtains the canonical
  NodeInterface outbox path, and writes a witness only after native V3
  verification succeeds.
- `ingress-audit` runs 21 named security-field mutations against that captured
  witness and requires every mutation to reject. SP1 proof-byte mutation and
  PFTL deposit replay remain execution-level cases after the one proof exists.

### 3.5 Current verified results

These targeted results were current through the corrected V3 source commit on
2026-07-18:

- Tier-4 type tests: **3 passed, 0 failed**.
- Ingress V2 public-value/finality-state type tests: **3 passed, 0 failed**.
- Pure egress proof tests: **4 passed, 0 failed**.
- Corrected ingress V3 library tests: **6 passed, 0 failed** when the library
  target is selected.
- Post-correction finality/manifest closure: **7 focused prover tests and 7
  ingress-library tests passed, 0 failed**. The changed ingress guest was built
  once to freeze its ELF/vkey; no SP1 proof was generated.
- Proof-native ingress execution test: **1 passed, 0 failed**.
- Targeted bridge-exit activation and egress-export node tests: **2 passed, 0
  failed**.
- Tier-4 Foundry tests: **11 passed, 0 failed**, including the production-anchor
  runtime-hash cycle regression test.
- Ingress capture/ABI/fork/mutation helper tests: **3 passed, 0 failed**.
- `cargo fmt --all -- --check`: **passed**.
- The earlier Gate-1 `cargo check --workspace --all-targets --locked` remains
  passed; V3 was checked only through its affected packages because the full
  workspace battery is reserved for the immutable launch candidate.
- Running the SP1 guest binary as a host unit test is invalid because
  `sp1_zkvm::io::read` only runs on the zkVM target. Use the library test target
  for unit tests and the SP1 toolchain for the guest execution/proof gate.

### 3.6 Gate-1 compile closure

The former compile failure was:

```text
crates/storage/src/lib.rs:1393
missing field `bridge_exit_root` in initializer of `BlockHeader`
```

Commit `887d98280bf9ff755966c322e156aaa1aee8794e` adds
`bridge_exit_root: None` to that legacy storage fixture. The complete workspace
all-target check now passes. There is no remaining Gate-1 compile blocker.

### 3.7 Live state

- The Tier-4 branch is not merged into current public `main`.
- The corrected Tier-4 contracts are deployed and exact readback passed on the
  Sepolia controlled-testnet path. This is not a production-main deployment.
- The corrected controlled PFTL route is staged but not active; height 20 is
  reserved for activation after the live finality bootstrap passes.
- No live Tier-4 fresh-wallet round trip has been completed.
- The controlled six-validator target is stopped and converged at height 19,
  state root
  `27fc319096949d5fa911761847e9b245455979887b75fd91eadfbafa8fe25623466f7d850aa0f717d4da6dd34ed6d3d4`.
  Its profile, asset, NAV binding, and route-authority receipts are accepted;
  all six validators have identical ledgers and governance state.
- StakeHub is unlocked and is not a blocker. The gas funding, corrected three
  contract deployments, corrected exact USDC deposit, and readbacks are
  complete. The only current wait is Ethereum/Arbitrum finality; no additional
  funding transaction is needed.
- The corrected V3 ingress build is complete. ELF SHA-256 is
  `7c581aa42a196bd5df5a1efc2c4569663744d9b597cc1cd2253e839f9ba2f921`;
  program vkey is
  `0x0033bd140207b97fb2442eb279cc2ce55714be6fbcd66beb325fe7c3786d4dfc`.
  The copied ELF is byte-identical to Cargo's final RISC-V release artifact and
  contains the corrected canonical ABI decoder. Native execution against the
  live witness succeeded; no SP1 proof has been generated.

The immediately preceding `f61cb50d...` ELF / `0x007a73f6...` vkey is also
invalidated. It used Alloy's checksummed `Display` representation for addresses
while every governed PFTL route/evidence/public-value address is required to be
canonical lowercase. The actual alphabetic Sepolia addresses therefore could
not pass the frozen route. Commit `7c0019b` uses one canonical lowercase
encoding in the guest and capture host, rebuilds the ELF once, regenerates the
NAV/route/bootstrap bundle, and remeasures the constructor-specific finality
verifier runtime hash on an Arbitrum-Sepolia fork.

The prior `03e6b9...` ELF / `0x007b629d...` vkey is explicitly invalidated.
Manifest/bootstrap work proved that `ingress_policy_hash_v2` returned a 48-byte
SHA3-384 string while the SP1 route and NAV profile contract requires an exact
32-byte policy hash. That made the old ELF impossible to register in a valid
Tier-4 NAV profile. Commit `5cafb31` corrects the policy commitment to a
domain-separated 32-byte Keccak hash, adds a fixed conformance vector, and
extends `vault-bridge-bootstrap-bundle` to emit the exact SP1 vkey, encoding,
proof bounds, ingress policy, and route-policy hash. The affected ingress
library tests and both legacy/Tier-4 bootstrap tests pass.

### 3.8 PFTL activation and route-profile correction

The existing local live snapshot is the legacy chain
`postfiat-wan-devnet-2` at height 598. Its genesis has no
`consensus_v2_activation_height`; its blocks have no `consensus_v2_commit` or
`bridge_exit_root`. Only validator-0's local data snapshot is present, while
the recorded topology points at the existing remote validators. Preserve this
snapshot as rollback evidence; do not mutate it to impersonate the new chain.

Two source-level checks change the execution order in the earlier plan:

1. `Genesis::genesis_hash()` commits the complete genesis document, including
   `consensus_v2_activation_height`. Adding that field changes the genesis hash
   and therefore the chain identity. Excluding it from the hash would make the
   consensus transition unauthenticated and is prohibited.
2. Tier-4 route activation checks that the active pfUSDC `NavProofProfile`
   exactly matches the route. The current profile is the legacy
   `vault_bridge` / `multi-fetch-quorum` Arbitrum-One observer profile, so it
   will reject the Tier-4 route even if route governance itself is valid.

The controlled target must therefore be initialized and activated once with:

- a fresh archived-and-recorded chain identity and six-validator registry;
- consensus-v2 active from height 1;
- post-genesis operations registering the exact Tier-4 ingress verifier/source
  class, route-policy hash, SP1 vkey, encoding, proof/public-value bounds,
  confirmations, timing, attestation, and bond fields in a registered NAV proof
  profile;
- pfUSDC registered or rebound to that exact profile; and
- the Tier-4 route epoch and governance state.

Finalize the first consensus-v2 block before freezing the final EVM
constructor manifest. The resulting real genesis hash, asset ID, committee
root, finalized height, checkpoint/state root, route hash, and route binding
are constructor commitments for the Arbitrum verifier/vault and Ethereum
anchor. Do not guess them and do not deploy against the legacy chain values.

### 3.9 Controlled-reset progress at the sleep handoff

The destructive part of the reset has **not** been performed against the legacy
snapshot. It remains untouched. A deterministic compressed rollback archive was
created at:

```text
/home/postfiat/archives/pfusdc-tier4-20260718/legacy-postfiat-wan-devnet-2-validator-0.tar.zst
SHA-256: ca2de5d16ad4123f6b99ddd128a7aea84ed055a5827798c36715e33a32bcab0a
legacy content-tree SHA-256: 44ce5643c52354315cf694a10176a80b61caad060270e77f236f32dbe7ef4d95
```

The replacement target is initialized at:

```text
/home/postfiat/tmp/pfusdc-tier4-target-20260718
chain ID: postfiat-wan-devnet-2
validators: 6
height: 0
consensus-v2 activation height: 1
genesis hash: ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9
state root: 4e50b6dd3a054cf72e89d472763e3475dd5ed24434385bcb5b5beaaab367066bf5c3d831982c3e3cbf9de96684e81ebf
genesis document SHA-256: d6ae81ee0732756ea8e67c2e6456e859ab345173d4ba2d4d055b554180fcb55c
validator registry SHA-256: d05436b6bbfc68954fa4731b2144c823f08069e9a4cd945250315fd2aad3bc30
```

Those values are the height-0 initialization baseline. The bounded checkpoint
round is now complete:

```text
height: 1
view: 0
proposer: validator-1
block ID: b9c3e38c523cc258dfbe106b45e000155dd8f0c193770d4d905f8b0777f91612519fc964ac890483b844c2ef7b6fdce8
state root: 77a53da28e603fe409698d5ccbd8356b7cb036e0ab47ae47aa2d254f59222371ff82a30392f1003513cf568a13ca6049
committee epoch: 1
committee root: a84d4b4cadc9c068d5c668e040efe9ba303c59560bfb4c315c5b23aa235b8a6a279f3886d1352810e0b83822a90fc5d0
prepare QC: 5 votes / quorum 5
precommit QC: 6 votes / quorum 5
receipt: 59cc3d56f63d2626e194755adb7287e6375c3133e0de0c0e72141f81f2e834905bbb7df68420f24092a7199fc7d80327, code=accepted
```

All six nodes have the identical tip, state root, accepted receipt, ledger,
governance state, bridge state, shielded state, block log, receipt log, ordered
batch log, and batch archive. `verify-state` and `verify-blocks` passed on each
node. The services were stopped after verification. Sanitized evidence is at
`docs/evidence/pfusdc-tier4-checkpoint-20260718T034713Z/`.

The first two former resume steps are complete and corrected at `7c0019b`: the
checkpoint-bound asset, NAV profile, route, activation operations, predicted
EVM addresses, constructors, runtime-code commitments, and bootstrap bundle
were frozen and cross-checked. The approved funding route was subsequently
pinned at `d14d74a`; no funds have been sent. Implement the bounded
finality-bootstrap capture path, acquire only the minimum testnet
assets under Section 2.4, and deploy/read back the pinned contracts. Execute the
separate live finality-state capture after deployment and before route
activation, then bind the first ingress witness to that exact checkpoint.

This remains the only execution thread. Do not run GitHub Actions, a workspace
battery, or an SP1 proof before the corresponding one-time acceptance gate.

## 4. Mandatory architecture closure before declaring ingress safe

The corrected `sendRoot` direction is promising but must close every item below.
Mock-only evidence is insufficient.

- [x] Prove against canonical Arbitrum Nitro encoding or official vectors that
      the output leaf hash, tree path, and `sendRoot` calculation are byte exact.
- [x] Prove that the output sender is populated by ArbOS from the executing L2
      contract and cannot be selected arbitrarily by calldata.
- [x] Bind the asserted L2 block hash to the confirmed rollup assertion under an
      Ethereum-finalized Rollup contract state proof.
- [ ] Bind the deployed Tier-4 vault and token runtime code at the asserted L2
      state through the canonical L2 header and account proofs. Bind the
      production ingress-anchor runtime code through an account proof at the
      Ethereum-finalized parent-chain state; it is not an L2 account. A route
      field that merely states an expected code hash is not a proof.
- [ ] Pin the production ingress-anchor address and its runtime code hash in the
      route profile. A test-only mock anchor is not sufficient.
- [x] Prove that `depositV2` measures the exact USDC balance delta, records the
      deposit, and emits/sends the canonical output atomically. A reverted token
      transfer or failed ArbSys send must create neither a valid output nor a
      mintable deposit.
- [x] Bind Arbitrum chain ID, Rollup address/code hash, confirmation slot,
      assertion hash, route epoch, vault, token, and all relevant runtime code
      hashes into the proof policy and public-values commitment.
- [x] Reject malformed and oversized output paths/calldata before expensive
      proof work or state mutation.
- [ ] Generate one real SP1 proof from a captured finalized Arbitrum witness and
      verify the exact proof in PFTL execution. Native/mock guest execution alone
      does not satisfy this item.

If any one of these bindings cannot be proven, ingress remains NO-GO and Tier 4
is not complete.

## 5. Binary acceptance gates

There are four Tier-4 protocol gates and three controlled-testnet launch gates.

- **Tier-4 protocol implemented:** Gates 1-4 are green (**4/4 core**).
- **Controlled-testnet product launch complete:** Gates 5-7 are also green
  (**3/3 launch**).

Do not describe 0/3 launch gates as blocking the Tier-4 protocol implementation.
Do not describe Tier 4 as live while its route is not activated.

### Core Gate 1 — integrated release candidate compiles

- [x] Preserve and finish the ingress V2 patch.
- [x] All direct initializers, codecs, RPCs, snapshots, replay, and history code
      understand the versioned `bridge_exit_root` and ingress V2 public values.
- [x] Formatting passes.
- [x] Targeted type, execution, node, proof, guest-library, and contract tests
      pass.
- [x] The six-file ingress correction is committed as one reviewable commit.
- [x] Confirm after the worktree is clean whether public `main` contains any
      Rust/Solidity changes that require integration. The two currently known
      CI-only commits require no code integration and do not justify running
      GitHub workflows.

**Gate evidence:** commit SHA, changed-file list, targeted command log, and zero
compile errors.

### Core Gate 2 — proof-verified ingress, no observer

- [ ] A real dust test-USDC deposit enters `ERC20BridgeVaultV2` on Arbitrum
      Sepolia and creates the canonical Nitro output. A fork-only receipt does
      not satisfy this gate because its modified state is not Ethereum-finalized.
- [ ] Ethereum finality, confirmed Rollup assertion, exact output path, exact
      vault code, exact token, exact route, exact recipient, amount, nonce, and
      deposit ID are verified by the ingress SP1 guest.
- [ ] PFTL execution verifies the real proof bytes with the route-pinned program
      vkey and exact public values.
- [ ] The deposit finalizes and credits exact pfUSDC with `code=accepted`.
- [ ] No observer attestation or observer signature is supplied or consulted.
- [ ] Wrong chain, checkpoint, assertion, output path, sender, destination,
      calldata, code hash, token, vault, recipient, amount, nonce, route, proof,
      and replay each reject before balance mutation.
- [ ] Restart/replay does not reopen the deposit ID.

**Gate evidence:** proof hash, public-values hash, program vkey, ELF hash,
accepted receipt, before/after balances, mutation-negative matrix, and replay
result.

### Core Gate 3 — proof-verified egress, no threshold signer

- [ ] An accepted `vault_bridge_burn_to_redeem` creates exactly one
      `BridgeExitLeafV1`; a rejected transaction creates none.
- [ ] The ordered exit root is bound into proposal, prepare, and precommit bytes
      for the activated block encoding.
- [ ] The exported witness proves a valid consensus-v2 precommit commit QC,
      distinct validators, correct committee root, parent/view ancestry,
      accepted receipt code, and exact Merkle path.
- [ ] Committee rotation is accepted only through a proof-verified finalized
      governance transition.
- [ ] A real egress SP1 proof is verified by the deployed
      `PFTLFinalityVerifierV1`, using the immutable program vkey.
- [ ] `ERC20BridgeVaultV2` pays exact USDC to the proof-bound recipient and
      consumes the withdrawal ID and proof nullifier atomically.
- [ ] No ECDSA threshold withdrawal bundle is created, requested, or accepted.
- [ ] Four-of-six, duplicate validator, wrong committee, bad ML-DSA context,
      proposal-only, prepare-only, rejected receipt, wrong leaf/path, altered
      packet, stale checkpoint, and replay all fail without value movement.

**Gate evidence:** finalized PFTL block/commit artifact, exit path, proof/public
values/vkey hashes, accepted EVM transaction, exact balance delta, nullifiers,
and negative matrix.

### Core Gate 4 — immutable route and contracts

- [ ] Deploy production, not mock, verifier/vault/anchor components on the pinned
      Sepolia/Arbitrum-Sepolia controlled target.
- [ ] Read back the anchor's constructor-set `governedRouteBinding` and require
      it to equal the manifest/route-profile binding. Its runtime hash is
      deliberately independent of this stored value to avoid the deployment
      hash cycle described in Section 3.4.
- [ ] Deployment manifest records compiler, optimizer, constructor arguments,
      creation/runtime bytecode hashes, program vkeys, initial checkpoints,
      chain IDs, vault, token, Rollup, anchor, and route epoch.
- [ ] Route profile pins both verifier kinds, proof bounds, program versions,
      vkeys, runtime code hashes, checkpoints, and activation height.
- [ ] Bootstrap consensus-v2 from height 1 on the single-reset controlled
      target, register and bind the exact Tier-4 NAV proof profile, and activate
      that exact route. Verify the already-generated ingress and egress
      artifacts bind to it. Do not regenerate either proof for activation.
- [ ] New Tier-4 work cannot downgrade to observer/threshold verification.
- [ ] Pause stops new work but cannot rewrite, redirect, duplicate, or fabricate
      an existing claim.
- [ ] Actual USDC balance deltas reject fee-on-transfer/rebasing behavior.
- [ ] The pinned Arbitrum fork suite and contract fuzz/invariant suite pass.

**Gate evidence:** deterministic manifests, deployed code-hash checks, route
profile hash, fork receipts, fuzz summary, and invariant summary.

### Launch Gate 5 — canonical CLI and StakeHub integration

- [ ] CLI bridge-in builds, verifies, submits, and resumes the ingress proof
      without an observer ceremony.
- [ ] CLI bridge-out exports the finality witness, proves, submits, claims, and
      resumes without a threshold-signature bundle.
- [ ] Commands are parameterized by wallet, route, amount, recipient, and RPC;
      no test wallet or address is hardcoded.
- [ ] StakeHub shows `Tier 4 - proof verified` only after live verifier kinds,
      vkeys, route hash, and runtime code hashes match.
- [ ] StakeHub shows source-finality age, proof generation time, settlement
      time, route epoch, accepted/rejected/unknown receipt state, and exact value
      movement.
- [ ] Rejected or unknown receipts never render as success.
- [ ] Existing NAV freshness, transparent swap, private swap, and private-egress
      surfaces continue to work.
- [ ] Retrying proof transport is idempotent and never repeats a deposit, burn,
      withdrawal, or claim.

**Gate evidence:** backend/API tests, browser screenshots, receipt-code proof,
and one GUI/API-driven proof operation.

### Launch Gate 6 — adversarial, restart, and conservation battery

- [ ] Complete the ingress and egress mutation matrices from Gates 2 and 3.
- [ ] Crash/restart prover, relayer, node, and StakeHub after every durable step;
      resume without repeating a money transition.
- [ ] Restart all six validators and replay to the identical state root.
- [ ] Partition/delay validators while proving PFTL finality; accept only a valid
      precommit commit QC.
- [ ] Complete both directions from a second relayer using only public artifacts.
- [ ] Rotate the PFTL committee and prove the first withdrawal after rotation.
- [ ] Rotate the route/program vkey through a new epoch without reopening replay.
- [ ] Reconcile `V = S + D + B - R` after each accepted operation and at terminal
      state.

**Gate evidence:** machine-readable battery matrix, restart roots, receipts,
proof/nullifier sets, balance snapshots, and conservation calculation.

### Launch Gate 7 — live six-validator Tier-4 demonstration

- [ ] Stage rollback artifacts.
- [ ] Perform one rolling validator deployment; after each node require six-node
      rejoin and identical tip/root before proceeding.
- [ ] Activate the exit-root encoding and Tier-4 route at explicit future heights.
- [ ] Confirm all six nodes report identical release, protocol version, route
      profile, vkeys, and activation heights.
- [ ] Complete two fresh-wallet round trips with different amounts/recipients:
      USDC -> proof ingress -> pfUSDC -> pfUSDC/a651 swap path -> pfUSDC -> proof
      egress -> exact USDC.
- [ ] Include one transparent swap and one private/warm-prover swap path.
- [ ] Every money step has `code=accepted`; all six validators agree in the
      untimed audit; exact balance deltas and conservation hold.
- [ ] No observer attestation and no threshold withdrawal signature appear in
      either run.
- [ ] Old-route creation is rejected after activation; explicitly pinned drain
      artifacts remain readable but cannot create new old-route work.

**Gate evidence:** two `ACCEPTANCE.json` files, receipts from both chains,
six-node status/root audit, proof and deployment hashes, before/after balances,
conservation, screenshots, and terminal gate record.

## 6. Clock-critical execution order

Timeboxes are escalation points, not permission to weaken a gate.

### Block A — stabilize current tree — COMPLETE

1. Add `bridge_exit_root: None` to the failing storage fixture.
2. Continue targeted compile checks and repair every mechanical initializer or
   codec mismatch caused by the versioned field/public-values change.
3. Run the already-green targeted Tier-4 tests and ingress library tests.
4. Run the Tier-4 Foundry test file.
5. Commit the complete ingress V2 correction. Do not include unrelated files.

**Exit:** Core Gate 1 is locally green on targeted checks. Commits:
`ce511818eab246d59d3aed66e4628c5f9045d802` and
`887d98280bf9ff755966c322e156aaa1aee8794e`.

### Block B — close real ingress proof (next bounded execution phase)

1. Official Nitro/BoLD vectors and the asserted runtime-code proof statement are
   frozen at `0b68a5b`; the policy-width correction is frozen at `5cafb31`. Do
   not reopen them without a concrete failing witness.
2. Build the frozen V3 ingress guest once and derive its ELF hash/program vkey.
   This must precede deployment because the route profile commits the vkey and
   the anchor constructor commits the derived route binding. Do not prove yet.
   This step is complete; use the corrected frozen values in Section 3.7. The
   superseded `03e6b9...` ELF must not be deployed or proved.
3. The legacy archive and fresh six-validator height-0 target are complete as
   recorded in Section 3.9. Do not recreate or reset them. Stage the individual
   validator signer files without printing key material.
4. **Complete through manifest freeze:** all six validators finalized the first
   consensus-v2 block and agree on its accepted receipt, block ID, state root,
   and committee root. The deterministic Sepolia deployment manifest and route
   profile are frozen at `7c0019b` from those values plus the asset ID, route
   binding, constructor arguments, predicted addresses, and code hashes. The
   exact NAV profile, PFUSDC asset, and NAV binding subsequently finalized in
   separate certified blocks 2-4; all six validators agreed on the height-4
   bootstrap checkpoint and read-back ledger bindings, then advanced through
   accepted certified blocks to the height-19 activation staging checkpoint.
5. Under the bounded real-value authorization in Section 2.4, acquire Ethereum
   Sepolia ETH, Arbitrum Sepolia ETH, and canonical Circle test USDC; deploy the
   production parent-chain anchor and asserted-L2 verifier/vault; then submit
   one dust deposit.
6. Capture the finalized witness and run the bounded 21-case native mutation
   audit once.
7. Generate exactly one real ingress proof and verify it in PFTL execution with
   deposit replay rejection.

**Exit:** Gate 2 passes; otherwise record the exact cryptographic binding that
remains open. Do not substitute a mock or hash-only assertion.

### Block C — close real egress proof and contracts (bounded after Gate 2)

1. Generate a real PFTL finality proof from a finalized accepted burn.
2. Verify it through the production SP1 verifier contract, not the mock.
3. Prove exact payment, nullifier/withdrawal replay protection, committee
   progression, and every consensus negative case.
4. Append the actual proof, receipt, nullifier, balance, and code-hash evidence
   to the already-frozen deployment manifest and route record.

**Exit:** Core Gates 3 and 4 pass. At this point the Tier-4 protocol
implementation is 4/4 core gates green. Record that result before starting
product-launch work.

### Block D — controlled-testnet product wiring (post-core)

1. Replace old observer/signature steps in the canonical CLI for Tier-4 profiles.
2. Wire the existing StakeHub backend/API/dashboard to those CLI/backend paths.
3. Prove receipt semantics, proof-age/status display, idempotent transport, and
   no regression to NAV/swap/private surfaces.

**Exit:** Launch Gate 5 passes. This work does not block the 4/4 protocol result.

### Block E — one launch-candidate validation (post-core)

Run, once, against the immutable launch candidate. Do not invoke these commands
as debugging loops:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
/home/postfiat/.foundry/bin/forge test --root crates/ethereum-contracts -vv
```

Run SP1 guest/proof, fork, CLI, StakeHub, replay, and adversarial gates separately
with their exact evidence paths. Do not invoke GitHub Actions as a substitute.

**Exit:** immutable binary, ELFs, vkeys, contracts, route profile, and manifests.

### Block F — controlled-testnet launch demonstration (post-core)

1. Do not reset again. The required controlled reset occurred in Block B before
   the real proof gates, and Gates 2-4 bind their evidence to that chain.
2. Run Gate 6 recovery/conservation cases that require the final binary.
3. Run both Gate 7 fresh-wallet demonstrations.
4. Write the controlled-testnet launch record only after 3/3 launch gates are
   green in addition to the already-recorded 4/4 core result.

## 7. Efficient test matrix

During Blocks A-D, select only the smallest relevant target:

| Change | Immediate gate |
|---|---|
| Canonical types/encodings | Tier-4 filtered `postfiat-types` tests |
| Pure PFTL finality proof | `postfiat-pfusdc-proofs` tests |
| Ingress guest logic | ingress program **library** tests, then one SP1 execution |
| Egress guest logic | egress guest/SP1 execution for the affected witness |
| Execution verifier | filtered vault-bridge/Tier-4 execution tests |
| Block root/replay/history | filtered node/storage/consensus Tier-4 tests |
| Solidity verifier/vault | `PFUSDCTier4.t.sol`, then fork/fuzz at Gate 4 |
| CLI | affected CLI parser/workflow tests |
| StakeHub | affected backend/API/browser tests |

Do not use `cargo test --workspace` as a debugging command. It is the Block E
release-candidate gate.

## 8. Evidence contract

Write append-only evidence under:

```text
docs/evidence/pfusdc-tier4-<gate>-<UTC>/
  ACCEPTANCE.json
  commands.log
  hashes.json
  receipts/
  proofs/
  public-values/
  balances-before.json
  balances-after.json
  deployment-manifests/
  screenshots/
```

Every `ACCEPTANCE.json` must include:

- gate number and boolean result;
- exact commit SHA and dirty-worktree boolean;
- binary, contract, ELF, vkey, route-profile, and manifest hashes;
- chain IDs, genesis hash, route epoch, activation height, and contract addresses;
- proof/public-values/nullifier/withdrawal/deposit identifiers;
- accepted receipt codes, not just inclusion;
- before/after balances and conservation equation;
- exact test commands, exit codes, and durations;
- unresolved findings, with no percentage-based completion claim.

## 9. Current next actions

Core Gate 1 is complete. Execution is active. Continue without a
repository-wide review:

1. **Complete:** the legacy archive and initialized target are preserved; six
   split signer files and the topology were staged without exposing key material.
2. **Complete:** all six validators finalized consensus-v2 block 1 and match on
   height, block ID, state root, accepted receipt, and consensus-v2 commit.
3. **Complete (corrected):** freeze and verify the deterministic two-chain
   deployment manifest, asset, NAV profile, route, and activation operations
   from the real genesis hash, committee root, checkpoint/state root, corrected
   proof policy/vkey, live deployer nonces/addresses, constructors, and code
   hashes. The current manifest SHA-256 is
   `5871fa73bcf5472198c6946095a388bdf7d32bd535429b53c3c45ce8ea408ad4`.
4. **Complete (tooling):** add the bounded finality-state bootstrap capture path required by
   `VaultBridgeRouteProfileActivationV1`, and require ingress capture to advance
   from its exact retained checkpoint. Seven focused prover tests and seven
   ingress-library tests pass. Execute and validate the one live
   `EthereumArbitrumFinalityStateV2` capture after Step 7 deploys and verifies
   the contracts, but before route activation in Step 10.
5. **Complete (corrected controlled-target bootstrap):** register the exact
   corrected Tier-4 NAV profile, create PFUSDC, bind PFUSDC to that profile, and
   activate route authority through four dependency-safe certified rounds.
   All receipts are accepted and all six validators recorded the identical
   height-5 state root
   `5c41ed042011b9eb80720c901c374657a37370254f127f007dab5dbdfa5c7dc24d4903fed7d08e5d29686748414e8774`.
6. **Complete (corrected activation staging):** advance the clean
   six-validator target through accepted certified height 19 and stop before
   the frozen activation height. All validators match on block
   `478e6ee5ff49c31f63f04ae6973a451d52c74bea4e6622e304b7e98e7f0bc2db050c1751814c2c1c39a4dae8458b33b7`
   and state root
   `27fc319096949d5fa911761847e9b245455979887b75fd91eadfbafa8fe25623466f7d850aa0f717d4da6dd34ed6d3d4`.
   Height 20 proposer is `validator-2` and remains reserved for the corrected
   route governance batch carrying live finality.
7. **Complete (corrected live deployment/deposit):** the corrected verifier,
   vault, and anchor are deployed and exact readback passed. The replacement
   exact `1.000000`-USDC deposit is confirmed with exact deltas and replay
   rejection at
   `docs/evidence/pfusdc-tier4-ingress-live-corrected/deposit-state.json`.
   The approved wallet remains
   `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`. Do not repeat funding, CCTP,
   deployment, approval, or deposit transactions.
8. **Complete:** the corrected governed finality bootstrap was captured at
   Ethereum finalized slot `10722016` for confirmed assertion
   `0xc8fc3aec8312ee745e7bab0a546f2136833ccdceb7a65dc2752d27af560701b3`.
9. **Active bounded wait:** capture the exact corrected ingress witness from a
   later finalized checkpoint and run the bounded 21-case ingress audit once.
   The persistent 60-second poll is active. Do not generate a proof during this
   step and no operator action is required.
10. **Complete:** the exact route activated at height 20 and the holder funding
   and PFUSDC trustline converged across all six validators at heights 21-22.
11. **Not run:** exactly zero SP1 proofs have been generated. After Step 9,
   generate the one ingress proof, certify propose/finalize/claim through height
   24, and verify exact `1.000000` PFUSDC credit.
12. **Pending after ingress:** certify the NAV checkpoint/finalization and exact
   burn through height 27, generate the one egress proof, submit it to the
   corrected vault, verify exact USDC conservation and replay rejection, then
   run only the bounded core acceptance assembler. The persistent finish runner
   is already staged for these steps.
13. Report status only as: current core gate, core gates passed out of four,
   exact blocker, last evidence path, and next bounded action. After 4/4 core,
   report launch gates separately out of three.

## 10. Completion statements

The valid protocol completion statement is:

> pfUSDC Tier 4 protocol implementation is 4/4 core gates green. A real
> finalized Arbitrum proof credited pfUSDC without an observer, a real finalized
> PFTL proof released exact USDC without a threshold signer, the route activated
> on the controlled target had no downgrade path, and the
> proof/vkey/ELF/contract/route hashes are bound
> in the core acceptance evidence.

The stronger controlled-testnet product-launch statement is:

> pfUSDC Tier 4 is 4/4 core and 3/3 launch gates green. Two fresh-wallet live round trips used real
> proof-verified ingress and egress, every money receipt was accepted, exact
> conservation held, six validators agreed, no observer authorized ingress, no
> threshold signer authorized egress, no downgrade path was available, and the
> terminal evidence manifests are hash-bound to the deployed release.

Do not withhold the 4/4 protocol result merely because product polish or the
broader launch demonstration remains open. Do not claim a controlled-testnet
product launch until the separate 3/3 launch gates are green.

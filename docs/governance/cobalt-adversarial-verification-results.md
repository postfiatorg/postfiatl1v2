# Cobalt Adversarial Verification Results

Result: **KEEP_ACTIVE**

Completed: **2026-08-26**

Cobalt ratifies validator-registry and trust-graph changes. A separate layer
decides which validators deserve trust. Current proposals and authorizations
originate from Foundation-administered validators.

The completed campaign proves protocol capability on the controlled devnet. It
does not prove operator decentralization, authorize mainnet use, or make Cobalt
a block-finality protocol. Consensus v2 remains responsible for blocks.

## Decision

All six locked experiments passed. The final authenticated live observation
found Cobalt active for validator-trust governance after the signed rollback and
return drills, with all six validators converged at height 924. None of the
published rollback stop conditions occurred, so the final gate is
**KEEP_ACTIVE**.

| Experiment | Scope | Result | Packet root |
| --- | --- | --- | --- |
| E1 | Independent oracle and generated trust graphs | 10,240 frozen cases passed after one harness-domain fixture remediation; no oracle or production defect. | 9151c9b7f43e2c75f367416b9087e7255ca1c03ae734bfdd362fc79ff0cbbc05 |
| E2 | Byzantine validator and searched-schedule campaign | 108 validator/strategy cases and 442,368 schedules passed; zero conflicting roots, false accepts, false halts, synchrony violations, or rejected-state mutations. | 8742d9603621408339d99c3d9fcc1ba8cc43dafdc900acdfccbf86cc60d7cba3 |
| E3 | Adversarial recovery | 24 tampered-history and 18 forged catch-up cases rejected; six interrupted recoveries restored byte-identical accepted history. | 9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600 |
| E4 | Consensus v2 finality isolation | Both 500-round lanes converged; no stop or fork; attack p95 delta +0.452099%, inside the 5% budget. | 93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508 |
| E5 | Live authority transitions, negatives, and stolen-key drill | Final rollback/return committed at 922/923; all nine negative cases rejected; legitimate validator-5 rotation committed at 924; all six nodes converged. | 0695284a7b38ac0129c47e1242f4a2227ad25096147920e79569a924e5f3b3db |
| E6 | Proposal source and operator independence | The independent-operator gate was reinstated as a separate mandatory follow-on milestone; no operator decentralization claim. | fa6255b5a5f31f2d7e8b2836eb005f894b815199ec1205a30fa99a6fb22de6e2 |

## What was attacked

The frozen campaign covered:

- generated uniform and non-uniform trust graphs at every locked linkage
  boundary;
- RBC, ABBA, MVBA, and DABC equivocation, withholding, re-proposals, and
  changing trust views;
- delay, drop, duplication, reordering, and partition schedules;
- truncated, padded, reordered, and modified durable history;
- fabricated transitions, wrong-root certificates, and incomplete catch-up;
- governance storms, repeated safe halts and view changes, near-limit
  certificates and RPC frames, sidecar flooding, and a crash-looping validator;
- signed live rollback and return transitions;
- early, stale, replayed, wrong-root, cross-chain, mixed-authority,
  self-authorized, and replayed-rollback attempts; and
- one treated-as-stolen validator key attempting to authorize its own rotation.

## What held

Across the frozen evidence:

- no two accepted registry roots conflicted;
- incompatible graphs preserved the last accepted registry;
- compatible graphs progressed inside the locked synchrony bound;
- tampered recovery material did not join accepted history;
- governance stress did not stop or fork Consensus v2;
- every required live negative case left durable governance and registry state
  unchanged;
- the stolen-key attempt had one signature and a decision certificate but
  rejected because it lacked the current-registry authorization/support
  binding;
- the legitimate rotation had authorizations from validators 0–4 and excluded
  the old validator-5 key; and
- all six validators accepted one height-920-through-924 history.

## What was fixed

The campaign preserved, remediated, and reran defects rather than discarding
them:

1. E1's first comparison used an invalid harness-domain genesis fixture. The
   frozen mismatch review found no oracle or production defect; the unchanged
   10,240-case corpus then passed twice.
2. E4's transport retry window was shorter than the deliberate validator
   restart outage. The window was corrected and focused recovery passed.
3. E4's postprocessor incorrectly required exact tip/root equality across
   independently authenticated lanes. It was corrected to require convergence
   within each lane and equality of signed-message-independent workload and
   outcomes; the unchanged 500+500 corpus passed.
4. The live post-rotation DABC path now follows the prior committed ratification
   anchor across registry-root changes.
5. The live helper now resolves legacy validator identities from the active
   validator count when the explicit vector is absent.
6. Already-compact decision certificates are no longer compacted a second time.
7. The first live rollback/return pair at heights 920/921 is retained. The
   height-921 return used a trust binding that did not match the protocol-native
   post-return graph. No fork, conflicting root, or finality interruption
   occurred. The corrective signed rollback/return at 922/923 established the
   final-gate state before the height-924 rotation.

## Live state after E5

The final authenticated observation ran from **2026-08-26T06:34:55Z** through
**2026-08-26T06:35:50Z**.

| Field | Result |
| --- | --- |
| Chain | postfiat-wan-devnet-2 |
| Height | 924 |
| Validators | 6, all converged |
| Authority | Cobalt validator-trust governance |
| Block finality | Consensus v2 |
| Registry root | 08a451e07aeaf9ada41a69e7c26dfd3fd86fce11c02f5567127c598b3cf775ac054b2add85295cc8c0d429bb6d2b9b1d |
| Trust root | 89f18aef2c5726ae43043407eb4d638ee8f3b6027e58ec3553296478602232cf3c2fc5d1dfebc4058d720b16508f0307 |
| Tip | ebeb0e1ee27f30ba480255728832719d94eac1a89d762a7aa7019eae269008fac53098cf6495f477a241d63a7649fbef |
| State root | 0854bc47f78996b2dcd279206cbdcc0b4858395c5937e0e0d56b3d645ca6b6a9d9c9578f5ac77bb14bea9dd1ee6f413e |
| Validator/RPC/shadow services | All active |
| Mempools | Empty on all validators |

This is a point-in-time observation, not a real-time claim. See
[Current State](../status/chain-state-current.md) for the canonical freshness
boundary and deployed/runtime/repository separation.

## Interfaces

The CLI and browser panel consume the same authenticated consolidated packet and
fail closed if a required file, checksum, experiment pin, publication document,
live authority field, or semantic check is missing or inconsistent.

CLI:

~~~bash
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial
~~~

Read-only browser:

~~~bash
PYTHONPATH=python python3 -m postfiat_rpc.cobalt_ui \
  --adversarial-packet benchmarks/cobalt-adversarial-verification/packet
~~~

The browser exposes GET and HEAD; a POST to /api/snapshot returns 405. It
exposes no proposal, authorization, transition, rotation, or mutation route.

## What remains open

The existing Foundation-administered operator boundary remains. E6 requires a
separate implementation milestone for a non-Foundation proposal envelope,
independent operator custody, and a graph in which no single administrator can
reach quorum or block it alone.

Also outside this result:

- mainnet authorization;
- HSM or remote validator signing;
- production transactional storage;
- public peer discovery and placement diversity; and
- a long-duration public WAN service-level claim.

## Verify

From repository root:

~~~bash
python3 benchmarks/cobalt-adversarial-verification/e1/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/e2/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/e3/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/e4/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/e5/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/e6/verify_packet.py
python3 benchmarks/cobalt-adversarial-verification/packet/verify_packet.py
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial
~~~

The public narrative is
[Cobalt: Further Evaluation](https://postfiat.org/blog/cobalt-further-evaluation/).

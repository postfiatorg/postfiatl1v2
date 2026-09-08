# StakeHub deprecation: current safety gates

**Status:** **INCOMPLETE — StakeHub is not deprecated for A666 reserve publication**

**Sole authoritative Markdown:** this file records the current deprecation
status. It restores the safety record removed during documentation consolidation;
it introduces no new implementation plan, qualification, or deployment authority.

The full original technical requirements remain unchanged in the
[retained execution plan](https://github.com/postfiatorg/postfiatl1v2/blob/6edf57f84fc78bada1f108f8c12a4a59277ce34a/docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md).
The [adapter inventory](../status/A666-PUBLIC-ADAPTER-READINESS-20260802.json)
still records six required adapters, none production-qualified, and
`stakehub_deprecated=false`. Historical proof reproduction is not qualification
of today's source, binary, or fleet.

## Acceptance gates

These are the last recorded dispositions, not fresh operational observations.
No gate is upgraded by restoring this record.

| Gate | Pass condition | Current state |
| --- | --- | --- |
| `G0` Wallet/runtime boundary | Shipped wallet, proxy, node, signer, and relays require no StakeHub code, API, path, token, or agent. | PASS |
| `G1` Public proof standard | Versioned bounded ABI and immutable profiles bind the complete proof context and trust classes. | PASS |
| `G2` Generic proof framework | Clean checkout reproduces generic guest/vkey and CPU execute/prove/verify/packet flow. | PASS |
| `G3` Public A666 adapters | All six source families have public collectors, source-state/ownership/quantity/liability and valuation verifiers, bounded parsers, adversarial tests, and fuzz qualification without aggregate-attestation substitution. | **FAIL/OPEN — 0/6 production-qualified** |
| `G4` Source-equivalent A666 proof | Fresh public proofs reproduce source results and NAV without StakeHub or unapproved trust downgrades. | **FAIL/OPEN** |
| `G5` Controlled migration | Six validators complete activation, transparent/private issue/redeem, export/return, restart, replay, conservation, pause, and rollback. | OPEN |
| `G6` Live migration | Existing A666 route is governed to the public successor and passes all live verification. | OPEN |
| `G7` Clean public reproduction | Tagged public checkout reproduces the complete lifecycle without internal filesystem or code access. | OPEN |
| `G8` Accurate UX | Wallet/RPC exposes freshness and quantity/valuation trust classes without provider-brand inference. | PARTIAL; requalify against successor |

StakeHub is deprecated only after every gate from `G0` through `G7` passes and
all six adapters are production-qualified. Partial framework or historical
proof passes do not satisfy that condition. StakeHub is not deprecated.

## Verification boundary

- `scripts/check-a666-public-adapter-readiness` checks this record, all six
  adapter dispositions, and every listed public implementation path.
- `scripts/check-a666-public-source-qualification` checks the retained two-epoch
  qualification packet against the current committed source inputs. Its archive
  mode reads exact Git objects; it does not regenerate proofs or claim a fresh
  independent proof verification.
- The full proof-public-input inventory remains mandatory. Repinning source
  files does not reproduce a guest ELF or authorize a new program key.
- Release, live migration, and value movement remain separately gated. See
  [Current State](../status/chain-state-current.md) for recorded fleet lineage
  and its observation-time limit.

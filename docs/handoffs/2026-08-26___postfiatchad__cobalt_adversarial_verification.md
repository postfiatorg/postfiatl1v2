# Cobalt adversarial verification complete

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-26 UTC

## BLUF

The locked Cobalt adversarial-verification milestone is complete. E1-E6,
the live E5 authority drills, authenticated CLI/browser interfaces, public
results, and release checks passed. The bounded controlled-devnet result is
`KEEP_ACTIVE`: Cobalt remains active for validator-registry and trust-graph
ratification, while Consensus v2 remains block finality. This proves protocol
capability, not operator decentralization.

Resume from this handoff. Use [Current State](../status/chain-state-current.md)
for the canonical operational snapshot and the
[completed milestone](../plans/completed/cobalt-adversarial-verification-milestone.md)
for the experiment record. Do not resume from the superseded active-plan path.
The completion continuation used neither Task Node nor subagents, per the
operator's explicit instruction.

## Current state

### Repository and publication

- Branch: `main`.
- Pushed completion payload commit:
  `7968b8da2876090f7319b007bb0a04bcd8eb42fe`.
- `origin/main` matched that commit immediately after the completion push. The
  commit containing this handoff is its immediate handoff-only descendant; use
  `git rev-parse HEAD` for that checkout identity.
- No uncommitted campaign work should remain after the handoff push.
- The deployed runtime source is distinct:
  `8cc7d15edc58b5f5a0b745143fef2d45203465ff`. Repository descendants are not
  themselves proven deployed without another all-six fleet receipt.
- E5 evidence commit:
  `ee6707c41aec972c2ecc53349b7eafd6872b275a`.
- Public article repository commit:
  `d4edd89bb781c8b41ccc21a1a6034d9839c45b63`; the article is live at
  <https://postfiat.org/blog/cobalt-further-evaluation/>.

### Last observed controlled devnet

The final authenticated live-drill and fleet observation ran from
`2026-08-26T06:34:55Z` through `2026-08-26T06:35:50Z`. This is the last
committed point-in-time observation, not a real-time probe when this handoff is
read.

- Chain: `postfiat-wan-devnet-2`.
- All six validators converged at height 924; all validator, RPC, and advisory
  shadow services were active.
- Tip:
  `ebeb0e1ee27f30ba480255728832719d94eac1a89d762a7aa7019eae269008fac53098cf6495f477a241d63a7649fbef`.
- State root:
  `0854bc47f78996b2dcd279206cbdcc0b4858395c5937e0e0d56b3d645ca6b6a9d9c9578f5ac77bb14bea9dd1ee6f413e`.
- Registry root:
  `08a451e07aeaf9ada41a69e7c26dfd3fd86fce11c02f5567127c598b3cf775ac054b2add85295cc8c0d429bb6d2b9b1d`.
- Trust-graph root:
  `89f18aef2c5726ae43043407eb4d638ee8f3b6027e58ec3553296478602232cf3c2fc5d1dfebc4058d720b16508f0307`.
- Ratification anchor: sequence 2,
  `5eada38d23c83709a44f2cfa7eb7897d9d4b1da906e6ef66fc5dfec7e64102edda2e82b33d71346c1d8f75ccc21153c8`.
- All six node binaries had SHA-256
  `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`;
  all six shadow binaries had SHA-256
  `d61e6d0f6767998c4abfbf4f85e1f6bd5edfeef8a7a27cf965c17b676b1a0a4a`.

The first rollback/return pair at heights 920/921 is preserved as remediation
history because the height-921 return used a non-protocol-native trust binding.
No fork or conflicting accepted root occurred. The corrective signed rollback
at 922 and return to Cobalt at 923 are the final-gate pair. The legitimate
validator-5 rotation committed at 924. Every block from 920 through 924 retained
Consensus v2 finality, and all nine live negative cases rejected without durable
state mutation.

### Evidence and result

The consolidated authenticated packet root is
`a789372819c173d3c290f84b7ad10bea3ddef01ffc5a012e837ba3dc32d36368`.
Its verifier reports `adversarial-packet-ok`, `KEEP_ACTIVE`, six passed
experiments, nine rejected live cases, and all 15 final checks passing.

Experiment roots:

- E1: `9151c9b7f43e2c75f367416b9087e7255ca1c03ae734bfdd362fc79ff0cbbc05`.
- E2: `8742d9603621408339d99c3d9fcc1ba8cc43dafdc900acdfccbf86cc60d7cba3`.
- E3: `9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600`.
- E4: `93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508`.
- E5: `0695284a7b38ac0129c47e1242f4a2227ad25096147920e79569a924e5f3b3db`.
- E6: `fa6255b5a5f31f2d7e8b2836eb005f894b815199ec1205a30fa99a6fb22de6e2`.

E6's design decision did not change, but its source pins were revalidated after
the E5 authority-lineage hardening so its standalone verifier matches the
final repository sources.

Final checks passed:

- all E1-E6 and consolidated packet verifiers;
- 27 Python Cobalt/observatory tests;
- 72 `postfiat-consensus-cobalt` tests;
- 29 focused `postfiat-node` Cobalt tests;
- focused checks for both E5 live-drill binaries;
- CLI rendering with 6/6 experiments, nine rejections, and 15 `[PASS]` checks;
- real browser `GET /api/snapshot` and `GET /` responses at HTTP 200, with
  mutation probe `POST /api/snapshot` rejected at HTTP 405;
- strict MkDocs build, documentation redaction check, packet redaction scan,
  formatting, and `git diff --check`.

### Scope boundary

Current proposals and authorizations still originate from
Foundation-administered validators. No independent operators were recruited,
no mainnet authority was granted, and no claim of operator decentralization is
supported. Cobalt ratifies bounded validator-trust changes; it does not select
who deserves trust and does not control block consensus.

## Next decision or action

The adversarial campaign itself has no remaining experiment. Do not restart E1
or continue into another adversarial phase unless the locked specification is
formally changed.

The next bounded program decision is whether to open the separate
independent-operator proposal-path milestone required by E6. That work must
recruit genuinely independent operators, refresh admission and topology
bindings to the live release and roots, prove no one operator can reach or block
quorum, and repeat the live migration evidence gates. It is not authorized by
this handoff.

Before any later claim about what is live "right now," perform a new
authenticated all-six fleet probe. Before any deployment, keep deployed binary,
source, repository, and evidence identities separate and stop on a root,
signer, lineage, service, or finality mismatch.

## References

- [Current State](../status/chain-state-current.md)
- [Completed adversarial-verification milestone](../plans/completed/cobalt-adversarial-verification-milestone.md)
- [Locked adversarial-verification specification](../governance/cobalt-adversarial-verification-research-spec.md)
- [Final adversarial results](../governance/cobalt-adversarial-verification-results.md)
- [Independent-operator proposal-path specification](../governance/cobalt-independent-operator-proposal-path-research-spec.md)
- Final consolidated packet: `benchmarks/cobalt-adversarial-verification/packet/`
- Live E5 packet: `benchmarks/cobalt-adversarial-verification/e5/`
- E6 decision packet: `benchmarks/cobalt-adversarial-verification/e6/`
- Public article: <https://postfiat.org/blog/cobalt-further-evaluation/>

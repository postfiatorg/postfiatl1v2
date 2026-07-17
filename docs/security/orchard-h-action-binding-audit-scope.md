# AssetOrchard `h_action` Binding: External Audit Scope

Status: P13 audit scope prepared; independent review required before P13 can be
closed.

## Decision to audit

AssetOrchard does not copy Orchard's value-binding construction. Upstream
Orchard uses homomorphic value commitments and a binding signature to prevent
an authorized collection of spends and outputs from creating value. The
AssetOrchard swap circuit instead:

1. proves a fixed two-input/two-output permutation for both private asset tags
   and private values;
2. constrains every input and output note opening, input Merkle path,
   nullifier, randomized spend key, and output commitment;
3. proves the at-NAV pricing relation and forces the fee to zero; and
4. hashes the complete public action statement into two `h_action` field
   elements that are themselves public inputs.

The implementation has tests for these individual constraints. What has not
been independently established is whether their composition supplies the
binding property required of this fixed-shape protocol, or whether replacing
Orchard's algebraic value binding introduced a gap at a circuit, proof,
serialization, authorization, or multi-action boundary.

This document defines that external review. It does not assert equivalence and
does not authorize a circuit change.

## Exact construction under review

The swap proof has exactly two private input legs and two private output legs.
For one Boolean permutation bit `s`, the circuit constrains each output asset
tag and value to be either the corresponding input (`s = 0`) or the other input
(`s = 1`). It also range-constrains the private values, requires nonzero values
and asset tags, and fixes the public fee to zero. Value splitting, combining,
minting, burning, variable arity, and a nonzero in-circuit fee are not supported
by this circuit version.

The public instance currently contains 28 Pallas base-field elements:

- pool domain and anchor;
- two nullifiers;
- both coordinates of two randomized verification keys;
- two output note commitments;
- three field limbs for each encrypted-output hash;
- fee;
- base and quote asset-tag limbs, pricing numerator and denominator, and three
  pricing-claim commitment limbs; and
- two `action_ctx` elements.

`h_action` hashes 32 fields with `P128Pow5T3`: five fixed protocol identifiers,
the first 26 public-instance fields, and a fixed pricing-claim version. Its
input also includes a domain field and the field count. The in-circuit sponge
is `halo2_gadgets::poseidon::Pow5Chip`; its two outputs are constrained to
public-instance rows 26 and 27. The serialized 64-byte swap binding hash is the
canonical encoding of those two outputs and validation recomputes it from the
action fields.

The review must use the current pinned circuit and artifacts, not the older
17-field description from the initial remediation report:

- circuit ID: `postfiat.asset_orchard.swap_conservation.v1`;
- parameter `k`: 15;
- public-instance layout hash:
  `4a97e6254fe6ce1416723ebc0908f6a2a617a8d223f902905a91d7f006a8d1e8cee4cb2ccd81decd29c85b4a2c0f7ed1`;
- Poseidon parameter hash:
  `7249e21c01fa7cd5020c40cd2aacf08b3e22990aae202a1cf37ce6fc73ae448536a77c6f668fa23749981a69fd6fcdf3`;
- VK attestation hash:
  `edc14bbd5fabf855817dfd02c30bf616376dbda29362524def869b6e2ce615c2a1a141389aafdea5461f137d84ce358a`;
- runtime pinned-VK fingerprint:
  `1b38a9d9906cbfce9addf9a500a1b4bec720a33118507946f427a628772fac48f1786bffa390a5254db215fadf7f3460`.

## Required security questions

The independent reviewer must answer all of the following:

- Does the fixed permutation constrain each `(asset_tag, value)` pair as a
  pair, without permitting cross-row selector substitution or field/value
  recombination?
- Can any satisfying witness increase an asset's total value, change an asset
  identity, split or combine value, or exploit Pallas-field wraparound while
  all public checks pass?
- Are private values constrained to the intended integer range before every
  arithmetic and pricing use, with no alternate field representation?
- Do note commitments, Merkle membership, nullifiers, randomized spend keys,
  output commitments, and encrypted-output hashes bind the same logical notes
  used by conservation and pricing?
- Does the Halo2 proof transcript already bind every public-instance field, and
  if so, what additional security role does `h_action` serve at the action
  serialization and authorization boundaries?
- Can an attacker substitute, omit, reorder, or duplicate a public field while
  preserving the two `action_ctx` elements or serialized swap binding hash?
- Are the domain identifier, explicit length, fixed protocol identifiers,
  padding rule, two squeezed outputs, and canonical field encodings sufficient
  to rule out cross-protocol, length-extension, and ambiguous-encoding attacks?
- Is collision resistance of this Poseidon instantiation the only hash
  assumption needed, and is using two consecutive sponge outputs sound for the
  64-byte binding representation?
- Across a certified batch containing multiple AssetOrchard actions, does
  per-action conservation suffice, or is an aggregate binding signature or
  transaction-level balance equation required to prevent cancellation,
  replay, or action-splicing attacks?
- Do spend-authorization signatures and their sighash commit to the exact
  action binding, proof, encrypted outputs, ordering, pricing claim, chain
  domain, and batch context needed to prevent an authorized spend from being
  attached to a different value-conserving action?
- Does concurrent verification/application or replay recovery admit any path
  that verifies one public statement and mutates state using another?
- Is private egress affected by the same reasoning, and if so must its
  13-field public instance and separate `h_action` construction be included in
  the final security claim?

The review should distinguish “cryptographically equivalent to Orchard” from
“adequate for this deliberately narrower fixed-shape protocol.” The latter is
acceptable only if every narrowing assumption is enforced by consensus and a
future protocol extension cannot silently bypass it.

## Required adversarial evidence

The final review packet must include reproducible vectors for:

- each non-conserving or asset-changing witness class the reviewer considered;
- selector splitting and asset/value cross-pair substitution;
- maximum-value and field-boundary inputs;
- every public-instance field changed independently while keeping the original
  proof, `action_ctx`, signature material, or serialized binding hash;
- reordered and duplicated actions in a multi-action certified batch;
- spend-authorization reuse against a different proof or action;
- cross-domain, cross-circuit, cross-version, and cross-pricing-claim replay;
- differential equality of host and in-circuit `h_action` over randomized and
  boundary vectors; and
- any private-egress vector required by the review's scope decision.

Existing positive tests are supporting evidence, not a substitute for this
adversarial analysis.

## Acceptance and escalation

P13 may be marked closed only when an independent cryptographic reviewer has:

- [ ] identified the exact commit, circuit metadata, and VK fingerprints
  reviewed;
- [ ] provided a written argument covering every required security question;
- [ ] supplied or reviewed the required adversarial vectors;
- [ ] stated whether the construction is adequate under its enforced
  fixed-shape assumptions;
- [ ] listed every assumption that must remain consensus-enforced; and
- [ ] signed or otherwise verifiably attested the final report.

If the review finds the construction inadequate or cannot establish the
binding property, P13 escalates to a new circuit-design item. Any such change
must receive its own threat model, differential vectors, full proof-acceptance
tests, regenerated and signed VK artifacts, canary-first deployment plan, and
explicit rollout authority. It must not be folded into this documentation
packet.

## Code and evidence map

- `crates/privacy_orchard/src/asset_orchard_circuit.rs`: conservation,
  pricing, note/spend constraints, in-circuit `h_action`, public-instance
  layout, proof creation, pinned metadata, and VK artifacts.
- `crates/privacy_orchard/src/asset_orchard.rs`: public fields, host
  `h_action`, serialized binding hash, validation, and authorization preimages.
- `crates/privacy_orchard/src/asset_orchard_action_builders.rs`: shipping action
  construction.
- `crates/privacy_orchard/src/verify.rs`: action validation and proof/signature
  verification.
- `crates/node/src/orchard_state_application.rs`: consensus state mutation.
- `crates/privacy_orchard/src/asset_orchard_circuit_tests.rs`: constraint,
  proof, tamper, differential Poseidon, and pinned-VK evidence.
- `crates/privacy_orchard/artifacts/asset_orchard_swap_vk_pinned_assembly.v1.bin`:
  embedded verifying-key assembly.

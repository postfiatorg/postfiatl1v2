# Consensus v2 signing rounds

Consensus v2 authorizes every signature against one durable round floor at each
height and committee domain. Let the prepare, precommit, and timeout high-water
marks be $r_P$, $r_C$, and $r_T$. A signature at round $r$ requires

$$r \ge \max(r_P,r_C,r_T).$$

An absent mark contributes no lower bound. The existing phase-specific rules
still prohibit duplicate votes and timeouts, and the lock rules govern which
block may receive a prepare. Same-round prepare, timeout, and precommit
progression remains possible as their certificates arrive.

The implementation is
`require_consensus_v2_monotone_round` in
`crates/ordering_fast/src/consensus_v2.rs`. All three authorization functions
call it. Node authorization serializes the read, predicate, and durable write
under the existing safety guard in `crates/node/src/consensus_v2_store.rs`;
the guard remains held through signature construction in
`crates/node/src/consensus_v2_finality.rs`, so a concurrent phase cannot advance
the floor between authorization and signing.

## Fixed-height safety argument

Assume a fixed committee of $n$ identities, at most
$f=\lfloor(n-1)/3\rfloor$ Byzantine identities,
$q=\lfloor2n/3\rfloor+1$, authentic signatures, verified certificates, and
durable correct-signer state. Quorums intersect in more than $f$ identities.
Correct signers prepare one block per view, lock a verified prepare certificate
before precommitting, and prepare a conflicting block only when justified by a
strictly higher prepare certificate from an earlier view.

Suppose block $X$ obtains a commit certificate at view $v$. At least $q-f$
correct signers precommitted $X$ and durably locked its prepare certificate.
Consider the earliest conflicting prepare certificate at any view $w>v$.
Its quorum intersects those correct precommit signers in at least
$2q-n-f>0$ identities.

For an intersecting correct signer, the shared round floor places its
view-$w$ prepare authorization after its view-$v$ precommit authorization.
The signer therefore had the $X$ lock when considering the conflicting
prepare, or had already replaced it through a higher prepare certificate.
Either action requires a conflicting prepare certificate earlier than $w$.
That contradicts the choice of $w$. Two conflicting prepare certificates at
one view would require a correct identity to prepare twice. Since every commit
requires its matching prepare certificate, two conflicting commits are
excluded under these assumptions.

The argument concerns one height and one committee domain. Registry transitions
add their separately checked continuity and quorum-intersection requirements.

## Recovery and compatibility

Progress requires a correct proposer, timely communication, and justification
that permits the correct validators to vote under their existing locks.
A timeout certificate selects the highest prepare certificate among the votes
it actually contains. The signing floor preserves this recovery mechanism:
a validator may advance to a newer view with valid timeout evidence and
complete prepare and precommit there. Recovery tests cover failed proposers
and restart; they establish the tested executions.

The guard uses existing Consensus v2 state fields. It changes neither the
serialized state schema nor signed proposal, vote, timeout, or certificate
bytes. Historical certificate verification and legacy finality remain
unchanged. The correction enforces the highest-voted-view contract within
the existing versioned Consensus v2 path. Running nodes acquire the corrected
authorization behavior only through an operator-controlled release.

## Regression coverage

- `crates/node/src/consensus_v2_delayed_certificate_tests.rs` exercises real
  signing and disk state: a withheld view-0 certificate is rejected after a
  view-1 prepare, the newer view still obtains a commit certificate, and a
  restored snapshot retains the round floor.
- `crates/ordering_fast/src/consensus_v2/tests/round_monotonicity.rs` checks all
  phase pairings, same-view progression, certificate verification, and 332
  bounded delayed-certificate cases over four- and six-member committees with
  all quorum memberships and zero or one Byzantine identity.
- `crates/node/src/consensus_v2_store.rs` pauses signature construction under
  the per-height guard and proves a competing authorization cannot enter until
  that callback returns.
- Existing Consensus v2 tests cover signed domains, locks, malformed ancestry,
  failed-proposer recovery, and snapshot compatibility.
- `crates/node/src/main_parts/tests/transport_batch_payload_tests.rs` contains
  the four- and six-validator transport recovery scenarios.

Run the focused signing and storage checks with:

```bash
cargo test -p postfiat-ordering-fast -p postfiat-node --lib consensus_v2 --locked
cargo test -p postfiat-node --bin postfiat-node consensus_v2 --locked
```

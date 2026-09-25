# Restore FastPay after an unrelated validator key rotation

Date: 25 September 2026. Status: proposed, awaiting the repository's full TIH gate.

## Outcome and scope

Restore FastPay transfers and unwraps on the existing controlled devnet without
undoing validator-5's legitimate key rotation, lowering quorum, editing replicated
state by hand, or activating a different consensus protocol. This is the immediate
node repair in the incident handoff on `main`
(`docs/handoffs/2026-09-25___postfiatchad__fastpay_down_stale_committee.md`),
not an early committee-rotation protocol. The user has authorized repairing the
incident. Task Node's skill and callable integration are unavailable in this
session; no Task Node acceptance or reward is claimed.

The existing six-member FastPay committee requires five signatures. Five current
validator keys still match it. Restoring their participation restores payment
availability, with **zero additional signer-outage tolerance** until a governed
committee rotation restores all six signers. This limitation must be explicit in
the operator result. A later rotation must preserve the drain and fencing rules
in the existing recovery specification; this repair does not shorten an epoch or
silently make the replacement key a committee member.

## Failure and authority model

`crates/node/src/fastpay_recovery_node.rs::validate_local_fastpay_committee`
currently requires equality of the entire live validator registry and the
replicated FastPay committee. The check runs before capabilities, transfer/unwrap
signing and transfer/unwrap application. A legitimate change to one registry key
therefore disables unrelated authorized signers and verification of otherwise
valid certificates.

The replicated committee already commits the exact chain/genesis/protocol domain,
epoch, validator identities, public keys and quorum. That committee remains the
authority for its FastPay certificates. The current registry controls which local
validator identity may participate now. A signing operation must satisfy both
authorities for its own `(validator_id, public_key)`; equality of other members is
unnecessary. Signature verification must bind the actual private key used to the
expected public key, not merely trust public-key metadata in a key file.

Capabilities and cryptographic certificate verification are reads of ledger
authority, not votes. They must not be disabled by an unrelated registry key
change. Existing genesis binding, canonical committee validation, order windows,
owner signatures, quorum, version/asset/value checks, durable locks, recovery
fences, speculative-effect journals and ordered-block reconciliation remain.

## Required implementation

1. Split committee/domain validation from local signer authorization. Capabilities
   report the valid replicated committee on every synchronized node. Transfer and
   unwrap votes authorize only the requested local identity against both the
   current registry and selected committee before reserving input locks. A rotated,
   absent or mismatched signer returns a specific error with no durable mutation.
2. Verify the generated vote against the pinned committee public key before
   reserving locks or returning it. A signing key inconsistent with registry
   metadata must not strand an input. A conflicting second order still cannot
   escape the existing durable lock, even though its signature is computed locally
   before the lock is committed.
3. Preserve certificate application and acknowledgement semantics. An application
   that promises a signed acknowledgement must establish its own eligible signer
   before mutation and verify the acknowledgement's key binding. The rotated node
   must not return an invalid acknowledgement, claim to be an old member, or
   report a successful quorum contribution. Nodes outside the committee still
   converge through the existing ordered/certified state synchronization path.
   Demonstrate that path after the payment; five successful acknowledgements alone
   are insufficient evidence of six-node convergence.
4. Keep the Python CLI, wallet SDK/proxy and UI on their existing five-of-six
   verified certificate/acknowledgement contract. Diagnose the handoff's separate
   four-of-six route warm-up observation using fresh status requests. Repair a
   demonstrated transport issue using an already-qualified bounded fix where
   needed; do not label a stale node converged or relax the quorum calculation.

## Qualification

Use deterministic local six-member fixtures reproducing a rotation of only member
5. Prove members 0–4 can sign a transfer and unwrap and assemble valid five-vote
certificates; member 5 cannot sign with its old or replacement key under the live
registry/old-committee combination. Prove capabilities remain available, changed
signer rejection is before lock/state mutation, a wrong private key rejects,
four votes reject, forged/duplicate/cross-domain votes reject, and replay remains
idempotent. Apply/acknowledge with the unchanged members, then reconcile the
non-signing node through a certified ordered block and compare the resulting state.

Run the affected node FastPay tests and execution recovery tests, plus relevant
Python CLI and wallet-proxy quorum/route tests. This changes neither Orchard proof
verification nor its accounting; do not run unrelated full proof suites. Preserve
test counts and failures honestly. No fixture uses a live validator private key.

Before deployment, record all six live heights, tips, state roots, registry hashes,
committee identity, service/executable hashes and mempools. Build a bounded patch
against the verified deployed source lineage, not an arbitrary replacement with
all newer main-branch changes. Qualify it against an isolated consistent copy of
the deployed state and verify that loading/status/replay preserve its roots.
Keep an exact rollback binary and service configuration. Stage the reviewed binary
on every host, verify hashes, then roll the affected validator/RPC services one
host at a time; require the remaining five healthy and each restarted node caught
up before proceeding. Never overwrite keys, registry or ledger files.

## Live acceptance and rollback

Acceptance requires all-six status agreement, working recovery capabilities,
five valid votes and acknowledgements for a small authorized test payment through
the existing CLI/proxy, the expected recipient output and sender change, and
subsequent all-six certified convergence. Exercise unwrap/recovery locally; avoid
additional live fund movements without a concrete need. Prefer the already-wrapped
test coin from the incident rather than creating another stranded wrap. Record
the exact transaction/certificate and observed timings without exporting secrets.
Verify the existing UI receives the final result and does not call a merely
submitted or four-vote payment complete.

If a host fails to recover, stop rollout and restore its previous binary/unit
binding while preserving ledger and locks; recheck convergence before resuming.
After an ambiguous payment submission, query the existing certificate/receipt and
object versions before any retry. Never issue a new order to obscure an uncertain
old one. Do not report the network repaired from unit tests or service health alone.
Publish the code, operator guidance and dated evidence, separating immediate
five-signer availability from the still-needed governed committee rotation.

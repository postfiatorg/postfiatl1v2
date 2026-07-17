# PostFiat Governance Agent Architecture Statement

Status: governed input for local Gate 1.5
Version: 1

The governance agent is an offline proposal generator for PostFiat L1
governance policy. It does not own validator authority, validator keys,
registry state, Cobalt ratification, or chain state. Its output is an input to
the existing governance review, Cobalt dry-run, and guarded-apply pipeline.

The live chain remains the source of truth for validator registry state,
governance amendments, ordered batches, receipts, and rollback evidence. Any
model-generated ruleset must be converted into typed PostFiat policy objects and
validated by deterministic local code before it can influence a Cobalt dry run.

The agent input bundle is made from stable bytes:

- this architecture statement;
- the objective statement;
- the constitutional constraints;
- the ruleset schema;
- the model/runtime profile;
- deterministic inference flags;
- rollback policy;
- frozen evidence roots supplied by later gates.

Gate 1.5 is a pre-model gate. It proves the bytes that a future model request
will consume are reproducible and constrained. It does not run GPU inference, it
does not submit transactions, and it does not mutate validator registry state.

All consensus-relevant hashes are domain separated and computed over canonical
encodings. Object key order in JSON is not a source of entropy. Wall-clock time,
random process state, network reads, provider inventory, or live chain reads are
not allowed in Gate 1.5 hash construction.

The initial deployment target is dry-run governance analysis. Authority transfer
from foundation-operated publication to model-assisted or verifier-assisted
policy remains a later governance decision, not a property of this bundle.

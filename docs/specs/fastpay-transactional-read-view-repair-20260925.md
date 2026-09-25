# FastPay must expose durable certificates over the active finalized database

## Objective and observed failure

Complete the already authorized FastPay incident repair. The committee-authority
repair and finalized-checkpoint restore prerequisite passed their locked gates and
were deployed as source `0989bd84`. A bounded live test then found an independent
storage integration defect. Five nodes verified the transfer certificate, retained
its speculative-effect journal and signed acknowledgements, but wrote the updated
ledger through `NodeStore::write_ledger`, which only changes legacy JSON. With
transactional storage active, `NodeStore::read_ledger` reads the unchanged database.
The input remained visible and the recipient output was absent. This is not a
completed payment despite the five acknowledgements.

The first transfer certificate is retained locally and on its five signers.
The test input contains 1001 atoms, intended payment 1000 atoms and fee 1 atom.
The ordinary funding and wrap finalized through height 1033. A subsequent CLI
resume attempted a new order because the stale read still showed the input; it
received zero votes because the original durable locks remained. Stop retries.
No further funding, wrapping or new nonce is needed to recover this operation.

Temporarily restore the original signed release on eligible signers, one host at
a time with all-six convergence checks, so the existing committee mismatch blocks
new FastPay certificates while the integration fix is qualified. Preserve all
journals, locks, keys, registry entries and the finalized database. Do not roll
back ledger state. The rotated validator may retain the already-qualified repair;
it cannot supply an epoch-1 FastPay signature.

## Verification and persistence boundaries

Keep the transactional database as the canonical finalized state. Do not write
uncertified speculative effects into its finalized ledger, root or history index.
Do not change storage schema, consensus rules, committee epoch, quorum, certificate
encoding, fee calculation, NAV invariants or validator keys.

Use the existing durable FastPay journal as the source of pending effects. Add a
node-internal `read_fastpay_ledger` or equivalent helper that reads the canonical
ledger and, only for active transactional storage, materializes the appropriate
pending journal effects in memory using the existing cryptographic replay code.
Legacy storage retains its existing ledger behavior. The helper must not write
state or convert a certificate into an acknowledgement.

Rules for materializing a journal:

1. Read and validate its existing schema, canonical lock ordering and bounds.
2. Read the canonical tip and finalized ledger under the same existing ordered
   mutation lock for mutating/voting operations; read-only callers must retry or
   fail if the tip changes during their snapshot acquisition. Never combine a
   journal from a newer tip with an older canonical ledger.
3. Ignore entries already represented by canonical fences. Canonical ordered
   recovery and cancellation take precedence over a historical local journal.
4. Only materialize unanchored entries decided at the current finalized height.
   An entry from a future height is invalid. An older unanchored entry omitted by
   an intervening certified block remains available for explicit recovery but
   must not be silently resurrected. This preserves existing minority-effect
   rollback semantics.
5. Bound the pending set by `MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK` and apply it
   atomically through `replay_confirmed_fastpay_fences_dependency_ordered`, not
   lexical lock order. That existing routine verifies the owner signature,
   committee/domain, five votes, versions, values, recovery window, resulting
   fence and fee conservation. A failed dependency or certificate publishes no
   partial view.
6. Preserve the durable inverse journal and original certificate. Do not treat a
   cached whole legacy ledger as authoritative over the active database.

Use this view where pending FastPay effects belong: transfer/unwrap signing and
apply/idempotence, recovery status, owned-object queries, account balance queries
needed for unwrap, transaction admission/preview, block proposal creation and
proposal validation, and ordered batch application. Audit the read sites explicitly
in `fastpay_recovery_node.rs`, `lifecycle_queries.rs`, `mempool_proposals.rs`,
`batch_snapshot.rs`, `governance.rs` and `shielded_batch_actions.rs`. Keep status,
canonical checkpoint/root verification, historical replay and canonical snapshot
export on finalized database state. Do not globally replace every ledger read.

For active storage, apply must retain the validated journal before acknowledging;
it must not rely on the ignored legacy write. Construct and cryptographically
verify the acknowledgement before durable mutation as required by the already
locked authority specification. Idempotent application must return the original
terminal acknowledgement without another fee or effect. Existing local input
locks continue to prevent a competing order. On restart the journal recreates the
same pending view. After an ordered block anchors the effect, the database supplies
that effect and the journal must not apply it twice.

The proposal builder must include locally durable effects. Voters with such effects
must reject an uncertified proposal omitting them. Existing certified-block
reconciliation may discard minority local effects, and the subsequent read view
must respect that decision. A non-signing validator must reproduce supplied valid
effects and converge through the ordinary certified block path.

## Client result compatibility

The live test also exposed two result-shape issues. The Python SDK nests the v3
apply response under `apply`, while the existing CLI expects top-level
`created_objects`. In addition, the proxy's v3 acknowledgements do not themselves
contain created objects, so simply flattening the response is insufficient.

Preserve top-level client result fields and the nested diagnostic `apply` form,
without trusting an empty or forged proxy output list. Reuse the execution engine's
existing deterministic owned-output construction in the Rust SDK verifier after
certificate votes and apply acknowledgements are verified. Extract a shared helper
if needed, retaining the exact existing ID algorithm and output version. Do not
implement a second incompatible ID formula in Python or change execution results.
The verifier may add expected created objects to its successful result; the Python
SDK can expose those verified outputs. Unwrap must preserve its corresponding
verified result contract. Quorum failure must still raise before success or any
trusted output is returned. The existing CLI's final input/recipient read-back
remains mandatory. A certificate-derived expected output alone is not proof that
a live read path exposes it.

## Qualification sequence

1. Reproduce the active-storage failure in a small deterministic fixture with
   consensus-v2 and storage activation at height 1. An acknowledgement followed by
   a fresh read currently leaves the input present. Preserve this negative control.
2. Test transfer and unwrap views, wrong-key/no-mutation cases, same-certificate
   replay, competing order rejection, restart and journal persistence. Canonical
   tip/root/database ledger remain unchanged before anchoring while business reads
   show exactly the certified effects.
3. Test missing dependencies, reverse lexical dependency order, corrupted journal
   certificates, future-height entries, forged votes and conflicting canonical
   fences. Failure must not mutate canonical state or publish a partial view.
4. Test an intervening certified block omitting a minority effect; it must not
   reappear after restart. Test identical anchored effects are not reapplied.
5. Use an activated six-node fixture with validator-5 rotated: five signers provide
   valid votes/acks, the rotated member refuses to sign, and a certified ordered
   block converges all six ledgers, tips and roots including the recipient output.
6. Run focused FastPay/execution/storage/checkpoint tests and Python/proxy result
   contract tests. The output-construction extraction needs exact identity/parity
   tests, including amount, owner, asset and output order. Avoid unrelated full
   proof suites; no Orchard proof or economic invariant changes are proposed.
7. Qualify the exact retained live certificate against a fresh signed height-1033
   checkpoint restore. The canonical checkpoint must still verify. Pending-view
   queries must expose its exact input consumption/output, and repeated queries or
   restart must not debit again. Tampered pending evidence must fail closed.

## Rollout and completion evidence

Build the bounded patch on the exact deployed source archive plus the two existing
qualified repairs. Keep publisher identity and signed manifests. Perform the
mandatory signed-backup round trip using finalized-checkpoint verification; do
not claim that the known historical block-1011 full replay anomaly was repaired.
Record source and binary hashes, test outputs, backup manifest hash and all-six
preflight. Use the existing safe-rollout entrypoint, one validator at a time.
Returning eligible signers to the original release before this rollout ensures
new five-vote certificates cannot form until all five have the storage repair.

If qualification or a host restart fails, stop, preserve the evidence and restore
that host's previous binary/unit binding without changing its ledger. Keep new
FastPay certificate formation blocked until the repaired implementation qualifies.
Do not lower the quorum or manufacture a terminal receipt to finish the task.

After all six run the replacement, reconcile the original test operation through
the existing CLI. Verify its retained five votes and five acknowledgements, exact
recipient object, input consumption and displayed accepted result in the real
wallet UI. Reuse the retained certificate or explicit recovery path if needed;
never generate another payment merely to conceal ambiguity. Anchor the effect
through a bounded ordinary certified transfer, preferably returning unused test
funds, and verify all six nodes' exact recipient output and certified state.
Capture timings honestly and separate the ordinary wrap from the FastPay phase.

Publish code, operator guidance and dated evidence; retire the implementation
milestone only after CLI/UI and all-six convergence pass. The zero additional
signer-outage tolerance remains: five eligible keys satisfy a five-signature
quorum until a separate governed committee rotation. Task Node integration is
unavailable in this session; record that administrative gap without inventing
acceptance, evidence submission or rewards.

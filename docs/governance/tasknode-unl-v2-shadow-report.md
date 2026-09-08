# Task Node UNL V2 shadow CLI and report

The UNL amendment V2 reference implementation is available through an
explicitly selected, local-only Python interface. It verifies the Section A
evidence contract, freezes the Section B graph and admission state, and renders
a root-bound V1-reference/V2 comparison. Every result remains `SHADOW_ONLY`.

The interface cannot submit a transaction, change a validator registry, invoke
Cobalt, access Task Node, or promote V2. It performs no network access. The
[locked amendment](tasknode-unl-amendment-v2-20260907.md) remains the source of
the rules, and the [completed milestone](../plans/completed/tasknode-unl-amendment-v2-milestone.md)
records the implementation boundary.

## Run the fixture workflow

From the repository root, derive canonical machine-readable JSON and the human
Markdown report from the same inputs:

```bash
PYTHONPATH=python python3 -m postfiat_rpc.tasknode_unl_v2 derive \
  --policy-version v2 \
  --evidence-bundle python/tests/fixtures/tasknode_unl_v2/evidence-golden.json \
  --admission-input python/tests/fixtures/tasknode_unl_v2/cli-admission-input.json \
  --output /tmp/tasknode-unl-v2-report.json \
  --markdown-output /tmp/tasknode-unl-v2-report.md
```

The `--policy-version v2` selection is mandatory. Unknown or missing versions
fail closed. Inputs have closed schemas, bounded arrays, bounded identifiers,
an eight-MiB file limit, canonical ordering, and fixed-width lowercase root
commitments. Errors name the failing field.

The `derive` command accepts either the committed evidence golden bundle or an
operational bundle with this closed envelope:

```json
{
  "schema": "tasknode-unl-v2-cli-evidence-bundle-v2",
  "version": 2,
  "mode": "SHADOW_ONLY",
  "snapshot": {},
  "control_registry": {}
}
```

The admission input binds the opening list and seeds, hypothetical registry
round, candidates, and a supplied V1 reference. Candidate identifiers must
match exactly across V1 and V2. The CLI verifies the V1 reference root and
labels it as supplied; it never reinterprets a V1 evidence packet as V2.

To verify a saved machine report's report root and render it again:

```bash
PYTHONPATH=python python3 -m postfiat_rpc.tasknode_unl_v2 render \
  --policy-version v2 \
  --input /tmp/tasknode-unl-v2-report.json \
  --output /tmp/tasknode-unl-v2-report-rerendered.md
```

The two Markdown files are byte-identical. The committed fixture result includes
this side-by-side line:

> `validator-bob-rotation: V1 HOLD (account_already_seated,control_epoch_changed) | V2 HOLD (ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED)`

## Read the report

The JSON and Markdown surfaces keep these categories separate:

- `SHADOW_ONLY` and the non-authority boundary;
- `HOLD_CONTINUITY` from an incomplete or mismatched post-epoch window;
- candidate admission denials with exact reason codes and per-candidate report
  roots;
- `EXISTING_BREACH`, including excess seats, review state, and causative
  evidence; and
- limitations of the public evidence: an unchanged-key account sale or other
  control transfer is undetectable, custody is not personhood, and funding is
  audit-only with neither positive trust mass nor a unilateral veto.

The report always exposes schema version, policy ID, policy/input/registry,
window/graph/declaration/frozen roots, the V1 reference root, locked constants,
candidate reason codes, and the final report root. Its overall label is
`SHADOW_ONLY_WITH_UNRESOLVED_IDENTITY_LIMITS`; there is deliberately no green
status that could conceal identity or cap uncertainty.

An invalid Section A commitment produces `NO_PROPOSAL`. Malformed top-level
input or a mismatched V1/report commitment is rejected with the offending field
rather than partially rendered.

## Evidence gate and activation boundary

The paired Section C gate admitted all fourteen consent-complete honest controls
and reported `PASS_SHADOW_ONLY`. Both normal runs and the reordered-input run
produced SHA-256
`ae08a625675ded91dfbfb08c7acad3991d0b027a188bd31069107aec51b0f024`.
That gate result and this completed interface demonstrate deterministic shadow
implementation only. They do not authorize live promotion, establish human
independence, resolve unchanged-key transfers, select incumbent evictions, or
change V1, registry signatures, Cobalt ratification, quorum, Foundation score
provenance, token economics, or any live system.

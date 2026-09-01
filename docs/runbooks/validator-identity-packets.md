# Validator Identity Packet Runbook

**Status:** Operational procedure for the `SHADOW_ONLY` identity-profile corpus

**Audience:** Foundation researchers, reviewers, and publication operators

**Current corpus:** `validator-identity-packets-20260901`

## 1. Purpose and safety boundary

Use this runbook to verify, review, resume, supersede, and publish the public
identity packets produced for XRPL and PostFiat validators.

An identity packet records public evidence about the entity most likely
associated with a validator key or claimed domain. It is **not** a legitimacy,
reputation, sanctions, association, credit, or risk score. It is not consensus
data and must not alter validator admission or weight. The later H200 scoring
stage consumes the frozen Markdown packet bytes; it does not conduct its own
web research.

Never treat any of the following as equivalent:

- presence on a validator list;
- a domain claimed in upstream metadata;
- a domain/entity match supported by public evidence; or
- cryptographic proof that the entity currently controls the validator key.

The packet must state which of these is actually established.

## 2. Artifact layout

Run commands from the repository root unless stated otherwise:

```bash
ARTIFACT=benchmarks/ai-governance/validator-identity-packets-20260901
```

The publication unit is the entire artifact directory:

| Path | Purpose |
| --- | --- |
| `inputs/validators.json` | Exact frozen validator corpus |
| `inputs/<network>/<key>.json` | Minimal coordinates supplied for one validator |
| `prompts/<network>/<key>.txt` | Exact initial Corbanu prompt |
| `packets/<network>/<key>.md` | Public identity packet and downstream scoring input |
| `logs/<network>/<key>.jsonl` | Complete Corbanu exec event log |
| `logs/<network>/<key>.stderr.log` | Captured process stderr; must be empty |
| `runs/<network>/<key>.json` | Session receipt, usage, thread ID, paths, and hashes |
| `index.md` | Human-readable packet index |
| `index.json` | Structured corpus index |
| `verification.json` | Per-corpus strict-verifier result |
| `manifest.json` | Counts, execution contract, and aggregate hashes |

A packet without its exact prompt, log, receipt, and manifest membership is not
a publishable profile.

## 3. Roles

Use two people when a corpus will be published or scored:

1. **Execution operator:** freezes inputs and runs Corbanu.
2. **Publication reviewer:** reviews public evidence, privacy, uncertainty, and
   hash/manifest results without changing generated packet text.

The same person may rehearse the process, but must not self-approve a production
publication. Human review is an evidence-quality gate, not permission to replace
unknown facts with guesses.

## 4. Verify the current published corpus

This procedure is local and makes **no model or web calls**.

```bash
git status --short
python3 "$ARTIFACT/finalize.py"
git diff --exit-code -- \
  "$ARTIFACT/index.json" \
  "$ARTIFACT/index.md" \
  "$ARTIFACT/manifest.json" \
  "$ARTIFACT/verification.json"
```

Expected result:

```json
{"exec_log_set_sha256":"3a72e90a410df1c1ce0681f63d5d581ab70d724bda35362f8a03078a305493b1","packet_set_sha256":"b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd","postfiat":20,"validators":55,"verdict":"PASS","xrpl":35}
```

Also confirm the recorded verdict and counts without relying on prose:

```bash
python3 - <<'PY'
import json
from pathlib import Path

root = Path("benchmarks/ai-governance/validator-identity-packets-20260901")
verification = json.loads((root / "verification.json").read_text())
manifest = json.loads((root / "manifest.json").read_text())
assert verification == {
    "failure_count": 0,
    "failures": [],
    "validator_count": 55,
    "verdict": "PASS",
    "verified_count": 55,
}
assert manifest["counts"] == {
    "exec_logs": 55,
    "packets": 55,
    "postfiat": 20,
    "validators": 55,
    "xrpl": 35,
}
assert manifest["shadow_only"] is True
assert manifest["consensus_input"] is False
print("identity packet corpus: PASS")
PY
```

Stop if `finalize.py` fails, generated files change, counts differ, or the
working tree contained unexplained changes before verification.

## 5. Review one profile

Find a validator in `index.md`, then review its coordinate, prompt, packet,
receipt, and log as one unit:

```bash
NETWORK=xrpl
VALIDATOR='<validator-master-public-key>'

cat "$ARTIFACT/inputs/$NETWORK/$VALIDATOR.json"
cat "$ARTIFACT/prompts/$NETWORK/$VALIDATOR.txt"
cat "$ARTIFACT/packets/$NETWORK/$VALIDATOR.md"
python3 -m json.tool "$ARTIFACT/runs/$NETWORK/$VALIDATOR.json"
```

Review the JSONL log with a JSON-aware viewer. Do not publish screenshots or
terminal output that has not been reviewed:

```bash
python3 -m json.tool --json-lines \
  <"$ARTIFACT/logs/$NETWORK/$VALIDATOR.jsonl" \
  >/tmp/validator-exec-review.json
less /tmp/validator-exec-review.json
rm -f /tmp/validator-exec-review.json
```

### Profile review checklist

- [ ] Network and validator master key exactly match the frozen coordinate.
- [ ] Claimed domain and its frozen verification value are labelled accurately.
- [ ] The domain/key-to-entity conclusion is supported, qualified, or unknown.
- [ ] Canonical entity, entity type, and aliases do not conflate similar names.
- [ ] Official URLs and X handle are supported by primary evidence.
- [ ] Incorporation and operating regions are separate; neither is inferred
      from hosting, WHOIS privacy data, language, or a country-code domain.
- [ ] Activities describe supported public facts and do not infer validator
      operation merely from list membership.
- [ ] Business Summary is one neutral 90–160 word paragraph with no score,
      recommendation, marketing claim, or unsupported financial/headcount data.
- [ ] Public-profile size uses the published rubric and clearly states when
      headcount is not established.
- [ ] Material claims have nearby citations and unresolved facts remain explicit.
- [ ] No private address, private email, personal phone number, credential,
      non-public contact data, or unnecessary personal data appears anywhere in
      the packet **or complete JSONL log**.
- [ ] Machine-readable JSON agrees with the prose.
- [ ] The packet is marked `SHADOW_ONLY`.

A broken citation or newly inaccessible page does not by itself justify editing
a frozen packet. Record the review date and decide whether a successor corpus is
needed.

## 6. Resume missing or failed sessions

This procedure makes paid model calls and performs live web searches.

### Preflight

```bash
test "$(corbanu --version)" = "corbanu 0.1.36"
python3 "$ARTIFACT/build_prompts.py"
git diff --exit-code -- "$ARTIFACT/inputs" "$ARTIFACT/prompts"
```

The current harness is pinned to Corbanu 0.1.36, model `gpt-5.6-sol`,
read-only sandboxing, approval policy `never`, and live search. Confirm the
operator account has sufficient provider capacity before starting. Do not put a
credential in a prompt, command line, log, or repository file.

Resume only missing, failed, or hash-stale sessions:

```bash
python3 "$ARTIFACT/run_all.py" --workers 6 --timeout-seconds 1200
python3 "$ARTIFACT/finalize.py"
```

Valid current receipts are skipped. To limit execution to one network:

```bash
python3 "$ARTIFACT/run_all.py" --network xrpl --workers 6
python3 "$ARTIFACT/run_all.py" --network postfiat --workers 6
```

!!! danger "Do not force-rerun a published corpus"

    `--force` overwrites every packet, log, stderr capture, and receipt in
    the selected set. Never use it in the published directory. Create a new
    dated successor corpus instead.

If one validator fails, retain the failed receipt for diagnosis, correct only
the operational cause, and rerun without `--force`; valid sessions will skip.
Do not manually repair model output and claim it is the logged final answer.

## 7. Create a successor corpus

Create a successor when the validator set changes, public identity evidence
materially changes, the prompt/model changes, a packet is disputed, or a
publication defect is found. Never rewrite the dated published directory.

1. Create `validator-identity-packets-YYYYMMDD-vN/`.
2. Freeze a newly collected `inputs/validators.json` and record its SHA-256,
   collection time, network/list source, and completed PostFiat round.
3. Copy the harness, then explicitly update its artifact name, expected source
   hash, expected counts, network labels, model/version metadata, and source
   path. Do not weaken the strict verifier to make a result pass.
4. Review the prompt template before any paid execution. A prompt change creates
   a new experiment and therefore a new corpus.
5. Run `build_prompts.py`; inspect coordinate and prompt diffs before running
   Corbanu.
6. Run a one-validator canary in an unpublished scratch copy. Confirm heading
   order, business-summary length, JSON agreement, evidence quality, privacy,
   and log completeness.
7. Run the full corpus, finalize it, and complete independent human review.
8. Publish the successor beside the prior corpus with an explicit supersession
   note. Preserve both sets of hashes.

The current scripts intentionally pin the 2026-09-01 source hash and 55-row
count. A simple directory copy is not sufficient for a new validator set.

## 8. Publication gate

Before committing a new corpus:

```bash
python3 "$ARTIFACT/finalize.py"
git add "$ARTIFACT"
scripts/public-secret-scan
scripts/docs-site-redaction-check
mkdocs build --strict
git diff --cached --check
```

Generated Markdown may intentionally contain two trailing spaces for a Markdown
line break. Review any `git diff --cached --check` finding; do not normalize a
generated packet unless the corresponding logged final message, receipt hashes,
indexes, and manifest are regenerated consistently.

The publication reviewer must confirm:

- strict verification is `PASS`;
- every input has exactly one packet, JSONL log, stderr capture, and receipt;
- stderr captures are empty;
- all complete logs, not just final packets, are safe to publish;
- no packet presents a score or a consensus decision;
- unknown identities remain unknown;
- hashes in the release note match `manifest.json`; and
- repository changes contain no unrelated files.

## 9. Correction and dispute handling

Do not edit a published packet in place. That would break packet/log equality,
the receipt, aggregate hashes, and the audit trail.

For a reported error:

1. identify the validator key and exact packet SHA-256;
2. classify the issue as factual error, weak attribution, stale evidence,
   privacy concern, or formatting/verification defect;
3. immediately remove the affected artifact from downstream scoring if it may
   expose private data or misidentify a person or entity;
4. preserve the original privately for audit when legally and ethically
   permissible;
5. produce a dated successor packet/corpus from corrected minimal coordinates
   and a new logged session;
6. have a different reviewer approve the correction; and
7. publish a supersession record linking old and new hashes without repeating
   removed private information.

A validator operator may provide public correcting evidence, but may not demand
a favorable size tier, identity conclusion, or later reputation score.

## 10. H200 handoff

The scoring operator receives only:

- exact Markdown bytes from `packets/<network>/<key>.md`;
- network and validator key;
- that packet's SHA-256 from `index.json`; and
- the corpus packet-set hash from `manifest.json`.

The scoring request must bind the packet hash. It must not use the mutable
`index.md` display text, Corbanu logs, an operator-supplied nickname, or a
fresh web search as a substitute for the packet.

Before replay, verify every packet hash against `index.json`. After replay,
record the identity-corpus manifest hash with the scoring results so that the
scores can be traced to the exact profile bytes.

Identity packets and model scores remain external `SHADOW_ONLY` evidence
unless a separately reviewed protocol/governance activation explicitly changes
that boundary.

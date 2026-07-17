# PostFiat.org Dynamic UNL Whitepaper Notes

Status: notes only
Date: 2026-05-22
Scope: local reading notes for the PostFiat.org governance whitepaper and the
Dynamic UNL scoring addendum. This is not an implementation plan and not an
edit to the Rust L1 whitepaper.

## Source Material Read

- Local PostFiat.org whitepaper source:
  `$POSTFIATORG_REPO/content/whitepaper.md`
- Local whitepaper drafts and lab notes:
  `$POSTFIATORG_REPO/docs/whitepaper_drafts/`
  and `$POSTFIATORG_REPO/docs/whitepaper_score_labbook.md`
- Dynamic UNL scoring repo:
  `$DYNAMIC_UNL_REPO`
- User-provided addendum/review in chat on 2026-05-22.

## Correct Mental Model

The PostFiat.org whitepaper is a governance-process whitepaper. It is about
auditable, model-assisted validator-list publication for XRPL-derived networks:
evidence collection, canonical normalization, model/runtime manifesting,
validator scoring, deterministic list construction, signed VL publication,
audit bundles, memo anchoring, shadow verification, and eventual authority
transfer.

This is separate from the `postfiatl1v2` Rust chain whitepaper and should not
be conflated with the Rust L1 implementation work. The governance whitepaper's
claim surface is the validator-list publication process, not transaction
execution, privacy circuits, Cobalt implementation, wallet RPCs, or other Rust
chain internals.

## Whitepaper Thesis As Read

The local PostFiat.org whitepaper frames validator-list publication as a real
governance surface. In XRPL-style networks, recommended validator lists are
security-critical because they influence trusted-set overlap and therefore the
consensus safety envelope.

The paper argues that opaque publisher discretion should be replaced by a
public pipeline:

1. collect validator evidence;
2. normalize evidence into a canonical scoring snapshot;
3. pin the model/runtime/scoring policy;
4. score validators under a published prompt;
5. select the UNL deterministically with churn controls;
6. publish signed validator-list artifacts;
7. anchor the audit bundle;
8. shift authority only after measured validator-side convergence.

The paper's narrow claim is comparative: a public, replayable judgment layer is
more auditable and contestable than an unpublished rubric or informal publisher
process. It does not claim that model scoring is inherently correct or that
authority transfer is already justified.

## Important Addendum State

The addendum says the whitepaper should be updated before treating it as
current. That recommendation is directionally right. The local whitepaper is
architecturally aligned with Dynamic UNL, but some implementation details are
now stale or need sharper Phase 1 versus Phase 2 framing.

Current implementation state from the addendum and local
`dynamic-unl-scoring` source:

- Phase 1 is live: the foundation scores validators, signs the validator list,
  and publishes the canonical VL.
- The canonical public testnet VL is served from
  `https://postfiat.org/testnet_vl.json`.
- Validators consume the signed VL through the standard validator-list
  mechanism, not primarily from the scoring service endpoint.
- The active scoring prompt is `prompts/scoring_v5.txt`.
- The active model/runtime is `Qwen/Qwen3.6-27B-FP8` through Modal/SGLang on
  H100 infrastructure.
- The service collects VHS, crawl, ASN, and DB-IP Lite evidence, then performs
  deterministic scoring and deterministic UNL selection.
- Phase 2 M2.0 defines staged verifier-ready audit bundles and execution
  manifests.
- Validator-side shadow verification is not live yet. Sidecars, commit-reveal,
  validator-owned scoring, convergence monitoring, and authority transfer are
  roadmap items.

## Whitepaper Details That Need Future Update

### Prompt And Model

The whitepaper already uses `Qwen/Qwen3.6-27B-FP8` in several places, but still
contains stale references to the active PFT Ledger `scoring_v2` contract. The
current implementation uses `prompts/scoring_v5.txt`, with `PromptBuilder`
pointing at that file in `dynamic-unl-scoring`.

Future whitepaper language should say the active service uses
`prompts/scoring_v5.txt` with `Qwen/Qwen3.6-27B-FP8` served through the pinned
Modal/SGLang H100 profile. Earlier Qwen or RunPod references should be framed
as historical benchmark context.

### Artifact Layout

The whitepaper's round bundle section still presents an older flat artifact
layout:

```text
raw/
snapshot.json
execution_manifest.json
scoring_prompt.txt
scores.json
selection_result.json
vl.json
metadata.json
```

The newer Dynamic UNL Phase 2 contract uses staged verifier-ready paths such as:

```text
bundle.json
inputs/validator_evidence.json
inputs/model_request.json
inputs/validator_map.json
runtime/execution_manifest.json
outputs/model_response.json
outputs/validator_scores.json
outputs/selected_unl.json
outputs/signed_validator_list.json
outputs/verification_hashes.json
```

Future whitepaper edits should distinguish old Phase 1/legacy flat artifacts
from the current main-branch/M2.0 staged bundle contract. If the public testnet
round API has not fully exposed the staged paths yet, phrase staged bundles as
the main-branch/M2.0 verifier-ready contract rather than as already universal
public endpoint behavior.

### Phase 1 Versus Phase 2

The whitepaper correctly says Phase 1 keeps foundation authority and Phase 2
measures independent execution. The risk is over-reading Phase 2 descriptions
as live behavior.

Safer framing:

- Live now: foundation-operated scoring, deterministic UNL selection, signed VL
  publication, GitHub Pages distribution, IPFS/Pinata audit publication, and
  PFTL memo anchoring.
- Partially implemented: Phase 2 M2.0 staged bundle and execution manifest
  work.
- Not live yet: validator sidecars, frozen input lifecycle, commit-reveal,
  validator-side model execution, convergence reporting, and
  validator-enforced scoring verification.

### Canonical VL Publication

The canonical testnet VL publication path is:

```text
https://postfiat.org/testnet_vl.json
```

The scoring service can expose debug/tooling copies, but validators should be
described as consuming the standard signed VL at `postfiat.org/{env}_vl.json`.
The current VL format also includes an `effective` timestamp lookahead so
validators can cache pending blobs and activate them at the coordinated time.

### Memo And Audit CID Language

Future wording should be tolerant of both legacy and newer field names. Older
or live public surfaces may expose `ipfs_cid`; newer Phase 2 code and docs use
`final_bundle_cid` for the final audit bundle CID.

The intended claim: completed rounds anchor the final audit bundle CID, signed
VL sequence, and round metadata through a PFTL memo. Memo failure should be
treated as an operational exception; the service can reach states where VL
publication succeeds while memo publication fails.

### Evidence And Geolocation

The current evidence pipeline uses:

- VHS validator and topology data;
- `/crawl` evidence including `pubkey_validator`;
- ASN enrichment through pyasn;
- country-level geolocation through DB-IP Lite.

Do not describe MaxMind as current unless the context is historical. Do not
overstate geolocation precision. Country-level geolocation is a public diversity
signal derived from endpoint evidence, not proof of operator jurisdiction or
physical validator location.

### Identity Signals

The active prompt treats missing formal identity as neutral and uses public
operational/accountability evidence where available. The whitepaper should not
imply that KYC, formal identity verification, or a fully deployed identity
attestation system is currently part of scoring unless explicitly marked as
future work.

Safer formulation: identity signals are currently limited to public operational
and accountability evidence; missing formal identity is not automatic negative
evidence.

### Authority Transfer

Authority transfer is not active. The foundation remains the canonical VL
publisher during Phase 1 and Phase 2. Phase 2 is meant to build confidence
through shadow verification. Authority transfer is a later Phase 3 objective
that depends on Phase 2 convergence and governance readiness.

### Override And Fallback Behavior

Admin override paths are temporary Phase 1/2 safety scaffolding, not the
long-term governance model. Override rounds should be auditable and clearly
distinguished from normal automated scoring rounds.

Fallback language should preserve continuity first: missed rounds, failed
publishes, stale manifests, or convergence drops should fall back to the last
known-good list or foundation publication. Avoid implying that a memo failure
universally blocks VL publication.

## Management Summary For Future Whitepaper Work

The PostFiat.org whitepaper should receive a targeted update, not a full
rewrite. It remains right at the architectural and governance-process level,
but it is stale around prompt version, artifact bundle shape, geolocation
source, canonical VL distribution, and Phase 2 deployment status.

Highest-risk issue: readers may conclude validator-side shadow verification,
commit-reveal, sidecars, or authority transfer are already live. They are not.

Recommended update scope:

- Replace `scoring_v2` current references with `prompts/scoring_v5.txt`.
- Keep `Qwen/Qwen3.6-27B-FP8` as the active model and frame earlier model/runtime
  references as historical.
- Replace or qualify the old flat artifact list with the staged Phase 2 bundle
  layout.
- State clearly that validator-side shadow verification, commit-reveal, and
  sidecars are not live yet.
- Clarify canonical testnet VL publication at
  `https://postfiat.org/testnet_vl.json`.
- Update geolocation language to DB-IP Lite and caveat it as country-level
  endpoint evidence.
- Clarify that foundation authority remains in place until a later Phase 3
  transfer decision.

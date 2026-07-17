# PostFiat Verifiable Constitution

Status: canonical human-readable constitution for the current implementation lane.
Date: 2026-05-25

This document is the readable Constitution. The VC reports, schemas, receipts,
and gate files are evidence for this document. They are not the thing a reviewer
should have to read first.

## Core Claim

PostFiat governance should not depend on an opaque foundation process, private
committee judgment, or unrecorded founder discretion.

The constitutional rule is:

> A protocol or validator-governance decision may become the default PostFiat
> position only when the question, evidence, model/runtime profile, output,
> replay evidence, conformance mapping, challenge route, and release or Cobalt
> gate are all public, typed, hash-bound, and machine-checkable.

This does not claim that an AI answer is automatically correct. It claims that a
governance judgment that used to be private must become inspectable, replayable,
challengeable, and hard to override silently.

## Current Live Qwen Decisions

These are the live Qwen decisions currently recorded by the Verifiable
Constitution lane. They are all no-live-effect decisions: they do not mutate
mainnet, the validator registry, Cobalt state, model authority, or any live
release gate.

Important distinction: if Qwen selects `hold-no-op`, the existing
implementation still remains in place by ordinary status quo. It means the
model did not authorize a constitutional change or an affirmative preserve
decision from that packet. Fixture baselines are not Qwen outputs.

| Area | Live Qwen answer | Plain English effect | Evidence status |
| --- | --- | --- | --- |
| Protocol architecture | `preserve-rust-l1` | Keep PostFiat as a Rust L1. Do not fully fork `rippled`; do not activate an XRPL compatibility layer now. | H100/H200 live replay plus full-vocabulary vector root. |
| Privacy | `hold-no-op` | Do not switch privacy design. Orchard/Halo2 remains the implementation status quo, but this live Qwen packet did not select the fixture's `preserve-orchard-halo2` answer. | H100/H200 live replay plus prior full-vocabulary vector root. |
| Post-quantum authorization | `hold-no-op` | Do not change authorization policy. ML-DSA-style authorization remains the implementation status quo, but this live Qwen packet did not select the fixture's `preserve-ml-dsa-baseline` answer. | H100/H200 live replay with matching top-logprob root. |
| Monetary policy | `preserve-fixed-supply-fee-burn` | Keep fixed supply, fee burn, and no native validator reward schedule. | H100/H200 live replay with matching top-logprob root. |
| Cobalt governance | `preserve-cobalt-hard-gates` | Keep linkedness, essential subsets, registry-transition gates, stale replay rejection, dry-run no-mutation checks, replay bundles, and override detection as hard gates. | H100/H200 live replay with matching top-logprob root. |
| Model governance | `preserve-current-profile` | Keep the current Qwen/SGLang deterministic profile until a later model-governance packet proves a replacement. | H100/H200 live replay with matching top-logprob root. |
| Validator evidence and UNL selection | `hold-no-op` | Do not author a new UNL evidence request or mutate validator selection from this packet. The validator evidence contract remains the implementation status quo. | H100/H200 live replay with matching top-logprob root. |

The practical answer is: the system stays on the current Rust L1,
Orchard/Halo2 privacy, ML-DSA-style authorization, fixed-supply fee-burn
monetary policy, hard Cobalt gates, and Qwen/SGLang profile. The reason differs
by card. Some were affirmative Qwen preserve decisions; some were `hold-no-op`
decisions that leave the existing implementation untouched because no live gate
authorizes a change.

Do not ship native validator rewards, do not switch to proof-of-stake
economics, do not fork `rippled`, do not replace Orchard with attestation
privacy, do not promote a new model, and do not mutate the validator registry
until later evidence, replay, challenge, conformance, and live-effect gates
pass.

## Actual Run Ledger

The 2026-05-25 live suite ran `Qwen/Qwen3.6-27B-FP8` through SGLang with TP=1,
temperature zero, deterministic inference enabled, and max running requests set
to one. Each listed question ran three times on a Vast H100 NVL instance and
three times on a Vast H200 instance. The rented machines were destroyed after
the reports were written.

| Question | Live answer | Cross-machine report | Parsed-output hash | Top-logprob root | Class |
| --- | --- | --- | --- | --- | --- |
| Privacy architecture | `hold-no-op` | `reports/verifiable-constitution/vc-095-cross-machine-privacy-report.json` | `29b94f774e6a827b2461146472349527d2ce7c5b12e875668bd7465d3a854022518980e178787c753f01ed5df2b14cbf` | `f168514f56ee4e10bf1bbb2f634804bf956c68699fca2fd08b77ac146298223f6b1f8038286fbbfd17ccc8acf5db9d1a` | `green-top-logprob` |
| Post-quantum authorization | `hold-no-op` | `reports/verifiable-constitution/vc-095-cross-machine-pq-report.json` | `8d63ef03ef99fe486a88a2bfc1ea7f6f29e4d431aa53679bf7cd698c08409d087e8661fc924d05cee553b49ecaf87d57` | `dc9fe5e3e2724ef9c181c2d766d15a5835430b731a3c9474916f201494994f8309298f0891179ee7c37486f09e2e5f17` | `green-top-logprob` |
| Monetary policy | `preserve-fixed-supply-fee-burn` | `reports/verifiable-constitution/vc-095-cross-machine-monetary-report.json` | `e6fe2975586342740150e6cb6e74113a8fef3d2ce3c44b53846c5db959f227c00b927b9dcd62a5b164af38548b4ae7ce` | `cb09c3c566a73abdd574042d0b0a2bb3ca93787833bfe3e9d10254b10aa16e87c38851aebb37df84e01199ec5f2e0ff9` | `green-top-logprob` |
| Model governance | `preserve-current-profile` | `reports/verifiable-constitution/vc-095-cross-machine-model-report.json` | `8ffc3d6e300509a2f4c77f4bc9af26107267b205efd47bb1d3a0f42b4e7f57db697e963625b544491e267e95bdf820b0` | `cf7e48eab3c58c38e985cc3704c4f82794810941af61ae4f9b706484939b95ef65871a7b91b65d028ed4bedbdb888944` | `green-top-logprob` |
| Cobalt governance | `preserve-cobalt-hard-gates` | `reports/verifiable-constitution/vc-095-cross-machine-cobalt-report.json` | `34f04cfaec98f7edbaf1ef293be2bd1ebdeeb2e17eb4e669160049072ee079efb107bc7258b1db61744df33e5470a749` | `2b660c2c65181c506712070ebac13630ff00cc63a05eb9d0e65ac00a9c2128b4e335afeb2de27b9ca27c23997b55a0a5` | `green-top-logprob` |
| Validator evidence and UNL selection | `hold-no-op` | `reports/verifiable-constitution/vc-095-cross-machine-validator-report.json` | `79c3a432f9502016eb53a82e1f914fad7928ebb7258ab34c95cd0b5d35ce14e6db34d2c70c9d876b9618ff6e12ea5094` | `3a5e1ee1c73ac4c899a37edd010e3329f2008fa1eb54e1aab79189608c9dec0aa769fc9a39ea5d8079ca5a898c594f2b` | `green-top-logprob` |

`green-top-logprob` means the H100 and H200 chat runs matched on parsed JSON,
raw content, prompt hash, chat request hash, and top-logprob commitment root.
It is real model execution evidence. It is not a full-vocabulary proof. The
full-vocabulary lane remains a stricter proof class and currently exists for
the protocol-architecture and privacy examples.

## Replay Equivalence Invariant

The selector does not consume raw provider responses. It consumes the
schema-valid parsed-output root:

```text
parsed_output_root = SHA3-384(canonical_json(validate(schema, parsed_output)))
```

Raw response hashes are audit evidence only. The VC-111 verifier proves why the
distinction matters: the six H100/H200 Qwen/SGLang question groups converge on
the same parsed-output roots and top-logprob roots, while raw response hashes
differ across the six runs. A raw-envelope split is not selector divergence. A
parsed-output-root split is selector divergence and forces hold/no-op.

Verifier:

```text
scripts/verifiable-constitution-replay-equivalence-invariant --verify-report \
  --output reports/verifiable-constitution/vc-111-replay-equivalence-invariant-report.json
```

Report hash:

```text
df62d9961809758c4b213fd460f79991d6bab39a1429586aa863b1a517219e2b5ccc3edbeaee26fd5863616731f76ffb
```

## Decision Card Format

Each constitutional decision should be readable in this order:

1. Summary of evidence packet in plain English.
2. Run status.
3. Feeder JSONs.
4. Question.
5. Choices in the question packet.
6. Live Qwen answer or fixture-only status.
7. Response and rationale.
8. Verification hashes.
9. Authority boundary.

The detailed JSON packets and reports are the audit trail. The decision card is
the human-facing Constitution.

## Decision Card: Protocol Architecture

### Summary Of Evidence Packet

The evidence packet says PostFiat is already implemented as a Rust L1, not a
`rippled` binary fork. The current architecture includes Cobalt governance,
ML-DSA-style authorization, Orchard/Halo2 privacy, fixed supply, fee burn, no
native validator reward schedule, and XRP-style wallet/RPC compatibility goals.

The packet also says a full XRPL/`rippled` fork would need new evidence proving
it can preserve PostFiat's post-quantum authorization, privacy, Cobalt
governance, operator migration, rollback, and release-gate requirements. It
records that an XRPL compatibility layer may be useful later, but no packet yet
authorizes implementing or activating one.

### Run Status

This is a real Qwen decision. H100/H200 replay selected `preserve-rust-l1` and
the stricter full-vocabulary lane matched on the 248,320-entry vector root. It
still grants no live effect.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_evidence_packet.json` |
| Prompt contract | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_decision_replay_prompt_fixture.json` |
| Live chat prompt manifest | `reports/verifiable-constitution/protocol-architecture-live-replay/prompt_manifest.json` |
| Full-vocabulary prompt manifest | `reports/verifiable-constitution/protocol-architecture-full-vocab-replay/prompt_manifest.json` |
| Qwen output report | `reports/verifiable-constitution/vc-053-protocol-architecture-superseding-replayed-decision-report.json` |
| Replayed decision packet | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_superseding_replayed_decision_packet.json` |

### Q&A

**Question:** Should PostFiat remain its own Rust L1, fully fork
XRPL/`rippled`, add a bounded XRPL compatibility layer, or hold/no-op?

**Choices in the question packet:**

- `preserve-rust-l1`
- `fully-fork-xrpl`
- `hybrid-xrpl-compatibility-layer`
- `hold-no-op`

**Answer:**

```text
preserve-rust-l1
```

**Response:** Keep PostFiat as its own Rust L1. Do not fully fork `rippled`.
Do not activate an XRPL compatibility layer yet. Preserve the current
architecture unless later evidence proves a change is safer and passes a later
release or Cobalt gate.

**Rationale:** The evidence packet supports the current Rust L1 status quo and
binds the existing Cobalt, ML-DSA, Orchard/Halo2, operator-readiness, and
release-gate evidence. The packet contains fork-delta and compatibility
evidence, but not enough implementation, migration, rollback, and release
evidence to authorize a full fork or compatibility activation.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `2f667c57807478582e6b2dfd4146a5ec60ec3f07354adae1200b2ebd746b1316cd11168ac06df421b61ed17fc08e7000` |
| Evidence-request file SHA3-384 | `5fdee24fa1e48cef8b727961250707bd8f04e0c5a32fdaf5d7a1b6f387e953f74106e1b6f4f08e9f91794a001032a6bd` |
| Evidence-packet file SHA3-384 | `46a4bbfcbc913e28a813211e1cf7437c8f9987e785bc334f0bd93cda53581e572527bb21b243bf71b1bab674680a808e` |
| Evidence root | `d9f1d159b3be6b34376c937a8cce10a08b457c7e13dbf73758c95c80e8b5833249a162ab408d2eb717fa0f1203c77c40` |
| Live chat prompt-manifest SHA3-384 | `a6fd5465c7124ad1d520ebeb3287e1461c81d64f7f9b28a3c4828305ca054f58b4c4b566667d32e8980ee0607b97dbfb` |
| Full-vocabulary prompt-manifest SHA3-384 | `6fca2130ee1727d00f2d72b4ea5ffc015d87c8b773c26d4ad9e4e8cadaba139a27898115e21e7641a02c3920695b3dcb` |
| Full-vocabulary prompt hash | `8d9e1473fe96155b123fc408e58ce2cae63bdd9a3473a6a35d78918e339e9a7148c9bad4ee6560994fad893a847685e5` |
| Full-vocabulary prompt payload hash | `d94d334b3db45b933e40c364e0fe6c0bc1d478935e838ff051c7428a3fa3d6f91c75993d2aeaa46d0b0d1f2ba7a3e51a` |
| Live parsed decision hash | `36bb5c03f73701ae71811a7b86b3228f0f23a640b802b7562fd054fc7b4b5e21bb875e1b27d2756cf16e6c8668e0c9b3` |
| Top-logprob root | `101dff18a4354fd29ae851c1efd595aaaa69d6961e54482b47abceaae1b3499a5d25116822dec3343024b3054a2a7529` |
| Full-vocabulary vector root | `0b3cf64b186ff9b8b130260d6a795fe56cf2a65bd6e324ea9ce92222ed1d7c23` |
| Replayed decision packet SHA3-384 | `3c044557717fe4dec8fd0bdeb7b1430d10d4878de3f1b7f4ef65955c40b0379a9c7fd237f5f716214ec4d3291051b034` |
| Superseding decision root | `4b756a052813aa468f63004048aa7cc04b35b580eb7f56855f0060bfaa5209eec9b36732f867ac1f36682661f2809df0` |
| Qwen output report SHA3-384 | `528308474df6b4d9d1d0c9ae7e1235e6d4a970d9972afe9ccf1e30dd518f48d5a162a1038b382cc2c240a43646cac7f2` |

### Authority Boundary

This card authorizes no code rewrite, no `rippled` import, no compatibility
activation, no Cobalt submission, no mainnet mutation, no validator-registry
mutation, and no authority transfer. It is a replay-bound constitutional
artifact for planning and later challenge/conformance gates.

## Decision Card: Privacy Architecture

### Summary Of Evidence Packet

The evidence packet says the current privacy baseline is Orchard/Halo2-style
shielded settlement: transparent-to-Orchard deposit, private spend, withdraw,
local scanning, selective disclosure packets, bounded RPC handling, and audit
evidence. Attestation-style privacy is treated as a future alternative, not as
the implementation that exists now.

### Run Status

This is a real Qwen decision. The 2026-05-25 H100/H200 live run selected
`hold-no-op`. That means the model did not affirmatively select the fixture
baseline `preserve-orchard-halo2`. Orchard/Halo2 remains the implementation
status quo because no live gate authorizes changing it.

The earlier privacy full-vocabulary run also selected `hold-no-op` and matched
across H100/H200 on the full-vocabulary vector root.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/privacy_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/privacy_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/privacy_constitutional_evidence_packet.json` |
| Fixture decision packet, not Qwen output | `docs/governance/agent/fixtures/verifiable_constitution/privacy_constitutional_decision_packet.json` |
| 2026-05-25 H100 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-privacy-h100-report.json` |
| 2026-05-25 H200 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-privacy-h200-report.json` |
| 2026-05-25 cross-machine report | `reports/verifiable-constitution/vc-095-cross-machine-privacy-report.json` |
| Prior full-vocabulary cross-machine report | `reports/verifiable-constitution/vc-022-cross-machine-model-receipts-report.json` |

### Q&A

**Question:** Should PostFiat preserve Orchard/Halo2 privacy, replace it with
attestation-style privacy, adopt a hybrid disclosure design, or hold/no-op?

**Choices in the question packet:**

- `preserve-orchard-halo2`
- `adopt-attestation-privacy`
- `hybrid-orchard-attestation-disclosure`
- `hold-no-op`

**Live Qwen answer:**

```text
hold-no-op
```

**Response:** Do not change privacy architecture from this packet. Do not
replace Orchard/Halo2 with attestation privacy, and do not add a hybrid
disclosure layer, because the packet does not supply the comparative evidence
needed to prove those changes are safer.

**Rationale:** Qwen treated the change evidence as missing. It rejected
attestation privacy and hybrid disclosure because they require new comparative
evidence. It also rejected the fixture's affirmative `preserve-orchard-halo2`
answer as less precise than `hold-no-op` for this no-live-effect packet.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `61ca07f1a36e8fe8c91127fb6f51ccd0d54861e083a171646d7a050d50c200113498dc34811c771b5d9bf850a3172abb` |
| Evidence-request file SHA3-384 | `1cc9092989711008b8a67f72d7cd6cd1648224e01807927474c5a38655796f1e07babdaaa75eb40799d7ab35a832dbd9` |
| Evidence-packet file SHA3-384 | `21504b2736c61c6961c05d612f84789b3a8c55caa3af459d36a95ed4333b79f7ebc7e751ccadad4a25c254bce5814fee` |
| Fixture decision-packet file SHA3-384, not Qwen output | `37b249e2c37764d4dfefd1690cdcaf7055101f1640984f882430f01726f2e3e9342b23153a725b4d9da54892cd1bc68f` |
| Evidence root | `b79321ac0df5cebd3fef5828e6994e1699dac86886e52f247e452a3cb92916c83a044fb9c897516ef019c41edc80634e` |
| 2026-05-25 parsed output hash | `29b94f774e6a827b2461146472349527d2ce7c5b12e875668bd7465d3a854022518980e178787c753f01ed5df2b14cbf` |
| 2026-05-25 top-logprob root | `f168514f56ee4e10bf1bbb2f634804bf956c68699fca2fd08b77ac146298223f6b1f8038286fbbfd17ccc8acf5db9d1a` |
| 2026-05-25 cross-machine receipt root | `05a45345355b25304bce2fc6e7bef18e27ab7c98bfbf2b3c4e6ce9551639566f62b5267c08d2b58439c5f3835d469f40` |
| Prior full-vocabulary vector root | `47937e0f19da588d92d46b677de95070134d1203575272add0f2af9d024361bc` |
| Prior full-vocabulary cross-machine receipt root | `ea19d6788e18f947b0f4156b0df7ffae2a38915a1b154f20a5c7632a968c99cddc91ce5e69f36c3ad296b12d5051b724` |

### Authority Boundary

This card authorizes no privacy-parameter mutation, no Cobalt submission, no
mainnet mutation, no model promotion, no sidecar activation, and no authority
transfer.

## Decision Card: Post-Quantum Authorization

### Summary Of Evidence Packet

The evidence packet says the current authorization baseline is ML-DSA-style
post-quantum account and validator authorization from genesis. It accepts larger
signatures and certificates as a design cost, with bounded certificates,
resource pricing, wallet/custody design, and key-rotation policy as the
mitigation path.

### Run Status

This is a real Qwen decision. The 2026-05-25 H100/H200 live run selected
`hold-no-op`. ML-DSA-style authorization remains the implementation status quo
because no live gate authorizes changing it.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/pq_authorization_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/pq_authorization_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/pq_authorization_constitutional_evidence_packet.json` |
| Fixture decision packet, not Qwen output | `docs/governance/agent/fixtures/verifiable_constitution/pq_authorization_constitutional_decision_packet.json` |
| 2026-05-25 H100 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-pq-h100-report.json` |
| 2026-05-25 H200 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-pq-h200-report.json` |
| 2026-05-25 cross-machine report | `reports/verifiable-constitution/vc-095-cross-machine-pq-report.json` |

### Q&A

**Question:** Should PostFiat preserve ML-DSA-style post-quantum
authorization, alter certificate/key-rotation policy, defer post-quantum
authorization, or hold/no-op?

**Choices in the question packet:**

- `preserve-ml-dsa-baseline`
- `alter-certificate-and-rotation-policy`
- `defer-pq-authorization`
- `hold-no-op`

**Live Qwen answer:**

```text
hold-no-op
```

**Response:** Do not change authorization policy from this packet. Do not defer
post-quantum authorization, and do not change certificate or key-rotation
policy without a later evidence packet.

**Rationale:** Qwen treated the current ML-DSA baseline as already active
status quo and selected the no-op answer rather than an affirmative preserve
decision. It rejected deferral and certificate/key-rotation changes because the
packet did not provide evidence that either would be safer.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `c4664369e1303ea03f016e54f381aa96a04ad37b3d340973010f70c499fa0ded51a09bfac038a2e228f5352ca5c221cf` |
| Evidence-request file SHA3-384 | `211791b19330952885daabff4c0b20a6746c5c59eee26bd565fbf95f9b341ada3e1368f550e854dec696270b4cd56dfe` |
| Evidence-packet file SHA3-384 | `c76ccfc0529b5828bc0274804b9f0fc03456471434ca291f5b599dce25dcb87a161e5dbc985fcab3dc02df7f067afec8` |
| Fixture decision-packet file SHA3-384, not Qwen output | `24cf2fed401ccfab16667ea53f8d7af8e64ae7eb3e2525e8156b3e638285641396b19828566b24b7bc6ec6a050772194` |
| Evidence root | `cc5d6ec6cc9995b947010b0e9186cb1dd705e42889313214e54ad9fb253a1e5801a4c5ab0e6bc65c75044e02996011d6` |
| 2026-05-25 parsed output hash | `8d63ef03ef99fe486a88a2bfc1ea7f6f29e4d431aa53679bf7cd698c08409d087e8661fc924d05cee553b49ecaf87d57` |
| 2026-05-25 top-logprob root | `dc9fe5e3e2724ef9c181c2d766d15a5835430b731a3c9474916f201494994f8309298f0891179ee7c37486f09e2e5f17` |
| 2026-05-25 cross-machine receipt root | `fb0f39021f0ff729a6c761a9294a1aff54476727cef8f6bd3538e1014b98c563c0be91593e4d891f78d20a370e5dd023` |

### Authority Boundary

This card authorizes no live account-authorization mutation, no live
validator-authorization mutation, no live Cobalt submission, and no authority
transfer.

## Decision Card: Monetary Policy

### Summary Of Evidence Packet

The evidence packet says the current monetary-policy baseline is fixed supply,
fee burn, and no native validator reward schedule. Fees price spam, state
growth, verifier work, and network resources; they are burned rather than
redirected into mining, staking, or validator yield.

### Run Status

This is a real Qwen decision. The 2026-05-25 H100/H200 live run selected
`preserve-fixed-supply-fee-burn`.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/monetary_policy_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/monetary_policy_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/monetary_policy_constitutional_evidence_packet.json` |
| Decision packet | `docs/governance/agent/fixtures/verifiable_constitution/monetary_policy_constitutional_decision_packet.json` |
| 2026-05-25 H100 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-monetary-h100-report.json` |
| 2026-05-25 H200 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-monetary-h200-report.json` |
| 2026-05-25 cross-machine report | `reports/verifiable-constitution/vc-095-cross-machine-monetary-report.json` |

### Q&A

**Question:** Should PostFiat preserve fixed supply, fee burn, and no native
validator rewards; alter fee-burn accounting; add native validator rewards; or
hold/no-op?

**Choices in the question packet:**

- `preserve-fixed-supply-fee-burn`
- `alter-fee-burn-accounting`
- `add-native-validator-reward`
- `hold-no-op`

**Live Qwen answer:**

```text
preserve-fixed-supply-fee-burn
```

**Response:** Keep fixed supply, fee burn, and no native validator reward
schedule. Do not add proof-of-stake-style validator yield, inflation, or fee
redistribution without a later evidence packet and explicit governed gate.

**Rationale:** The approved evidence packet supports preserving the current
monetary status quo. It does not contain evidence sufficient to alter fee-burn
accounting or add a native validator reward schedule.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `1c89a929f700b5ed7de672e568d769221cf017dcf775d8a67db64091acf8518ea62aa81204362ecdebaddeff74ded51d` |
| Evidence-request file SHA3-384 | `3ffd4f5260578b4e9266a0caddf6186389576b1d3f2f06e066e55357d715ea6687ae63d0bea469666d56b5f1a7a915ff` |
| Evidence-packet file SHA3-384 | `2e5c3507d6a736c7f198b927c8ebb09a2cbddb97a18403b946720f09c9244828b5b2a0709b13d878dcb64c4edc90c4be` |
| Decision-packet file SHA3-384 | `bfdc089e6fc334cbdb2bd2e7294dc6f14629c1eac9f31c8744a9ed792a7d1e1f56e9d62879b0528f23e19ff8f6779fee` |
| Evidence root | `c98e850f270cd97c5db858ce54754289d2960b4d9d26d0836708118fc5e3b703fc302a480b84a4a461e79869ad7fd394` |
| 2026-05-25 parsed output hash | `e6fe2975586342740150e6cb6e74113a8fef3d2ce3c44b53846c5db959f227c00b927b9dcd62a5b164af38548b4ae7ce` |
| 2026-05-25 top-logprob root | `cb09c3c566a73abdd574042d0b0a2bb3ca93787833bfe3e9d10254b10aa16e87c38851aebb37df84e01199ec5f2e0ff9` |
| 2026-05-25 cross-machine receipt root | `9b6b1c58e83bfb4a9e8d5cb5f2eea975ac5af9f81ea3a6639e40e117bf24235b503e6d76af408824693cf45dd4fbd127` |

### Authority Boundary

This card authorizes no live supply mutation, no live fee-accounting mutation,
no live validator-reward activation, no Cobalt submission, and no authority
transfer.

## Decision Card: Cobalt Governance

### Summary Of Evidence Packet

The evidence packet says Cobalt linkedness, essential-subset threshold checks,
validator-registry transition rules, stale replay rejection, dry-run no-mutation
checks, replay bundles, adversarial coverage, rollback evidence, and override
detection are hard gates before any validator-registry or trust-graph change can
become live authority.

### Run Status

This is a real Qwen decision. The 2026-05-25 H100/H200 live run selected
`preserve-cobalt-hard-gates`.

The older VC-016 internal replay was not a model execution; it replayed a
fixture decision output. It remains useful as a fixture-replay check, but the
live Qwen answer for this card is the 2026-05-25 provider run.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_evidence_packet.json` |
| Decision packet | `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_decision_packet.json` |
| Internal decision replay report | `reports/verifiable-constitution/vc-016-internal-decision-replay-report.json` |
| Cobalt dry-run binding report | `reports/verifiable-constitution/vc-019-cobalt-dry-run-binding-report.json` |
| 2026-05-25 H100 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-cobalt-h100-report.json` |
| 2026-05-25 H200 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-cobalt-h200-report.json` |
| 2026-05-25 cross-machine report | `reports/verifiable-constitution/vc-095-cross-machine-cobalt-report.json` |

### Q&A

**Question:** Which Cobalt trust-graph and validator-registry transition gates
should remain constitutional hard requirements?

**Choices in the question packet:**

- `preserve-cobalt-hard-gates`
- `alter-trust-graph-thresholds`
- `alter-registry-transition-gates`
- `hold-no-op`

**Live Qwen answer:**

```text
preserve-cobalt-hard-gates
```

**Response:** Keep linkedness, essential-subset thresholds,
registry-transition gates, stale replay rejection, dry-run no-mutation checks,
replay bundles, rollback/supersession, and override detection as hard Cobalt
requirements.

**Rationale:** The evidence packet supplies the current linkedness,
essential-subset, registry-transition, no-mutation, replay-bundle,
adversarial-coverage, rollback, and override-detection evidence. It does not
supply a safer replacement threshold packet or a Cobalt-ratified transition.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `27e81d431fcbd4af2aae368b146ee05ce8c195cd7aaff21e7007e82e2cbad5c3cbf9df5188e78e56bfb4ed8a79d39cd7` |
| Evidence-request file SHA3-384 | `9880de615d5ec4fe5784e2814cf8a0b0ce6df851d2b617ccd05f9fd33dca7a3f4a454275d32b8703032131a78c949725` |
| Evidence-packet file SHA3-384 | `9e2d6a480e1fba1c4953c8bce2e76248d4f8f4f05f9f262cd7dd53c56ee2a63be622c15e4f08f14041bcc1bd5a56030e` |
| Decision-packet file SHA3-384 | `3c4073c9b7eb9ead951f25bb1de56a6bb5d8d8661e663e1585c255dcd29df3be5ff3e3d09ed4c12fe4bd82b6d5a183c0` |
| Evidence root | `e9a54bd3bd6219341478578057e64c0903cf7c27145a90e6e6f57ec35e61ad71867a525b2d119dab79b8a06e944e6b22` |
| 2026-05-25 parsed output hash | `34f04cfaec98f7edbaf1ef293be2bd1ebdeeb2e17eb4e669160049072ee079efb107bc7258b1db61744df33e5470a749` |
| 2026-05-25 top-logprob root | `2b660c2c65181c506712070ebac13630ff00cc63a05eb9d0e65ac00a9c2128b4e335afeb2de27b9ca27c23997b55a0a5` |
| 2026-05-25 cross-machine receipt root | `2154e7f37b322310c0c3773f534816e22760dc8aa026d1ef6ff4ea76c0b3e8d3e0189c360ff9165279871059fa013650` |

### Authority Boundary

This card authorizes no live validator-registry mutation, no live trust-graph
activation, no live Cobalt submission, no sidecar activation, no commit-reveal
activation, and no authority transfer.

## Decision Card: Model Governance

### Summary Of Evidence Packet

The evidence packet says the current internal model profile is
`Qwen/Qwen3.6-27B-FP8` served through SGLang TP=1 with deterministic replay
settings. Candidate model admission evidence is missing, so model promotion
remains a future governed decision.

### Run Status

This is a real Qwen decision. The 2026-05-25 H100/H200 live run selected
`preserve-current-profile`.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/valid_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/valid_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/valid_constitutional_evidence_packet.json` |
| Decision packet | `docs/governance/agent/fixtures/verifiable_constitution/valid_constitutional_decision_packet.json` |
| 2026-05-25 H100 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-model-h100-report.json` |
| 2026-05-25 H200 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-model-h200-report.json` |
| 2026-05-25 cross-machine report | `reports/verifiable-constitution/vc-095-cross-machine-model-report.json` |

### Q&A

**Question:** Should PostFiat keep the current Qwen/SGLang deterministic model
profile, admit a candidate model now, freeze model changes, or hold/no-op?

**Choices in the question packet:**

- `preserve-current-profile`
- `admit-candidate-now`
- `freeze-model-profile`
- `hold-no-op`

**Live Qwen answer:**

```text
preserve-current-profile
```

**Response:** Keep the current Qwen/SGLang TP=1 deterministic profile for
internal constitutional replay. Do not promote a candidate model until its
identity, replay convergence, adversarial comparison, cross-machine receipts,
and rollback evidence are present.

**Rationale:** The packet proves the active Qwen/SGLang profile identity and
deterministic controls. Candidate-model admission fields are absent, so the
safe decision is to preserve the current profile and reject promotion.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `f40b8d50a697f92b16acc958decd9328cd83c2af33d3daa528654a1c885d2b9e0197c896c8335deb6b94cf8735fcd99b` |
| Evidence-request file SHA3-384 | `0b7162b4f8060a49bbba255c7388849c38e2fea0f932b9435750e6225dd3a4d8da3ac5e2db4f471cb5414db2e6d5871a` |
| Evidence-packet file SHA3-384 | `2440fd9d257cad3e10e4068899ad2f6f3f270583f1a42e25a884e3199c96697c7267fc6508980bc5f3c2f3845d0d88e3` |
| Decision-packet file SHA3-384 | `9581c668a383f21326f0ebaccf369c61e4f7ce3b63c73c965c6c0945054d8572c3fca3f2813e4ded27820a109349dd6a` |
| Evidence root | `094c5836c049e6394b1981d3e119f599af5f1a6af3429cb8f060d021b514c06b7aa1e515e07a7ffda54aadfa06097e51` |
| 2026-05-25 parsed output hash | `8ffc3d6e300509a2f4c77f4bc9af26107267b205efd47bb1d3a0f42b4e7f57db697e963625b544491e267e95bdf820b0` |
| 2026-05-25 top-logprob root | `cf7e48eab3c58c38e985cc3704c4f82794810941af61ae4f9b706484939b95ef65871a7b91b65d028ed4bedbdb888944` |
| 2026-05-25 cross-machine receipt root | `616ae3b87de785bc1c1d00b21859cd4ae9d0d9630ef73c9c983ee22200b6c2799592e1b2b9dd2724393b001474f20abb` |

### Authority Boundary

This card authorizes no model promotion, no live Cobalt submission, no mainnet
mutation, no sidecar activation, and no authority transfer.

## Decision Card: Validator Evidence And UNL Selection

### Summary Of Evidence Packet

The question says PostFiat has a typed validator-evidence packet schema,
reviewed field registry, field-weight sidecar boundary, ruleset binding,
open-question disposition, and Qwen/Cobalt internal validation plan. UNL
selection must cite registered public or redaction-safe fields and route any
selector or registry effect through deterministic conformance and Cobalt or
release gates.

### Run Status

This is a real Qwen decision. The 2026-05-25 H100/H200 live run selected
`hold-no-op`. It did not authorize Qwen to author a new UNL evidence request
from this packet, and it did not mutate validator selection.

### Feeder JSONs

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/validator_evidence_constitutional_question.json` |
| Question report | `reports/verifiable-constitution/vc-041-validator-evidence-constitutional-question-report.json` |
| 2026-05-25 H100 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-validator-h100-report.json` |
| 2026-05-25 H200 live report | `reports/verifiable-constitution/vc-095-live-model-receipt-validator-h200-report.json` |
| 2026-05-25 cross-machine report | `reports/verifiable-constitution/vc-095-cross-machine-validator-report.json` |

### Q&A

**Question:** What evidence-request lane should govern AI-assisted UNL selection
fields and deterministic selector constraints?

**Choices in the question packet:**

- `preserve-current-validator-evidence-contract`
- `author-qwen-evidence-request-over-registered-fields`
- `require-field-registry-revision-first`
- `hold-no-op`

**Live Qwen answer:**

```text
hold-no-op
```

**Response:** Do not author a new validator-evidence request or mutate UNL
selection from this packet. The current validator-evidence contract remains the
implementation status quo until a future packet identifies a concrete field gap
or supplies a safer registered-field request.

**Rationale:** Qwen found no specific deficiency in the current field registry
or packet schema. It rejected authoring a new evidence request because the input
did not identify a concrete gap. It rejected requiring a registry revision
because no revision-triggering change was being proposed.

### Verification Hashes

| Item | Hash |
| --- | --- |
| Question file SHA3-384 | `8a44f4cdc36cb0cce8a8d8eccb099990db01b9974f3c59236061ad138ea162827cd519e4cbe107301f17895ee6b963d1` |
| 2026-05-25 parsed output hash | `79c3a432f9502016eb53a82e1f914fad7928ebb7258ab34c95cd0b5d35ce14e6db34d2c70c9d876b9618ff6e12ea5094` |
| 2026-05-25 top-logprob root | `3a5e1ee1c73ac4c899a37edd010e3329f2008fa1eb54e1aab79189608c9dec0aa769fc9a39ea5d8079ca5a898c594f2b` |
| 2026-05-25 cross-machine receipt root | `17f27801afa5260c0dca610bf2b366cf5c8e4fd87e7e20de5956628d6e91dbe15d62c3b4855db32c25c0754222b824e3` |

### Authority Boundary

This card authorizes no live UNL publication, no validator-registry mutation, no
field-weight activation, no sidecar activation, no commit-reveal activation, no
Cobalt submission, and no authority transfer.

## Prompts And Evidence Packets

The driver stack is:

```text
context packet -> constitutional question -> evidence request -> evidence packet
-> prompt manifest -> Qwen output -> decision packet -> conformance/challenge gate
```

The global context is:

- `docs/governance/agent/fixtures/verifiable_constitution/initial_context_packet.json`

The older governance-agent dry-run prompt bundle is:

- `docs/governance/agent/fixtures/dry_run_model_request.json`
- `docs/governance/agent/architecture_statement.md`
- `docs/governance/agent/objective_statement.md`
- `docs/governance/agent/constitutional_constraints.md`

That dry-run bundle is the ancestor of the Constitution lane. It binds the
architecture statement, objective statement, hard constraints, model/runtime
profile, ruleset schema, and no-live-mutation boundary.

### Protocol Architecture Driver

This is the best current example of the full Qwen Constitution flow.

| Role | Artifact |
| --- | --- |
| Question | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_question.json` |
| Evidence request | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_evidence_request.json` |
| Evidence packet | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_evidence_packet.json` |
| Prompt contract | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_decision_replay_prompt_fixture.json` |
| Live chat prompt manifest | `reports/verifiable-constitution/protocol-architecture-live-replay/prompt_manifest.json` |
| Full-vocabulary prompt manifest | `reports/verifiable-constitution/protocol-architecture-full-vocab-replay/prompt_manifest.json` |
| Original decision packet | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_constitutional_decision_packet.json` |
| Superseding replayed decision | `docs/governance/agent/fixtures/verifiable_constitution/protocol_architecture_superseding_replayed_decision_packet.json` |

The protocol-architecture prompt contract requires Qwen to:

- emit one JSON object only;
- conform to `postfiat-constitutional-decision-packet-v1`;
- select exactly one option from `preserve-rust-l1`, `fully-fork-xrpl`,
  `hybrid-xrpl-compatibility-layer`, or `hold-no-op`;
- use only the approved protocol-architecture evidence packet;
- cite packet field paths for every rationale, rejection, consequence, and
  invalidation;
- not use the target decision packet as prompt input;
- not authorize live action, `rippled` import, consensus rewrite, XRPL
  compatibility activation, Cobalt submission, mainnet mutation, or authority
  transfer.

The protocol-architecture evidence packet fields are:

| Field | What It Binds |
| --- | --- |
| `protocol_architecture.status_quo_hash` | Current Rust L1 status quo from architecture docs, whitepaper, and governance overview. |
| `protocol_architecture.rust_l1_surface_hash` | Rust workspace and core protocol crates. |
| `protocol_architecture.xrpl_fork_delta_hash` | Research and whitepaper evidence about the delta from a direct XRPL/`rippled` fork. |
| `protocol_architecture.xrpl_compatibility_requirements_hash` | XRP-style compatibility requirements in architecture, transaction, evidence, and RPC docs. |
| `protocol_architecture.pq_privacy_integration_cost_hash` | ML-DSA and Orchard/Halo2 integration cost evidence. |
| `protocol_architecture.cobalt_governance_fit_hash` | Cobalt governance, replay, challenge, and dry-run evidence. |
| `protocol_architecture.operator_migration_impact_hash` | Operator readiness, consent, and source-authorization evidence. |
| `protocol_architecture.release_gate_readiness_hash` | Evidence that any live effect still needs a later named release or Cobalt gate. |

The live prompt manifest hash for the full-vocabulary run is:

```text
prompt_hash = 8d9e1473fe96155b123fc408e58ce2cae63bdd9a3473a6a35d78918e339e9a7148c9bad4ee6560994fad893a847685e5
prompt_payload_hash = d94d334b3db45b933e40c364e0fe6c0bc1d478935e838ff051c7428a3fa3d6f91c75993d2aeaa46d0b0d1f2ba7a3e51a
```

### Cobalt Driver

The Cobalt lane is driven by:

- `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_question.json`
- `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_evidence_request.json`
- `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_evidence_packet.json`
- `docs/governance/agent/fixtures/verifiable_constitution/cobalt_constitutional_decision_packet.json`

The Cobalt evidence packet fields are:

- `cobalt.trust_graph_linkedness_report_hash`
- `cobalt.essential_subset_thresholds`
- `cobalt.registry_transition_rules_hash`
- `cobalt.dry_run_no_mutation_report_hash`
- `cobalt.replay_bundle_root`
- `cobalt.adversarial_packet_coverage`
- `cobalt.rollback_supersession_evidence_hash`
- `cobalt.override_detection_report_hash`

The fixture/internal replay reports are:

- `reports/verifiable-constitution/vc-015-internal-evidence-request-replay-report.json`
- `reports/verifiable-constitution/vc-016-internal-decision-replay-report.json`
- `reports/verifiable-constitution/vc-019-cobalt-dry-run-binding-report.json`

VC-016 replayed a fixture decision output and did not execute a live model
request. It records prompt hash
`05d3a96c76996499d9acc876f35177656103f75131f793aed06617e956d8be40906185bc2f4dd406e74014da3b7cdb43`
and prompt payload hash
`9d4548a6103920f85b3d5f3656b12bd8c8daf1e6c05d334e46bb31c83d32070f3c05f989e3f31b11bf507bac2642cbdb`.

The live Cobalt model decision is the 2026-05-25 H100/H200 run in
`reports/verifiable-constitution/vc-095-cross-machine-cobalt-report.json`,
which selected `preserve-cobalt-hard-gates`.

### Privacy, PQ, And Monetary Drivers

These lanes have fixture sets plus live Qwen receipts. The fixture decision
packets are useful baselines, but they are not model outputs.

| Area | Question | Evidence Request | Evidence Packet | Decision Packet |
| --- | --- | --- | --- | --- |
| Privacy | `privacy_constitutional_question.json` | `privacy_constitutional_evidence_request.json` | `privacy_constitutional_evidence_packet.json` | `privacy_constitutional_decision_packet.json` |
| Post-quantum authorization | `pq_authorization_constitutional_question.json` | `pq_authorization_constitutional_evidence_request.json` | `pq_authorization_constitutional_evidence_packet.json` | `pq_authorization_constitutional_decision_packet.json` |
| Monetary policy | `monetary_policy_constitutional_question.json` | `monetary_policy_constitutional_evidence_request.json` | `monetary_policy_constitutional_evidence_packet.json` | `monetary_policy_constitutional_decision_packet.json` |

The live model receipt runs are:

| Area | Live answer | Reports |
| --- | --- | --- |
| Privacy | `hold-no-op` | `vc-095-live-model-receipt-privacy-h100-report.json`, `vc-095-live-model-receipt-privacy-h200-report.json`, `vc-095-cross-machine-privacy-report.json`; prior full-vocabulary support in `vc-022-cross-machine-model-receipts-report.json` |
| Post-quantum authorization | `hold-no-op` | `vc-095-live-model-receipt-pq-h100-report.json`, `vc-095-live-model-receipt-pq-h200-report.json`, `vc-095-cross-machine-pq-report.json` |
| Monetary policy | `preserve-fixed-supply-fee-burn` | `vc-095-live-model-receipt-monetary-h100-report.json`, `vc-095-live-model-receipt-monetary-h200-report.json`, `vc-095-cross-machine-monetary-report.json` |

The privacy run binds question `vc-privacy-architecture-001`, prompt hash
`78ed00ee0d2e014a1ea356ff960d70d853fc035262edcfe1098974bc51191c7de9a785a999c703510f52a141bda00ae0`,
and prompt payload hash
`4ae953b334746ce7b9ab11421625b2cadd517bae8b7bd256a56d8acdfcd410a574a7f8c91e07b4a12efd5fdb488f3f77`.

### Validator Evidence Driver

The current validator-evidence constitutional question is:

- `docs/governance/agent/fixtures/verifiable_constitution/validator_evidence_constitutional_question.json`

Its current options are:

- preserve the current validator evidence contract;
- ask Qwen to author a UNL evidence request over registered fields;
- require field-registry revision first;
- hold/no-op.

The 2026-05-25 live H100/H200 run selected `hold-no-op` and is bound in:

- `reports/verifiable-constitution/vc-095-live-model-receipt-validator-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-validator-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-validator-report.json`

This lane is not a live UNL decision. The supporting docs are:

- `docs/governance/validator-evidence-field-registry.md`
- `docs/governance/validator-evidence-packet-schema.md`
- `docs/governance/validator-evidence-ruleset-binding.md`
- `docs/governance/validator-evidence-collector-cobalt-integration.md`

## Decisions Not Yet Made

The downstream Cobalt path has now been exercised in a controlled internal
live-effect drill: it admitted one local validator into a governed registry and
ratified `authority_mode = 1` (`cobalt-ratified`) on a project-controlled local
chain. That is not a public testnet or mainnet authority transfer.

The Constitution has not yet made these live decisions:

- no mainnet launch decision;
- no public testnet or mainnet validator-registry mutation;
- no public-network Cobalt amendment submission;
- no public testnet or mainnet authority transfer from the foundation;
- no validator-side sidecar activation;
- no commit-reveal activation;
- no validator-enforced Qwen scoring activation;
- no new model promotion;
- no XRPL/`rippled` fork;
- no XRPL compatibility-layer activation.

## Current Authority Boundary

The Verifiable Constitution now has one controlled internal live-effect drill.
It can prove that Qwen/Cobalt-bound artifacts can feed real local governance
state changes without touching public infrastructure.

The completed drill did this on a local project-controlled chain:

- admitted `validator-3` through the validator-registry governance path;
- staged and validated the admitted validator key;
- ratified `authority_mode = 1`, meaning Cobalt-ratified governance authority;
- prepared, but did not apply, a rollback amendment back to foundation mode.

Verify it with:

```bash
scripts/qwen-cobalt-live-registry-authority-drill --verify-report
```

It still does not authorize:

- mainnet mutation;
- public testnet or mainnet validator-registry mutation;
- public-network Cobalt amendment submission;
- validator-side sidecar activation;
- commit-reveal activation;
- validator-enforced scoring activation;
- model promotion;
- public testnet or mainnet authority transfer.

Current artifacts may guide docs, planning, rebuttals, internal drills, and
implementation preparation. Public live effect still requires a later explicit
gate with exact authority, operator-visible commands, rollback, challenge
handling, and Cobalt or release approval.

## Controlled Live Effect Drill

The controlled live-effect report is:

```text
reports/verifiable-constitution/vc-110-qwen-cobalt-live-registry-authority-drill-report.json
```

Plain English result:

- upstream Qwen/Cobalt reports are hash-bound into the drill;
- a local chain starts with three validators and foundation authority
  (`authority_mode = 0`);
- the registry governance path admits a fourth validator and verifies the new
  registry root;
- the authority governance path applies a Cobalt-ratified authority amendment
  (`authority_mode = 1`);
- no public testnet, mainnet, external validator, provider, or sidecar is
  touched.

This is the first implementation evidence that the path can mutate governed
state. It is not the final public authority-transfer event.

## Constitutional Objects

Every constitutional decision is built from typed packets.

| Object | Purpose |
| --- | --- |
| `ConstitutionalContextPacket` | Describes the status quo, protocol premise, available decision areas, and current authority boundary. |
| `ConstitutionalQuestion` | States one bounded question with a closed option set. |
| `ConstitutionalEvidenceRequest` | Defines exactly what evidence fields are needed before the question can be answered. |
| `ConstitutionalEvidencePacket` | Provides the evidence, source bindings, hashes, missing-field status, and conflict status. |
| `ConstitutionalDecisionPacket` | Records the selected option, rationale, cited evidence fields, holds, rejects, and authority limits. |
| `ConstitutionalModelReceipt` | Binds the model profile, prompt hash, raw output hash, parsed output hash, and logit or full-vocabulary roots where available. |
| `ConformancePacket` | Maps an accepted decision to docs, code, tests, release gates, rollback, and challenge paths. |
| `ChallengePacket` | Gives a concrete way to hold, supersede, or roll back a bad or stale decision. |

The schema files live under `docs/governance/agent/constitutional_*.json`.
The fixture packets live under
`docs/governance/agent/fixtures/verifiable_constitution/`.

## Process

1. Freeze the status quo in a context packet.
2. Ask one bounded constitutional question with closed options.
3. Generate or validate an evidence request for that question.
4. Assemble an evidence packet from registered, source-bound fields.
5. Run the pinned model/runtime profile against the prompt packet.
6. Parse the model output into the required decision schema.
7. Bind the result with replay evidence and hashes.
8. Map the decision to conformance targets.
9. Run adversarial checks and challenge drills.
10. Hold, supersede, or advance the decision through a release or Cobalt gate.

If any step is missing, stale, conflicting, untyped, or unverifiable, the
default outcome is hold/no-op.

## Role Of Qwen

Qwen is not a monarch and not permanent authority. Qwen is the current model
profile used to produce typed, replayable judgment over bounded evidence
packets.

The model may be used for questions where static rules are too brittle, such as:

- whether apparent validator diversity is real or cosmetic;
- whether public operator evidence is stale, contradictory, or sufficient;
- whether privacy, authorization, or architecture tradeoffs should hold pending
  more evidence;
- whether an implementation decision is compatible with the frozen context and
  authority limits.

Static rules remain preferred for exact binary invariants. For example, schema
validation, unknown-field rejection, source-hash checks, signature checks,
registry-root comparisons, stale-root checks, churn caps, and forbidden live
effects are deterministic rules, not model opinions.

## Replay And Receipts

Replay is not treated as correctness. Replay is treated as accountability.

For a model decision to matter, the artifacts must bind:

- the prompt payload hash;
- the model/runtime profile hash;
- the raw model response hash;
- the parsed JSON output hash;
- the evidence root;
- the accepted schema root;
- the conformance or selector root;
- available logprob, top-logprob, or full-vocabulary roots;
- the cleanup and provider inventory record for live machine runs.

The current evidence includes live provider-backed Qwen/SGLang runs on H100 and
H200-class machines. The 2026-05-25 suite ran privacy, post-quantum
authorization, monetary policy, model governance, Cobalt governance, and
validator-evidence questions on both machines. Those reports bind matching
parsed outputs and matching top-logprob roots.

The stricter full-vocabulary lane is narrower. It currently binds matching
248,320-entry full-vocabulary vector roots for the protocol-architecture and
privacy examples. The 2026-05-25 `vc-095-*` chat reports are real live model
execution receipts, but they are classified `green-top-logprob`, not
full-vocabulary proofs.

This proves the current lane can produce replay-bound constitutional artifacts.
It does not prove every future prompt, model, hardware class, or runtime profile
is automatically acceptable.

## Founder And Foundation Constraint

The founder or foundation may create initial proposals, write code, and submit
packets. They may not silently change the accepted process without creating a
detectable mismatch.

The accepted process binds the prompt, schemas, model profile, evidence root,
output hash, conformance root, and release or Cobalt gate root. If a later list,
decision, or implementation path does not match those roots, the deviation is a
machine-checkable event, not a private judgment call.

A visible governed amendment can change the process. A silent override cannot
pretend to be the accepted process.

## Validator Evidence Example

A validator-selection question should not ask Qwen to invent standards from
unstated criteria. It should provide a typed evidence packet.

Example question:

```text
Should validator candidate V be eligible for inclusion in the next UNL
candidate set under the current validator-evidence policy?
```

Example evidence fields:

| Field | Meaning |
| --- | --- |
| `validator_public_key` | Candidate validator key. |
| `operator_manifest_url` | Public URL or manifest location claimed by the operator. |
| `domain_control_evidence` | Source-bound proof that the operator controls the claimed domain or URL. |
| `uptime_window` | Measured availability over the relevant window. |
| `amendment_vote_history` | Prior voting behavior where available. |
| `topology_evidence` | Public topology, crawl, peer, or endpoint evidence. |
| `country_level_geo` | Country-level diversity signal, not proof of legal jurisdiction. |
| `asn_and_hosting_evidence` | ASN and hosting concentration signal. |
| `operator_independence_evidence` | Evidence for or against independent operation. |
| `source_hashes` | Hashes for every source used in the packet. |
| `missing_fields` | Required fields that are absent. |
| `conflict_set` | Contradictory evidence that must be resolved or held. |

Qwen may then return a typed decision such as:

```json
{
  "selected_option": "hold_for_more_evidence",
  "cited_fields": [
    "operator_manifest_url",
    "domain_control_evidence",
    "operator_independence_evidence"
  ],
  "rationale": "The validator has acceptable uptime evidence, but operator control and independence are not sufficiently bound to source hashes.",
  "required_followup_evidence": [
    "domain_control_evidence",
    "operator_independence_evidence"
  ],
  "authority": "no_unl_publication_no_registry_mutation"
}
```

A deterministic selector may only consume the typed output after schema,
evidence, replay, conformance, challenge, and Cobalt gates pass. If the model
tries to cite a field that is not registered, invents private knowledge, expands
the option set, ignores missing evidence, or asks for live authority, the packet
fails closed.

## Model Updates

PostFiat is not stuck on Qwen forever.

A model update must itself be a constitutional decision:

1. state the current model/runtime profile;
2. define the candidate model/runtime profile;
3. provide replay, determinism, cost, security, and operator evidence;
4. compare failure modes and rollback paths;
5. run challenge and adversarial cases;
6. produce a conformance packet;
7. pass an explicit release or Cobalt gate.

Until that process passes, Qwen remains the current profile and no replacement
model has authority.

## What Exists Now

Current supporting pages:

- Verifiable Constitution Readiness Summary
- Verifiable Constitution Proof Summary
- Verifiable Constitution Attack-Hardness Readiness Summary
- [Verifiable Constitution Plan](verifiable-constitution-plan.md)

Current supporting reports:

- `reports/verifiable-constitution/vc-001-constitutional-question-report.json`
- `reports/verifiable-constitution/vc-010-initial-context-packet-report.json`
- `reports/verifiable-constitution/vc-020-readiness-summary-report.json`
- `reports/verifiable-constitution/vc-022-cross-machine-model-receipts-report.json`
- `reports/verifiable-constitution/vc-030-ai-validation-attack-hardness-readiness-report.json`
- `reports/verifiable-constitution/vc-031-accepted-process-activation-report.json`
- `reports/verifiable-constitution/vc-032-silent-override-detector-report.json`
- `reports/verifiable-constitution/vc-033-foundation-override-simulation-report.json`
- `reports/verifiable-constitution/vc-040-verifiable-constitution-proof-summary-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-privacy-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-privacy-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-privacy-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-pq-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-pq-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-pq-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-monetary-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-monetary-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-monetary-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-model-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-model-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-model-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-cobalt-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-cobalt-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-cobalt-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-validator-h100-report.json`
- `reports/verifiable-constitution/vc-095-live-model-receipt-validator-h200-report.json`
- `reports/verifiable-constitution/vc-095-cross-machine-validator-report.json`
- `reports/verifiable-constitution/vc-110-qwen-cobalt-live-registry-authority-drill-report.json`
- `reports/verifiable-constitution/vc-101-protocol-architecture-live-effect-release-effect-post-execution-release-record-archive-durability-gate-report.json`
- `reports/verifiable-constitution/vc-102-protocol-architecture-live-effect-release-effect-post-execution-release-record-archive-redundancy-gate-report.json`
- `reports/verifiable-constitution/vc-103-protocol-architecture-live-effect-release-effect-post-execution-release-record-archive-integrity-gate-report.json`

Verifier commands:

```bash
scripts/verifiable-constitution-readiness-summary --verify-report
scripts/verifiable-constitution-proof-summary --verify-report
scripts/verifiable-constitution-attack-hardness-readiness-summary --verify-report
scripts/verifiable-constitution-accepted-process-activation --verify-report
scripts/verifiable-constitution-silent-override-detector --verify-report
scripts/verifiable-constitution-foundation-override-simulation --verify-report
scripts/verifiable-constitution-live-model-receipt --verify-report --report reports/verifiable-constitution/vc-095-live-model-receipt-model-h100-report.json
scripts/verifiable-constitution-cross-machine-receipts --verify-report --report reports/verifiable-constitution/vc-095-cross-machine-model-report.json
scripts/qwen-cobalt-live-registry-authority-drill --verify-report
scripts/verifiable-constitution-protocol-architecture-live-effect-release-effect-post-execution-release-record-archive-durability-gate --verify-report
scripts/verifiable-constitution-protocol-architecture-live-effect-release-effect-post-execution-release-record-archive-redundancy-gate --verify-report
scripts/verifiable-constitution-protocol-architecture-live-effect-release-effect-post-execution-release-record-archive-integrity-gate --verify-report
```

## Bottom Line

The Constitution is a commitment device. It does not remove all human choice at
genesis, and it does not make model judgment magic. It makes ongoing discretion
public, typed, replayable, challengeable, and gated.

If PostFiat later deviates from the accepted process, the deviation should be
obvious by construction.

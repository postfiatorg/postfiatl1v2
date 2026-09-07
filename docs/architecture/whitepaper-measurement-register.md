# Historical Whitepaper Measurement Register

This register is preserved from the whitepaper at commit
`035e2026ad698192d9c9238a91d010dc0853bdee`, before the September 2026 editorial
revision separated protocol reasoning from implementation and measurement
records. Section references below refer to that historical paper. The original
provenance limitations remain; relocation does not validate the measurements.

See the [whitepaper overview](whitepaper-overview.md) for the associated audit
and [Cobalt implementation](../governance/cobalt-implementation.md) for the
separate deployment history.

## Retained register

Each entry is limited to the sentence and experiment it identifies. The [alignment inventory](whitepaper-alignment.md#appendix-a) records which original packets are available at the audit baseline. E1/E2/E3/E5/E6/E7 have incomplete original measurement provenance in this checkout; retained prose and digests alone cannot reproduce their claims. E4 has five retained admission tests; E8 maps to current finality/governance tests and separate reference-ordering machinery. The subsequent [blog publication review](cobalt-admission-publication-review.md#additional-historical-artifacts-found-through-the-blog) located additional cover and replay summaries in the website's June bundle and verified its 15 manifest entries. Those dated summaries add provenance, especially for E3/E7, without supplying every original raw measurement input and output.

**[E1] ML-DSA-65 verification budget.** Supports the historical estimate in §9.2: ~6,000 verifications/s (~160 µs each), with ≈4/11 ms illustrative quorum verification. The 80,184/223,847-byte values are simplified one-signature-set arithmetic, not encoded V2 certificate sizes. The original timing packet is unavailable in the audit checkout; no current-release benchmark is claimed.

**[E2] Orchard verification budget.** Supports §7.2. Local release-build run: two-action output proof of 7,264 bytes; cached verification 80 ms median. Justifies per-block action caps and the priced-class design.

**[E3] Cobalt cover sizing.** Supports §6.6. Grouped institutional-style trust views under the $M_{cover}=64$ profile: 35 validators in five seven-validator groups → $M=12$; 100 validators in ten ten-validator groups → $M=22$.

**[E4] Admission selector fixtures.** Supports §5.3. Deterministic selector over `ValidatorAdmissionEvidencePacket` inputs with fixture coverage: clean admit, shared-control reject, missing-domain hold, contradictory-evidence hold, unknown-model-field hold.

**[E5] Same-stack repeatability.** Supports §8.5. Qwen3.6/SGLang pinned profile; saved 29-validator XRPL UNL cohort; one validator-domain record per request; temperature 0, JSON response mode, non-thinking output; prompt: "score this validator's credibility on a scale from 0-100 where credibility is defined as useful institutional proof of a blockchain's legitimacy"; parser accepts only JSON with one integer score field in [0,100]. Two batches × 50 repeats per domain: 2,900/2,900 parseable responses, 100 complete score maps, zero score variance, zero raw-output variance. Score-map hash: `9f7f95a7be238e2b6bb1cc081986f8b5dffc07b9397578d723c6f6d7c77c81c8`.

**[E6] Cross-hardware convergence.** Supports §8.5. One named Qwen/SGLang profile family; six governed questions, three runs each on H100 NVL and on H200; parsed-output roots and top-logprob commitment roots converged for every question. Separate H100/H200 full-vocabulary next-token probe converged over 248,320 fixed-point logprob entries; vector root: `560ea13c99f73a60c184ec07ba3554ea11c72487d56ac28c366495d58ce8913c`.

**[E7] Cross-runtime constitutional packet.** Supports §8.5. Closed options `adopt-cobalt-registrar | retain-offchain-unl | hold-no-op`; prompt hash `277e4174662841fe8d0802f0d055fec0528afbae09173a49d1f9067fc9a5ad68`. Apple M5 MLX BF16 with Qwen3-1.7B: 300/300 parseable. Vast H200 SGLang deterministic-inference profile with Qwen3-1.7B: 100/100 parseable, one top-logprob root. Both selected `adopt-cobalt-registrar`. Decision root: `08b3d570e746a4bd4c761ab280aa1f6f4992704810f03de2377c8a38b0fc0cf8`. Parsed-output root: `1f667e5d8d63fbc8852b10085062e13579f864f27f3b3c53481b68bc4b2fbc1e`. H200 top-logprob root: `af228110a9782fcfdca48dd681636360d24d3a995933443bd06e1374e8cbda07`. Machine reports: `reports/qwen-mlx-profile-portability/20260528T155652Z/machine_report.json`, `reports/qwen-sglang-profile-portability/20260528T162243Z/machine_report.json`.

**[E8] Consensus and registry fixtures.** Current source anchors are `crates/ordering_fast/src/consensus_v2/tests.rs` for domain, lock, quorum and timeout/commit tests and `crates/node/src/cobalt_handoff.rs` for signed scoped-authority, replay and certificate rejection tests. Reference admission-receipt/ordering fixtures do not establish production censorship attribution. See the [validation record](whitepaper-validation.md) for tests actually run during this audit; these tests do not prove all assumptions in §6.7.


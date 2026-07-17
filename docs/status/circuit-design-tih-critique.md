# Consolidated Cryptographic Critique (from TIH multi-model review: GPT 92.6 / Opus 88.2 / DeepSeek 91.8)

The doc's strengths (preserve these): rigorous consensus-verifiable zk-SNARK-over-hidden-witness framing; asset identity bound into note commitments + nullifiers + constraints; concrete public/private circuit statement; domain binding (chain/genesis/protocol/pool/circuit/transcript); honest separation of the insecure SHA3-transcript scaffold from the required production design; separates transparent backing from private transfer.

## Cryptographic concerns the rewrite MUST resolve

1. **`swap_binding_hash` binding is unresolved (foundational).** The circuit currently only constrains supplied field limbs as *public inputs* — it does **not prove** the binding hash. Decide and specify: recompute `swap_binding_hash` **inside the circuit** (so the proof binds the full action transcript), or prove an explicit external-binding relation. Any "external public input" path must show exactly how signature verification + public-instance checks bind the transcript without an in-circuit hash, and why that resists replay/substitution/malformed-action. This is the load-bearing binding property — make it concrete, not an open question.

2. **Asset-distinctness constraint (`asset_tag_0 != asset_tag_1`) is doing rhetorical work it may not need, and is underspecified.** The soundness argument leans on distinctness for "cannot be merged/split/relabelled/inflated," but the **permutation constraint already guarantees per-asset conservation**. Resolve: either (a) remove distinctness from the soundness argument and rely on permutation, or (b) keep it but justify precisely what additional property it provides. If kept: it is currently enforced only on the two **input** notes — it must also cover **output** tags and **nonzero values**, or degenerate/zero-value edge cases must be handled explicitly. Also: distinctness **forbids same-asset swaps** — decide whether that's intended (state it) or a limitation to remove.

3. **`AssetTag` collision-resistance is asserted, not argued.** SHA3-384 → truncated 256-bit → split into two 128-bit field-element limbs: give an explicit **collision-target and second-preimage argument** for this exact construction, including the truncation+splitting. Asset confusion is the core threat this circuit exists to prevent — the collision argument must be airtight.

4. **Underspecified security-critical primitives.** Give full **domain-separation, encoding, and algorithm definitions** for: `HashToPallasBase`, `AssetDeriveNullifier`, `AssetOutputRho`, `OrchardPsi`, `OrchardRcm`, `H_action`, `H_sig`. "Reuse Orchard where possible" / "derive or constrain" / "review later" is not acceptable for consensus-critical primitives — define them.

5. **Soundness is presented as settled when it is conjectured (unimplemented, unreviewed).** Reframe Section 10's conservation claim as **conjectured-pending-external-cryptographic-review**, and back it with an explicit **failure-mode analysis** + a **breaking-test suite**: verifying-key pinning checks, asset-distinctness/permutation adversarial tests, and the **forged-non-conserving-proof-rejected** test (test 4) actually specified as executable.

6. **Normative vs open questions are mixed.** Separate fixed normative requirements from genuine open review questions (Section 15). Do not present open questions (in-circuit binding hash, nullifier structure, rho derivation, asset-tag representation, fee support) as settled.

7. **Add a circuit-size / performance estimate** (constraint count, proof/verify cost) so implementers can gauge feasibility before committing, plus a **verifying-key-pinning failure-mode checklist** for the consensus-integration section.

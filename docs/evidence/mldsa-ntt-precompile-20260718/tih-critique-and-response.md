# Text Improvement Harness critique and response

The founder-specified command was run exactly once on 2026-07-18:

```text
PYTHONPATH=/home/postfiat/repos/text-improvement-harness-codex-plugin \
  /home/postfiat/repos/text-improvement-harness-codex-plugin/.venv/bin/python \
  -m text_improvement_harness criticize \
  docs/specs/mldsa-ntt-precompile-spec.md --mode gpt
```

## Critique as returned

The final harness response ended mid-sentence; that truncation is preserved
below rather than rerunning the command.

```text
question=What is the best thing about this document?
{
  "answer": "The best thing about this document is its evidence-driven, soundness-first decision process: it measures the real bottleneck, selects an optimization that remains fully constrained inside the SP1 guest, and refuses the tempting but unsound shortcut of unconstrained host computation.",
  "supporting_reasons": [
    "The recommendation follows profiling data showing that repeated ExpandA preparation is the largest avoidable cost, rather than relying on the initial NTT-focused hypothesis.",
    "It states a precise consensus invariant: the accelerated verifier must match unmodified fips204 0.4.6 for every input tuple, with malformed inputs rejecting rather than panicking.",
    "It clearly defines the proof-system boundary and explains why an unconstrained NTT host hint would let a malicious prover forge acceptance.",
    "The proposed cache is deterministic, byte-keyed, bounded, and limited to public-key-derived state, so it improves performance without changing signature-dependent behavior.",
    "The release gates are unusually concrete, covering independent-oracle differential tests, malformed inputs, cache eviction, byte-identical public values, and a verified CUDA
question=What is the worst thing about this document?
{
  "answer": "The document presents a deployment-relevant cryptographic implementation decision even though its mandatory validation is entirely missing.",
  "supporting_reasons": [
    "The header says validation results are recorded in section 11, but section 11 is only a placeholder.",
    "No reference-versus-accelerated cycle result, public-value comparison, ACVP result, mutation-test result, peak-memory result, or CUDA Groth16 proof result is provided.",
    "The proposed optimization changes cryptographic arithmetic and consensus-critical verification behavior, so algebraic arguments and planned tests are not sufficient evidence.",
    "The stated performance target and deployment posture therefore cannot be evaluated, despite the document reading as a completed decision."
  ],
  "suggested_edit": "Change the status to \"proposed implementation; not approved\" and the section 1 wording to a conditional recommendation until section 11 contains exact commands, artifact hashes, pass/fail counts, accelerated cycle and memory measurements, byte-equivalence results, and CUDA prove-and-verify evidence. Approve the decision only after every release gate is documented as passed."
}
question=What do you hate about this document and what could be improved?
{
  "answer": "The biggest problem is that the document reads like an approved, validated implementation decision while explicitly admitting that validation has not happened. The header says validation results are recorded in section 11, section 1 says “Use,” and the status is “implementation decision,” but section 11 is empty and says the design is not approved for deployment. That contradiction undermines the otherwise strong measurement and soundness work. The document is also too repetitive, mixes decision rationale with implementation notes and process bookkeeping, and makes several correctness claims more confidently than the supplied evidence supports.",
  "supporting_reasons": [
    "The status is internally inconsistent: section 11 contains no results, despite the header claiming otherwise, and the document simultaneously presents a final decision and an unapproved proposal.",
    "The central performance result is only a target. There are no accelerated ELF hashes, post-change cycle counts, wall times, peak RSS figures, cache hit statistics, proof artifacts, or CUDA prove-and-verify results.",
    "The universal consensus-equivalence claim is stronger than the validation
```

## Action taken

The critique identified a real status/evidence contradiction. Immediately
after the one-shot critique, the draft was changed to:

- says `proposed implementation; not approved`;
- makes section 1 a conditional recommendation;
- conditions selection on every release gate passing; and
- explicitly requires section 11 to contain commands, pass/fail counts, hashes,
  cycle and memory measurements, public-value equivalence, and CUDA proof
  verification before approval.

The request for actual evidence was then addressed by the implementation
workflow, not by speculative prose. Only after every section 7 gate passed was
the status promoted to `validated for code review; not deployed`, and section
11 was populated with exact commands, pass counts, ELF/proof identities,
before/after measurements, public-value equivalence, and CUDA verification.
The harness was not rerun and its score was not chased.

# Identity-packet H200 replay: XRPL + PostFiat

**Frozen inputs:** 2026-09-03

**Status:** `SHADOW_ONLY` research replay. Not consensus data, not validator
weight, not public production use.

**Result:** pending — runs launched, outputs not yet collected.

This is the packet-based successor to
[`institution-reputation-unl-20260901`](../institution-reputation-unl-20260901/).
The pinned model scores the institution behind each validator from the exact
frozen Markdown identity packet in
[`validator-identity-packets-20260901`](../validator-identity-packets-20260901/),
per [Institution Legitimacy Scoring](../../../docs/governance/institution-legitimacy-scoring.md)
and Section 10 of the
[identity-packet runbook](../../../docs/runbooks/validator-identity-packets.md).
The three earlier packages are not modified.

## Corpus verification and review status

Before this package was built, the frozen corpus verifier was rerun:

```bash
python3 benchmarks/ai-governance/validator-identity-packets-20260901/finalize.py
git diff --exit-code -- benchmarks/ai-governance/validator-identity-packets-20260901/{index.json,index.md,manifest.json,verification.json}
```

Result: `PASS`, 55/55, zero diff; packet-set SHA-256
`b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd`.

Machine verification of the identity corpus is complete. **The independent
human publication review required by runbook Section 8 has not been
performed.** This replay is therefore SHADOW_ONLY research on a
machine-verified corpus; it is not evidence of human review and must not be
presented as public production use.

## Input contract

Every scoring request contains, in the user message:

- the exact UTF-8 bytes of `packets/<network>/<validator>.md` between fixed
  begin/end markers (verified byte-equal at build time and again by
  `run_host.py` before any request is sent);
- the network and validator key;
- the per-packet SHA-256 copied from the corpus `index.json`; and
- the corpus packet-set SHA-256 copied from the corpus `manifest.json`.

The response schema requires the model to echo `validator_id`, `network`, and
`packet_sha256`, so each raw response is also self-bound to its packet.

Not used: live web search, any Corbanu/Codex rerun, the JSONL exec logs, the
`index.md` display text, operator nicknames, or an entity-name mapping.

## Pinned execution profile

Identical to the proven two-host institution replay:

- model `Qwen/Qwen3.8-27B-FP8`, revision
  `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a`;
- SGLang image digest pinned in `manifest.json`;
- `--enable-deterministic-inference`, seed `438916795`;
- temperature 0, top-p 1, thinking disabled, `max_tokens` 2048;
- `--disable-radix-cache` so every request takes the fresh-prefill path (the
  cache-hit path is not deterministic-mode-covered on this image);
- CUDA graphs and overlap scheduling disabled; Triton attention and
  linear-attention backends;
- two fixed batches of 32 slots: 55 scoring requests plus 9 fixed padding
  requests;
- loopback-only self-hosted inference; two runs on each of two distinct-owner
  H200-class Vast.ai hosts; OpenRouter not used.

## Prompt

`inputs/prompt.txt` keeps the twenty five-point bands from the institution
replay and adapts the rules to packet input: the packet is identity evidence,
not legitimacy; the model must genuinely recognize the institution the packet
names before any positive score; unknown, not-established, individual-person,
or bare-domain entities score exactly 0; sanctions/integrity risk, prestige,
and Layer-1 reputational value are weighed for recognized institutions; the
packet's confidence labels and profile-size tier are never treated as a score.

## Files

- `inputs/prompt.txt`: exact system prompt and bands
- `inputs/packet_index.json`: slot → network, validator, packet path, packet
  SHA-256, byte length
- `inputs/requests.json`: exact 64 model requests with embedded packet bytes
- `inputs/batch_schedule.json`: fixed two-batch schedule
- `manifest.json`: model/runtime/settings/batch/padding identities and the
  identity-corpus hashes
- `build_package.py`: deterministic builder from the frozen corpus
- `bootstrap_host.sh`: pinned model download and loopback SGLang launch
- `run_host.py`: input re-verification and fixed-batch executor
- `compare_runs.py`: raw-byte comparator and canonical score writer
- `test_package.py`: package invariants
- `outputs/`: four raw runs, host identities, comparison, scores, teardown
  receipt (added when collected)

## Verify

```bash
cd benchmarks/ai-governance/identity-packet-replay-20260903
python3 test_package.py
python3 compare_runs.py   # once outputs/ holds all four runs
```

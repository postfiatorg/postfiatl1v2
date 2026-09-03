# Institution Legitimacy Scoring

**Status:** Active operator direction — 2026-09-01; packet-input revision executed 2026-09-03

## Purpose

Score the legitimacy and Layer-1 reputational value of the institution claimed
by a validator provider. If the pinned model does not recognize the institution,
the score is **0**. Do not replace this judgment with validator-performance
formulas.

## Runtime boundary

Production and replay inference uses the pinned, self-hosted
`Qwen/Qwen3.8-27B-FP8` revision through a loopback-only SGLang server. It does
not call OpenRouter or any other external inference API. Consensus never makes
a network inference call: a score run produces a frozen, content-addressed
`SHADOW_ONLY` artifact for review and later ratification.

The OpenRouter call made on 2026-09-01 was only the operator-requested two-name
sanity check. It is not part of the runtime design.

The direct CLI is `python/postfiat_rpc/institution_reputation.py`. It refuses
non-loopback endpoints:

```bash
PYTHONPATH=python python3 -m postfiat_rpc.institution_reputation \
  "University of Waterloo" "University of Zuzaluca"
```

The [two-UNL H200 results](institution-reputation-unl-h200-results-20260901.md)
record the complete 5-point rubric, frozen inputs, four raw replay outputs, and
per-validator scores. The complete artifact lives under
`benchmarks/ai-governance/institution-reputation-unl-20260901/`.

The packet-input successor described under "Scoring rule" has now run: see the
[identity-packet H200 results](institution-reputation-packets-h200-results-20260903.md)
(192/192 byte-identical on two distinct-owner hosts) and
`benchmarks/ai-governance/institution-reputation-packets-20260903/`, which also
publishes a deterministic packet-derived validator correlation.

## Scoring rule

For the next scoring revision, the [validator identity-packet stage](validator-identity-packets.md)
first converts each frozen validator key/domain coordinate into cited public
identity evidence. The pinned model receives the exact frozen Markdown packet,
not an improvised entity-name mapping. The scoring prompt requires:

- genuine model recognition before any positive score;
- exactly 0 for missing or unrecognized institutions;
- explicit consideration of sanctions and integrity risk;
- institutional prestige; and
- positive or negative reputational value to a Layer-1 blockchain.

The H200 package defines every 5-point band from 0–4 through 95–100. Domain
ownership and matching against an operator-supplied authoritative list remain
separate identity checks. A domain match does not force a positive reputation
score, and the model score does not prove domain control.

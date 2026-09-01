# Institution Legitimacy Scoring

**Status:** Active operator direction — 2026-09-01

## Purpose

Score the legitimacy of the institution claimed by a provider. This is an AI
judgment task, not a replacement for deterministic validator-performance
metrics.

The rule is deliberately simple:

- ask `qwen/qwen3.8-27b` whether it recognizes the institution;
- request a 0–100 legitimacy score with a two-to-three-paragraph explanation;
- if the model does not recognize the institution, the score is **0**; and
- do not give an unrecognized institution weight merely because it may exist.

The exact prompt is:

```text
Score how legitimate ENTITY <{entity}> is. Write 2-3 paragraphs justifying your explanation and give a score from 0-100. If you do not recognize the institution, the score is 0.
```

Run it through OpenRouter:

```bash
OPENROUTER_API_KEY="$(corbanu vault auth-helper YOUR_OPENROUTER_LABEL)" \
  PYTHONPATH=python python3 -m postfiat_rpc.institution_reputation \
  "University of Waterloo" "University of Zuzaluca"
```

The CLI is `python/postfiat_rpc/institution_reputation.py`. It sends the exact
prompt to the exact model at temperature zero and prints the model response
without applying a formula or rewriting the score.

## Scope correction

The H200 experiment remains a historical experiment. Its censorship,
sanctions, validator-performance, deterministic-sub-scorer, UNL-overlap, and
"obscure real operator" rules do not define this institution-legitimacy score.
In particular, the prior rule that an obscure real organization should receive
a positive floor is superseded for this score: **unrecognized means zero**.

Domain ownership and comparison with an operator-supplied authoritative domain
list are separate identity checks. Passing those checks does not force a
positive legitimacy score, and this model score does not prove domain control.

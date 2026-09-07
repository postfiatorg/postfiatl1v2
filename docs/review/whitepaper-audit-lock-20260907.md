# Whitepaper audit specification lock

The [specification](../specs/whitepaper-alignment-audit-20260907.md) is locked after its first compliant full gate on 2026-09-07. Preserve its scored bytes; do not rewrite or rescore it.

- Task: `task_e8a5201ae49adfd0e32b5496bb7f7188`.
- Audit source: `d351353e57b295368450a57866ace17b5e1ce6ad`.
- Scored SHA-256: `4b683479a1d82fd1ef387797a5cf5bd751b53f75063b8985ec15441441f914c0`.
- Round: `round-20260907T175956Z`; run group `whitepaper-audit-first-full`.
- Five real calls per selected model; 15 successful scores, no mocks or model substitution.

| Model | Runs | Mean | Range |
| --- | ---: | ---: | --- |
| `openai/gpt-5.6-sol-pro` | 5 | 92.00 | 92–92 |
| `anthropic/claude-fable-5` | 5 | 81.60 | 80–84 |
| `z-ai/glm-5.3` | 5 | 87.40 | 85–90 |
| **Overall** | **15** | **87.00** | |

The repository's >=86 stop condition overrides the harness's generic suggestion to improve. This score assesses the research text, not protocol safety or implementation correctness.

Command actually run (credential resolved by the harness; no key is recorded):

```bash
PYTHONPATH=/home/postfiatchad/repos/text-improvement-harness-codex-plugin \
  python3 -m text_improvement_harness round \
  docs/specs/whitepaper-alignment-audit-20260907.md \
  --db .tih/whitepaper-audit.sqlite3 --project default --runs 5 --concurrency 15 \
  --run-group whitepaper-audit-first-full --skip-critiques
```

Raw judge responses and scores remain in ignored `.tih/whitepaper-audit.sqlite3`; the separate lock record preserves the scored file identity and full-gate result.

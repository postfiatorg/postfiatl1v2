# UNL amendment V2 research lock

The separate [amendment](../governance/tasknode-unl-amendment-v2-20260907.md) passed its **first compliant full gate** on 2026-09-07 and is locked without rewriting or rescoring, per `AGENTS.md`.

- Task Node: `task_c26d27a9e112a7ad1c27682b95f0790e` — **Rewarded** after verification of the exact scored file hash. This separate lock record is not part of the scored text.
- Exact scored SHA-256: `a74bdf603c70ebc3432499b555b470d87da93e13c54424b3687f834bf02f892f`.
- Harness run group: `unl-v2-first-full`; round `round-20260907T150257Z`.
- Full gate, five real OpenRouter calls per configured model; no mocks, substitutions, retries or prior scored drafts.

| Model | Scores in run order | Average |
| --- | --- | ---: |
| `openai/gpt-5.6-sol-pro` | 94, 95, 94, 94, 94 | 94.20 |
| `anthropic/claude-fable-5` | 88, 89, 90, 86, 87 | 88.00 |
| `z-ai/glm-5.3` | 90, 88, 89, 90, 90 | 89.40 |
| **All 15 scores** | | **90.53 / 100** |

Reproduction command (credential supplied at use time, not stored here):

```bash
PYTHONPATH=../text-improvement-harness-codex-plugin python3 -m text_improvement_harness round \
  docs/governance/tasknode-unl-amendment-v2-20260907.md \
  --db .tih/unl-v2.sqlite3 --project default --runs 5 --concurrency 15 \
  --run-group unl-v2-first-full --skip-critiques
```

This records the already completed call; **do not rerun a passed specification**. Raw scores and responses are in the ignored local `.tih/unl-v2.sqlite3`; terminal output is `.tih/unl-v2-first-full.log`. The harness's generic suggestion to improve is overridden by the repository's >=86 stop rule.

The original blog text remains at SHA-256 `319c1588c6575f6d9bd6c06c9fc1f055336b00cc394befd3cff54e146d68e584`. All `python/postfiat_rpc/tasknode_unl_*.py` sources and the V1 attack simulation remain unchanged. This gate evaluates the research text, not V2 implementation correctness, Sybil resistance, consensus safety or production readiness. V2 is not implemented or activated. No blog edit, commit, push, deployment, registry change or capital-moving/signing operation was performed.

# Research-lock initial evidence

Task: task_aaeebe5e21ba7286e29503a3696d80c4

The revision-3 research specification passed its first compliant text-improvement-harness full gate at 86.20/100 and was locked immediately. Five valid independent scores per selected judge were recorded; no document rewrite or post-pass rescore occurred.

| Judge | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Average |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| gpt-6-astra | 89 | 89 | 89 | 90 | 89 | 89.20 |
| anthropic/claude-fable-5.1 | 84 | 79 | 80 | 84 | 80 | 81.40 |
| z-ai/glm-5.3 | 90 | 87 | 85 | 88 | 90 | 88.00 |

Full arithmetic average: 1293 / 15 = 86.20. Harness parse/provider retries are in the run log; there were no rewrite rounds and therefore no OpenRouter rewrite call was required or made.

Artifacts:

- /home/postfiat/repos/postfiatl1v2/docs/specs/shielded-perp-venue-funding-privacy-hardening-20260905.md
- /home/postfiat/repos/postfiatl1v2/docs/status/shielded-funding-hardening-research-lock-20260905/score-table.md
- The same evidence directory contains candidate-r3.md (exact scored bytes), scores.json (all 15 stored reviews/raw responses), r3-score.log, lock.json, initial-state.json, and task-before-evidence.json.

Scored SHA-256: 939c1d2cd4282fb4fa4ee626b1afaf8717eba7b1083dbdf1fb38e32680978c33

Locked SHA-256: 5fea0fa99d8372f7e09828b1f7cae081cbefa600d90d39aada75eedeaa626d48

Lock header: "Status: research locked on the first compliant full gate, 86.20/100. Implementation, deployment, and production privacy remain unverified." The file uses Markdown emphasis on research locked. The scored candidate remains byte-identical; only the canonical file's status/lock declaration changed. A direct comparison confirms every byte from Objective onward is identical.

Section 3.3 remains: "Operator-funded cover is an implementation requirement." It requires a separate operator application using the same ingress-v2, proof, egress, sponsorship, and settlement code as user requests, real D-sized notes backed by operator capital, isolated operator keys, a bounded statistical schedule and budget, actual Hyperliquid funding, behavior analysis, orderly depletion handling, and no automatic recycling. The no-CEX self-custody objective and distinct public-observer/operator-aware claims are unchanged.

Verification:

- Harness command: PYTHONPATH=/home/postfiat/repos/text-improvement-harness-codex-plugin /home/postfiat/repos/StakeHub/.venv/bin/python -u -m text_improvement_harness score docs/specs/shielded-perp-venue-funding-privacy-hardening-20260905.md --db /home/postfiat/repos/.tih/harness.sqlite3 --project shielded-funding-hardening-20260905 --gate full --runs 5 --concurrency 15 --force --run-group shielded-hardening-r3-full-20260905
- Verified 15 rows, exact model set, run indices 1 through 5 per judge, matching scored SHA, and average >=86 before writing a single lock declaration.
- Verified immutable research body, hashes, required claims/lifecycle, local document links, and unchanged repository HEADs.
- Existing shielded-funding tests: 53 passed. Existing StakeHub strict MkDocs build: exit 0. These are baseline checks, not evidence of the new implementation.

No commits, pushes, deployments, live fund movement, or generated worker sessions were performed. Existing dirty implementation files were preserved. This submission completes only research locking; observer model parameterization, cover implementation, CLI/interface, and operational privacy evidence remain for the subsequent implementation task.

# Research lock evidence

Locked 2026-09-05T13:11:06.153059+00:00. Task Node `task_aaeebe5e21ba7286e29503a3696d80c4`.

First compliant full gate: **86.20/100**, 15 valid scores; threshold 86.00.

| Judge | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| anthropic/claude-fable-5.1 | 84 | 79 | 80 | 84 | 80 | 81.40 |
| gpt-6-astra | 89 | 89 | 89 | 90 | 89 | 89.20 |
| z-ai/glm-5.3 | 90 | 87 | 85 | 88 | 90 | 88.00 |

No rewrite rounds. No scores were rerun after this first passing full gate.
Harness parse/provider retries are logged; they are not document rewrite rounds.

Scored SHA-256: `939c1d2cd4282fb4fa4ee626b1afaf8717eba7b1083dbdf1fb38e32680978c33`.
Locked-file SHA-256: `5fea0fa99d8372f7e09828b1f7cae081cbefa600d90d39aada75eedeaa626d48`.
The sole change after scoring is the status/lock declaration; the research body is identical.

- [Exact scored document](candidate-r3.md)
- [Per-run reviews and responses](scores.json)
- [Lock declaration](lock.json)
- [Harness output](r3-score.log)
- [Baseline state and test evidence](initial-state.json)

This is a research lock, not evidence of implementation, deployment, or production privacy.
The observer model must be parameterized and validated under the frozen research requirements in the subsequent Task Node implementation milestone.

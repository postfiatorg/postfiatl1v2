# Institution reputation from frozen identity packets (H200 replay, 2026-09-03)

**SHADOW_ONLY.** Successor to `institution-reputation-unl-20260901`. The pinned
`Qwen/Qwen3.8-27B-FP8` (revision `017b9c7a…`) scored the institution behind each of
the 55 frozen validators (35 XRPL mainnet, 20 PostFiat round 20) from the **exact
Markdown bytes** of its identity packet in `validator-identity-packets-20260901`,
with the packet SHA-256 and the corpus packet-set SHA-256 bound into every request.
No live search, no Corbanu rerun, no JSONL logs, no `index.md` display text.

## Result

| Check | Result |
| --- | --- |
| Hosts | two distinct Vast.ai owners: H200 NVL (driver 575.57.08, host 214845) and H200 (driver 580.159.03, host 445596) |
| Runs | two per host, fixed two-batch schedule of 32 requests, temperature 0, seed `438916795`, thinking off |
| Byte comparison | **192/192 identical** (165 scoring + 27 padding), zero failures |
| Aggregate response SHA-256 (all four runs) | `3458f72dd4614e5e8e02c47f85f68d39ffeae1ff798e8d1c4eb0fceb6c9e4d20` |
| Comparison SHA-256 | `4800d68dbe849969c98a7a4e22d0c6ff97b027c93d1b30f104ce4bf0a8f83c3d` |
| Identity corpus packet-set SHA-256 | `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd` |
| Rentals | both destroyed; ≈ $3.34 total |

Against the name-only predecessor: 14 validators newly recognized (0 → positive), 9
lowered once the packet named the actual legal entity, 0 lost recognition, 32 unchanged.
Zero-scores fell from 45 to 31; mean rose from 10.8 to 18.9. All 20 PostFiat validators
still score 0: the model does not recognize Post Fiat or its community operators as
institutions, and the recognition rule requires 0 in that case.

## Layout

| Path | Purpose |
| --- | --- |
| `build_package.py` | Verifies the corpus hashes, then freezes prompt, packet-bound requests, schedule, padding, manifest |
| `inputs/prompt.txt` | Scoring prompt: same 5-point bands as the predecessor, plus packet-handling rules |
| `inputs/packets.json` | Validator → packet path, per-packet SHA-256, size |
| `inputs/requests.json` | 64 exact request bodies (55 scoring + 9 padding); each scoring body embeds the packet bytes |
| `inputs/batch_schedule.json` | Two fixed batches of 32 |
| `bootstrap_host.sh` | Pinned SGLang launch on one H200-class GPU |
| `orchestrate_host.sh` | Push package, verify hashes on host, bootstrap, run twice, pull outputs |
| `run_host.py` | One fixed-batch pass; validates the echoed `validator_id`/`network`/`packet_sha256` |
| `compare_runs.py` | Byte comparison of the four runs; publishes `outputs/comparison.json`, `outputs/scores.json` |
| `correlate_packets.py` | Deterministic validator correlation from packet bytes; no model |
| `outputs/primary-run*.json`, `outputs/replay-run*.json` | Raw runs, per-slot content and hashes |
| `outputs/host_identity_*.txt` | GPU name, driver, UUID per host |
| `outputs/scores.json` | Canonical scores, bands, sanctions risk, explanations, packet hashes |
| `outputs/delta-vs-unl-20260901.json` | Per-validator delta against the name-only run, with correlation cluster ids |
| `outputs/correlation.json`, `outputs/correlation.md` | Pairwise packet-derived signals and strong-link clusters |
| `outputs/rental-teardown.json` | Redaction-safe rental and destruction receipt |
| `test_package.py` | Offline checks: counts, corpus hash binding, exact packet bytes in every request |

## Verify offline

```bash
cd benchmarks/ai-governance/institution-reputation-packets-20260903
python3 test_package.py
python3 compare_runs.py
python3 correlate_packets.py && git diff --exit-code -- outputs/correlation.json
```

## Correlation

`correlate_packets.py` reads only the Machine-Readable Summary of each frozen packet
and computes, for every pair, whether they share a canonical entity, alias, X handle,
or registrable domain (strong links), and the overlap of operator-owned official hosts,
non-generic evidence hosts, incorporation region, and operating regions (weak
signals). Shared-hosting suffixes (`github.io` etc.) and platform hosts (`github.com`,
`x.com`, `xrpl.org`, …) are excluded and listed in the output. Result on this corpus:
one strong cluster, the three Post Fiat foundation validators (same entity, domain,
and X handle); 227 of 1,485 pairs share some weak signal; the strongest non-cluster
pair is Ripple Labs / Blockdaemon at 0.38 (shared Delaware incorporation and evidence
hosts). A link means the packets share public-identity signals; it does not prove
common key control.

## Boundaries

- Scores and correlation are external `SHADOW_ONLY` research. They are not consensus
  data and do not change validator weight.
- The independent human publication review required by the identity-packet runbook
  has not been performed; `manifest.json` records this.
- A packet's identity conclusion is cited research, not proof of legal identity or key
  control; the prompt forbids raising a score for packet length, citation count, or
  stated confidence.
- The three frozen predecessors (`institution-reputation-unl-20260901`,
  `validator-identity-packets-20260901`, `reputation-h200-20260901`) are untouched.

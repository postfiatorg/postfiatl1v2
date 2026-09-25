# Institution reputation from identity-only profiles (H200 replay, 2026-09-04)

**SHADOW_ONLY.** Successor to `institution-reputation-packets-20260903`, rerun after the
packet template and scoring prompt were reduced to qualitative identity only. The model
sees nothing but "Assess the organization described in this profile." followed by the
profile. No list membership, verification flags, UNL rounds, hashes, or shadow labels
appear in anything the model reads; hash binding lives in the request record.

## Result

| Check | Result |
| --- | --- |
| Identity corpus | `validator-identity-packets-20260904`, packet-set SHA-256 `8051f392e60d84a687076dc241ddf722859db7c06718dd12139c3109548523df` (55/55 strict PASS) |
| Hosts | two distinct Vast.ai owners: H200 NVL (driver 575.57.08, host 214845) and H200 (driver 580.159.03, host 445596) |
| Runs | two per host, fixed two-batch schedule of 32, temperature 0, seed `438916795`, thinking off |
| Byte comparison | **192/192 identical**, zero failures |
| Aggregate response SHA-256 (all four runs) | `9d6935e2c194441f7a09dc33324d276001ef7548dd6a371d7fd524a80ed4328f` |
| Comparison SHA-256 | `95a97ade7965d793d9731952c4b02afeebcf05e32fa2643c064258bcb627a4ef` |

Scores: 36 of 55 at zero, mean 14.64, max 72. Against the 09-03 packet run
(whose packets carried list-membership lines in 42 of 55 cases): 3 raised, 12 lowered,
40 unchanged, 5 lost recognition, 0 newly recognized.
The five that lost recognition did so because the identity-only researcher reached a
stricter identity conclusion (two domain-less keys previously attributed to Berkeley
Haas and Bitso are now "not established"; onXRP, Eminence, and Cabbit were re-identified
under names the model does not know). All 20 PostFiat validators score 0.

## Layout

Same as the predecessor: `build_package.py`, `inputs/`, `run_host.py`, `compare_runs.py`,
`correlate_packets.py`, `orchestrate_host.sh`, `bootstrap_host.sh`, `test_package.py`,
`outputs/` (raw runs, host identities, `scores.json`, `comparison.json`,
`correlation.json`/`.md`, `delta-vs-packets-20260903.json`, `rental-teardown.json`).

## Verify offline

```bash
cd benchmarks/ai-governance/institution-reputation-packets-20260904
python3 test_package.py && python3 compare_runs.py
python3 correlate_packets.py && git diff --exit-code -- outputs/correlation.json
```

## Correlation

Same deterministic method as the predecessor, reading the profile summaries plus the
input claimed domain. One strong cluster: the three Post Fiat foundation validators.
106 of 1485 pairs share a weak signal.

## Boundaries

Scores and correlation are external `SHADOW_ONLY` research, not consensus data. The
independent human publication review has not been performed. A profile is cited public
research, not proof of legal identity or key control.

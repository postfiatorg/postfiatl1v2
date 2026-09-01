# Deterministic sub-scorer v2 shadow evaluation — per-round tables

Rounds evaluated: 8; fetch/evaluation failures: none.

### testnet-r12 (prompt v5, baseline: model overall score, gap 5)

| Metric | Value |
|---|---|
| Validators compared | 45 |
| consensus mean/max abs delta | 3.6 / 70 |
| reliability mean/max abs delta | 16.22 / 50 |
| software mean/max abs delta | 0 / 0 |
| diversity mean/max abs delta | 14.67 / 30 |
| identity mean/max abs delta | 0.33 / 5 |
| Final score mean/max abs delta | 8.84 / 23 |
| Cutoff flips (out / in) | 1 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 20/20 / 0 |

Cutoff-boundary cases (final within 5 of 40 under either scorer):

| Validator | Baseline final | Deterministic final | Flipped |
|---|---|---|---|
| `nHU5tRRbniyS...` | 45 | 28 | YES |

### testnet-r13 (prompt v5, baseline: model overall score, gap 5)

| Metric | Value |
|---|---|
| Validators compared | 45 |
| consensus mean/max abs delta | 5.07 / 38 |
| reliability mean/max abs delta | 19 / 70 |
| software mean/max abs delta | 0.67 / 10 |
| diversity mean/max abs delta | 10.56 / 30 |
| identity mean/max abs delta | 1.33 / 25 |
| Final score mean/max abs delta | 8.31 / 25 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 20/20 / 0 |

No cutoff-boundary cases.

### testnet-r14 (prompt v6, baseline: model overall score, gap 5)

| Metric | Value |
|---|---|
| Validators compared | 42 |
| consensus mean/max abs delta | 5.31 / 38 |
| reliability mean/max abs delta | 14.88 / 50 |
| software mean/max abs delta | 0.71 / 10 |
| diversity mean/max abs delta | 6.55 / 30 |
| identity mean/max abs delta | 0.6 / 10 |
| Final score mean/max abs delta | 6.02 / 25 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 20/20 / 0 |

No cutoff-boundary cases.

### testnet-r15 (prompt v6, baseline: model overall score, gap 5)

| Metric | Value |
|---|---|
| Validators compared | 50 |
| consensus mean/max abs delta | 4.38 / 38 |
| reliability mean/max abs delta | 20.4 / 75 |
| software mean/max abs delta | 0 / 0 |
| diversity mean/max abs delta | 10.8 / 40 |
| identity mean/max abs delta | 0.8 / 20 |
| Final score mean/max abs delta | 7.92 / 22 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 19/20 / 1 |

No cutoff-boundary cases.

### testnet-r16 (prompt v8, baseline: formula(model sub-scores), gap 5)

| Metric | Value |
|---|---|
| Validators compared | 51 |
| consensus mean/max abs delta | 0.69 / 1 |
| reliability mean/max abs delta | 16.08 / 45 |
| software mean/max abs delta | 10 / 10 |
| diversity mean/max abs delta | 15.88 / 40 |
| identity mean/max abs delta | 0.78 / 20 |
| Final score mean/max abs delta | 2.8 / 8 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 19/20 / 1 |

Cutoff-boundary cases (final within 5 of 40 under either scorer):

| Validator | Baseline final | Deterministic final | Flipped |
|---|---|---|---|
| `nHBWFVzxVYAV...` | 41 | 41 | no |

### testnet-r17 (prompt v9, baseline: formula(model sub-scores), gap 5)

| Metric | Value |
|---|---|
| Validators compared | 53 |
| consensus mean/max abs delta | 0.43 / 1 |
| reliability mean/max abs delta | 2.45 / 35 |
| software mean/max abs delta | 0 / 0 |
| diversity mean/max abs delta | 6.79 / 20 |
| identity mean/max abs delta | 1.32 / 5 |
| Final score mean/max abs delta | 1.17 / 8 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 20/20 / 0 |

No cutoff-boundary cases.

### testnet-r18 (prompt v9, baseline: formula(model sub-scores), gap 5)

| Metric | Value |
|---|---|
| Validators compared | 55 |
| consensus mean/max abs delta | 0.33 / 1 |
| reliability mean/max abs delta | 5.09 / 35 |
| software mean/max abs delta | 0 / 0 |
| diversity mean/max abs delta | 3.36 / 20 |
| identity mean/max abs delta | 1.36 / 5 |
| Final score mean/max abs delta | 1.36 / 8 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 20/20 / 0 |

No cutoff-boundary cases.

### testnet-r19 (prompt v10, baseline: formula(model sub-scores), gap 3)

| Metric | Value |
|---|---|
| Validators compared | 54 |
| consensus mean/max abs delta | 0.37 / 1 |
| reliability mean/max abs delta | 3.98 / 35 |
| software mean/max abs delta | 0 / 0 |
| diversity mean/max abs delta | 4.72 / 20 |
| identity mean/max abs delta | 1.39 / 5 |
| Final score mean/max abs delta | 1.24 / 8 |
| Cutoff flips (out / in) | 0 / 0 |
| Baseline reproduces published UNL | True |
| UNL overlap / seats changed | 19/20 / 1 |

No cutoff-boundary cases.

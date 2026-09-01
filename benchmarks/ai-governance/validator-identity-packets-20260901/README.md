# Validator identity packets: XRPL and PostFiat

**Status:** `SHADOW_ONLY`

**Frozen:** 2026-09-01

**Verification:** **PASS — 55/55**

This corpus contains one foundation-publishable Markdown public-identity packet
and one complete Corbanu Terminal exec JSONL log for every validator in the
frozen validator set:

- 35 XRP Ledger mainnet validators from the identical frozen Ripple and XRP
  Ledger Foundation publisher lists;
- 20 validators from the current completed PostFiat testnet UNL, round 20.

These packets are external research evidence, not consensus data and not
legitimacy, reputation, sanctions, association, credit, or risk scores.

## Execution

Each validator received its own rendered initial prompt and independent
`corbanu --search exec` session:

- Corbanu Terminal: 0.1.36
- configured model: `gpt-5.6-sol`
- provider: OpenAI
- live web search: enabled
- sandbox: read-only
- approval policy: never
- Codex fallback: not used
- OpenRouter: not used

All 55 exec processes returned successfully. The strict finalizer then checked
the Markdown heading contract, exact input coordinates, 90–160 word
single-paragraph business summary, machine-readable JSON agreement, valid JSONL
lifecycle, empty stderr, packet/log final-message equality, and every recorded
hash.

## Organization

```text
inputs/
  validators.json                 exact frozen 55-validator corpus
  index.json                      per-validator coordinates and prompt hashes
  xrpl/<validator>.json           35 unbiased coordinate inputs
  postfiat/<validator>.json       20 unbiased coordinate inputs
prompts/
  xrpl/<validator>.txt            exact Corbanu initial prompts
  postfiat/<validator>.txt
packets/
  xrpl/<validator>.md             generated identity packets
  postfiat/<validator>.md
logs/
  xrpl/<validator>.jsonl          complete Corbanu exec event logs
  xrpl/<validator>.stderr.log     captured stderr
  postfiat/...
runs/
  xrpl/<validator>.json           command receipts, thread IDs, usage and hashes
  postfiat/...
index.md                           human-readable packet index
index.json                         structured packet index and business summaries
manifest.json                      corpus-level hashes and execution contract
verification.json                  55/55 verification result
```

## Packet contents

Every packet has the same headings:

1. packet status;
2. validator coordinates;
3. claimed domain and official URLs;
4. public identity and aliases;
5. one-paragraph business-reference summary;
6. public X handle;
7. incorporation and operating regions;
8. activities;
9. estimated public-profile size;
10. cited evidence;
11. uncertainty and conflicts;
12. machine-readable summary.

Unknown or unsupported fields remain null or explicitly “Not established.”
Packets exclude private contact and residential information. A claimed or
upstream-verified domain is not silently promoted into proof of current
validator-key control.

## Corpus result

- identity established or likely: 45
- identity not established: 10
- public X handle established: 41
- incorporation region established or qualified: 25
- cited evidence URLs: 541

Profile tiers:

| Tier | Count |
| --- | ---: |
| Unknown | 10 |
| Individual | 12 |
| Micro | 15 |
| Small | 8 |
| Medium | 2 |
| Large | 4 |
| Very large | 4 |

## Frozen hashes

| Artifact set | SHA-256 |
| --- | --- |
| source validator corpus | `7687dcd9a23638dca4e0fbe50c2dd3782c6db89fa645802cd5dd9586feb87f27` |
| prompt template | `48a03cabd80cfd0f8fac6ef57cdc700ce1ea45c88a87b9fbdf7f9ee0f6d3769b` |
| all 55 packets | `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd` |
| all 55 exec logs | `3a72e90a410df1c1ce0681f63d5d581ab70d724bda35362f8a03078a305493b1` |
| all 55 run receipts | `94d53c7fd0d0a5e1b1149d5d0b51a0ef81645e90f1b43060da8c75e75955b541` |
| structured index | `52611131e415f6ca47b4365191b86eaf6dc55770d104b1005c71453cf3f8b9f4` |

## Commands

Build the frozen coordinate and prompt corpus:

```bash
python3 build_prompts.py
```

Run missing or stale Corbanu sessions, six at a time:

```bash
python3 run_all.py --workers 6
```

Verify and deterministically regenerate the index and manifest:

```bash
python3 finalize.py
```

## H200 boundary

The downstream H200 run must consume the exact bytes under `packets/` and bind
the packet hash for each validator into every scoring request. It must not call
web search, rerun Corbanu exec, or reinterpret the session logs during replay.
The JSONL logs are publication/audit evidence; the Markdown packets are the
scoring inputs.

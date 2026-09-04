# H200 institution-reputation results from identity packets: XRPL and PostFiat

**Date:** 2026-09-03

**Mode:** `SHADOW_ONLY`

**Replay verdict:** **PASS** (192/192 byte-identical across two distinct-owner H200-class hosts, two runs each)

**Superseded** by the [2026-09-04 identity-only results](institution-reputation-packets-h200-results-20260904.md): the packets scored here carried validator-list membership lines in 42 of 55 cases, which the scoring rule excludes.

## Plain-English result

This is the scoring revision the [scoring rule](institution-legitimacy-scoring.md)
and the [identity-packet runbook](../runbooks/validator-identity-packets.md#10-h200-handoff)
defined and the 2026-09-02 handoff paused: the pinned model no longer sees a bare
name and domain. It receives the exact frozen Markdown identity packet for each
validator, researched earlier by an auditable `corbanu --search exec` session, with
the packet's SHA-256 and the corpus packet-set SHA-256 bound into the request. The
model must first decide which institution the packet identifies, then apply the same
recognition gate and 5-point bands as before. Packet length, citation count, and
stated confidence are forbidden as reasons to raise a score.

The same 55 requests ran twice on an H200 NVL in Czechia and twice on an H200 in
Saudi Arabia, different Vast.ai owners and driver versions. All 192 responses were
byte-identical. The scores below are therefore a property of the frozen packet bytes,
the frozen prompt, and the pinned model revision, and anyone can reproduce them.

Compared with the name-only run of 2026-09-01: **14 validators became
recognized** (0 → positive) because the packet resolved a domain to an institution the
model knows (Berkeley Haas, the Australian National University, Waterloo, UNC
Kenan-Flagler, Bitso, Interledger Foundation, and others); **9 were lowered**
because the packet named the actual operating legal entity rather than a brand (XRPL
Labs → The Integrators B.V., XRPScan → Scrambled Egg Technologies, Bithomp → Bithomp
AB with a one-employee registry record); **none lost recognition**; **32 were
unchanged**, of which all 20 PostFiat validators remain at 0. Zero-scores fell from
45 to 31; the mean rose from 10.84 to 18.87.

The PostFiat result is the honest one: the model does not recognize Post Fiat or its
community validator operators as institutions, and the rule requires 0 for an
unrecognized entity. The foundation-run validators' packets say so themselves
(micro-tier, no established registered legal name), and the model cites that.

## Determinism proof

| Measurement | Result |
| --- | --- |
| Model | `Qwen/Qwen3.8-27B-FP8`, revision `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a` |
| Runtime | pinned SGLang image, loopback only, deterministic inference, radix cache off, CUDA graphs off, seed `438916795` |
| Primary host | NVIDIA H200 NVL, driver 575.57.08, Vast host 214845 |
| Replay host | NVIDIA H200, driver 580.159.03, Vast host 445596 |
| Schedule | two fixed batches of 32 (55 scoring + 9 padding) |
| Byte comparison | 192/192 identical; 165 scoring, 27 padding |
| Aggregate response SHA-256, all four runs | `3458f72dd4614e5e8e02c47f85f68d39ffeae1ff798e8d1c4eb0fceb6c9e4d20` |
| Comparison SHA-256 | `4800d68dbe849969c98a7a4e22d0c6ff97b027c93d1b30f104ce4bf0a8f83c3d` |
| Identity corpus packet-set SHA-256 | `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd` |
| Package | [`benchmarks/ai-governance/institution-reputation-packets-20260903/`](../../benchmarks/ai-governance/institution-reputation-packets-20260903/README.md) |

Verify without paid calls or network access:

```bash
cd benchmarks/ai-governance/institution-reputation-packets-20260903
python3 test_package.py && python3 compare_runs.py
```

## Per-validator scores

Sorted by packet score. Full explanations, bands, and packet hashes are in
`outputs/scores.json`; the per-row delta is in `outputs/delta-vs-unl-20260901.json`.

| Packet score | Name-only | Network | Institution scored from packet | Name-only entity | Sanctions risk |
| --- | --- | --- | --- | --- | --- |
| 78 | 0 | xrpl | Haas School of Business, University of California, Berkeley | — | negligible |
| 72 | 78 | xrpl | Ripple Labs Inc. | Ripple | low |
| 72 | 0 | xrpl | The Australian National University | — | negligible |
| 62 | 0 | xrpl | University of Waterloo | — | negligible |
| 62 | 0 | xrpl | University of North Carolina Kenan-Flagler Business School | — | low |
| 57 | 0 | xrpl | Bitso | — | low |
| 57 | 57 | xrpl | Blockdaemon Inc. | Blockdaemon | low |
| 47 | 52 | xrpl | University of Nicosia | University of Nicosia | low |
| 45 | 72 | xrpl | University of Kansas Institute for Information Sciences (I2S) | University of Kansas | low |
| 45 | 0 | xrpl | Interledger Foundation Inc. | Interledger | low |
| 42 | 57 | xrpl | XRPL Commons | XRPL Commons | low |
| 42 | 52 | xrpl | PEERSYST TECHNOLOGY S.L. | Peersyst | low |
| 42 | 72 | xrpl | The Integrators B.V. | XRPL Labs | low |
| 42 | 52 | xrpl | GateHub Limited | GateHub | low |
| 42 | 0 | xrpl | Stralink Innovations Technologies Limitada | — | low |
| 27 | 52 | xrpl | Scrambled Egg Technologies, LLC | XRPScan | low |
| 27 | 0 | xrpl | Ceiba Network Inc. | squidrouter.com | low |
| 27 | 0 | xrpl | Cabbit Technology LLC | cabbit.tech | low |
| 25 | 52 | xrpl | Bithomp AB | Bithomp | low |
| 25 | 0 | xrpl | onXRP GmbH | onXRP | low |
| 25 | 0 | xrpl | Eminence Ltd | verum.eminence.im | low |
| 25 | 0 | xrpl | Aesthetes S.r.l. | xrpl.aesthetes.art | low |
| 25 | 0 | xrpl | Aureus Ox LLC | aureusox.com | low |
| 25 | 0 | xrpl | Anodos Labs Inc. | Anodos Finance | low |
| 0 | 0 | postfiat | — | pft.permanentupperclass.com | unknown |
| 0 | 0 | postfiat | — | jollydinger.com | unknown |
| 0 | 0 | postfiat | — | pft.hit0ri.xyz | unknown |
| 0 | 0 | postfiat | — | sendoeth.com | unknown |
| 0 | 0 | postfiat | — | dex3333.github.io | unknown |
| 0 | 0 | postfiat | — | pft.bigwoodnode.com | unknown |
| 0 | 0 | postfiat | — | pft.xbtseal.com | unknown |
| 0 | 0 | postfiat | — | postfiat.live | unknown |
| 0 | 0 | postfiat | — | validator.pftperry.com | unknown |
| 0 | 0 | postfiat | — | lc66validator.postfiatcn.org | unknown |
| 0 | 0 | postfiat | — | pft.akirax.xyz | unknown |
| 0 | 0 | postfiat | — | auri0x.io | unknown |
| 0 | 0 | postfiat | — | local-maxi.github.io | unknown |
| 0 | 0 | postfiat | — | Post Fiat | unknown |
| 0 | 0 | postfiat | — | pft.whiteguy.eu | unknown |
| 0 | 0 | postfiat | — | preaware.org | unknown |
| 0 | 0 | postfiat | — | pfthaploid.com | unknown |
| 0 | 0 | postfiat | — | Post Fiat | unknown |
| 0 | 0 | postfiat | — | Post Fiat | unknown |
| 0 | 0 | postfiat | — | pft.wizbubba.xyz | unknown |
| 0 | 0 | xrpl | — | xrp.vet | unknown |
| 0 | 0 | xrpl | — | validator.aspired.nz | negligible |
| 0 | 0 | xrpl | — | ekiserrepe.es | unknown |
| 0 | 0 | xrpl | — | XPMarket | unknown |
| 0 | 0 | xrpl | — | validator.xrpl.robertswarthout.com | negligible |
| 0 | 0 | xrpl | — | xrpkuwait.com | unknown |
| 0 | 0 | xrpl | — | katczynski.net | unknown |
| 0 | 0 | xrpl | — | gen3labs.xyz | unknown |
| 0 | 0 | xrpl | — | jon-nilsen.no | negligible |
| 0 | 0 | xrpl | — | xrpgoat.com | unknown |
| 0 | 0 | xrpl | — | tequ.dev | unknown |

## Validator correlation from the packets

`correlate_packets.py` derives pairwise relationships from the packets' machine-readable
summaries alone, with no model call: shared canonical entity, alias, X handle, or
registrable domain count as strong links; overlap of operator-owned official hosts,
non-generic evidence hosts, and incorporation or operating regions count as weak
signals. Shared-hosting suffixes and platform hosts are excluded and enumerated in the
output. Result: one strong cluster, the three Post Fiat foundation validators (same
entity, `postfiat.org`, `@postfiatorg`); 227 of 1485 pairs share a weak
signal; the strongest non-cluster pair is Ripple Labs / Blockdaemon at 0.38 (Delaware
incorporation and shared evidence hosts). A link means two packets share public-identity
signals; it does not prove common key control. See `outputs/correlation.md`.

## Interpretation

- The packet stage does what it was built for: recognition now depends on researched,
  cited identity rather than on whether a domain string happens to be memorable. It cuts
  both ways, which is the point: brands that read as prominent were re-scored as the
  small legal entities that actually operate the validator.
- All scores remain `SHADOW_ONLY` research. Nothing here touches consensus, validator
  weight, or the UNL. The independent human publication review required by the runbook
  has not been performed and is recorded as such in the package manifest.
- A packet is cited public research, not proof of legal identity or key control; a
  score is model judgment about the institution the packet identifies, not a
  sanctions-database result.

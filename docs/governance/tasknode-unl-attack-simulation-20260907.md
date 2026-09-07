# Task Node UNL attack simulation and weight sensitivity

This evidence document answers the published UNL-from-Task-Node proposal's two
open judge asks: an adversarial simulation and a one-at-a-time sensitivity
analysis of the trust-graph constants. The experiment is deterministic,
synthetic, local, and shadow-only. It authorizes no registry change.

## Result

At the published constants, 14 honest accounts were admissible across
successive one-add rounds. Only the aged-account purchase gained a seat: one
strategically chosen account was sufficient. The other four attacks gained no
seat within their sweeps. Two non-seat failures matter just as much:

- one unit of dust from an active validator created a majority-of-window
  funding edge to an already funded rival and caused the policy's funding
  correlation check to deny that otherwise eligible rival; and
- cross-cluster attack edges could merge clusters that already contained more
  than the two-seat cap. The engine reported the cap breach and admitted no
  attacker, but it did not make the incumbent seats disappear.

| Attack | Cheapest seat-gaining budget | Other observed threshold | Proposal claim |
| --- | ---: | --- | --- |
| Vouch ring | No seat through 64 accounts | Walk mass stayed zero | Supported in this population |
| Buying aged accounts | 1 bought account | 3 common-funded purchases produced an over-cap existing cluster | Partly supported; the first seat is immediately buyable |
| Farming parallel identities | No seat through 16 identities and 32 outside vouches | Two sponsors per identity were not sufficient to cross the mass floor | Seat defense supported; sponsor equivalence not supported |
| First-funder manipulation | No new seat through 64 dust transfers | 1 one-unit transfer denied the rival | Refuted for a quiet-window, already-funded wallet |
| Foundation choosing who wins | No seat through 12 support actions | 2 actions produced an over-cap existing cluster | Seat defense supported; cap stability not supported |

"No seat through" means no seat within the specified sweep, not proof that no
larger or different attack can work. A budget is a maximum available budget.
The detailed tables show both the exact deployed budget and the best seat count
using any tested amount at or below it.

## Method

The driver is
`benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/run_simulation.py`.
It imports and calls the implementation rather than reproducing its rules:

- `tasknode_unl_accountability.evaluate_accountability` computes every
  synthetic account's work score;
- `tasknode_unl_edges.extract_funding_edges` decides whether the dust
  transfers create a first-funder or majority-inflow edge;
- `tasknode_unl_trust_graph.derive_trust_graph` performs edge aggregation,
  row normalization, the exact rational trust walk, connectivity tests,
  conductance clustering, and cap reporting; and
- `tasknode_unl_policy._graph_projection`,
  `tasknode_unl_policy._funding_links`, and
  `tasknode_unl_policy._evaluate_v1_projection` apply the production graph,
  funding-correlation, and final Admission Policy V1 decisions with every
  unrelated synthetic gate set to pass.

The experiment changes no engine module. Sensitivity runs temporarily bind one
published module constant to its low or high value, call the same production
functions, and restore it before the next run. All scoring and walk arithmetic
uses `fractions.Fraction`; reported percentages are integer parts per million
converted for display.

The list starts at 20 seats. Each selected candidate consumes one conceptual
one-add round, matching Admission Policy V1's one-add limit. To isolate the
graph constants, the list size and start-of-window seed vector stay fixed
through each sweep. The seat totals are therefore fixed-window exposure, not a
multi-window forecast in which a newly admitted account becomes a seed.

### Synthetic population

The population generator uses explicit integer seeds:

| Input | Seed |
| --- | ---: |
| Population topology | 2,026,090,701 |
| Accountability evidence | 2,026,090,702 |
| Attack choices | 2,026,090,703 |

The 240 honest Task Node accounts are divided into 20 twelve-account work
communities. Each community has one incumbent validator, giving a list size of
20. The final three incumbents are Foundation-bound. The seed vector is
exactly the proposal's rule: the other 17 ratified accounts, uniformly
weighted. Three sparse links from seeded communities give the Foundation
accounts ordinary graph reach without putting them in the seed vector.

The graph contains 443 directed vouch facts, 260 shared-work facts, and 60
undirected funding facts: 763 facts total, or 3.179 facts per account. Within a
community, one incumbent and one central collaborator share three work units;
peripheral accounts have a co-work ring, directed vouches, and sparse funding
pairs. Fourteen communities have one seat and one connected central candidate.
Three seeded communities are joined to the three Foundation communities and
therefore begin with two seats at the cap. These are deliberately sparse,
plausible densities, not measurements from Task Node.

Accountability evidence is complete for every account. Scores range from 63 to
100, with median 88; 234 of 240 meet the score floor. That high pass rate makes
the graph, not synthetic work failure, the limiting variable. Attack candidates
other than bought accounts receive a score of 100 to test the graph defenses
under a strong attacker.

### Measures

- **Seats** is the number admitted when the exact attack budget is deployed.
- **Best seats <= budget** lets a rational attacker leave resources unused.
- **Controlled mass** is stationary mass on attacker-controlled accounts.
- **Touched-cluster mass** is the largest total stationary mass of a derived
  cluster containing any attacker-controlled account.
- **Connected** counts attack candidates at or above `1/(2N) = 1/40`.
- **Cap clean** is false if the engine reports any cluster above
  `max(2, 10% of N) = 2` after the simulated additions.
- **Floor enforced** is true only if every gained seat meets the production
  connectivity floor. With no gained seat it means the policy admitted no
  below-floor account.

## Attack results

### Vouch ring

The attacker creates a directed ring with no edge from a seed-connected
account. Adding identities never creates trust mass.

| Ring accounts | Seats | Best seats <= budget | Controlled mass | Connected | Cap clean | Floor enforced |
| ---: | ---: | ---: | ---: | ---: | :---: | :---: |
| 0 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 1 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 2 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 4 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 8 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 16 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 32 | 0 | 0 | 0.0000% | 0 | yes | yes |
| 64 | 0 | 0 | 0.0000% | 0 | yes | yes |

This supports the narrow claim that an unconnected vouch ring receives no
mass. It does not test a ring with a purchased or colluding bridge into the
honest graph.

### Buying aged accounts

The attacker buys the highest-mass eligible accounts from distinct clusters.
Each bought account retains its old vouch and co-work edges. A common
majority-settlement funding edge to the buyer is then added.

| Accounts bought | Seats at exact budget | Best seats <= budget | Controlled mass | Largest touched cluster | Connected | Cap clean | Floor enforced |
| ---: | ---: | ---: | ---: | ---: | ---: | :---: | :---: |
| 0 | 0 | 0 | 0.0000% | 0.0000% | 0 | yes | yes |
| 1 | 1 | 1 | 3.3986% | 5.8823% | 1 | yes | yes |
| 2 | 0 | 1 | 6.7972% | 11.7647% | 2 | yes | yes |
| 3 | 0 | 1 | 10.1958% | 17.6470% | 3 | no | yes |
| 4 | 0 | 1 | 13.5945% | 23.5294% | 4 | no | yes |
| 6 | 0 | 1 | 20.3917% | 35.2941% | 6 | no | yes |
| 8 | 0 | 1 | 27.1890% | 47.0588% | 8 | no | yes |
| 12 | 0 | 1 | 40.7835% | 70.5882% | 12 | no | yes |
| 16 | 0 | 1 | 47.5808% | 82.3529% | 14 | no | yes |

The defense does not prevent buying one eligible identity. At exactly two
purchases, the common funding node merges two one-seat clusters: both
candidates still meet the floor, but either addition would be a third seat in
a two-seat cluster. At three purchases, the merged cluster already contains
three incumbents, so the graph reports an over-cap state before admitting
anyone. The cap blocks additions but is not a self-maintaining invariant under
reclustering.

### Farming work with parallel identities

Every farmed identity is given a passing work score, two vouches from
seed-connected accounts in distinct existing clusters, and a common funding
edge to the farmer.

| Farmed identities | Outside vouches | Seats | Best seats <= budget | Controlled mass | Largest touched cluster | Connected | Cap clean | Floor enforced |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: | :---: |
| 0 | 0 | 0 | 0 | 0.0000% | 0.0000% | 0 | yes | yes |
| 1 | 2 | 0 | 0 | 3.9664% | 11.7647% | 0 | no | yes |
| 2 | 4 | 0 | 0 | 7.9328% | 17.6470% | 0 | no | yes |
| 3 | 6 | 0 | 0 | 11.1461% | 23.5294% | 0 | no | yes |
| 4 | 8 | 0 | 0 | 13.5788% | 29.4117% | 0 | no | yes |
| 6 | 12 | 0 | 0 | 20.0054% | 41.1764% | 0 | no | yes |
| 8 | 16 | 0 | 0 | 25.6788% | 52.9411% | 0 | no | yes |
| 12 | 24 | 0 | 0 | 37.7513% | 76.4705% | 0 | no | yes |
| 16 | 32 | 0 | 0 | 50.5771% | 100.0000% | 0 | no | yes |

The aggregate attacker mass grows, but no individual identity reaches 2.5%.
The seat-defense hypothesis holds in this sweep. The proposal's stronger
explanation that the floor "in practice means" two outside sponsors does not:
two sponsors from distinct clusters were necessary here but not sufficient.
The same bridge edges also merge already seated sponsor clusters, so the cap
report becomes unclean at the first attacked identity.

### First-funder manipulation

An active validator sends one-unit transfers to a rival that was first funded
with 100 units before the 180-day window. The real edge extractor correctly
does not call the attacker the historical first funder. It does, however, see
the attacker's dust as more than half of all inbound value *inside the quiet
window*.

| Dust transfers | Funding edge created | Otherwise eligible rival denied | Seats gained | Controlled mass | Cap clean | Floor enforced |
| ---: | :---: | :---: | ---: | ---: | :---: | :---: |
| 0 | no | no | 0 | 3.2844% | yes | yes |
| 1 | yes | yes | 0 | 3.2844% | yes | yes |
| 2 | yes | yes | 0 | 3.2844% | yes | yes |
| 4 | yes | yes | 0 | 3.2844% | yes | yes |
| 8 | yes | yes | 0 | 3.2844% | yes | yes |
| 16 | yes | yes | 0 | 3.2844% | yes | yes |
| 32 | yes | yes | 0 | 3.2844% | yes | yes |
| 64 | yes | yes | 0 | 3.2844% | yes | yes |

This refutes the table's claim that a one-off transfer to an already funded
wallet creates no edge. The first-funder half is safe in this scenario; the
majority-of-window half is not. A minimum absolute-value threshold, a minimum
transaction count, or a denominator that accounts for pre-window funding
would need separate specification and testing. This experiment does not pick
among those remedies.

### Foundation choosing who wins

The Foundation's three incumbent accounts are excluded from the seed vector.
The preferred candidate starts disconnected and receives up to three public
Foundation vouches followed by up to three co-work units with each Foundation
account. No private input or direct seed override is used.

| Support actions | Vouches | Co-work units | Seats | Best seats <= budget | Controlled mass | Largest touched cluster | Connected | Cap clean | Floor enforced |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: | :---: |
| 0 | 0 | 0 | 0 | 0 | 3.5461% | 5.8823% | 0 | yes | yes |
| 1 | 1 | 0 | 0 | 0 | 3.3712% | 5.9287% | 0 | yes | yes |
| 2 | 2 | 0 | 0 | 0 | 3.1909% | 10.3646% | 0 | no | yes |
| 3 | 3 | 0 | 0 | 0 | 3.0052% | 15.6745% | 0 | no | yes |
| 4 | 3 | 1 | 0 | 0 | 4.5232% | 17.6470% | 0 | no | yes |
| 6 | 3 | 3 | 0 | 0 | 4.7239% | 17.6470% | 0 | no | yes |
| 8 | 3 | 5 | 0 | 0 | 4.9635% | 17.6470% | 0 | no | yes |
| 10 | 3 | 7 | 0 | 0 | 5.1529% | 17.6470% | 0 | no | yes |
| 12 | 3 | 9 | 0 | 0 | 5.3128% | 17.6470% | 0 | no | yes |

The preferred candidate never crosses the connectivity floor, so the narrow
seed-exclusion defense holds. This does not establish that the Foundation
cannot choose winners in practice: the Foundation controls the work-score
inputs excluded from this simulation, and two public support actions already
merge seated clusters into an over-cap report.

## Weight sensitivity

Each row varies one constant while every other constant remains at the
published value. The attack vector is ordered **ring / aged / farm / first
funder / Foundation** and reports the cheapest seat-gaining budget. An em dash
means no seat within that attack's sweep.

| Constant | Low -> published -> high | Honest admissions | Cheapest attack-budget vector |
| --- | --- | --- | --- |
| Vouch weight | 1/2 -> 1 -> 2 | 14 -> 14 -> 14 | —/1/—/—/— throughout |
| Co-work weight | 1/2 -> 1 -> 2 | 14 -> 14 -> 14 | —/1/—/—/— throughout |
| Co-work cap | 1 -> 3 -> 5 | 14 -> 14 -> 14 | —/1/—/—/— throughout |
| Funding weight | 1 -> 2 -> 4 | 14 -> 14 -> 14 | —/1/—/—/— throughout |
| Walk damping | 0.75 -> 0.85 -> 0.90 | 14 -> 14 -> 0 | —/1/—/—/— -> —/1/—/—/— -> all — |
| Walk steps | 10 -> 20 -> 40 | 0 -> 14 -> 14 | all — -> —/1/—/—/— -> —/1/—/—/— |
| Conductance cut | 0.05 -> 0.10 -> 0.15 | 14 -> 14 -> 14 | —/1/—/—/— throughout |
| Connectivity floor | 1/N -> 1/(2N) -> 1/(4N) | 0 -> 14 -> 14 | all — -> —/1/—/—/— -> —/1/—/—/— |
| Minimum cluster seats | 1 -> 2 -> 3 | 14 -> 14 -> 14 | —/1/—/—/— throughout |
| Cluster-seat fraction | 5% -> 10% -> 15% | 14 -> 14 -> 14 | —/1/—/—/— throughout |

The three outcome-critical constants in this population are the connectivity
floor, walk iteration count, and damping:

1. Tightening the floor from `1/(2N)` to `1/N` removes every honest
   admission and the aged-account seat. Loosening it to `1/(4N)` changes
   neither result.
2. Ten walk steps leave every candidate below the floor; 20 and 40 produce
   the same 14 honest admissions and the same cheapest attack budget.
3. Raising damping from 0.85 to 0.90 removes every honest admission and the
   aged-account seat because less mass returns directly to the uniform seed
   vector. Lowering it to 0.75 does not change either headline result.

The tested edge weights, conductance threshold, and cap parameters did not
change those two headline measures. That is topology-specific, not evidence
that they are generally unimportant. In this graph, disconnected community
boundaries dominate base clustering, and the connectivity floor dominates
candidate outcomes. The complete 30-row matrix is in `sensitivity.csv`.

## What this strengthens

This run supplies executable evidence for five concrete attack strategies
against the checked-in engine. It strengthens four narrow conclusions:

- disconnected Sybil rings get no personalized-walk mass;
- the connectivity floor prevents low-mass identities from receiving seats;
- the cap projection blocks additions to a cluster already at its limit; and
- excluding Foundation validators from the seed vector prevents
  Foundation-only support from admitting the tested favorite.

It also turns two proposal caveats into actionable findings: the
majority-of-window funding rule permits one-unit tainting after a quiet window,
and graph reclustering can put existing seats above the cap. Those are policy
design findings, not engine nondeterminism or implementation crashes.

## What this does not test

This is not empirical evidence about a real Task Node population. The chosen
densities and community structure are stated assumptions, not fitted data. It
does not test collusion that leaves no vouch, co-work, funding, key, or manifest
edge. It does not test whether the Task Node work, quality, standing, badge, or
accountability-score inputs are true; it supplies complete synthetic inputs
and uses the production formula.

It also does not test payload-review correctness, account sale detection,
operator-key compromise, model replay, Cobalt ratification, registry churn
across changing seed windows, removals needed to repair an already over-cap
cluster, or live consensus safety. A favorable row is synthetic shadow
evidence only.

## Reproduction and evidence identity

Run:

```bash
python3 benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/run_simulation.py
```

The run takes about two minutes on the measurement host; the 40-step
sensitivity endpoint dominates runtime. No network endpoint, credential,
database, wall clock, or ambient randomness is read.

Two complete runs produced byte-identical outputs. The `results.json`
SHA-256 was
`81e27d4c16d689e2a1361b76720abe10030faee0e8962efe6387fd543137c2bd`
in both runs. `determinism.json` records both hashes for every output.
`output-manifest.json` binds the committed JSON and CSV artifacts.

The scored proposal source SHA-256 was
`319c1588c6575f6d9bd6c06c9fc1f055336b00cc394befd3cff54e146d68e584`.
The machine result also records SHA-256 values for all four unchanged engine
modules. The authoritative artifacts are:

- `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/results.json`
- `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/attack-sweep.csv`
- `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/sensitivity.csv`
- `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/determinism.json`
- `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/output-manifest.json`

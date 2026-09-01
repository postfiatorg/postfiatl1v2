# H200 institution-reputation results: XRPL and PostFiat

**Date:** 2026-09-01

**Mode:** `SHADOW_ONLY`

**Replay verdict:** **PASS**

## Plain-English result

The current lists were frozen at one instant: 35 XRPL validator keys and 20
PostFiat validator keys. The same 55 scoring requests were run twice on a
standard H200 and twice on an H200 NVL, on different Vast.ai owners. Every
validator response was identical byte-for-byte across all four runs.

This is the AI question the operator requested: does the pinned model recognize
the claimed institution, does the declared domain plausibly match it, and—only
if it does—what legitimacy and reputational value would that institution bring
to a Layer-1? An unrecognized or mismatched institution receives exactly zero.
Validator uptime and performance are not part of this score.

This does **not** call AI from consensus. It produces a frozen review artifact.
It also does not prove control of a domain or replace a current sanctions
database check. Those are separate identity/compliance inputs.

## Determinism proof

| Measurement | Result |
| --- | --- |
| Model | `Qwen/Qwen3.8-27B-FP8` |
| Revision | `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a` |
| Hosts | distinct owners: H200 and H200 NVL |
| Runs | 2 per host; 4 total |
| Validator byte comparisons | **165/165 identical** |
| Padding byte comparisons | **27/27 identical** |
| Total | **192/192 identical; 0 failures** |
| Aggregate response SHA-256, every run | `a1875309748195422b6bdfd0ac951fda54930a4d0bd3c7090026d1250a7c45cf` |
| Comparison SHA-256 | `7b10b3ba83b79820b48a4355a2a8f12ee021f8f8c5a1634de42a6f2486f088f7` |
| External model API | none; OpenRouter not used |
| Rentals | destroyed after evidence download |

The XRPL Ripple and XRPL Foundation publisher endpoints both contained the same
35 keys. PostFiat used the current completed published UNL, round 20. Exact
source snapshots, prompt, requests, four raw outputs, detailed explanations,
and comparisons are in
`benchmarks/ai-governance/institution-reputation-unl-20260901/`.

## Per-validator scores

The exact two-or-three-paragraph justifications are in
`benchmarks/ai-governance/institution-reputation-unl-20260901/outputs/scores.json`.
“Recognized” is the model's answer under the frozen prompt, not an independent
legal determination.

| Network | Validator key | Claimed entity/domain | Recognized | Score | Band | Sanctions risk |
| --- | --- | --- | --- | ---: | --- | --- |
| `xrpl` | `nHB8QMKGt9VB4Vg71VszjBVQnDW3v3QudM4DwFaJfy96bj4Pv9fA` | Bithomp | true | 52 | `B50` | `low` |
| `xrpl` | `nHBVACxZaNbUjZZkBfj7gRxF3xgG2vbcP4m48KzVwntdTogi5Tfs` | onXRP | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHBWa56Vr7csoFcCnEPzCCKVvnDQw3L28mATgHYQMGtbEfUjuYyB` | xrp.vet | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHBgyVGAEhgU6GoEqoriKmkNBjzhy6WJhX9Z7cZ71yJbv28dzvVN` | XRPL Commons | true | 57 | `B55` | `low` |
| `xrpl` | `nHBidG3pZK11zQD6kpNDoAhDxH6WLGui6ZxSbUx7LSqLHsgzMPec` | — | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHDB2PAPYqF86j9j3c6w1F1ZqwvQfiWcFShZ9Pokg9q4ohNDSkAz` | XRPScan | true | 52 | `B50` | `negligible` |
| `xrpl` | `nHDH7bQJpVfDhVSqdui3Z8GPvKEBQpo6AKHcnXe21zoD4nABA6xj` | — | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHDHzXZKtmMHCkTVgdWY4dqdigDrESiseUF8JkzE93DUtfbt6s3W` | validator.aspired.nz | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHU2k8Po4dgygiQUG8wAADMk9RqkrActeKwsaC9MdtJ9KBvcpVji` | verum.eminence.im | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHU3AenyRuJ4Yei4YHkh6frZg8y2RwXznkMAomUE1ptV5Spvqsih` | xrpl.aesthetes.art | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHU4bLE3EmSqNwfL4AP1UZeTNPrSPPP6FXLKXo2uqfHuvBQxDVKd` | Ripple | true | 78 | `B75` | `low` |
| `xrpl` | `nHUDpRzvY8fSRfQkmJMqjmVSaFmMEVxBNn2tNQy5VAhFJ6is6GFk` | ekiserrepe.es | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUED59jjpQ5QbNhesXMhqii9gA8UfbBmv3i5StgyxG98qjsT4yn` | Blockdaemon | true | 57 | `B55` | `low` |
| `xrpl` | `nHUFCyRCrUjvtZmKiLeF8ReopzKuUoKeDeXo3wEUBVSaawzcSBpW` | — | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUHeq3QdVyLTUENPHAAJ1d5M1SbvY49rajs31mJS8CEfrvTfjn3` | squidrouter.com | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUKgFa4diHC9sN7YnVt4AUPGkaHtShrT76xJBUFdH2B5Tz5nwVQ` | aureusox.com | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHULqGBkJtWeNFjhTzYeAsHA3qKKS7HoBh8CV3BAGTGMZuepEhWC` | — | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUP4RcLQdPHh3kMtFm9NFGnjEYLGXiQAyyB7qFsjATHMw2YVxHi` | XPMarket | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUUgpUVNxXfxkkoyh2QDjfLfHapcut8gYwKeShnJYd3SdPui19A` | Peersyst | true | 52 | `B50` | `low` |
| `xrpl` | `nHUVPzAmAmQ2QSc4oE1iLfsGi17qN2ado8PhxvgEkou76FLxAz7C` | University of Kansas | true | 72 | `B70` | `negligible` |
| `xrpl` | `nHUXeusfwk61c4xJPneb9Lgy7Ga6DVaVLEyB29ftUdt9k2KxD6Hw` | XRPL Labs | true | 72 | `B70` | `low` |
| `xrpl` | `nHUY14bKLLm72ukzo2t6AVnQiu4bCd1jkimwWyJk3txvLeGhvro5` | GateHub | true | 52 | `B50` | `low` |
| `xrpl` | `nHUbgDd63HiuP68VRWazKwZRzS61N37K3NbfQaZLhSQ24LGGmjtn` | — | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUcNC5ni7XjVYfCMe38Rm3KQaq27jw7wJpcUYdo4miWwpNePRTw` | cabbit.tech | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUdjQgg33FRu88GQDtzLWRw95xKnBurUZcqPpe3qC9XVeBNrHeJ` | validator.xrpl.robertswarthout.com | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUfPizyJyhAJZzeq3duRVrZmsTZfcLn7yLF5s2adzHdcHMb9HmQ` | University of Nicosia | true | 52 | `B50` | `low` |
| `xrpl` | `nHUfxETNHsA9reyYCVYwNztEbifMg6U9YUdcgVvzMwGNpphKSSf6` | xrpkuwait.com | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUge3GFusbqmfYAJjxfKgm2j4JXGxrRsfYMcEViHrFSzQDdk5Hq` | katczynski.net | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUinfdpmtfXsMUnoSoBeZ93mTwDkPkA2gmCiQTyEi1c1Ybx6unE` | gen3labs.xyz | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUr8EhgKeTc9ESNt4nMYzWC2Pu7GgRHMRTsNEyGBTCfnHPxmXcm` | Anodos Finance | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUrUNXCy4DgPPNABX9C6mUctpoq7CwgLKAUxjw6zYtTfiqsj1ew` | Interledger | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUryiyDqEtyWVtFG24AAhaYjMf9FRLietbGzviF3piJsMm9qyDR` | — | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUvcCcmoH1FJMMC6NtF9KKA4LpCWhjsxk2reCQidsp5AHQ7QY9H` | jon-nilsen.no | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUwGQrfZfieeLFeGRdGnAmGpHBCZq9wvm5c59wTc2JhJMjoXmd8` | xrpgoat.com | false | 0 | `B00` | `unknown` |
| `xrpl` | `nHUxjxKPeErbN7pNk9UWA5Ee7ZPMtesSeRGJtmdqkTxe94tqM2YX` | tequ.dev | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHBM2nzq3pZUg8JsxvEt3G7gAAtc5Sukaef6YmvVx64uAoRK4QWM` | jollydinger.com | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHBYKjxjbzxRzrS3XkhpgM7KXJ25jbgPHkbAjjSCQPm8PaPS5y4v` | pft.hit0ri.xyz | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHBcLEB4S6moQGrhMjJo1jbp58WL5psHY9EMDWNAtdqykUYiA1rF` | sendoeth.com | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUUXMXfPEdnKAT8u2AB89LxTWT1tWsTecDPQURoMw2XJ2WP85MK` | pft.akirax.xyz | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUdwzTWTQJzebbxcanZG2ERXikMLU9aAZa8cHtxosfiKq5N7Vd5` | pft.whiteguy.eu | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUzc5XzsmweRV6aNyUQJD5eRUM4TT4tVCBv5DKx6T7Buq4gn9Lr` | pft.wizbubba.xyz | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHBmj7zUY2yMc9f5wgNcCuWj79BUieNmc1GG8PqiEP1tgNZZWLVM` | dex3333.github.io | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHDHCvsJi8UwHMAbkinuJhPYsG5ZR9rtmGAjJZwo8uibf8N1tz3e` | pft.xbtseal.com | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHDfSHLutR7QqttAkx3AiYG2bukCdfAa3P9hrYL4yNJZQpvEpW1A` | postfiat.live | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUCEXpC5LhFAm1Mmf8TqrzVGt3QCuwWoW2V8PYynDpjZe8m8mHj` | lc66validator.postfiatcn.org | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUWciHX8W9PgM3sQmgkRiKpkuaJjTxFSyvZce4bP4WeMz81HefX` | auri0x.io | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUatddiVB2GN6zHHCk1gtepjANF8BdjPKuVcN6jwG2JwBYPea3k` | local-maxi.github.io | false | 0 | `B00` | `negligible` |
| `postfiat` | `nHB6Zc7mhr7swksEgpwTE7Hw7SvZ9cz22T2MECMSXjBeMTeHXQB7` | pft.permanentupperclass.com | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHByMXejvHJgjcGJ1f9bhcAGcFeNR6ecsDmzN4t3HkhyRHZtM6Lj` | pft.bigwoodnode.com | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUhL4QzULuXt2WK5v5mvzEMZSo6wM9ZWUaaQC1eYw2qa1ATFw1c` | preaware.org | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUc7VSYA6xvFakSvuTojJQucBNukKwmtguUG2HMT9Xp9dKzkpvJ` | Post Fiat | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUkhbZe9ncdmhn6dbd5x7391ymwCS3YZEMWjysP9fSiDtau9YEe` | Post Fiat | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUso5gdgQnewAsk5QT1aFr897g6YLaL697iyuknmSqd7pbqz5Td` | Post Fiat | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHU74qX4tCQDSpE6zBS5PB3jybuGZJ7QMbeyLWDQRy3Lhb4DYDSR` | validator.pftperry.com | false | 0 | `B00` | `unknown` |
| `postfiat` | `nHUif4sukXu9pJGyyBaeVMwmE8L1fJ5KJj4X4ksgTKhgjG6k96s2` | pfthaploid.com | false | 0 | `B00` | `unknown` |

## Interpretation

The positive XRPL results were Ripple (78), University of Kansas (72), XRPL
Labs (72), XRPL Commons (57), Blockdaemon (57), and five entities at 52:
Bithomp, XRPScan, Peersyst, GateHub, and University of Nicosia. All other XRPL
claims scored zero under the hard recognition/domain-pairing rule.

Every current PostFiat claim scored zero. That does not mean the validators are
malicious or technically poor; it means the pinned model did not recognize a
qualifying institution/domain pairing and the prompt forbids partial credit in
that case. This is precisely the requested recognition-first behavior.

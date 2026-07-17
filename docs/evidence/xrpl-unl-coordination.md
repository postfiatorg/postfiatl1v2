# XRPL UNL Coordination Snapshot

Status: live-data evidence packet, no claim of private control.

This packet tests one narrow question:

> How concentrated is the live XRPL default validator-list surface that public
> servers are expected to coordinate around?

It does not prove who talks to whom privately, and it does not prove that one
person controls every validator. It proves publisher-list convergence and the
current amendment-state facts exposed by public XRPL endpoints.

## Command

```bash
scripts/xrpl-unl-coordination-snapshot \
  --out reports/xrpl-unl-coordination/20260527/live-snapshot.json
```

## Live Result

The 2026-05-27 snapshot fetched the live validator-list blobs from:

- `https://vl.xrplf.org/`
- `https://vl.ripple.com/`

It also checked the public JSON-RPC `feature` and raw `Amendments` ledger object
through `https://s1.ripple.com:51234/`.

Result:

```text
XRPLF list:        35 validators
Ripple list:       35 validators
Shared validators: 31
Union:             39
XRPLF-only:         4
Ripple-only:        4
Jaccard overlap:    0.7948717949
Overlap/list:       0.8857142857
```

The validator-list membership buckets were:

```text
ripple+xrplf: 31
xrplf only:    4
ripple only:   4
```

This produces an effective publisher-membership bucket count of:

```text
N_eff = 1.5317220544
```

That number is not a validator-independence count. It is a concentration
measure over publisher-list membership buckets. It says the two public
publisher lists mostly define one shared default validator core, plus two small
marginal tails.

## Amendment State

The same snapshot checked `fixCleanup3_1_3` against the raw on-ledger
`Amendments` object.

```text
fixCleanup3_1_3 enabled: true
feature RPC visible:     false
id source:               sha512_half_name
id: 303ACB16CF8DBD3B5C34F131A9D19A7DE01AE05F480A8A682B869D1B4AAC8CFC
```

The important governance fact is not that `fixCleanup3_1_3` was controversial.
The important fact is that amendment activation is trusted-validator
coordination around software support and UNL membership, not raw node count.

## What This Proves

This supports a narrow, defensible claim:

> XRPL's live default trust surface has much lower publisher-list diversity than
> its nominal validator count suggests. The two major public list publishers
> currently expose a 31-validator shared core out of 35 validators per list.

It also supports the PostFiat design choice:

> Authority validation can be good, but authority-list mutation should be
> protocol state with evidence roots, safety witnesses, challenge state, and
> explicit transition certificates rather than opaque list-publisher
> coordination.

## What This Does Not Prove

This snapshot does not prove:

- private Telegram coordination;
- a single human decision-maker;
- validator vote timing or upgrade timing;
- validator economic independence;
- validator-list signature validity, because the current script records blob
  hashes and signature presence but does not perform local Ed25519 validation.

Those require a time-series event study:

1. capture amendment votes over many ledgers;
2. record publisher-list changes and software-default changes;
3. cluster validators by vote timing and upgrade behavior;
4. compute behavioral `N_eff`;
5. compare observed synchrony against a null model of independent validator
   adoption.

## Artifact

Primary artifact:

```text
docs/assets/evidence/xrpl-unl-coordination/live-snapshot-20260527.json
sha256: 7161388992dfb881df01015322b09ae79740882f588cdf3caf317d0bdf03d7b2
```

Local ignored working copy:

```text
reports/xrpl-unl-coordination/20260527/live-snapshot.json
```

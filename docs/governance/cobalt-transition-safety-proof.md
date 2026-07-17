# Cobalt Transition Safety Proof Fixture

The Cobalt transition-safety proof fixture makes the whitepaper's Section 2
proof obligations executable. It is a controlled-testnet checker for bounded
old/new registry transitions, not a replacement for a production consensus
implementation.

## Checked Obligations

| Obligation | Check |
| --- | --- |
| Local Cobalt rows | Every active subset satisfies `0 <= t_S,q_S <= n_S`, `t_S < 2q_S - n_S`, and `2t_S < q_S`. |
| Global budget binding | The transition budget must satisfy `B <= min(t_S)` across the extracted old and new cover. |
| Cover bound | The old plus new cover must fit inside `M_cover`. |
| Old-checker authority | The parent transition must be validated by the previous active checker. |
| Challenge state | The challenge state must be closed before activation. |
| Key-continuity intersection | Every covered old quorum and new quorum must share more than `B` key-continuity validators. |
| Same-registry conflict | Conflicting certificates under one registry reject because their quorums contain a correct shared signer. |
| Old/new conflict | A new certificate that does not extend the imported old lock rejects. |

## Fixture

The valid fixture models a one-validator rotation in a 10-validator subset:

```text
old = {A,B,C,D,E,F,G,H,I,J}
new = {A,B,C,D,E,F,G,H,I,K}
q = 8
t = 2
B = 2
```

Every 8-of-10 old quorum and every 8-of-10 new quorum share more than `B`
validators with key continuity. The fixture also includes two negative
certificate cases inside the valid packet: a same-registry conflict and an
old/new child that does not extend the imported old lock.

Separate invalid fixtures cover intersection failure, `B > min(t_S)`,
incomplete local rows, open challenge state, oversized cover, missing
old-checker validation, and missing key continuity.

## Verification

```bash
scripts/cobalt-transition-safety-proof-verify --fixtures
scripts/cobalt-transition-safety-proof-verify --write-report
scripts/cobalt-transition-safety-proof-verify --verify-report
```

Current fixture roots are recorded in the report:

```text
reports/cobalt-transition-safety-proof/20260529T081141Z/cobalt-transition-safety-proof-report.json
```

| Root | Value |
| --- | --- |
| Valid packet hash | `0865f7610cf867d98c90f076959f3e8c314bcb054a5ce8fad900e9f3830b414695ab3ffa5e5bd1d1db9d7e69edd3c6bf` |
| Material root hash | `eb23af6dbb175cdb496550dd18294c7f99864e59e8264f4ffa0cc551966ae53b67d749a4dc35cbe26135e0acd32ec297` |
| Statement hash | `77970fea7f82efe2d9f5fb64c12abfb2769648f9f769f9fbf911798a76972de5ddc1db1426c10bffd2319b1d3af4e5a6` |

# pfUSDC transferable-inventory report — 2026-07-23

Status: **NO TRANSFERABLE INVENTORY / NO MINT / NO MUTATION**.

The signed, independently imported height-296 checkpoint contains seven
pfUSDC trustlines. Every balance is zero, including holder
`pfab9b9228942e5c529633a13aa271d5297bec6353`. Orchard accounting is
`ingress=80`, `egress=80`, `live=0`. Therefore no controlled wallet or live
private note has transferable pfUSDC.

The finalized NAV circulating supply is 10 atoms. Those atoms are already
accounted for by the sole pending vault-bridge redemption:

```text
redemption 316ccbb79d8778a1cbae5e8e68103c30ec6690e314eb5e8c1eb4aa453bc16f11b057aa3119b52edb76eca2e4c057d4d7
owner      pfab9b9228942e5c529633a13aa271d5297bec6353
amount     10
settled    0
state      pending
```

The active bucket confirms `counted_value=10`, `redemption_queue=10`,
`outstanding=0`, and no other allocation. This pending redemption is queue
state, not a wallet balance, and is not transferable through an issued
payment.

No mint or transfer was attempted. The legitimate funding path is a new
backed dust bridge-in, or the separately authorized founder finalizing
five-dollar bridge-in. The latter was not read, consumed, or modified.

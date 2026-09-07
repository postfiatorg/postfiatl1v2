# Configurable funding denomination

Status: user-directed implementation amendment, 2026-09-05.

The user explicitly rejected an arbitrary 1 ETH funding requirement and directed
implementation of a smaller functional pilot. This amendment supersedes the
1 ETH route size in the original Hyperliquid and Lighter specifications for the
configurable implementation. Their scored source files remain unchanged as
historical research evidence. No new research score is claimed for this amendment.

## Amount selection

- A new source check, proof preparation or venue settlement must specify its
  principal. Missing principal must not silently select 1 ETH.
- Accept decimal ETH strings at the CLI boundary. Convert once to positive
  integer pfETH atoms, with nine decimal places and the native u64 ceiling.
  One pfETH atom maps exactly to 1,000,000,000 wei. Reject float input,
  excess decimal precision, zero, negative quantities and overflow.
- Keep the exact selected principal through deposit, mint, shield, private
  egress, certified burn and WETH release. Bind it to the durable operation and
  signed calldata. Changing the amount cannot reuse an existing proof or payment.
- The current local pilot example is 0.005 ETH, or 5,000,000 pfETH atoms.
  It is configurable and is neither a protocol minimum nor a production price.
- Cover reservations use `denomination_atoms * 1,000,000,000` wei per note.
  A seven-note example at 0.005 ETH reserves 0.035 ETH of principal, not 7 ETH.
  User wallet initialization checks one selected principal rather than the
  operator minimum inventory. Cover requirements must not be imposed on a mechanics-only test as evidence
  that the source wallet lacks enough principal for its selected deposit.

## Actual route constraints

Keep token precision, contract amount limits, current venue minimums/capacity,
and separately bounded gas, native PFT and proof costs. Do not substitute a
chosen USD figure for those requirements. Hyperliquid output need not be a
multiple of 100 USDC: use an exact selected output that satisfies Bridge2 and
the quoted swap bound. Lighter must satisfy its current asset metadata and
on-chain deposit tick/minimum/capacity. Recheck the route before a new send.

## Privacy and functional evidence

Use the versioned `funding-destination-denomination-v3` policy for configurable
analysis. Bind the report and authorization to the selected denomination and
venue. Assignment edges require exact asset and amount equality. Retain other
amounts and venues as spend/ownership constraints rather than silently removing
history. A 0.005 ETH exit cannot borrow candidate destinations from 1 ETH exits.

A small functional run establishes only the mechanics it actually executes.
It does not qualify another denomination, invent independent participants,
or turn simulation receipts into funded privacy evidence. Existing requirements
for complete observed history and separately reported public/operator views
continue to apply when a privacy claim is requested.

Legacy persisted whole-ETH operations may still be reconciled against their
original signed bytes. New operations must select the amount explicitly.
Legacy count-based planning is not the privacy authorization path for this
configurable implementation.

## Implementation and evidence

The active Task Node implementation tasks remain
`task_ed942369a9d92007a389dc32ffbefa5c` and
`task_1b81da68342923f814770ea149d3f048`.
See the [Hyperliquid milestone](../plans/active/shielded-hyperliquid-cover-implementation-20260905.md)
and [Lighter milestone](../plans/active/shielded-lighter-funding-implementation-20260905.md).

StakeHub code: `shielded_exit_amount.py`, `shielded_exit_source.py`,
`shielded_exit_local.py`, `shielded_exit_pftl_transport.py`,
`shielded_exit_bridge.py`, `shielded_exit_cover.py`, `shielded_exit_privacy.py`,
`shielded_exit_flow.py`, both venue settlement adapters and the funding CLI/UI. The older planner,
withdrawal observer and job builder also retain exact decimal bands and no
longer reject a quantity merely because it is not a whole ETH. Their historical
count labels remain distinct from complete-history privacy authorization.

Native verification at 0.005 ETH equivalent includes real encrypted ingress,
private-egress proof creation and verification, local batch simulation,
applied receipt identities, exact restored balance and restart reuse. This is
a disposable local-chain test, not a new mainnet deposit.

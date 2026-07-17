# Deposit, Spend, Withdraw

PostFiat's current shielded flow has three production-shaped operations:

1. transparent-to-Orchard deposit;
2. Orchard private spend;
3. Orchard-to-transparent withdraw.

## Shielded Action Structure

A shielded action exposes enough data for consensus verification while keeping
the spent note, owner, amount, asset details, and memo inside the proof.

```mermaid
flowchart LR
  subgraph Public[Public fields visible to validators]
    Root[anchor root<br/>accepted commitment root]
    Nullifier[nullifier<br/>double-spend prevention]
    Outputs[output commitments<br/>new encrypted notes]
    Fee[fee<br/>burned in transparent accounting]
    Burn[burn amount<br/>transparent principal destroyed on deposit]
    Policy[policy hash<br/>governed action rules]
    Disclosure[disclosure hash<br/>holder-controlled disclosure binding]
  end

  subgraph Hidden[Hidden witness inside the proof]
    Asset[asset id]
    Value[value amount]
    Owner[owner spending authority]
    Memo[memo]
    Randomness[note randomness]
    Path[commitment-tree witness path]
  end

  Public --> Verify[Consensus verifier checks<br/>proof validity<br/>nullifier uniqueness<br/>root availability<br/>resource limits<br/>policy binding]
  Hidden -. proven by Halo2 without reveal .-> Verify
```

## Turnstile Accounting

The transparent and shielded states meet at a turnstile. Supply integrity is
checked with public counters in addition to proof verification.

```mermaid
flowchart LR
  subgraph Transparent[Transparent state]
    Accounts[Account balances<br/>public and auditable]
  end

  subgraph Shielded[Shielded pool]
    Deposits[Net deposits<br/>cumulative value entering]
    Withdrawals[Cumulative withdrawals<br/>value exiting]
    Notes[Note commitment tree<br/>plus nullifier set]
  end

  Accounts -->|deposit burns transparent principal| Deposits
  Deposits --> Notes
  Notes -->|private spend| Notes
  Notes -->|withdraw reveals recipient envelope| Withdrawals
  Withdrawals -->|credit transparent recipient| Accounts

  Invariant[Turnstile invariant<br/>cumulative withdrawals <= net deposits<br/>violation freezes shielded action class]
  Deposits -. counter input .-> Invariant
  Withdrawals -. counter input .-> Invariant
```

## Deposit

A deposit envelope contains a signed transparent funding transfer to the
protocol burn sink and an Orchard/Halo2 output action. The Orchard action binds
the funding transfer id, amount, fee, policy id, and disclosure hash into the
authorization domain.

Accepted apply:

- verifies funding signature and sequence;
- burns transparent principal plus deposit resource fee;
- mints the Orchard note;
- updates pool roots and public counters;
- leaves the recipient note scan-spendable.

## Spend

`orchard-spend-create` builds a real Orchard spend from one decrypted note. It
can send full value minus fee or send an amount plus default change.

Accepted apply:

- verifies the Orchard proof;
- persists the nullifier;
- appends output commitments;
- burns the signed fee;
- rejects duplicate nullifiers.

## Withdraw

`orchard-withdraw-create` builds a one-note withdraw action bound to a
transparent recipient envelope.

Accepted apply:

- verifies the external binding;
- nullifies the spent note;
- burns the signed fee;
- credits the transparent recipient in the same committed block.

## Evidence

- `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-20260515T132406Z/testnet-orchard-wallet-finality-smoke.json`
- `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-20260515T134027Z/testnet-orchard-peer-certified-smoke.json`
- `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T153630Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`

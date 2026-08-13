# pfUSDC pool protocol repair — live recovery report

Date: 2026-08-13 UTC

## Outcome

The source-labeled pfUSDC protocol repair is live on all six validators. The
already-finalized 15.000000-USDC Ethereum deposit was claimed without another
Ethereum approval or deposit transaction.

- Ethereum deposit transaction:
  `0x7d91722c2e8071827a5d06144d30b2612af319fd9b0d76687bbb014c9ebc6364`
- Deposit ID:
  `0xa0ce20f2cce4131b43dfa0108856e731a0be3c232f627a061d4e09ccf9f64266`
- PFTL claim height: `906`
- PFTL receipt:
  `0543772189803d79e1ed3e1b69ad6206cb4e6c0d230a0ac2d3b25d52f98cfcdd4964b9e24c79f6f8b240c91e53c66e88`
- Source-series asset:
  `c6d2a21646091dc721332d114f6288ede715d0a6325b3cf41ec7b4865f9f2e85014e79445fb66a7141747e4f42584062`
- Source-series balance added: `15,000,000` atoms (`15.000000 pfUSDC`)
- Legacy pooled balance: unchanged at `73,097,570` atoms
- Wallet family balance: `88,097,570` atoms
- Six-validator state root:
  `d62ec7003ed36a90c767cbf6cf306184258104b5b0d2a72fe2bdb78f90e7f0c018042398cbfd319824ddb40d724d2cce`
- Six-validator mempool: empty

The new receipt is counted at `15,000,000` atoms and has one exact
`15,000,000`-atom `vault_bridge_supply` allocation. The active epoch-6 bucket
is healthy, has impairment factor `10,000` basis points, and reports counted
value equal to outstanding obligations.

## Deployment and activation

- Protocol commit: `8a62cf95b3bdd05c40ebf762d1c9c6791bfd1fac`
- Wallet migration commit: `5b93aed`
- Release: `pfusdc-pool-repair-8a62cf9`
- Validator binary SHA-256:
  `e6b31e715a025170747b4222f4afd703e0d9a4e7fe7f6ac998715848905d0ec5`
- Orchard-aware claim amendment: certified at height `903`, activates at
  height `906`
- Source-series amendment: certified at height `904`, activates at height
  `906`
- Height `905` was an empty activation barrier. The generic round wrapper
  records `round_ok=false` because an empty batch has no accepted transaction,
  but all six validators certified and converged at height `905`; the exact
  six-node ingress preflight was `ready=true` for height `906`.

## Durable job migration

The old job snapshot contained the retired `growing_backed_cap` stage. The
approved migration verified its exact config digest and the complete chained
hashes of all six pre-claim checkpoints, then archived them under the job's
`migrations/remove-fake-cap-2056911b955a` directory. It installed the pinned
six-stage driver and replayed only idempotent checks over the existing deposit,
witness and proof. Its migration policy explicitly sets
`ethereum_transaction_replay_allowed=false`.

## Replay result

A new-sequence admission attempt for the same claim failed with:

```text
mempool admission rejected `proof_bounded_nav_cap_exceeded`:
claim exceeds finalized unclaimed Ethereum SP1 backing
```

The attempt left height `906`, the state root, and the zero-entry mempool
unchanged.

## Verification

- `postfiat-types`: 122 passed
- `postfiat-execution`: 189 passed
- `postfiat-node`: 267 passed, 0 failed, 2 ignored
- wallet library: 262 passed
- wallet proxy suite: 35 passed
- durable driver, live-wrapper, HTTP and retry/resume regressions: passed
- wallet production build: passed
- local wallet API and localhost TLS edge: healthy

## Contract decision

No new Ethereum contract was required. The active epoch-6 vault already held
the finalized USDC and had the correct immutable route binding. The defect was
in PFTL source accounting, activation, preflight, and the wallet relay's stale
release/checkpoint binding. Deploying another contract would not repair the
immutable, impaired epoch-5 vault; that historical bucket remains explicitly
impaired unless supported by a valid old-vault proof path or real
recapitalization.

## Remaining acceptance item

The stuck live ingress and its replay protection are complete. The separate
fresh 1.000000-USDC ingress/private-custody/egress acceptance loop in Phase 8
still requires a new user-authorized Ethereum deposit and is not represented as
completed by this recovery.

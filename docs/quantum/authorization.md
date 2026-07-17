# Quantum Authorization

PostFiat starts the base transparent account and validator authorization paths
with ML-DSA-style signatures.

## Why

A new settlement chain does not need to begin with long-lived classical account
and validator keys and hope to migrate later. If quantum migration risk matters
on an institutional horizon, the chain should price larger signatures and
certificates from genesis.

## What Exists

- post-quantum account signing flows;
- wallet vectors;
- post-quantum validator transport envelopes;
- certificate handling for larger signature material;
- ML-DSA performance and size evidence;
- SDK wallet finality using signed transparent transfers.

## ML-DSA Certificate Structure

```mermaid
flowchart TD
  Header[Block header<br/>height, parent, state root,<br/>payload hash, certificate digest]
  Cert[Detached ML-DSA certificate<br/>registryRoot<br/>certificate domain<br/>validator signatures]
  Pair1[validatorID + ML-DSA signature]
  Pair2[validatorID + ML-DSA signature]
  PairN[validatorID + ML-DSA signature]
  Registry[Active registry root<br/>validator identities and public keys]
  Quorum[Quorum check<br/>q = floor(2n/3) + 1]

  Header --> Cert
  Cert --> Pair1
  Cert --> Pair2
  Cert --> PairN
  Cert --> Registry
  Pair1 --> Quorum
  Pair2 --> Quorum
  PairN --> Quorum
  Registry --> Quorum
```

## Post-Quantum Authorization Flow

```mermaid
flowchart LR
  Wallet[Wallet signs transaction<br/>ML-DSA account key]
  Submit[Submit signed transfer<br/>bounded write path]
  Validators[Validators verify<br/>signature, sequence, fee, balance]
  Order[Order and certify block<br/>validator ML-DSA votes]
  Header[Block header commits<br/>detached certificate digest]
  Audit[Audit nodes fetch certificate<br/>and re-verify signatures<br/>against registryRoot]

  Wallet --> Submit --> Validators --> Order --> Header --> Audit
```

## Recovery Path

```mermaid
flowchart TD
  Monitor[Cryptographic monitoring<br/>ML-DSA break or credible emergency]
  Emergency[Governed emergency path<br/>freeze affected authorization class<br/>collect evidence packet]
  Cobalt[Cobalt-ratified activation<br/>old rules authorize recovery]
  SLH[Activate precommitted SLH-DSA keys<br/>already bound at genesis]
  Rotate[Rotate validator and account authority<br/>new signatures accepted<br/>old ML-DSA path rejected or constrained]

  Monitor --> Emergency --> Cobalt --> SLH --> Rotate
```

## Sources

- `crates/crypto_provider/src/lib.rs`
- `crates/types/src/lib.rs`
- `crates/node/src/transport_cli.rs`
- `crates/rpc_sdk/src/lib.rs`
- `scripts/testnet-ml-dsa-performance-smoke`
- `scripts/testnet-wallet-test-vectors-smoke`

# New Schwab NAVCoin feasibility on release 4

This branch starts at `abc228eda0599256c4a760e575e43f9fd1914508` and changes no
consensus crates. It records where a new asset can be created and where real
attestation, finality and return compatibility prevent the requested round trip.
See `report.md` for the decision and founder dependencies.

`check_copy.py` uses only an exported snapshot and two deterministic PUBLIC TEST
wallets. It adds fee balances for those new accounts in the copy, preserves all
existing rows, and invokes the actual release-4 transaction executor. Rejected
operations must leave the ledger unchanged. It never calls a transaction RPC.
The snapshot tip is provenance, not finality for the resulting test states.

Build and run (substitute public artifact paths):

```sh
cargo build --locked --release --manifest-path tools/new-navcoin-canary/Cargo.toml
python3 ops/new-navcoin/check_copy.py \
  --snapshot /path/to/read-only-live-snapshot.json \
  --capture /path/to/opening3-run --proof /path/to/real-proof \
  --node /path/to/release4/postfiat-node \
  --canary /path/to/release/postfiat-b23-nav-canary \
  --nav-cli /path/to/release/postfiat-b23-nav \
  --output /new/output/directory
```

`export` in the Rust helper opens `NodeStore::try_new_read_only` and requires an
unchanged chain tip across the read. `inspect-route` checks the legacy receipt
fixture's commitments using release-4 types. `decode-return` exercises release
4's actual V1/V2 decoders on a V3 event. `packet-context` is retained from the
TEE lane's read-only canary helper for reserve-packet arithmetic.

`receipt-qualification/` contains the ancestry-patched Solidity source from the
security candidate and genuine Groth16 proofs of an archived **synthetic test
committee** receipt and rotation. The receipt proof was freshly generated on
the authorized GPU; the rotation proof is the previously qualified artifact.
They are unrelated to Schwab backing or the
live devnet genesis. They are not evidence that an export transaction executed.
The tests run constructors at fixture-bound addresses in local EVM state. They
do not inject accepted-proof flags, mint balances, or consumed-proof state.

```sh
python3 ops/new-navcoin/qualify_archived_receipt.py \
  --block RECENT_ETHEREUM_BLOCK --output /new/fork/output
```

This starts two owned loopback Anvil forks and runs 13 tests per fork against the
actual mainnet SP1 gateway runtime at
`0x397a5f7f3dbd538f23de225b51f532c34448da9b`. The test configuration pins the proof
selector directly: this gateway does not expose a `VERIFIER_HASH()` getter.
Use a recent block with a non-archive public RPC. The tests cover real-proof
acceptance, exactly-once mint/burn, invalid proofs, wrong domain/program/selector,
changed verifier code, untrusted committees, replay and retired committees.
They do not seed a pool or import a burn to native L1.

`prove_archived_fixture.sh` is an opt-in reproduction command for the already
authorized Vast host, not a cloud provisioner. It locks
`/workspace/b23-gpu/prover.lock`, requires the existing SP1 server to be exactly
6.3.1, and proves only the named public fixture using the frozen V2 ELF. It does
not install/replace the GPU server or change a running service. Input hashes and
output proof reports identify exactly what was proved.

The compact `evidence/` directory contains successful checks and provenance.
Full snapshots and intermediate rejected attempts remain under the coordinator's
`evidence/pool/phase2/`. No production wallet material is included.

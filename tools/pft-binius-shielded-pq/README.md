# Peel-Binius-Shielded (`PFSH02`)

Post-quantum global-pool confidential payment prototype for Post Fiat L1:

- **Depth-24 Global Merkle Accumulator (`2^24 = 16,777,216` coins)** using NIST FIPS-202 `SHAKE256` (56 Keccak-f[1600] permutations, native `is_dummy` gating, 66-bit exact integer conservation covering ZK-Shielded Mint `v_mint`, 2-in/2-out transfers, 1-in+1-dummy transfers, and transparent Unshield exits `v_unshield`).
- **0-Round Non-Interactive Stealth Receive** using NIST FIPS-203 `ML-KEM-1024` + `HMAC-SHA-384` KDF + `AES-256-GCM` (`AAD = coin`), with `tx_ctx` header/envelope binding and zero on-chain signatures on shielded spends.
- **512-Bit-State Single Fixed-Range Compiler Oracle** (`H: {0,1}^* -> {0,1}^{1536}` via `3 x SHA-512`), disjoint from in-circuit `SHAKE256`.
- **Dual-Language Implementation**: Rust streaming ANTT prover/verifier (`rust-prover/`) and Go 1.25.1 verifier & offline ML-KEM-1024 scanner (`go-verifier/`).

Each envelope derives its 32-byte AES key and 12-byte nonce by truncating separate HMAC-SHA-384 outputs. The MAC key is the 32-byte KEM shared secret. The MAC input is the ASCII label (`PFT-SH-KEM-KEY-V2` or `PFT-SH-KEM-NONCE-V2`), followed by the 1,568-byte KEM ciphertext and the 48-byte output coin. Earlier prototype envelopes use a different KDF and require their original scanner.

## Depositor authorization

A positive mint appends the 1,952-byte ML-DSA-65 public key and 3,309-byte signature after the length-prefixed proof. The source account is the canonical Post Fiat address derived from that key. The wallet signs `SHA384("PFT-SH-MINT-AUTH-V2" || ASCII(source) || tx_ctx)` with the L1 FIPS-204 context `postfiat-l1-v2/tx/v1`. The context commits to the amount, ordered output coins and envelopes, and the rest of the public header. Both parsers require the exact suffix for positive mints and end-of-input after the proof for all other transactions.

The Rust wallet entry point `create_shielded_tx` takes the depositor’s `fips204::ml_dsa_65::PrivateKey` for a positive mint and returns the signed transaction. `MintAuth::source()` yields the account to debit after verification. Go 1.25.1 has no ML-DSA package, so the Go verifier imports CIRCL's `sign/mldsa/mldsa65` (v1.6.1). Transparent balance checks and atomic debits belong to the chain integration. The `generate` command uses a public deterministic test signer.

## One-Command Independent 7-Step Replay

```bash
./verify.sh
```

Or via Docker:

```bash
docker build -t pft-binius-shielded-pq .
docker run --rm pft-binius-shielded-pq
```

## Full epoch replay

The full-tree check materializes 16,777,214 distinct synthetic commitments, then signs and settles a deposit into the final two positions. Rust builds the depth-24 tree bottom-up and retains its authentication paths. The wallet spends leaf 16,777,215 using a single real input and a dummy. Settlement appends its two outputs to the next epoch and checks global nullifier and commitment uniqueness. Go independently constructs both roots from the commitment recipe and verifies both wire transactions. Snapshot commitments are in global insertion order, partitioned into epochs of exactly 2²⁴.

This implements section 1.4 option (a). An input pair uses one epoch's anchor. A wallet can migrate an old note into the current epoch before combining it with a newer one. The Rust settlement methods return global output indices. `LedgerState::locate` resolves each to its epoch tree and local witness index.

The replay requires Linux, `taskset`, GNU `time`, several GiB of available memory, and Python with `z3-solver==5.4.1.0` for the checked-in equivalence report. It samples CPU and SMT sibling activity, then pins the replay to one CPU, or accepts an explicit `BENCH_CPU`. Each lifecycle verification runs in a fresh process. All four lifecycle verifications and the full-tree transfer must each finish under 50 ms on that core. Build-and-fill time includes leaf generation, commitment indexing, Merkle construction and boundary mint settlement. Rust reports transfer proving separately. Peak RSS covers the entire full-tree process in each language. Transaction parsing is timed. Ledger loading and file I/O are outside the verification timer.

The [security specification and audit](../../docs/security/peel-binius-shielded-pq-payments.md), `bench_results.json` and pre-existing benchmark logs describe the original prototype revision. Use the replay output for current measurements. Validator settlement and transparent-account accounting require chain integration.

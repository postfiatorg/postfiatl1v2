# Peel-Binius-Shielded (`PFSH02`)

Post-quantum global-pool confidential payment prototype for Post Fiat L1:

- **Depth-24 Global Merkle Accumulator (`2^24 = 16,777,216` coins)** using NIST FIPS-202 `SHAKE256` (56 Keccak-f[1600] permutations, native `is_dummy` gating, 66-bit exact integer conservation covering ZK-Shielded Mint `v_mint`, 2-in/2-out transfers, 1-in+1-dummy transfers, and transparent Unshield exits `v_unshield`).
- **0-Round Non-Interactive Stealth Receive** using NIST FIPS-203 `ML-KEM-1024` + `SHA-384` KDF + `AES-256-GCM` (`AAD = coin`), with `tx_ctx` header/envelope binding and zero on-chain signatures on shielded spends.
- **512-Bit-State Single Fixed-Range Compiler Oracle** (`H: {0,1}^* -> {0,1}^{1536}` via `3 x SHA-512`), disjoint from in-circuit `SHAKE256`.
- **Dual-Language Implementation**: Rust streaming ANTT prover/verifier (`rust-prover/`) and zero-external-dependency Go 1.25.1 stdlib verifier & offline ML-KEM-1024 scanner (`go-verifier/`).

## One-Command Independent 7-Step Replay

```bash
sha256sum -c SHA256SUMS
./verify.sh
```

Or via Docker:

```bash
docker build -t pft-binius-shielded-pq .
docker run --rm pft-binius-shielded-pq
```

## Security Specification & Audit Report

See [`docs/security/peel-binius-shielded-pq-payments.md`](../../docs/security/peel-binius-shielded-pq-payments.md) for the complete protocol specification, formal proofs, theorem-by-theorem dependency audit, composed security floor table, and empirical benchmark methodology.

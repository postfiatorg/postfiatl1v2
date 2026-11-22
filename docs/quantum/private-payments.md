# Quantum-Safe Private Payments

Post Fiat L1 is building private payments that stay private and sound against a large quantum computer. The
system is called **Peel-Binius-Shielded**, and its transaction format is **PFSH02**. This page says what it does,
what works today, what is still missing and how to check it yourself.

## Why a new system

Accounts and validators on Post Fiat L1 already sign with ML-DSA-65, a post-quantum signature standard. The
existing shielded pool does not. It uses Orchard and Halo2, which rely on elliptic curves. Someone who records the
chain today and has a large quantum computer later could decrypt those notes and learn who paid whom and how much.
They could also forge Halo2 proofs, which would let them spend money they do not have.

PFSH02 uses only hash functions (SHAKE256 and SHA-384) and ML-KEM-1024, the post-quantum key-exchange standard.
Neither has a known quantum attack beyond the generic speed-up that larger parameters absorb.

## What a payment hides

A PFSH02 payment spends two private notes and creates two new ones. The chain sees:

- that a payment happened, its fee and the time it was included;
- two nullifiers, which stop the same note being spent twice without saying which note it was;
- two new note commitments and two encrypted envelopes, one for the recipient and one for change.

The chain does not see the sender, the recipient or any amount. All notes share one pool, so a spent note could be
any earlier note of the same asset. Deposits into the pool and exits out of it show their public account and
amount, as any bridge between public and private balances must.

The recipient finds their payment by trial-decrypting envelopes with their own ML-KEM key. Nobody else can tell
which envelope is theirs. The sender does not need the recipient to be online.

## What works today

The protocol runs as a standalone tool in
[`tools/pft-binius-shielded-pq`](../../tools/pft-binius-shielded-pq/README.md). It has a Rust prover and verifier
and a second verifier written separately in Go with no third-party dependencies. Four sample transactions cover a
deposit, a two-input payment, a one-input payment and a payment that spends a note created 25 deposits earlier and
exits part of it to a public account.

| Measure | Budget | Measured |
| --- | --- | --- |
| Size of one payment | 149,130 bytes | 137,788 to 138,508 bytes; 140,476 bytes worst case |
| Verification, one core, Rust | under 50 ms | 23.1 ms median |
| Verification, one core, Go | under 50 ms | 36.5 ms median (with the CPU's carry-less multiply instruction) |
| Making a payment (proving) | — | 2.9 s on 16 cores, 26.7 s on one core, 562 MiB memory |
| Wallet scanning other people's outputs | — | 4,722 outputs per second per core |

Timings were measured on an AMD EPYC host, one core pinned with no other load on it, and repeated on GitHub's
public runners. The Go verifier without that instruction takes about 72 ms, over budget.

`./verify.sh` replays everything in seven steps: Rust unit tests, Rust verification of the four payments, 35 named
attacks and 150 random corruptions that must all be rejected, the independent Go verifier on the same payments and
attacks, a recipient recovering their payment with Go's standard-library ML-KEM, an exact-arithmetic check of the
security bounds and Z3 proofs that the circuit enforces value conservation.

```sh
cd tools/pft-binius-shielded-pq
sha256sum -c SHA256SUMS
./verify.sh            # ends with: ALL 7/7 INDEPENDENT CHECKS PASSED
```

## What is missing

PFSH02 is not yet part of the chain. Validators do not accept it, there is no ledger pool for it and no wallet can
make one. The build plan is [Integrating PFSH02 private payments into Post Fiat L1](../specs/pq-private-payments-integration.md).
It has eight milestones, each tested and merged on its own:

| Milestone | Delivers | Status |
| --- | --- | --- |
| M1 | A local validator verifies and finalizes a real PFSH02 payment | Next |
| M2 | The on-chain transaction format, frozen with test vectors in Rust and Go | Planned |
| M3 | The pool's own ledger state: note trees, nullifiers, root history, tree rollover | Planned |
| M4 | Deposits signed by the account's ML-DSA-65 key, payments and exits, for PFT and pfUSDC | Planned |
| M5 | Activation height, per-block verification limits, every public submission path | Planned |
| M6 | A wallet that scans for its notes, recovers from backup and pays from the command line | Planned |
| M7 | The browser wallet flow; moving funds from the old Orchard pool | Planned |
| M8 | Upgrade rehearsal on a six-validator chain and external review before activation | Planned |

Work on the standalone tool in parallel with M1:

- **Deposit signature.** The specification requires a depositor's post-quantum signature over the exact amount and
  outputs of a deposit. It is being implemented in the Rust and Go verifiers now.
- **Key derivation.** An ad-hoc SHA-384 construction is being replaced with standard HMAC-SHA-384.
- **Full-size ledger.** The samples use about 100 notes. A test at the tree's full capacity of 16,777,216 notes is
  being added, with rollover to a fresh tree when one fills.

## What rests on assumptions

The security argument is in the [PFSH02 specification](../security/peel-binius-shielded-pq-payments.md). Its
quantum soundness bound rests on a new reduction in Appendix A.3 that no one outside the project has reviewed yet,
and its multi-proof extraction in Appendix A.4 is stated as a conjecture. Privacy also assumes standard properties
of SHAKE256 and ML-KEM-1024 that are believed but not proven. External review of the verifier and these proofs is a
condition of activation (M8).

Privacy here covers the contents of payments. It does not hide network-level information such as the IP address or
timing of a submission, and a small pool hides less than a large one.

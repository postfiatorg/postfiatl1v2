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

- the exact asset, fee, inclusion time and tree anchor;
- two nullifiers, which stop the same note being spent twice without saying which note it was;
- two new note commitments and two encrypted envelopes, one for the recipient and one for change.

The payment carries no public sender account, key, sequence or signature. The transferred amount and recipient stay inside the notes. Each source series has its own tree. Deposits into the pool and exits out of it show their public account and
amount, as any bridge between public and private balances must.

The recipient finds their payment by trial-decrypting envelopes with their own ML-KEM key. Nobody else can tell
which envelope is theirs. The sender does not need the recipient to be online.

## What works today

The protocol runs as a standalone tool in
[`tools/pft-binius-shielded-pq`](https://github.com/postfiatorg/postfiatl1v2/tree/feat/peel-binius-shielded-pq/tools/pft-binius-shielded-pq). It has a Rust prover and verifier
and a second verifier written separately in Go. The Go verifier uses only the standard library except for
ML-DSA-65 deposit signatures, which come from Cloudflare's CIRCL library (Go's standard library has no ML-DSA yet).
Four sample transactions cover a deposit, a two-input payment, a one-input payment and a payment that spends a note created 25 deposits earlier and
exits part of it to a public account.

| Measure | Budget | Measured |
| --- | --- | --- |
| Size of one payment | 149,130 bytes | 138,268 to 139,036 bytes; a signed deposit 143,337 bytes (145,737 worst case) |
| Verification, one core, Rust | under 50 ms | about 22 ms |
| Verification, one core, Go | under 50 ms | about 32 ms (with the CPU's carry-less multiply instruction) |
| Spending the last note of a full tree (16,777,216 notes) | under 50 ms | 20.3 ms Rust, 30.4 to 35.3 ms Go; 138,364 bytes |
| Making a payment (proving) | — | 2.9 s on 16 cores, 26.7 s on one core, 562 MiB memory |
| Wallet scanning other people's outputs | — | 4,722 outputs per second per core |

Timings were measured on an AMD EPYC host, one core pinned with no other load on it, and repeated on GitHub's
public runners. The Go verifier without that instruction takes about 72 ms, over budget.

`./verify.sh` replays everything in seven steps: Rust unit tests, Rust verification of the four payments and of a
spend from a full-size tree, 35 named attacks and 150 random corruptions that must all be rejected, forged deposit
signatures (wrong amount, swapped outputs, wrong key, missing signature, replay) that must be rejected, the independent Go verifier on the same payments and
attacks, a recipient recovering their payment with Go's standard-library ML-KEM, an exact-arithmetic check of the
security bounds and Z3 proofs that the circuit enforces value conservation.

```sh
cd tools/pft-binius-shielded-pq
sha256sum -c SHA256SUMS
./verify.sh            # ends with: ALL 7/7 INDEPENDENT CHECKS PASSED
```

## What is missing

PFSH02 is not yet switched on for any public chain. On local test chains everything a user needs now works:
validators verify and settle payments, the pool keeps its own ledger, and a command-line wallet and the browser wallet
shield, pay and exit for PFT and pfUSDC. The six-validator upgrade rehearsal is done. What remains is an external review of the cryptography;
the package for reviewers is ready. Only after review does a signed amendment choose an activation height. The build plan is [Integrating PFSH02 private payments into Post Fiat L1](../specs/pq-private-payments-integration.md).
It has eight milestones, each tested and merged on its own:

| Milestone | Delivers | Status |
| --- | --- | --- |
| M1 | A local validator verifies and finalizes a real PFSH02 payment | Done 22 Nov: six validators finalize and replay the sample payment; 21.5 to 34.3 ms inside the validator |
| M2 | The on-chain transaction format, frozen with test vectors in Rust and Go | Done 22 Nov: 8 byte-exact payment vectors (PFT, pfUSDC, deposits, payments, exits); largest stored payment 146,546 of 149,130 bytes; validators check the proof and deposit signature before voting |
| M3 | The pool's own ledger state: note trees, nullifiers, root history, tree rollover | Done 22 Nov: separate pool per asset; old notes spendable after two full-tree rollovers; restart, snapshot and genesis replay agree; a real 1,734-block archived chain replays to identical block hashes and state roots |
| M4 | Deposits signed by the account's ML-DSA-65 key, payments and exits, for PFT and pfUSDC | Done 22 Nov on a test chain: deposit, private payment, partial and full exit for PFT and pfUSDC; supply reconciles at every block (public + private pool + burned fees = 1,000,000,000 PFT) |
| M5 | Activation height, per-block verification limits, every public submission path | Done 23 Nov in rehearsal: off unless a signed amendment sets a height; blocks before it are invalid; at most 4 payments per block; 25 to 36 ms per payment inside a validator; a six-validator round takes 0.51 to 0.57 s with one payment (0.31 s without) |
| M6 | A wallet that scans for its notes, recovers from backup and pays from the command line | Done 23 Nov on a local chain: two wallets shield, pay and unshield PFT and pfUSDC from the Python CLI; Bob restores from backup and spends across a tree rollover; no secrets in 89 RPC requests or logs; a payment takes about 25 s to prove on one core (585 MB) |
| M7 | The browser wallet flow; moving funds from the old Orchard pool | Done 23 Nov on a local chain: Alice and Bob, in two browser sessions, shield, pay, spend a received note and exit for PFT and pfUSDC, checked against 10 validator receipts; Orchard funds move out in two approved steps (exit to the public account, then a fresh PQ deposit) and survive a restart at either step; keys and proving stay on the user's machine; the old Orchard wallet still works. [Screenshots](../evidence/pq-private-payments-m7/) |
| M8 | Upgrade rehearsal on a six-validator chain and external review before activation | Rehearsal done 23 Nov: on a fresh six-validator chain payments switch on at the scheduled height, mix with ordinary traffic, pause and reopen, and survive cold restart, snapshot import and full replay; the real 1,734-block archive replays identically; a validator that has not upgraded stops at the scheduling block instead of diverging. [Rehearsal record](../evidence/pq-private-payments-m8/rehearsal-record.md). External review: package ready ([review package](../evidence/pq-private-payments-m8/external-review-package.md)), reviewers not yet engaged |

Closed on 22 November in the standalone tool:

- **Deposit signature.** A deposit now carries the depositor's ML-DSA-65 signature, the same scheme Post Fiat accounts
  use, over the exact amount and both outputs. Both verifiers check it.
- **Key derivation.** Envelope keys and nonces come from HMAC-SHA-384 with versioned labels instead of an ad-hoc
  SHA-384 construction.
- **Full-size ledger.** A test fills a tree to its capacity of 16,777,216 notes, spends the last note, and rolls over
  to a fresh tree while old notes stay spendable.

## What rests on assumptions

The security argument is in the [PFSH02 specification](../security/peel-binius-shielded-pq-payments.md). Its
quantum soundness bound rests on a new reduction in Appendix A.3 that no one outside the project has reviewed yet,
and its multi-proof extraction in Appendix A.4 is stated as a conjecture. Privacy also assumes standard properties
of SHAKE256 and ML-KEM-1024 that are believed but not proven. External review of the verifier and these proofs is a
condition of activation (M8).

Privacy here covers the contents of payments. It does not hide network-level information such as the IP address or
timing of a submission, and a small pool hides less than a large one.

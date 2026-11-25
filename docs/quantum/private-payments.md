# Quantum-Safe Private Payments

Peel-Binius-Shielded is Post Fiat L1's proposed quantum-safe private payment system.
The release candidate uses **PFSH03**, which adds an ML-DSA-65 spend authorization
for each input note. Public-chain activation requires a signed governance amendment.
[Release candidate](https://github.com/postfiatorg/postfiatl1v2/pull/232) · [Protocol specification](../security/peel-binius-shielded-pq-payments.md)

The benchmark figures and milestone rehearsals below were measured on **PFSH02**.

## Why a new system

Accounts and validators on Post Fiat L1 already sign with ML-DSA-65, a post-quantum signature standard. The
existing shielded pool does not. It uses Orchard and Halo2, which rely on elliptic curves. Someone who records the
chain today and has a large quantum computer later could decrypt those notes and learn who paid whom and how much.
They could also forge Halo2 proofs, which would let them spend money they do not have.

PFSH02 uses no elliptic curves. It is built from hash functions (SHAKE256 inside the proof, SHA-512 and SHA-384
around it), AES-256-GCM, ML-KEM-1024 for the encrypted envelopes and ML-DSA-65 for deposit and fee signatures.
None of these has a known quantum attack that breaks it at the chosen sizes.

## What a payment hides

A PFSH02 payment spends two private notes and creates two new ones. The chain sees:

- the exact asset, fee, inclusion time and tree anchor;
- two nullifiers, which stop the same note being spent twice without saying which note it was;
- two new note commitments and two encrypted envelopes, one for the recipient and one for change.

The payment carries no public sender account, key, sequence or signature. The transferred amount and recipient stay inside the notes. Each source series has its own tree. Deposits into the pool and exits out of it show their public account and
amount, as any bridge between public and private balances must.

Issued payments and exits use L1 profile 2. Both wallets read the exact fee for the next inclusion height from finalized pool state. The proposed starting fee is 1 atom (0.000001 pfUSDC), equal in atom count to PFT's minimum. The gateway sees the caller's network address and timing. Validators see the relay address and timing, or the wallet's network address on direct requests. The gateway uses an independent receipt capability for each payerless spend. The direct CLI uses a common admission scope. A node admission receipt is readable with the intent ID.

Replaying a rehearsal archive that contains an issued payment or exit in the original public-payer format requires its original decoder.

The recipient finds their payment by trial-decrypting envelopes with their own ML-KEM key. Nobody else should be
able to tell which envelope is theirs; this rests on an assumption about ML-KEM-1024 that is proven for its
predecessor Kyber but not yet for the standardized version. The sender does not need the recipient to be online.

## What works today

The protocol runs as a standalone tool in
[`tools/pft-binius-shielded-pq`](https://github.com/postfiatorg/postfiatl1v2/tree/feat/peel-binius-shielded-pq/tools/pft-binius-shielded-pq). It has a Rust prover and verifier
and a second verifier in Go, written by the same team from the same specification. The Go verifier uses only the standard library except for
ML-DSA-65 deposit signatures, which come from Cloudflare's CIRCL library (Go's standard library has no ML-DSA yet).
Four sample transactions cover a deposit, a two-input payment, a one-input payment and a payment that spends a note created 25 deposits earlier and
exits part of it to a public account.

| Measure | Budget | Measured on PFSH02 |
| --- | --- | --- |
| Size of the three non-mint fixtures | 149,130 bytes | 138,268 to 139,036 bytes. The signed deposit was 143,337 bytes (145,737 worst case). |
| Single-core Rust verification of the four fixtures | under 50 ms | 21.44 to 21.88 ms |
| Single-core Go verification of the four fixtures | under 50 ms | 32.29 to 36.92 ms with carry-less multiplication |
| Spending the last note of a full tree (16,777,216 notes) | under 50 ms | 20.58 ms Rust, 30.61 ms Go, 138,364 bytes |
| Proving a payment | — | 2.89 s median with 32 threads, 26.73 s median with one thread. Peak RSS for the 32-thread run was 562 MiB. |
| Wallet scanning nonmatching outputs | — | 4,722 outputs per second per core |

For comparison, the PFSH03 candidate's three non-mint fixture files contain
148,598 to 149,174 bytes, and its signed mint contains 154,387 bytes.
These are byte counts for the four checked-in files. [PFSH03 fixtures](https://github.com/postfiatorg/postfiatl1v2/tree/9428258f09140a846f455fd3f5553826d8a45517/tools/pft-binius-shielded-pq/payments)

The AMD EPYC replay timed each verifier on one pinned core. The proving medians come
from separate runs. A separate portable Go run measured a 58.87 ms median against the
50 ms budget. See the [PFSH02 fixture replay](https://github.com/postfiatorg/postfiatl1v2/blob/8ab36a13919ceb1d67a4e052a47f7b92b532508e/tools/pft-binius-shielded-pq/README.md#host-replay-on-2026-11-22),
[32-thread log](https://github.com/postfiatorg/postfiatl1v2/blob/8ab36a13919ceb1d67a4e052a47f7b92b532508e/tools/pft-binius-shielded-pq/bench_results.json), [peak memory log](https://github.com/postfiatorg/postfiatl1v2/blob/8ab36a13919ceb1d67a4e052a47f7b92b532508e/tools/pft-binius-shielded-pq/rust_bench_rss.log), [one-thread log](https://github.com/postfiatorg/postfiatl1v2/blob/8ab36a13919ceb1d67a4e052a47f7b92b532508e/tools/pft-binius-shielded-pq/rust_1core_bench.json)
and [wallet scan measurement](https://github.com/postfiatorg/postfiatl1v2/blob/8ab36a13919ceb1d67a4e052a47f7b92b532508e/docs/specs/pq-private-payments-integration.md#L375).

`./verify.sh` replays everything in seven steps: Rust unit tests, Rust verification of the four payments and of a
spend from a full-size tree, 35 named attacks and 150 random corruptions that must all be rejected, forged deposit
signatures (wrong amount, swapped outputs, wrong key, missing signature, replay) that must be rejected, the Go verifier on the same payments and
attacks, a recipient recovering their payment with Go's standard-library ML-KEM, a script that
recomputes the specification's security formulas in exact arithmetic (it checks the arithmetic, not the proofs) and
Z3 proofs that the adder logic used for value conservation is exact (they do not cover how note values are wired
into the hashes).

```sh
cd tools/pft-binius-shielded-pq
sha256sum -c SHA256SUMS
./verify.sh            # ends with: ALL 7/7 INDEPENDENT CHECKS PASSED
```

## What is missing

PFSH02 is not yet switched on for any public chain. On local test chains everything a user needs now works:
validators verify and settle payments, the pool keeps its own ledger, and a command-line wallet and the browser wallet
shield, pay and exit for PFT and pfUSDC. The six-validator upgrade rehearsal is done. Before activation there remain an external review of the cryptography and the code
(the package for reviewers is ready). Only then does a signed amendment choose an activation height. The build plan is [Integrating PFSH02 private payments into Post Fiat L1](../specs/pq-private-payments-integration.md).
The historical M1–M8 results below cover the standalone fixture and the original L1 profile 1, which charged a public PFT payer on issued spends.

| Milestone | Delivers | Status |
| --- | --- | --- |
| M1 | A local validator verifies and finalizes a real PFSH02 payment | Done 22 Nov: six validators finalize and replay the sample payment; 21.5 to 34.3 ms inside the validator |
| M2 | The on-chain transaction format, frozen with test vectors in Rust and Go | Done 22 Nov: 8 byte-exact payment vectors (PFT, pfUSDC, deposits, payments, exits); largest stored payment 144,770 bytes, at most 146,546 of 149,130; validators check the proof and deposit signature before voting |
| M3 | The pool's own ledger state: note trees, nullifiers, root history, tree rollover | Done 22 Nov: separate pool per asset; old notes spendable after two full-tree rollovers; restart, snapshot and genesis replay agree; a real 1,734-block archived chain replays to identical block hashes and state roots |
| M4 | Deposits signed by the account's ML-DSA-65 key, payments and exits, for PFT and pfUSDC | Done 22 Nov on a test chain: deposit, private payment, partial and full exit for PFT and pfUSDC; supply reconciles at every block of the test chain and on replay (public + private pool + burned fees = 1,000,000,000 PFT) |
| M5 | Activation height, per-block verification limits, every public submission path | Done 23 Nov in rehearsal: off unless a signed amendment sets a height; blocks before it are invalid; at most 4 payments per block; 24 to 45 ms per payment inside a validator, up to 55 ms when cold; a six-validator round with four payments took 0.98 to 1.03 s (0.31 to 0.84 s without) |
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

The security argument is in the [PFSH02 specification](../security/peel-binius-shielded-pq-payments.md). The proof
system is new. Its whole security analysis was written by the project and has not been reviewed by anyone outside
it: the classical soundness argument (Appendix A.1–A.2), the reduction for quantum attackers (A.3) and the
zero-knowledge argument that privacy depends on (Appendix B). Protection against an attacker who reuses parts of
other people's proofs (A.4) is stated as a conjecture. Privacy and authorization also rely on assumptions about
SHAKE256 and ML-KEM-1024 that the specification marks as non-standard and gives no numbers for. External review of the verifier and these proofs is a
condition of activation (M8).

Privacy here covers the contents of payments. It does not hide network-level information such as the IP address or
timing of a submission, and a small pool hides less than a large one.

# pfusdc-sol-ingress

The deposit proof for the trustless Solana route. Specification:
`docs/specs/pfusdc-solana-trustless-bridge-spec-20261118.md` on branch
`docs/solana-trustless-bridge-spec-20261118`.

## State

| Spec step (section 4.1) | State |
| --- | --- |
| 1. Shreds to block ID | Built. Matches Agave's shredder and blockstore in `tools/pfusdc-sol-ingress-difftest` |
| 2. Block components to the footer's bank hash | Built. Components are split where shreds carry DATA_COMPLETE, as Agave does; exactly one footer required |
| 3. Bank hash to lattice sums | Built, tested |
| 4. Account delta | Built, tested against Solana's `solana-lattice-hash` crate |
| 5. Receipts | Built, tested |
| 6. Finalization certificate | Built. Fast (80%) and slow (60% + 60%) finality agree with Agave's `bls-cert-verify`; fallback votes are not accepted |

All six steps are joined in `verify_deposit_witness`, which is what the guest runs: from raw shreds
and a certificate to deposit receipts in a block a named validator set finalized.

What the program does **not** establish, and the ledger must:

- that the validator set is Solana's. The program outputs the set's hash; the ledger's Solana light
  client has to hold that hash for the block's epoch. The section 4.2 proof below supplies the
  transition that the ledger checks before advancing its light client.
- that the vault program and route binding are the route's, and that each deposit ID is new.

Nothing here has run on a real mainnet block. Every block and certificate in the tests is built by
Agave's own code on this machine.

## Validator-set handoff (section 4.2)

`verify_key_table_witness` authenticates a batch of registered BLS keys under the current set.
The client takes the union of its table and the returned keys. `verify_handoff_witness`
derives the next ranked set from the stake recorder account after a creation or update. Both proofs
use the deposit verifier's shred, footer, bank-preimage, distinct-account delta and certificate
checks. They can select one recorder in a block containing other writes. The selected account
must have a different old and new state. Its new state must have positive lamports, be
non-executable and have the pinned recorder program as owner. An unchanged target contributes
zero to the lattice delta. For a changed target, the delta authenticates the full new image,
including rows accumulated in earlier blocks. The parser checks the account magic and block epoch.

### Recorder accounts

`src/recorder.rs` defines the wire constants. Integers are unsigned little-endian. The
accounts have exact lengths with no padding. The recorder must read the epoch from the
runtime, the BLS and node identities from genuine vote accounts, and the individual and total
stakes from the runtime for that same epoch. For key registration the vote program's proof of
possession is the basis for admitting the recorded key. The proof authenticates the bytes
written by the recorder. Its program identity and behavior are therefore part of the route.

| Offset | Key account field | Width in bytes |
| ---: | --- | ---: |
| 0 | ASCII `PFSOLKY1` | 8 |
| 8 | Epoch | 8 |
| 16 | Number of entries, nonzero | 4 |
| 20 | Compressed G1 keys | 48 per entry |

| Offset | Stake account field | Width in bytes |
| ---: | --- | ---: |
| 0 | ASCII `PFSOLST1` | 8 |
| 8 | Epoch of these runtime stakes | 8 |
| 16 | Total runtime epoch stake in lamports | 8 |
| 24 | Number of vote-account rows, nonzero | 4 |
| 28 | Vote-account rows | 120 per row |

Within each stake row, offsets 0, 32, 64 and 72 hold the vote-account address (32 bytes),
node identity (32), stake (8) and compressed BLS key (48). An absent key is 48 zero bytes.
Vote-account addresses must be distinct. As in Agave's `runtime/src/epoch_stakes.rs`,
zero-stake rows and undecodable keys are ineligible. Among the eligible rows **all**
occurrences of a repeated BLS key or node identity drop out. The remaining rows sort by
decreasing stake and then increasing compressed BLS bytes. The denominator retains the runtime
total, including stake outside those ranks. The recorder must supply the vote-account population
needed to reproduce the runtime ranks, including duplicates before filtering.

### Witness and public bytes

Build all three SP1 guests with `cargo prove build --locked --features guest`. The ledger
pins a separate verification key for deposit, key registration and handoff. The shipped ELFs,
SHA-256 hashes and verification keys are listed in [program-info.json](program-info.json). Each recorder
guest takes one witness vector directly, with no kind prefix. The witness fields are:

1. Trusted epoch (u64), slots per epoch (u64), warmup (u8, 0 or 1).
2. Trusted set in the deposit witness encoding, with total stake (u128), count (u32) and
   ranked entries of compressed BLS key (48) and stake (u64).
3. For handoff only, known-key count (u32) and that many compressed keys (48 each), strictly
   increasing in byte order. The hash is SHA-256 over the ASCII domain
   `postfiat.pfusdc.solana.known_keys.v1`, count (u32 LE) and concatenated keys.
4. The deposit finality encoding (kind 0 fast, kind 1 slow), followed by the block witness.
   In the state header the two 32-byte fields after the bank hash select **recorder program**
   and **recorder account address**, respectively. The remaining state bytes use the deposit
   delta encoding.

The slot from the shreds determines the epoch using Solana's `EpochSchedule`. A key registration
must be in the trusted epoch. A handoff must advance exactly one epoch. The recorder's
epoch must match that result. Every signer in either bitmap must be in the committed table,
and the candidate set must meet its ordinary fast or slow certificate thresholds. Old stake
is looked up by BLS key using the previous epoch's weights and full denominator. For slow
handoffs, only old signers in **both** bitmaps count toward the two-thirds threshold.
`Finalize` signs just the slot. Requiring the counted signers to have notarized the block
ID binds their finalizing quorum to the recorded set. Slow certificates with less overlap can
therefore be ineligible even if each component meets its ordinary threshold.

Both journal encodings start with an 8-byte tag, recorder program (32), slot (u64),
shred version (u16), slots per epoch (u64), warmup (u8), block epoch (u64) and trusted
set hash (32). The remaining fields are:

| Tag | Remaining bytes in order |
| --- | --- |
| `PFSOLKP1` | Distinct key count (u32), then that many sorted compressed BLS keys (48 each) |
| `PFSOLHP2` | New ranked set hash (32), runtime total stake (u64), listed stake (u64), known-key table hash (32) |

The ledger must pin the guest and journal kind, recorder program, epoch schedule, shred version
and trusted set hash. Key registration uses the trusted set for the block epoch. A handoff
for epoch E always uses the set for E−1 and its authenticated key table. The new set hash
commits to the rank order and runtime denominator using the deposit verifier's u128 encoding.

**Ledger admission for partial sets.** Store the installed epoch, set hash, runtime total,
and listed stake. Listed stake sums the eligible validators in the committed
ranked set after duplicate-key and duplicate-node filtering. A partial set can lack the votes
to reach 80% fast finality. Every certificate threshold still uses the full runtime total.
For the first handoff, require E to be the checked successor of the current epoch. Retain
the authenticated E−1 set hash and key-table commitment used for that handoff. A later proof
for the installed E must match those predecessor bindings and the same runtime total. Replace
the installed set only if the new proof lists strictly greater stake. Reject equal and
smaller weights, including replays. Once the client advances past E, this rule no longer
updates E. The handoff journal is exactly 179 bytes; listed stake occupies bytes 139–146.

## Run

```
cargo test                                   # in programs/pfusdc-sol-ingress
cargo prove build --features guest           # guest ELF
cd ../../tools/pfusdc-sol-ingress-host && cargo build --release
./target/release/pfusdc-sol-ingress-host synth 2300 /tmp/w.bin
./target/release/pfusdc-sol-ingress-host execute <elf> /tmp/w.bin
```

## Differential check against Agave

`tools/pfusdc-sol-ingress-difftest` builds real blocks with Agave's shredder (commit `141da0c3`,
read from the reference clone on the main machine) and requires our verifier to derive the same
erasure-set root Agave reports for every data shred and the same block ID Agave's blockstore
computes. It also requires an altered shred, swapped shreds and a dropped erasure set to be
refused, and a different parent block ID to change the result. It also builds leader-style blocks
(header, entries, footer, final tick) and requires the footer's bank hash, a refusal with no footer
or two, and the full path from shreds to a deposit receipt. Passed on 2026-11-19 for blocks of 1 and 4
erasure sets. It needs Rust 1.99.1, libclang, and Agave's own crate patches.

Run `./solana/test-programs.sh` from the repository root before running the differential tool.
The latter loads the resulting recorder SBF into ProgramTest. It submits key batches in three
slots of one epoch and stake batches in four slots of its successor. It takes the funded parent
and updated account images from BanksClient, wraps the final appends in Agave-shredded test blocks with
synthetic bank preimages and Agave-built certificates, and checks both native proof entry points.
The last stake append exercises a same-epoch handoff with higher listed stake.

## First cost measurement (2026-11-18)

Guest executed without proving, SP1 6.3.1, synthetic blocks: mostly 165-byte token accounts,
one account in fifty between 10 and 130 KB. These are not mainnet blocks.

| Changed accounts | Witness | Cycles |
| ---: | ---: | ---: |
| 100 | 0.13 MB | 17.4 million |
| 2,300 | 7.2 MB | 514 million |
| 3,013 | 9.5 MB | 675 million |

About 224,000 cycles per changed account, almost all of it BLAKE3: each account state is
hashed to 2,048 bytes, and SP1 has no BLAKE3 precompile. A mainnet block sampled the same day
(slot 471086018) had 2,303 distinct writable accounts, which bounds its changed accounts.

Proving time is not measured. These figures cover steps 3 to 5 only.

The certificate is cheap too at test size: with 20 validators, fast finality (15 signers) added about
4.5 million cycles and slow finality about 8.6 million. Mainnet has about 660 validators; the cost
with a set that size is not measured.

Steps 1 and 2 are cheap by comparison: a block of 128 data shreds with one changed account ran in
2.5 million cycles, about 19,000 per shred, so a block of a few thousand shreds adds tens of
millions of cycles to the hundreds of millions the account changes cost.

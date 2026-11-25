# Private PFT and pfUSDC payments: live findings and design

Written 21 November 2026, revised the same day after review. Status: design for review; nothing in
section 4 is built. Evidence files are in `docs/evidence/private-transactions-20261121/`.

## 1. Goal

A user can hold PFT and pfUSDC privately and pay another wallet privately, first from the command
line on the network and then in the Task Node, and the wallet tells them plainly what is hidden,
from whom, and what is not.

## 2. What the live chain does today

Tested on `postfiat-wan-devnet-2` (release 6.4) on 21 November, heights 1725 to 1730, from the test
wallet `pf5c266a…afb2` to a fresh wallet `pf1c9189…5e2c`. Actions were built against an offline copy
of validator 0's state with the live binary and submitted as certified rounds.

| Step | Result |
| --- | --- |
| Shield 1.0 and 0.5 pfUSDC | Accepted at 1725 and 1726. Both notes spendable. |
| Shield PFT | **Fails.** `Orchard pool id asset-orchard-v1 does not match expected pool orchard-v1`. The chain has one shielded pool and it is the asset pool; PFT is not an issued asset, so it has no way in. |
| Private transfer of pfUSDC to another wallet | **Rejected by consensus** at 1727: `asset_orchard_fx_fix_packet_mismatch`. The only two-note private action is a swap, and consensus accepts a swap only at a NAV price or a reserved FX fix. It is not a payment. |
| Unshield part of a note | **Refused** by the builder: the amount must equal the whole note. |
| Unshield a whole note to a new address | **Rejected** at 1728: `missing_recipient`. The destination account must already exist. |
| Unshield a whole note to an existing address | Accepted at 1730. The recipient received 0.5 pfUSDC and a trustline was created for it. |

So today a "private payment" of pfUSDC is: shield a note of exactly the payment amount, then
unshield it to the recipient.

### 2.1 What each published action shows

Scanned from the batches as published (the three files are in the evidence folder).

| Action | Sender address | Recipient address | Asset | Amount |
| --- | --- | --- | --- | --- |
| Shield | public | none | public | public |
| Swap (the rejected transfer) | absent | absent | public (asset tags of both legs) | absent, but the **ratio of the two note values is public** |
| Unshield | absent | public | public | public |

An observer sees 0.5 pfUSDC enter at 1726 from one address and 0.5 pfUSDC leave at 1730 to another.
The pool holds at most 13 unspent notes (177 outputs, 164 spent). Same asset, same amount, four
blocks apart, in a pool that small: this is a strong inference that the two are one payment, not
a proof.

### 2.2 Proving time, measured

The binary writes a timing report beside each unshield. The 5 minutes 35 seconds first observed is
almost all one-time setup, not proving.

| Hardware threads (AMD EPYC server) | Build the proving key | Make the proof |
| --- | --- | --- |
| 32 | 326 s | 6.2 s |
| 8 | 330 s | 9.0 s |
| 4 | 330 s | 15.1 s |

- The proving key build runs on one thread and takes five and a half minutes whatever the machine
  has. It is rebuilt every time the program starts. The proof library (`halo2_proofs` 0.3.5) can
  save its parameters to disk but not its keys. The verifying key already has a saved form in
  this codebase and loads in 29 ms; the proving key has none.
- Once the key exists in a running program, a proof takes 6 to 15 seconds on 4 to 32 threads.

**A program that stays running already exists, and it works.** The repository has
`asset-orchard-local-service`: a loopback service that builds the keys at start-up and then makes
proofs on request. Built from the release 6.4 source and run against the offline copy:

| Hardware threads | Start-up (both keys) | Four proofs in a row, each verified |
| --- | --- | --- |
| 8 | 361 s | 11.1, 9.2, 9.4, 9.3 s |
| 4 | 342 s | 15.6, 15.5, 15.6, 16.2 s |

A proof made by that service on 4 threads was submitted to the live chain and accepted at height
1732 (it unshielded the test's leftover 1.0 pfUSDC note back to the sender).

- **Not measured: a laptop.** These are server threads. A benchmark that needs no chain data is
  on branch `privacy/local-prover-benchmark` (`4c0eb46ac`) and reproduces the 4-thread numbers
  here: key 331 s once, then 15.2 to 16.0 s per proof. On a Mac with Rust installed:
  `cargo run --release -p postfiat-privacy-orchard --example private_egress_laptop_benchmark`.
- **The browser does not work today.** The proving library fails to compile for the browser:
  `The multicore feature flag is not supported on wasm32 architectures without atomics`. A browser
  build would need either a single-thread build or a threaded one, and would rebuild the key on
  every page load unless the key can be saved.

### 2.3 Other facts the test established

- **There is no public way to submit a shielded action.** The validators' RPC does not enable it.
  Every shielded action today is submitted by an operator with shell access to the proposing
  validator.
- **A recipient cannot be paid by address.** In the asset pool an output note is derived from a
  seed the sender picks; the recipient finds the note on chain only if the sender gives them that
  seed some other way.
- **A rejected batch cannot be resubmitted.** The batch id is recorded on rejection, so the retry
  after `missing_recipient` needed a new proof.
- **pfUSDC is several assets.** Deposits from Solana are credited as child assets of the pfUSDC
  family. Each child has its own tag in the pool.
- **Existing shielded actions charge no fee.** The swap circuit requires `fee == 0`, and the
  unshield was built with fee 0.

The earlier amount-and-timing work (shielded funding of Hyperliquid and Lighter, September) is
published as a failed attempt for the same root cause: whole-note, exact-amount exits. Its
specified fix, private split, merge and change, was never built.

## 3. Who can see what

The design is judged against five observers. "Private" in this document always means private
from the observers marked hidden in this table, and nothing more.

| Observer | Sees today | Sees under this design |
| --- | --- | --- |
| Anyone reading the chain | Everything in 2.1 | Shield and unshield in full. For a payment inside the pool: that a payment happened, when, and the flat fee. Not sender, recipient, asset or amount. |
| Whoever receives the submission (RPC, Task Node relay) | Not applicable; operators submit | The same as the chain, plus the network address and exact time of the submitter. A relay does not see inside the action. |
| Whoever makes the proof, if it is not the user | Not applicable | **Everything:** sender, recipient, asset, amount. |
| The recipient | Amount and asset on unshield | Amount and asset. Not the sender, unless the sender says so. |
| Someone who records the chain now and has a large quantum computer later | Everything already public | Could decrypt recorded notes and learn recipients and amounts after the fact. The shielded part is Orchard/Halo2 and is not post-quantum; the rest of the chain is. |

Two things no cryptography here fixes, and the wallet must say so:

- **Linking at the edges.** Shielding and unshielding are public. A user who shields an amount
  and soon unshields the same amount has linked the two, as in 2.1.
- **A small crowd.** Hidden among 13 notes is not hidden. The repository's own calibration
  (`docs/privacy/floor-calibration.md`) sets the floor for calling a route private at 16 candidates
  and 128 actions in the activity window. Those numbers come from a stated single-target observer
  model; they are a policy line, not a guarantee. Today's pool is below them. Section 4.8 says
  what follows from that.

## 4. Design

One pool, one payment action, real addresses, honest edges.

### 4.1 One pool for every asset, including PFT

Keep the asset pool as the only pool. Register PFT in it under a reserved asset tag. Shielding PFT
burns native PFT, as shielding pfUSDC burns the issued asset; unshielding credits native PFT. No
second pool, no second wallet, no second crowd.

Inside the pool, pfUSDC is one asset. A note carries the family tag. Which child is burned on
shielding and which is credited on unshielding is decided at the public edge, where it is visible
anyway. The chain keeps per-child public accounting at the edge so supply stays checkable; inside
the pool the children are fungible.

This has a consequence that needs its own decision before it is built. Each child is backed by a
different bridge. If children are fungible inside the pool, a user can shield Solana-backed
pfUSDC and unshield Ethereum-backed pfUSDC, so the pool becomes a way to move between backings,
limited by how much of each child the pool holds. The rule proposed is that the user names the
child on unshield and the action fails if the pool's public balance of that child is short. The
alternative is to keep the children separate in the pool and accept thinner crowds. Which one is
right depends on whether the two backings are meant to be interchangeable; that is a product
decision (section 6).

### 4.2 A payment action with change

The draft implementation of `asset_orchard_transfer_v1` requires a finalized
`asset_orchard_transfer_activation_height` governance amendment. Every network
starts with the transfer disabled. The amendment value must be strictly after
its block height. Admission checks the next block, and validators check the
executed block height during proposal and replay. Certified block import and
archive replay require the activation amendment’s validator authorizations.
The earliest unpaused value
applies. The private readiness report exposes the scheduled height under
`asset_orchard_transfers`.

Release approval requires closure of the independent review findings and a
byte-identical replay of the live chain against the running release. The
activation tests cover synthetic state.

A new circuit and action, `asset_orchard_transfer_v1`:

- two payment inputs and two payment outputs of **one** asset, with dummy notes allowed, so one
  shape covers pay-with-change, merge and split, and every payment looks the same on chain;
- the rule is inputs = outputs, enforced on hidden values with range checks. The existing swap
  already carries a hidden value commitment per note; this reuses that accounting;
- **no public price, asset tag or ratio.** The asset is proven equal across the payment notes
  without being shown.

The swap stays as it is, for swaps.

### 4.3 The fee

A fee taken from the payment notes would be paid in the payment's asset, and a public fee in a
public asset would give the asset away. A flat fee counted in the hidden asset's own units would
cost different amounts for different assets. So the fee is separate from the payment:

- every action has a **fee leg**: one shielded PFT note in, **always two** shielded PFT notes
  out, and a public flat fee in PFT burned from the difference. The circuit proves the fee leg
  is PFT. The second output is a zero-value dummy unless the sender uses it (below), so every
  action has the same shape whether or not it is used;
- the fee is the same for every action, so it identifies nothing;
- a payment of PFT uses the same shape, so PFT payments do not stand out.

Costs of this choice, and how each is handled:

- **A larger circuit.** Three inputs and four outputs instead of two and two. Its proving time
  is re-measured in step 3 of section 5.
- **Someone paid privately may hold no shielded PFT**, and so could not spend or unshield what
  they received without first shielding PFT in public, which would mark their address just as
  they start. Two rules prevent that. An unshield may pay its fee in public PFT from the
  destination account, since an unshield is public anyway. And the fee leg's second output can
  carry a few fees' worth of shielded PFT to the recipient with the payment; the wallet does
  this by default the first time it pays an address. Because that output is always present,
  an observer cannot tell whether it was used.
- **The first user of all** still has to shield PFT publicly once. That is unavoidable: value
  enters the pool only through a public shield.

Until the fee leg exists, the alternative is what the chain does now: no fee on shielded actions,
with the submission limits in 4.6 as the only brake. That is acceptable on a test network and not
beyond it, because every action adds permanent state.

### 4.4 Unshield any amount

Unshield is the payment action with one public output: hidden notes go in, a public amount goes to
a public address, and the change returns to the pool as a hidden note. The public amount need not
equal anything that was shielded.

The destination account is created if it does not exist, which removes the `missing_recipient`
failure. An ordinary transfer already creates its recipient: the 0.5 PFT sent to the fresh wallet
at height 1729 did. Unshield follows the same rule and the same charges as that transfer, so it
adds no cheaper way to create accounts.

Shielding stays public by nature. The wallet shields round amounts and tops up rarely.

### 4.5 Shielded addresses

Each wallet gets a shielded address derived from its seed. The sender encrypts the note to that
address inside the action. The recipient's wallet finds its notes by scanning the chain with a
viewing key; nobody passes seeds around. The original Orchard path in the codebase already works
this way (`orchard-keygen`, view keys, `orchard-scan`), so this is carried over to the asset pool,
not invented.

A viewing key can be handed to an auditor without giving up spending. The existing disclosure
packet covers single notes.

### 4.6 A public way to submit

Enable shielded submission on the validators' RPC behind the existing rate, size and concurrency
limits, so a wallet needs no operator. The Task Node relay forwards it like any other transaction.

Retry rule: an action that was rejected without spending its notes may be submitted again
unchanged. Today the rejected batch id blocks that. Recording batch ids is what stops a batch
being applied twice, and that protection is not needed for shielded actions: an accepted action
spends its notes, and a spent note can never be spent again, so replaying an accepted action
fails on its own. The change is therefore narrow: keep recording accepted batch ids, and stop
recording rejected shielded ones. The proof binds the notes, the outputs and the destination, so
a resubmitted action cannot be altered. The cost is that a failing action can be retried
repeatedly; the submission limits bound that.

The submitter's network address is visible to whoever receives the submission (section 3). The
command line can submit through any RPC the user picks. A Task Node user submits through the Task
Node, which therefore knows which account sent a private payment and when, though not to whom or
how much.

### 4.7 Where the proof is made

| Option | Who learns the payment | Time, from 2.2 |
| --- | --- | --- |
| A program that stays running on the user's machine | Nobody | 9 to 16 s per proof on 4 to 8 server threads, after a start-up of about six minutes; accepted by the live chain; laptop not measured |
| The command line as it is now | Nobody | 5.5 minutes every time |
| The browser | Nobody | Does not build today |
| Our prover | **We do** | About 45 s for a swap in the July demo runs on a warm GPU prover; not re-measured here |

The measurement changes the plan. The proof is fast; the start-up is slow and it is repeated for
no reason. Two pieces of work follow, both before any new circuit:

1. **A resident prover in the wallet** (the `pft` tool already has a prover daemon; the desktop
   wallet is a running program). The key is built once when the wallet starts, in the
   background, and every proof after that takes seconds. This needs no change to the proof
   library and is the sure path.
2. **Save the proving key**, so start-up is a file read. The library has no way to write a key,
   so this means writing one, as was done for the verifying key. How much work that is, and how
   large the file is, is not known yet; it is an improvement on item 1, not a dependency.

With those, proving on the user's machine is the default and the only thing called private. A
server-proved payment is offered, if at all, under its own name and with the sentence "the Task
Node operator can see this payment".

### 4.8 What the wallet says, and when it may say "private"

Two balances and three verbs.

- **Public balance** and **private balance** per asset.
- **Make private** (shield): "Public. Anyone can see this address moved this amount into the
  private pool."
- **Send privately**: "Hidden on the public record: sender, recipient, amount, asset. The service
  you submit through can see that you sent a payment and when."
- **Make public** (unshield): "Public. Anyone can see this amount arrive at this address."

One standing warning: making an amount private and making the same amount public again soon
afterwards links the two.

The pool is below the floor in section 3 today, and a design cannot conjure a crowd. So:

- the wallet shows the pool's size from the chain's own report, and while the pool is below the
  floor it labels payments **"hidden, small pool"** with one line of explanation, not "private";
- inside the pool a payment shows no asset, so every note of every asset is a candidate for every
  payment. One pool for all assets (4.1) is what makes the crowd as large as it can be;
- the crowd grows from use that would happen anyway: Task Node rewards and faucet grants paid
  straight to shielded addresses, and private balances as the wallet's default holding. No cover
  traffic is proposed; the September attempt showed what that costs;
- the label is decided per payment, not per pool. Pool size is only the ceiling. The wallet uses
  the repository's observer model (`docs/privacy/observer-inference-model.md`), which counts the
  candidates left after timing, fee class and submission path are taken into account, and shows
  "private" only when that count clears the floor for this payment;
- **release gate:** the Task Node does not use the word "private" for payments until a
  two-week window on the network clears the floor for an ordinary payment. Until then the
  feature ships under the "hidden, small pool" label or not at all (section 6).

## 5. Order of work

The evidence so far is one day of tests on one network. The order is arranged so that each step
is the smallest thing that can prove or kill the next, and so that nothing large is built on an
unmeasured assumption.

1. **Proving on the user's machine.** Partly done (2.2): the resident service exists, makes a
   proof in 9 to 16 seconds on 4 to 8 server threads, and its proof is accepted by the live
   chain. Still to do:
   - run the benchmark on a laptop;
   - put the service behind the command line, so `pft` starts it and uses it instead of a remote
     prover;
   - the start-up. A wallet that has just been opened cannot prove for about six minutes. Either
     the key is built in the background while the user does something else, and the wallet says
     "preparing private payments" until it is ready, or the key is saved to disk (4.7).

   Exit test: a proof in under 30 seconds on a laptop once the wallet is ready, and a decision
   on whether a six-minute first start is acceptable. If the laptop number fails, stop and
   decide 6.1 before building anything else.
2. **Decide how the pool gets its crowd** (6.5) before paying for a circuit. A payment action
   is only worth building if there will be enough shielded activity for it to hide in. If the
   answer is that rewards and grants go to shielded addresses, that becomes part of step 3's
   scope. If there is no answer, stop here: the resident prover and the existing unshield are
   still useful, and the circuit is not.
3. **Circuit and action.** `asset_orchard_transfer_v1` with change, the fee leg and the unshield
   form; PFT and the pfUSDC family as pool assets; shielded addresses and scanning. New verifying
   key and an independent review, as the current design document requires for any new circuit.
   Proving time re-measured on the new, larger circuit against the same 30-second test.
4. **Network path.** Public submission, the retry rule, account creation on unshield. Rehearsed on
   a disposable network, then devnet-2 with the same leak scan as 2.1, which must show no asset,
   amount, sender or recipient for a payment inside the pool.
5. **Command line.** `pft shield`, `pft send --private`, `pft unshield`, `pft balance`.
6. **Task Node.** The two balances and three verbs, with the labels in 4.8.

## 6. Decisions needed

1. If step 1 fails on a laptop: whether to ship server-side proving under an honest name, or wait.
2. The fee amount, and whether the first test-network release runs without the fee leg.
3. Whether the non-post-quantum limit in section 3 is acceptable for a first release, stated
   plainly in the wallet.
4. Whether Solana-backed and Ethereum-backed pfUSDC are meant to be interchangeable (4.1).
5. Whether Task Node rewards and faucet grants are paid to shielded addresses by default, which
   is the main way the crowd grows (4.8), and whether the feature ships before the release gate
   is met under the "hidden, small pool" label.

## 7. Test leftovers

Nothing is left shielded. The 1.0 pfUSDC note was unshielded back to the sender at height 1732,
and the sender's balance is back to its starting amount less the 0.5 pfUSDC paid to the fresh
wallet. The fresh wallet `pf1c9189…5e2c` holds 0.5 pfUSDC and 0.5 PFT; its keys are in the lab
directory on the relay host (`…/stakehub-hyperliquid/private-tx-lab/keys`). Heights 1727 and 1728
carry the two rejected actions.

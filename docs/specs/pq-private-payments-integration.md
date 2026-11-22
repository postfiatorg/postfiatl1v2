# Integrating PFSH02 private payments into Post Fiat L1

Build on **`public/main`** (design baseline `071b6dc461`). Import the PFSH02 source and fixtures from
`feat/peel-binius-shielded-pq` (tool baseline `d76e5a8c6`, spec baseline `dd9be3ba9`). Reconcile any
subsequent tool or spec corrections against those vectors before pinning. The Orchard transfer diff is a map
of validator and wallet entry points. Its proof, deposit authorization, historical roots and note keys cannot
establish backing or spending authority for this pool.

The first deliverable is a local validator finalizing a block containing the existing two-input transfer. The
user deliverable is Alice shielding PFT or a named pfUSDC issue, paying Bob, and Bob discovering his notes,
spending some and withdrawing the remainder with his own wallet. The checkboxes below track implementation progress. Sizes
are engineer-days for implementation and focused tests, excluding external review and network rollout.

## Decisions that affect the wire and the wallet

**One denomination per tree.** PFSH02 proves conservation of integers. Its notes commit to owner, value and
blinder. Neither the note nor the public statement identifies an asset. Start one PQ pool with separate trees
and reserves for native PFT and each explicitly enabled pfUSDC asset ID. Pin the issue and its backing using
`pfusdc_source_series_id` in `crates/types/src/market_nav_asset_types.rs`. Look up issuer, precision and
effective policy from finalized state. A public pool selector reveals the asset even on an internal payment.
Bob receives and exits the same issue Alice deposited. Mixing assets inside one anonymity set or exchanging
pfUSDC backings would need a different relation and accounting review.

**Two proof profiles.** Keep the published stand-alone bytes and transcript as the M1 compatibility vector.
For settlement use the same PFSH02 header and circuit with a separately identified L1 transcript profile. Keep
the spec's `tx_ctx = SHA-384("PFT-SH-TXCTX-V2" || header)`. For the L1 profile feed a new domain-separated
SHA-384 digest of the canonical L1 context and `tx_ctx` into the statement's 48-byte context slot before
Round 1. Bind chain ID, genesis hash, protocol/profile version, pool ID, exact asset selector, **input tree epoch**,
edge kind and unsigned public fee/debit terms. Derive the output epoch from chain state when appending. Use
fixed-width or length-prefixed canonical bytes for every field. A signed outer wrapper alone would leave an
ordinary private spend free of this binding. Give the L1 profile fresh Rust and Go vectors and review the
changed transcript before enabling it. Raw M1 proofs stay confined to the fixture chain.

**Fee unit.** PFT fees burn. For a native payment the circuit fee is PFT taken from its note balance,
including on deposit. For pfUSDC set the circuit fee to zero. Charge the network's minimum PFT fee or more to
one explicit public payer in the *same* atomic action. On a pfUSDC deposit the depositor also pays, using one
account signature. On a private transfer or exit a relayer can pay using a fee-only ML-DSA-65 authorization
tied to the statement, fee and next sequence. That signature authorizes only the PFT fee. The proof authorizes
the spend. Show who pays before proving. A user paying their own fee exposes their public account. This design
avoids a second proof and keeps pfUSDC reserves in pfUSDC units.

**Reference material.** Wire, relations, R0–R6, rollover, measured costs and security dependencies are in the
[PFSH02 specification](../security/peel-binius-shielded-pq-payments.md) and
[replay tool](../../tools/pft-binius-shielded-pq/README.md). Compare
`git diff public/main privacy/transfer-fix` and
`git show privacy/transfer-gateway:docs/specs/private-transactions-design-20261121.md`. The ledger and circuit
reviews in `/home/postfiat/var/transfer-review2/{ledger,circuit}.md` motivate the funding, provenance, policy,
parse, retry and scanner tests below.

Merge milestones in order. M2–M4 can merge with production activation absent and can exercise their code
directly in local-chain tests.

## M1. Finalize one real transfer on a disposable chain · 3–5 days

- [x] Add workspace crate `crates/privacy_binius/{Cargo.toml,src/lib.rs}` and wire it through the root
  `Cargo.toml`, `Cargo.lock` and `crates/node/Cargo.toml`. Wrap/extract the checked reader, statement encoder
  and verifier from
  `tools/pft-binius-shielded-pq/rust-prover/src/{payment,binius_pcs,circuit,field,shake256}.rs` so the tool
  and node call the same Rust implementation. Its entry point accepts bounded bytes and a typed context and
  returns either verified public fields or a stable error. Ledger membership and account authorization belong
  to the node.
- [x] Keep the verifier single-threaded and deterministic. Use fixed-width integers, checked length arithmetic
  and bounded allocation. Keep I/O, clocks, entropy, proving, the Rayon pool, the JSON ledger loader and KEM
  outside the verifier. The node uses `std`. A literal `no_std` build is optional. Precompute immutable tables
  at startup and keep a portable field path equivalent to CPU acceleration. Pin circuit constants and
  transcript version in both languages.
- [x] In `crates/types/src/shielded_bridge_governance.rs` and
  `crates/node/src/{execution_actions,shielded_batch_actions,mempool_proposals,block_replay_wallet}.rs`, carry
  a local-only proof-bearing payment through the actual shielded batch proposal, vote and execution paths.
  Restrict it to a pinned disposable genesis and a single transfer. Initialize that test chain with the
  committed fixture pre-state for `payments/tx_hop1.bin` from `ledger_hop1.json`. Recompute its root from the
  leaves and pin its pre-state digest and test reserve in the harness. Use it to check R1–R6, apply both
  nullifiers and both outputs and produce an accepted receipt. Commit the small fixture post-state in
  `crates/node/src/state_commitment.rs` for this genesis. Reject fixture initialization or
  fixture-profile actions on every other genesis as a block-validity error. No public endpoint or deposit is
  needed for this slice.
- [x] **Merge test.** Add `crates/node/src/main_parts/tests/pq_payment_local_chain_tests.rs` using the fleet
  harness in `private_action_six_validator_tests.rs` in that directory. Finalize the actual hop, independently
  read its certificate, accepted receipt, two spent serials and post-root on another validator, then reopen
  and replay the block. A changed proof must fail validator verification with the same pre-state, including
  when proposed directly. A second use of the accepted hop fails. Use the independent Go verifier in
  `go-verifier/{main,binius_pcs,circuit,field}.go` and recompute the post-root in Go from fixture leaves
  followed by the two accepted output commitments. Keep the original four fixture pairs and all seven stages
  of `verify.sh` passing. Run the matching verifier vectors through the portable and accelerated arithmetic
  paths.

## M2. Freeze the L1 action and its archival encoding · 4–7 days

- [x] Add `crates/types/src/pq_payment.rs`, include it from `crates/types/src/lib.rs`, and extend
  `ShieldedAction` and `private_actions_v1.rs` with `pq_payment_v1`. Derive the mode from the header. A
  positive mint means shield, a positive unshield means exit, and two zeros mean internal payment. Reject
  simultaneous edges, kind/field disagreement, unknown versions or profiles and surplus authorizations. The
  private bits selecting real and dummy inputs remain private. A wallet makes a pure deposit with two dummies.
  The existing relation also permits a funded top-up with real inputs, so use the same exact-debit rules for
  it.
- [x] Freeze this inner layout in parse and serialization vectors, in the following order. Start with eight
  magic bytes `PFSH02\0\0`, 48-byte anchor, LE u64 mint and unshield, 32-byte exit destination, LE u64 fee,
  two 48-byte nullifiers, two 48-byte output commitments, two 1,670-byte envelopes, LE u32 proof length, exact
  proof bytes. The fixed prefix including length is 3,648 bytes. For L1 encode the chain's canonical 20-byte
  account ID as twelve zero bytes then the ID in the destination field. Require a zero field without an exit
  and a canonical, nonzero, permitted recipient on exit. Use the address codec and active recipient rules in
  `crates/crypto_provider/src/address.rs` and `crates/execution/src/recipient_format.rs`. Test round trips
  against real account address encodings.
- [x] Define a compact binary outer action with a profile tag, fixed-size governed asset selector mapping to
  an immutable issue ID, read epoch, and optional fixed-size account fields, public key and signature. Ship
  byte-exact vectors for all modes. At most one account signature per action under this fee design. Enforce
  **149,130 bytes for the entire archived payment**, including its allocated batch framing and authorization.
  The tool's calculated inner maximum is 140,476 bytes, leaving 8,654 for framing and authorization (ML-DSA-65
  public key 1,952 and signature 3,309). Cap lengths before allocating, count the actual serialized bytes,
  require exact inner and outer exhaustion, and enforce the independent 1,048,576-byte JSON request limit
  before decoding.
- [x] The current `BatchArchiveEntry.payload_json` stores text. Hex or base64 persisted there would exceed the
  archival target. In `crates/types/src/core_chain.rs`,
  `crates/node/src/{storage_commit,batch_snapshot,block_replay_wallet}.rs` and
  `crates/storage/src/transactional.rs`, add a versioned binary payload record addressed by its authenticated
  digest. Archive compact references plus the binary body once. Require validators to possess and verify the
  body before voting. Bind its exact bytes to the certified batch ID, commit body and reference atomically,
  and include both in snapshot export and historical reads. Use one canonical transport encoding (padded
  standard base64 in strict JSON) at the API boundary. Measure incremental archive bytes including reference
  overhead separately from wallet indexes, snapshots and replicated database overhead.
- [x] **Merge test.** Add `crates/privacy_binius/tests/l1_vectors.rs` and
  `tools/pft-binius-shielded-pq/go-verifier/l1_profile_test.go`. Check Rust/Go agreement for all modes, wrong chain/genesis/asset/epoch/fee terms and
  fixture-profile rejection under L1. Round-trip Rust, Python and JS transport to identical binary bytes.
  Exercise truncation, duplicate JSON fields, illegal padding, inner/outer trailing data, overlong proof
  counts, limits at and one above each cap. Store and replay a largest-case authorized payment and reject a
  missing or mismatched body. Enable this profile only on development chains pending the M8 activation decision.

The outer wire is `PFPQ`, one-byte version, profile and mode, 48-byte asset ID (zero for PFT,
otherwise the source-series digest), LE u64 input epoch and public PFT fee, then a one-byte
authorization flag. When set, it carries the 20-byte payer, LE u64 sequence, 1,952-byte public
key and 3,309-byte signature. The exact PFSH02 inner record follows. The eight
[`l1-vectors`](../../tools/pft-binius-shielded-pq/l1-vectors) cover both assets and a funded top-up.
Validators verify the L1 profile on the pinned disposable genesis. Its receipt records
`pq_settlement_inactive` pending M3–M4.

The redb store publishes the reference and body in one transaction. The file-backed store fsyncs
the immutable body before publishing its reference. A crash between the two can leave an
unreferenced body file.

The largest certified vector occupied 144,470 bytes including the logical reference, 144,770 with
redb keys and authentication, or 144,804 with the JSONL envelope and digest filename. For the
140,476-byte inner ceiling those sizes were 146,246, 146,546 and 146,580 bytes, respectively.
The largest measured total leaves 2,550 of 149,130 bytes. These are incremental single-payment
archive bytes. Indexes, snapshots, database pages and replicas are outside this measurement.
The ceiling test stores and reopens a signed header with a maximum-length proof encoding. Consensus
replay and snapshot restoration use the largest genuine proof vector. Its sampled Merkle opening
is shorter than the calculated maximum.

## M3. Commit an isolated pool and make old notes spendable after rollover · 6–10 days

- [x] Add optional PQ state alongside `ShieldedState.orchard` in
  `crates/types/src/shielded_bridge_governance.rs`. Define records in `crates/types/src/pq_payment.rs` and
  transition code in new `crates/node/src/pq_pool_state.rs`. Production starts empty. Only an accepted PQ
  proof transition can write leaves. The spec's SHAKE256 binary depth-24 tree uses 48-byte nodes, empty leaf
  zero, and recursively hashed empty subtrees. Maintain count, frontier and indexed nodes per (asset, epoch).
  Compare incremental appends and wallet witness paths with the reference tree.
- [x] Implement R1–R6 against the ordered execution view. Check a retained anchor of the selected tree, two
  distinct fresh nullifiers, two distinct fresh nonzero commitments, valid public flows with checked fee plus
  exit, valid proof and room for **two** leaves. Record both nullifiers including dummy serials. Index
  nullifiers and commitment uniqueness across the entire PQ pool and all epochs. Append outputs in wire order
  and record only the root after both. Store authenticated output kind, asset, epoch, index, height,
  commitment and exact ciphertext. Track checked per-asset live reserve and ingress, exit and fee totals. Use
  a transactional delta for the pair, indexes, reserve and account writes. Compute receipt and all fallible
  checks before publication.
- [x] Retain a consensus window of 256 roots per active tree, counting the opening root until evicted. Freeze
  the retained window at epoch close and keep at least its terminal root spendable indefinitely. Keep history
  needed for wallet witnesses available through archival pages. This Dual-Anchor rollover window is fixed by
  consensus, independently of the node's RPC snapshot cache. At exactly 16,777,216 leaves freeze the tree and open an empty successor
  for the next pair. Inputs may use an accepted old root while outputs always append to the current writable
  tree of the selected asset. Both real inputs share **one** root. A wallet consolidates across epochs by a
  one-real/one-dummy self-payment from the old epoch, waits for finality, then spends in the new tree. Two
  independently specified input anchors require a future wire/circuit version. Rollover never resets serials,
  reserve or commitment uniqueness.
- [x] Update both replicated root paths, the exhaustive state inventory and `append_shielded_state` in
  `crates/node/src/state_commitment.rs`. Commit tagged, length-delimited per-tree frontier/count, ordered
  retained roots and epochs, sets/index roots, output records including ciphertext digest, reserves and
  cumulative flows. Derive indexes deterministically, sort unordered keys and avoid full-history copying or
  hashing per payment. Wire versioned authenticated store roots into
  `crates/node/src/{storage_commit,batch_snapshot,block_replay_wallet,lifecycle_checkpoint}.rs`. Import into
  staging, rebuild/check derived indexes and roots against a certified snapshot point before publishing. An
  absent PQ section is valid for pre-activation history. Never accept arbitrary JSON anchors, occupancy
  overrides or post-activation missing data.
- [x] **Merge test.** Add `crates/node/src/tests/pq_pool_state.rs` and extend `tests/snapshot_deployment.rs`
  under the same node source directory. Cover root eviction and exact window edge, duplicate and zero outputs,
  intra/inter-block conflicting spends, old-root spends after two rollovers, a pair at capacity and the next
  pair, rejection of mixed-epoch inputs, and global replay after rollover. Use a checked near-full frontier
  plus independently computed root at real depth. Compare crash/restart, snapshot restore and genesis replay
  with identical receipts, roots, serials, output order and reserves. Corrupt each committed component
  individually. Replay an exact pre-upgrade archived chain through its recorded tip and compare block hashes
  and state roots byte for byte.

The node verifies proofs and prepares the account delta in `pq_pool_state.rs`. Execution and cold storage
recovery share the deterministic pair transition in `crates/types/src/pq_tree.rs`. Immutable pair pages
retain the ciphertexts and supply the inputs for rebuilding the wallet indexes. The six-validator merge test
settled a funded native deposit and spend on the pinned disposable genesis. Governed pfUSDC account settlement
remains in M4. Both assets were exercised in the pool tests.

Rust and Go independently built two full depth-24 trees. The node test used their checked near-full frontiers
to exercise rollover and spend an epoch-0 note in epoch 2. Journal recovery, signed snapshot restore and genesis
replay agreed at the certified tip. The exact R64 replay matched all 1,734 archived block hashes and state roots.

## M4. Settle deposits, transfers and exits for both assets · 5–9 days

- [ ] Implement new `crates/node/src/pq_payment_actions.rs` invoked by `execution_actions.rs`. Put account
  checks and prepared debit/credit helpers in
  `crates/execution/src/{entrypoints,issued_asset_ledger_helpers}.rs`. Reuse the account's FIPS-204
  **ML-DSA-65** provider in `crates/crypto_provider/src/lib.rs`, including source derivation, first-spend key
  registration, pinned key, namespace and checked next sequence. Define a shield-only signing preimage in
  `crates/types/src/pq_payment.rs`. Include the spec's R0-Auth domain `PFT-SH-MINT-AUTH-V2`, source and raw
  `tx_ctx`, plus the length-framed L1 profile/context, next sequence, exact amount and fee terms. The profile
  defines this extended preimage identically for signer and verifier. The raw context already binds both
  output commitments and both envelopes. The signer builds or independently checks those commitments against
  the locally intended recipient and amount before releasing the signature. Signatures, proof bytes and
  transaction IDs are outside their own preimage. Give fee-only sponsorship its own signing domain. A signed
  shield authorization has no ordinary burn, transfer or legacy-ingress interpretation.
- [ ] For each accepted action update the live reserve by `+v_mint − v_unshield − inner_fee` using checked
  wide intermediates. Shield debits the signed source by exactly the mint amount. Internal payments have no
  public recipient. Exit credits the named public recipient by exactly the exit amount. On native PFT require
  the active minimum fee and book the inner fee once to the chain's fee burn accounting. For pfUSDC burn the
  separately signed PFT fee and preserve total issued units. Charge and advance an authorizing account once.
  Validate total available funds including minimum balance, applicable fees, trustline restrictions and
  account creation rules before applying any change. Use the normal native recipient-creation cost where
  supported and refuse an unfundable fresh recipient. Require an eligible existing account and authorized line
  for a restricted issued exit. A pool exit cannot create issuer authority, widen limits implicitly, or mint
  as issuer.
- [ ] Activate a specific issue only when both its deposit and exit policy can be enforced. Check effective
  pause, issuer/freeze, authorization, reserve and recipient eligibility at the inclusion height. For an issue
  whose policy requires an unsupported disclosure, refuse entry as well as exit. In
  `crates/node/src/state_commitment.rs`, add the PQ live reserve to `global_issued_asset_supply`, cap checks
  and the custody inventory. Count each child once in the family aggregation, alongside public, FastLane,
  external bridge and Orchard custody. Cumulative inflows are audit counters. Include public source/exit and
  PFT fee effects in `crates/node/src/{block_finality,account_tx_disk_index,lifecycle_queries}.rs`. Keep
  internal note owners out of account history.
- [ ] **Merge test.** Add `crates/node/src/tests/pq_payment_accounting.rs`. On a fresh local chain shield →
  pay → partial exit → full exit for PFT and a selected pfUSDC child. Reconcile public balances, notes
  recovered by two wallets, reserve, burns, account sequence and total family supply after every block. In the
  closed native fixture, public PFT plus live PQ PFT plus cumulative burned PFT must equal starting supply.
  Test unfunded or unsigned mint, reused sequence, substituted outputs/envelopes, standalone burn replay,
  sibling issue, wrong recipient, underflow, overflow, frozen/unauthorized/full line, required disclosure and
  exhausted sponsor. Failed actions and failed atomic batches leave fees, balances, keys, sequence, PQ state
  and history consistent. Include a cap-at-limit issuance attempt with most units already in the PQ pool.

## M5. Enforce activation, resource limits and public admission · 5–8 days

- [ ] In `crates/types/src/shielded_bridge_governance.rs` define a chain/genesis-bound amendment with earliest
  inclusion height H, allowed profile/assets, fee schedule and block limits. Unconfigured means inactive.
  Check presence of the kind and limits *before* execution in
  `crates/node/src/{consensus_artifacts,mempool_proposals,execution_actions,block_replay_wallet}.rs`. Apply at
  the candidate height in proposer, validator vote/acceptance and replay. Before H reject the **whole block**
  carrying the variant, even a non-atomic batch with a rejected receipt. Keep the upgrade opt-in until
  validators and archive readers support its wire version.
- [ ] Charge deterministic proof and byte units for every attempted PQ action in every batch in the block.
  Start rehearsal at four proofs and 596,520 archived payment bytes per block, subject also to the chain's
  smaller applicable byte limits. Four units reserve up to 200 ms of one core against a 50 ms/action
  engineering budget. Set the release cap from the measured consensus round headroom including ordinary
  traffic. Wall-clock timing is never a validity rule. Benchmark full parse, proof, ML-DSA where present,
  state lookup, append and accounting on each supported CPU class, warm/cold, valid and worst bounded
  rejection, at representative history size. Require measured full verification below 50 ms/tx single core and
  a full block inside the round deadline. Record median, p95, max, CPU/build flags, RSS and persistence
  latency separately. The recorded stand-alone Rust median is 22.53 ms. Portable Go exceeds 50 ms in the
  recorded run. A one-off check of the existing binaries during this design also took 34.56 ms in Rust and
  52.00 ms in accelerated Go for the same hop. These untuned fixture runs did not measure validator
  settlement.
- [ ] Extend
  `crates/node/src/{private_action_admission,private_action_rpc,rpc_dispatch,rpc_cli,atomic_swap_rpc_server,shielded_batch_actions}.rs`.
  Scope the public allowlist by pool and caller. Cover `private_action_submit`, the queue service's persisted
  records on startup and immediately before dispatch, direct `shield_batch_finality`, proposal/peer round
  ingress, CLI batch application and simulation. Check `/api/private/v1/submit`, relay forwarding and the
  legacy `/api/shielded-nav-swap/{ingress,swap,egress}` routes with legacy certification both on and off.
  Legacy batch builders and authenticated operator rounds must respect PQ pool ownership and block validity. Share the consensus verifier across paths. Bound transport bytes,
  per-peer attempts, global queued bytes, concurrent verification and replay work before acquiring the ledger
  write lock. Verify against a pinned view outside the admission lock. Recheck tip and conflicting
  reservations in one short durable transaction.
- [ ] Reserve a hash of the **canonical public statement and unsigned economic terms** independent of proof
  randomness, ML-DSA randomness, client intent ID or JSON spelling. Also reserve each nullifier, each output
  and applicable public account/sequence. Conflicting statements sharing even one nullifier cannot both queue.
  Same statement returns the existing record after caller checks. Consensus checks spent keys again
  independent of local queues. Recover reservations and final receipts after crashes, release them on
  expiration/abandonment, and distinguish transient rejection from an accepted spend. Check chain inclusion
  before retrying an ambiguous dispatch.
- [ ] **Merge test.** In `crates/node/src/tests/private_action_admission_tests.rs`, new
  `crates/node/src/tests/pq_payment_activation.rs`, and `crates/node/tests/private_actions_gateway.rs`, test
  every route at H−1, H, H+1 and without an amendment, including mixed and non-atomic blocks, stale queued
  records and a direct peer proposal. Stress two simultaneous reproves of the same statement, different
  statements sharing one serial, JSON aliases, timeout/restart, stale tip, pause/lift and limit+1 spread over
  several batches. Confirm exactly one final spend, deterministic rejection at all validators and continued
  service for valid traffic.

## M6. Recover notes and pay from the Python CLI · 6–10 days

- [ ] Add `crates/node/src/bin/pftl_pq_walletd.rs` as a user-owned native prover/scanner accessed over an
  owner-only local socket. It uses `rust-prover/src/payment.rs` and the L1 prover profile for both
  construction and verification before submission. The prototype recorded 2.9 s median on 16 cores/32 threads,
  26.7 s on one core, about 562 MiB peak RSS. Limit concurrency and expose progress and cancellation.
  Benchmark on a supported laptop. Browser proving would need its own memory, thread, randomness and
  portability measurements.
- [ ] Add `crates/privacy_binius_wallet/src/{lib,keys}.rs` and its manifest. Start from high-entropy wallet
  seed material. Specify HKDF-Extract/Expand with HMAC-SHA-384, a fixed versioned protocol salt and
  length-framed info including genesis/network, account index and address index. Publish the exact salt,
  integer widths and byte order with cross-language derivation vectors. Use separate fixed versioned labels
  `postfiat/pq-wallet/v1/spend` (48-byte S) and `postfiat/pq-wallet/v1/kem` (64 bytes split into ML-KEM-1024 d
  and z). Keep account-signing and Orchard derivations independent. The circuit derives the nullifier from S
  and the coin. Preserve that relation and the specified envelope SHA-384 KDF. Use fresh OS randomness for
  blinders, encapsulation, proof and dummy inputs. Encode a checksummed network/version receive address
  carrying the 1,568-byte encapsulation key and 48-byte owner commitment. Recovery records carry version,
  account/index range and birthday. Fresh entropy is required after an old seed disclosure.
- [ ] Extend `crates/node/src/{private_pool_reads,private_pool_snapshot_points}.rs` and
  `crates/rpc_sdk/src/{protocol_requests,response_validation}.rs` with bounded finalized pages of PQ
  commitments, envelopes, serials, retained anchors and epochs. Pin all pages to one certified (height, block
  hash, state root), verify authenticated page coverage or reconstruct and compare roots before spending.
  Handle cursor expiry by resuming from an independently verified checkpoint. A wallet downloads public ranges
  for all recipients, reconstructs the tree/witness locally, and trials each envelope with the KEM
  decapsulation key. Accept only on valid AES-256-GCM (AAD = coin) **and** a matching reconstructed coin.
  Record opening, leaf and epoch in encrypted local storage. Derive serials locally and remove spent matches.
  Never send a private note index, scan/spend key or witness to a gateway. A rejected or undecryptable
  individual record cannot break another payment's recovery.
- [ ] A measured 4,722 nonmatching outputs/s/core is about 37 s/core for 172,800 outputs (one day at one tx/s,
  two outputs per tx, one scan key). Track keys, pages and sync lag separately. Choose the latest finalized
  eligible anchor, randomize payment/change slot, and preserve the old tree terminal witness through rollover.
  Add `python/postfiat_rpc/pq_wallet.py` with dispatch in `__main__.py` and transport in `client.py`. Expose
  address, sync, balance per asset/issue, shield, pay, unshield, status/receipt and restore. Show pending and
  finalized balances separately. Use owner-only encrypted files, atomic writes and distinct paths for secrets
  and shareable artifacts.
- [ ] **Merge test.** Add `python/tests/test_pq_wallet_cli.py` and wallet-crate vectors. Two clean wallet
  directories run the CLI against a local chain for both assets. Restore Bob from backup, rescan from birthday
  across an epoch, find only his funds and spend them. Compare Rust and Go trial-decryption results, including
  corrupted envelope, wrong key and authentic ciphertext with the wrong opening. Check distinct
  network/role/index derivations, deterministic recovery, no secrets in RPC/logs, path-collision rejection and
  cancel/restart without duplicate spend.

## M7. Expose the wallet flow and keep Orchard usable · 3–6 days

- [ ] Extend `wallet-proxy/{private-actions,server}.js`, `crates/wallet_wasm/src/lib.rs` and
  `wallet-web/src/App.jsx`. Route PQ snapshot/submit/receipt through a versioned contract using the same
  canonical statement vectors as Rust and Python. Build the browser's local, authenticated wallet-daemon
  connection with origin and session checks. Keep witnesses and recipient scanning on the user device. WASM
  can encode/check addresses and review/sign exact account terms locally. A hosted proxy never receives the
  wallet seed. Show public and private balances per issue, fee payer, proof progress, finality and the three
  actions **Shield**, **Pay**, **Unshield**.
- [ ] Show the asset/issue, fees, public deposit or exit account and amount, and any sponsor. The submission
  service can observe the caller and timing. Use measured activity for the selected issue and route when
  applying the [observer model](../privacy/observer-inference-model.md) and
  [privacy floors](../privacy/floor-calibration.md). Show “hidden, small pool” where appropriate. A large
  aggregate count across issues gives no privacy evidence for one thin pool.
- [ ] Keep `crates/privacy_orchard/src/{lib,asset_orchard}.rs` and its legacy reader separate. Use different
  pool IDs, committed state, serialization, keys and scan routes. Migration takes two transactions. Authorize
  an Orchard exit to a public account, wait for finality, then sign a fresh PQ shield. Display the publicly
  linkable amounts, fees and existing exit restrictions. Prevent automatic re-shield retries after uncertain
  finality. Copying historical commitments, reserves or wallet keys would bypass the PQ deposit check.
- [ ] **Merge test.** Extend
  `wallet-proxy/{test_private_actions,test_legacy_shielded_certification_node_binary}.js` and add
  `wallet-web/scripts/test-pq-payments.mjs`. Drive two user sessions through both assets and verify the
  recipient's second spend and exit against validator receipts. Cover proxy/node allowlist agreement,
  direct-finality and legacy HTTP routing, signer review of changed terms, disconnected daemon and partial
  sync. Run the old Orchard wallet flow and a policy-permitted exit → PQ shield, including a stop/restart
  between steps. Reconcile public and both pool reserves. Verify historical Orchard-only snapshot and
  ciphertext recovery separately.

## M8. Qualify the release and activation · 4–7 days plus independent review

- [ ] **Merge test.** Rehearse an upgrade with an exact archived chain and a fresh six-validator chain. In new
  `crates/node/src/main_parts/tests/pq_payment_rehearsal_tests.rs`, exercise pre/post-H binaries, mixed
  traffic, resource budgets, rollover, cold restart, signed snapshot import, complete replay and
  certificate/read-back for both assets. Record commit, genesis, H, hardware, largest stored payment, worst
  verified time, total supply and roots. Schedule H only after the release checks pass and validators have
  installed the compatible build. A pause blocks new inclusion under explicit height rules and preserves
  recovery/replay of finalized history.
- [ ] Obtain external cryptographic and implementation review of the verifier, L1 context, account binding,
  compiler assumptions and key handling. Repeat the seven-stage independent-language replay, all new
  integration negatives and the affected Orchard archived replay. Reserve the broad release suite for this
  final state-commitment/compatibility gate. Document measured behavior in
  `docs/privacy/{overview,deposit-spend-withdraw,rpc-and-resource-policy}.md`,
  `docs/quantum/wallet-implications.md` and `docs/wallets/shielded-wallet.md`. Once the CLI, interface and
  release evidence agree, retire the completed implementation journal under `docs/plans/completed/`.

### What remains unproven

- The concrete quantum extraction bound depends on the unreviewed compressed-oracle bridge in Appendix A.3.
  Multi-proof simulation extractability in A.4 is conjectural. The Peel/RS argument and multi-proof
  zero-knowledge simulator/reprogramming analysis in Appendices A–B also need independent review. Published
  generic multi-round bounds do not establish the quoted concrete security level. Numerical bounds depend on
  query/proof counts and idealized oracle assumptions.
- Authorization and privacy additionally assume SHAKE256 joint-PRF, hiding and multi-target resistance
  properties, the key-in-middle SHA-384 envelope KDF, ML-KEM-1024 recipient anonymity under active scanning
  (IK-CCA), AEAD security and the outer account signature. Standard ML-KEM confidentiality alone does not
  prove recipient anonymity. The Rust, Go and Python implementations share authorship and specification.
  Agreement checks implementation consistency. The full platform, dependencies and side channels lack a
  constant-time audit.
- A proof binds ciphertext bytes to the transaction. It does not establish that they decrypt for the intended
  recipient or encode the proved opening. A sender can make funds unrecoverable. Static scanning keys expose
  archived deliveries if compromised. The prototype provides neither a protocol-defined full viewing key nor
  diversified addresses, encrypted memos, or variable arity. The single-challenge privacy game excludes later
  spends of challenge outputs, IP/timing, fee and anchor correlation. It gives no end-to-end unlinkability
  guarantee.
- The prototype measures single-tree verification and local proving. Its JSON ledger assumes a supplied
  history. It does not prove authenticated origin of each root, account debits, policy-correct exits, L1 asset
  separation, crash atomicity, rollover, validator deadlines or real anonymity-set size. M1–M8 must establish
  those integration properties separately. Real-value activation needs an explicit decision on residual
  cryptographic assumptions and measured operating limits.

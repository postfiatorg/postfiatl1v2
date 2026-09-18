# Cobalt handoff and authority review — 2026-09-16

This is A3 of the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). The review read all 4,931 lines of the four allowed files at `204fd33f`: `crates/node/src/cobalt_handoff.rs` (2,044), `cobalt_authority_certificate.rs` (910), `cobalt_shadow_runtime.rs` (643), and `cobalt_handoff_rehearsal.rs` (1,334), including their in-file tests. References below describe that revision. The focus was certificate/quorum requirements, replay, rehearsal/live separation, shadow write authority, and malformed certificates.

## Findings

### CHO-01. P2 — full-knowledge checkpoints are not bound to the decision being verified

**Source:** `crates/node/src/cobalt_authority_certificate.rs:708-774`.

The verifier validates the current ratification and each full-knowledge checkpoint independently, but does not require the checkpoint interval or signed pending pair to match that ratification. With two valid transcripts under the same registry and trust graph, replace the newer transcript's full-knowledge checkpoints with the older transcript's checkpoints and recompress. The old checkpoint signatures remain valid and the signer sets match; the verifier returns a decision for the newer payload even though its full-knowledge evidence concerns the older slot and candidate.

A local regression using the existing four-validator fixture reproduced acceptance: a height-12 decision accepted height-11 checkpoints carrying the older pending pair. This is a certificate-stage binding defect, not evidence that an unsigned governance update can pass: the other protocol signatures and outer validator-update authorizations remain required.

The minimal repair is to require each checkpoint's interval/coverage and each signed check's height and pending pair to bind the current ratification's activation height, amendment slot, and output candidate before accepting its support. Regress the valid earlier-checkpoint substitution and retain valid-certificate coverage. This repair is **consensus-affecting** because it tightens authority-certificate admission; no excluded-file change is required.

### CHO-02. P2 — compact transcript expansion multiplies unchecked shared data

**Source:** `crates/node/src/cobalt_authority_certificate.rs:266-309,476-494,727-739`.

The 16 MiB decompressed bound applies to the compact JSON. Expansion then clones its shared full-knowledge checks into every caller-supplied checkpoint, before the live-registry checkpoint-count check. A small, highly compressible malformed certificate can contain many checkpoint shells and a large shared check vector, causing their product to be allocated repeatedly before signature or cardinality rejection. The 1 MiB certificate and 2 MiB shadow frame limits do not bound this expanded product.

The minimal repair is to calculate the expanded transcript size with checked arithmetic before cloning and reject expansion beyond the existing 16 MiB value bound. Regress a compact input below the limit whose expanded transcript exceeds it, with an ordinary expansion still accepted. This repair is **consensus-affecting** because the bound applies to authority-certificate admission. It changes no signature encoding and requires no excluded file.

### CHO-03. P2 — a failed response write terminates the shadow listener

**Source:** `crates/node/src/cobalt_shadow_runtime.rs:141-168,517-524`.

Both malformed-request error responses and ordinary responses propagate `write_response` errors out of the listener loop. A peer that resets its connection before the reply, or requests a response exceeding the frame limit, can stop the entire listener instead of losing only its own connection. This interrupts advisory signing/verification service availability; it does not grant live write authority.

The minimal repair is to contain response-delivery failures within the current connection and continue serving subsequent requests, retaining explicit shutdown policy and request bounds. Regress failed writes for malformed and ordinary requests followed by a successful probe, using in-memory streams so no network service is contacted. This repair is **not consensus-affecting**.

### CHO-04. P2 — successful update rehearsal panics on the valid Foundation scope route

**Source:** `crates/node/src/cobalt_handoff_rehearsal.rs:1017-1059`; routing at `crates/node/src/cobalt_handoff.rs:176-190,223-230`.

After verifying a valid signed Cobalt update, `finalize_update` constructs an unrelated crypto-policy-only batch and calls `expect_err`. The reviewed router deliberately returns the Foundation route for that batch, so the command panics before writing the successful update result. The normal rehearsal cannot finish or supply its result to the rollback preparation step.

The minimal repair is to test the intended Cobalt scope interlock with a mixed validator-update/unrelated-amendment batch, handle an unexpected success as an ordinary error, and label the resulting evidence as mixed-batch rejection. Pure unrelated governance must retain its Foundation route. Regress the full finalization function with a valid signed update and check the result and scope evidence. This repair is **not consensus-affecting**; the production authority rule already behaves correctly.

### CHO-05. P3 — negative rehearsal probes discard the approvals they need to test

**Source:** `crates/node/src/cobalt_handoff_rehearsal.rs:458-563`.

`negative_cases` replaces all transition approvals with an empty vector before running its probes. The stale-height case can therefore fail only for absent quorum while the report still labels it a stale rejection; the replay setup also applies an approval-free transition directly to a temporary governance value. An operator can read `all_rejected` as evidence of independent signed-input checks that were not exercised.

A future minimal repair would retain a verified positive signed fixture, vary one binding per case, assert the expected rejection reason, and report only the in-memory scope actually exercised. This is recorded without repair as required.

### CHO-06. P3 — rehearsal manifest digest validation admits letters outside hexadecimal

**Source:** `crates/node/src/cobalt_handoff_rehearsal.rs:140-155,175-193`.

The predicate accepts every lowercase ASCII letter, so a 96-character `g` string passes the claimed lowercase-hex check for the block/state anchor, trust root, or Cobalt lock. The genesis identity is independently recomputed, but a malformed block/state anchor can still be carried into prepared rehearsal evidence without rejection. Later production transition checks reject malformed lock/root digests; no live admission bypass is claimed.

A future minimal repair would require lowercase hexadecimal digits for every declared digest and test non-hex lowercase input. This is recorded without repair as required.

## Areas with no findings

- `cobalt_handoff.rs:29-611,752-997`: transition signatures bind chain/genesis, modes, parent, old registry, lock, graph, height, sequence, protocol, scope and quorum; approvals must be sorted, unique, active and ML-DSA-65 verified. Exact-height and parent/sequence checks reject replay, mixed authority is rejected, Cobalt updates require a decision certificate plus validator authorizations, and unrelated governance retains Foundation routing.
- `cobalt_handoff.rs:613-750,899-959`: reviewed history/progress bookkeeping checks forward event order and lock/graph extension. It is a structural history checker, not an independent historical signature audit; registry reconstruction and replay callers remain outside A3.
- `cobalt_authority_certificate.rs:93-135,137-264,334-459,471-725,776-910`: compressed values check headers, digest and canonical encoding; live registry/domain bindings, sorted unique active senders, shared stage signer sets, delegated RBC/ABBA quorum verification and recomputed MVBA/DABC results are present. These observations do not remove CHO-01 or CHO-02, and delegated cryptographic implementations were not reviewed.
- `cobalt_shadow_runtime.rs:28-139,172-279,414-505,547-587`: requests route to shadow service methods; no direct node governance mutation or block-finality authority is introduced in this file. Frame bounds, explicit shutdown policy and loopback/private-listen address validation are present. Actual service persistence and caller-enforced listen policy were not reviewed.
- `cobalt_handoff_rehearsal.rs:76-98,353-447,599-700,724-941,979-1015,1117-1172`: JSON reads are bounded; activation/update/rollback finalization invokes authority verification before changing its local governance value. The live-context function reads node state and writes requested artifact paths. Signing helpers intentionally emit production-domain signatures, so their output is not intrinsically confined to a clone; actual authority still requires production verification and ordering. No remote command or node-state write API appears here.

## Review limits and skips

Only the four named source files were reviewed. Called implementations in `cobalt_shadow`, consensus protocol/cryptography, types, execution, storage, governance admission, state commitments, RPC dispatch and command wrappers were not reviewed. In particular, A3 does not certify historical registry reconstruction, durable signer locks, the shadow service's own authority flags, output-path isolation, or all consumers of rehearsal reports. No finding's minimal repair requires an excluded file.

Release-candidate source files, previously reviewed surfaces, excluded crates and frozen artifacts were not reviewed or edited. Remote-ref filename metadata confirmed none of the four A3 files differs on the release ref. A broad instruction-filename search also returned the release checkout's `AGENTS.md` path; its contents were not opened and that checkout was not mutated. No release branch operation occurred.

The checkpoint reproduction ran entirely on local temporary fixture state, without sockets or validator hosts. One initial fixture compilation failed on a missing `std::fs` qualification and executed no tests; two subsequent runs each reproduced the same failed assertion (0 passed, 1 failed). The exploratory test is removed before the findings-only commit and will be retained with the repair.

Socket/network drills, live-chain/fleet operations, physical fault testing, full Rust/Orchard suites and CI status queries are skipped under the brief's network and scope limits. Focused regression tests will use local files or in-memory streams. The full Rust suite is CI's verdict. A4, A5 and B, including inventory/scoring, remain unstarted. No Task Node or fleet action occurred.

## Repair result

CHO-01 through CHO-04 are repaired; CHO-05 and CHO-06 remain recorded without repair. Only the four A3 source files changed.

- **CHO-01 — consensus-affecting:** checkpoint interval/coverage and every signed check's height and pending pair must bind the current ratification's activation height, amendment slot and candidate. The regression first verifies current evidence, then rejects valid older checkpoints substituted into the newer transcript.
- **CHO-02 — consensus-affecting:** checked size arithmetic bounds the serialized expanded transcript to 16 MiB before shared-check cloning. The same guard prevents the compressor from producing an over-bound transcript. A correctly encoded compact fixture below 16 MiB that previously expanded beyond 32 MiB now fails at the expansion guard; ordinary and 20-validator certificates still pass. This is a serialized-data bound, not an exact process-memory limit.
- **CHO-03 — not consensus-affecting:** response-write failures stay within their connection. The serving loop has a generic stream seam used by both the TCP listener and in-memory regression. The regression covers malformed-request and normal-probe write failures, disabled shutdown, a subsequent successful probe, unchanged shadow state and disabled authority flags. No sockets were opened.
- **CHO-04 — not consensus-affecting:** finalization checks rejection of a mixed Cobalt-update/unrelated-amendment batch and returns an ordinary error if that interlock unexpectedly fails. Its report now names `mixed_authority_batch_rejected`. The regression calls the finalization function with valid signed input, reads its successful output and verifies the restored governance history. The existing pure-unrelated-governance Foundation-routing test remains green.

The certificate format and signed/hashed encodings are unchanged; the two new authority admission rules are source changes only. **Full Rust suite verdict pending** CI. No live activation or deployed behavior is claimed. Rehearsal report consumers outside A3 were not tested.

All Cargo build/test commands used `CARGO_NET_OFFLINE=true`:

- Before repair, `cargo test -p postfiat-node burn5_ --lib --locked`: 4 passed, 4 failed, 359 filtered out. Three A3 failures reproduced CHO-01, CHO-03 and CHO-04; the expansion fixture initially failed canonical decoding before reaching CHO-02. The broad filter also reran four existing A1/A2 regressions, all passing; no other surface was reviewed or edited.
- After correcting that fixture to preserve typed canonical field order, `cargo test -p postfiat-node burn5_cobalt_authority_bounds_shared_check_expansion --lib --locked`: 0 passed, 1 failed, 366 filtered out, because the original decoder returned the over-bound expanded transcript. The failure assertion was then changed to avoid printing that large public fixture.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node cobalt_handoff::tests --lib --locked`: 13 passed, 0 failed, 0 ignored, 354 filtered out. This includes both authority-certificate regressions, the finalization regression, quorum/replay/scope checks and the existing 20-validator certificate.
- `cargo test -p postfiat-node cobalt_shadow_runtime::tests --lib --locked -- --skip local_network_drill_runs_real_signed_protocol_over_three_sockets`: 3 passed, 0 failed, 0 ignored, 364 filtered out. The socket drill was explicitly excluded by the network boundary.
- `cargo test -p postfiat-node cobalt_handoff_rehearsal::tests --lib --locked`: 1 passed, 0 failed, 0 ignored, 366 filtered out.
- Post-repair total: **17 passed, 0 failed**. No broad Rust/Orchard suite or CI status query ran.

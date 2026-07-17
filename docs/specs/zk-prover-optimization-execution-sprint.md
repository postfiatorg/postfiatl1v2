# ZK Prover Optimization — Overnight Execution Sprint

> **Purpose:** a self-contained, thorough work plan for autonomous overnight execution. Derived from the separate website-repository analysis `heavy-zk-optimization-v2.md`, which is not a dependency of this source tree. The orc executes this phase-by-phase, measuring real benchmarks at each step, with explicit contingency plans + stop conditions.
>
> **Primary deliverable:** MEASURED proving-time benchmarks at each optimization tier (the evidence gap the TIH flagged). The blog post's projected speedups must be replaced with real numbers.

## Objective

Reduce the Halo2 proving time for the AssetOrchard shielded-swap circuit from **~minutes (current, stock CPU)** to **as fast as possible on available hardware (32-core CPU)**, with the GPU path scoped for the Akash/io.net deployment.

- **Target:** <5s per proof on the 32-core box.
- **Stretch:** sub-second (requires GPU — scope it, don't implement it here).
- **Hard constraint:** no soundness test may break at any phase. Soundness > speed. Always.

## Preconditions (already done — do NOT redo)

- The circuit: `crates/privacy_orchard/src/asset_orchard_circuit.rs` (K=16, all constraints), `asset_orchard_sinsemilla.rs` (note commitments), `verify.rs` (the consensus verifier).
- The tests: the soundness regression (`swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation`), the 10-finding fixes, the spendability regression (`asset_orchard_ingress_notes_are_spendable_in_real_swap`) — all green.
- The existing benchmark-capable tests: the release-only full-swap proof tests (they create + verify real Halo2 proofs — use these as the benchmark harness).
- The circuit is rolled to the WAN devnet (binary `8a463893`).
- Branch: `navcoin-market-ops-envelope`.

---

## Authorizations (full operational authority for overnight autonomous execution)

The operator has authorized FULL operational authority for this sprint. The orc may execute all of the following WITHOUT asking the operator:

### Fully authorized

- **Modify + commit + push code** across `postfiatl1v2` AND `StakeHub` repos. Push to remote when a unit is green.
- **Spend real Arbitrum USDC** from the StakeHub wallet (`0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`, ~8 USDC available + gas ETH) for: vault deposits, bridge relays, live shielded swaps, ingress operations, benchmark flows that need real pfUSDC. This is authorized — the amounts are small-dollar.
- **Deploy + operate StakeHub** — modify the StakeHub repo, deploy changes, run the agent daemon, sign transactions, move USDC between the wallet and the vault.
- **SSH to the WAN devnet validators** — roll binaries, restart services, query state, execute certified rounds. Full devnet operational authority.
  - **Vultr boxes** (validators 3/4/5): `ssh -i ~/.ssh/id_ed25519 root@<ip>` (SSH key auth).
  - **Hetzner boxes** (validators 0/1/2): paramiko from the TIH venv (`text-improvement-harness-codex-plugin/.venv/bin/python`) reading the password from `~/machinemucket.txt`. NEVER sshpass.
- **Roll optimized binaries to the WAN devnet** after circuit/fork changes — one-at-a-time with consensus checks.
- **Execute live shielded swaps** on the WAN devnet with real pfUSDC/a651 (to benchmark the real end-to-end path, not just local tests).
- **Lease GPUs on Akash/io.net** if the AKT wallet (`akash1pt7ta3yrc856msgl9sduwt46rksks47mhlsf42`, key at `~/.local/share/postfiat/akt-wallet.json`) is funded — deploy the ICICLE-Halo2 prover container, run real GPU proofs, measure the actual speedup. (If the wallet is NOT funded yet, scope the lease + note "awaiting AKT funding" — proceed with the CPU path.)
- **Capture + update VK pin constants** after any circuit change.
- **Write docs + update blogs** across both repos.

### The only hard constraints

1. **Soundness > speed** — never ship an optimization that breaks a soundness/consensus test. If a test fails, fix the root cause or revert. Never mask.
2. **Don't spend more than available** — the StakeHub wallet has ~8 USDC + gas ETH. Don't attempt transactions that exceed the balance. The AKT wallet has 0 AKT (awaiting funding) — don't attempt Akash leases until it's funded.
3. **SSH hygiene** — Vultr via SSH key, Hetzner via paramiko+machinemucket.txt. NO sshpass, EVER. No passwords in commands.
4. **No private keys in commits** — pre-commit secret scan. The VK pin constants are public (safe). Private keys, mnemonics, passwords stay in secure files.
5. **Commit each unit** with a clear message + benchmark delta. Push when green.

---

## Phase 1: Baseline measurement (CRITICAL — do this FIRST)

**Goal:** measure the EXACT current proving time. This is the missing evidence.

### Steps

1. **Write a focused benchmark test** in `crates/privacy_orchard/src/asset_orchard_circuit.rs` (under `#[cfg(test)]`):
   - Construct the full swap circuit (`AssetOrchardSwapConservationCircuit::new_with_note_witnesses(...)` with valid test witnesses).
   - Time `create_proof` with `std::time::Instant` (wall-clock).
   - Time `verify_proof` separately.
   - Measure proof size (bytes).
   - Print: `baseline_prove_ms`, `baseline_verify_ms`, `proof_bytes`, `K`.
   - Mark `#[ignore]` (release-only, benchmark-scale — same pattern as the other heavy tests).

2. **Run the benchmark** in release: `cargo test -p postfiat-privacy-orchard zk_prover_baseline_benchmark --release -- --ignored --nocapture`.

3. **Measure CPU utilization** during proving: run the benchmark + simultaneously `top -H` or `ps -o %cpu` on the test process. Record how many of the 32 cores are active. This answers: is the current prover serial (1 core) or partially parallel?

4. **Record the results** in `docs/status/zk-prover-baseline-benchmark.md`:
   - Prove time (ms), verify time (ms), proof size (bytes), K, CPU utilization (cores active / 32).
   - The serial-vs-parallel determination.

5. **Commit** the benchmark test + the report.

### Contingency

- If the benchmark test fails to compile → fix the circuit constructor API (it may have changed since the last test was written). Don't paper over.
- If `create_proof` errors (witness invalid) → fix the witness construction. The proof MUST verify.
- If the test runs out of memory → note it; reduce the circuit for the baseline (but record that the full circuit OOMs — that's a finding).
- **If the baseline is already <5s** → the optimization is mostly done; proceed to Phase 6 (document + publish).

---

## Phase 2: Multicore investigation + enablement

**Goal:** determine whether stock `halo2_proofs 0.3` supports real multicore proving + enable it if so.

### Steps

1. **Inspect `halo2_proofs 0.3.2`**: does it expose a `multicore` or `parallel` feature flag?
   - Check `~/.cargo/registry/src/*/halo2_proofs-0.3.2/Cargo.toml` for `[features]`.
   - Check the `maybe-rayon` dependency: is `maybe-rayon/parallel` enabled (real rayon) or is it the serial stub?
   - Run `cargo tree -p postfiat-privacy-orchard -e features | grep -i 'maybe-rayon\|parallel\|multicore'` to see if `parallel` is active.

2. **If a `multicore`/`parallel` feature EXISTS + is NOT enabled:**
   - Enable it: `halo2_proofs = { version = "0.3", features = ["multicore"] }` in `crates/privacy_orchard/Cargo.toml`.
   - Rebuild (`cargo build --release -p postfiat-privacy-orchard`).
   - Re-run the Phase 1 benchmark. Record the new proving time + CPU utilization (did it parallelize across all 32 cores?).
   - Run the full privacy-orchard test suite + the swap soundness regression. ALL must pass.
   - If speedup >4×: commit the multicore flag + update the benchmark report with the measured delta.
   - If speedup <2× (parallelism didn't help — maybe the bottleneck is elsewhere): note it + proceed to Phase 3.

3. **If the feature does NOT exist on stock halo2 0.3** (the TIH reviewer's concern):
   - Document: "stock `halo2_proofs 0.3.2` does not expose a multicore feature flag. Parallelism requires the ecc-cash halo2 fork (Phase 3)."
   - Proceed directly to Phase 3.

### Contingency

- If enabling `multicore` breaks compilation (API incompatibility, feature conflict) → revert the flag, note the error, proceed to Phase 3.
- If it compiles but the benchmark shows no parallelism (still ~1 core) → the `maybe-rayon` serial stub is active despite the flag. Note it + proceed to Phase 3.
- If parallelism works but a test FAILS (the parallel prover produces a different/invalid proof) → that's a real bug. STOP + investigate. Do NOT ship a parallel prover that breaks soundness. Revert to serial + document.

---

## Phase 3: Ecc-cash halo2 fork migration

**Goal:** swap to the production-optimized halo2 fork (parallel Pippenger MSM, pasta-curve assembly, optimized FFT) for maximum CPU proving speed.

### Steps

1. **Identify the ecc-cash halo2 fork:**
   - Check: does `orchard 0.14.0` already depend on an ecc-cash halo2 fork? (`cargo tree -p postfiat-privacy-orchard | grep halo2`). If orchard pulls a specific halo2 source/version, that's the ecc-cash lineage.
   - Check: is `halo2_proofs 0.3.2` (our current dep) the same crate orchard uses, or a different source?
   - The ecc-cash fork is at `github.com/zcash/halo2` (the `ecc` org). The published `halo2_proofs` on crates.io IS the ecc-cash fork (maintained by ECC). So `halo2_proofs = "0.3"` may already BE the ecc-cash fork — but WITHOUT the `multicore` feature enabled.

2. **If the ecc-cash fork IS the crate we're using** (just without multicore):
   - The issue is the feature flag (Phase 2). If Phase 2 couldn't enable multicore, check whether a newer halo2_proofs version (0.3.x patch) exposes it, OR whether a `--cfg` flag or an env var activates it.
   - Try: `RUSTFLAGS="--cfg rayon"` or checking the `maybe-rayon` source for how `parallel` is gated.

3. **If a different halo2 source/version is needed:**
   - Update `crates/privacy_orchard/Cargo.toml`: `halo2_proofs = { git = "https://github.com/zcash/halo2", branch = "..." }` (or the appropriate version).
   - Handle API changes: function signatures, type renames, trait bounds. The circuit + verifier code may need adjustments.
   - Re-run ALL tests (privacy-orchard + node + types). ALL must pass.
   - Re-run the Phase 1 benchmark with the fork. Record the proving time + CPU utilization.

4. **Commit** the fork migration (if it works + improves speed) with the benchmark delta.

### Contingency

- If the fork breaks the circuit API (compilation errors) → make the minimum changes to restore compilation. If the API change is too large (many call sites) → evaluate whether the speedup justifies the migration effort. If not → pin the stock version + document.
- If a test fails with the fork → debug (the fork may have different default behavior, different lookup-table handling, different proof serialization). The proof + VK must still verify against the pinned constants.
- If the fork changes the VK (different circuit compilation) → re-capture the pinned fingerprint (via the release metadata test). Update the VK pin constants. Re-run the soundness regression against the new VK.
- **If the fork is fundamentally incompatible** (proofs don't verify, circuit semantics change) → REVERT. Pin the stock version. Document the incompatibility + the gap. Proceed to Phase 4 (circuit-level optimizations on the stock prover) or Phase 5 (GPU scoping).

---

## Phase 4: Circuit-level optimizations

**Goal:** reduce the circuit size / improve proving efficiency (works on ANY prover backend).

### Steps

1. **Measure constraint utilization:**
   - How many of the K=16 (65,536) rows are actually used? The MockProver or the circuit layout reports the active row count.
   - If utilization <50% (i.e., <32K rows used): try **K=15** (32,768 rows).
   - Change `ASSET_ORCHARD_SWAP_V1_K` from 16 to 15.
   - Re-keygen the VK (the smaller K changes the VK). Capture the new pinned fingerprint via the release metadata test. Update ALL VK pin constants.
   - Re-run ALL tests + the soundness regression + the Phase 1 benchmark.
   - If K=15 works (tests pass + proving is faster): commit.
   - If K=15 fails (not enough rows for the constraints): revert to K=16.

2. **Sinsemilla gadget inspection:**
   - Read `asset_orchard_sinsemilla.rs` for redundant gates (unnecessary range checks, suboptimal piece layout, redundant copy constraints).
   - If an optimization is found: implement it, test (the circuit==host match test MUST still pass), benchmark.
   - Commit only if it improves proving time WITHOUT breaking any test.

3. **Lookup table tuning:**
   - Inspect the range-check lookup table configuration (`LookupRangeCheck`, table width).
   - Wider tables = fewer lookup operations = faster. Test a wider table if the configuration allows.
   - Benchmark after each change.

4. **Poseidon round optimization:**
   - Inspect the Poseidon parameterization (full rounds, partial rounds). If the round count can be reduced without weakening the hash → fewer constraints. (CAUTION: this affects soundness — only reduce rounds if the security level is maintained.)

### Contingency

- If reducing K causes a constraint failure → revert immediately. The circuit MUST have enough rows.
- If a Sinsemilla/lookup/Poseidon optimization breaks the circuit==host match test → revert. The gadget MUST produce the same commitment as the host.
- If ANY optimization breaks a soundness test → revert. Soundness > speed.
- After EACH circuit change: re-capture the VK pin constants (the circuit change alters the VK). Update ALL pin constants. Re-run the soundness regression.

---

## Phase 5: GPU integration scoping (ICICLE-Halo2)

**Goal:** SCOPE the GPU proving path (do NOT implement — no GPU on this box). Produce a design doc for the Akash/io.net deployment.

### Steps

1. **Research ICICLE-Halo2:**
   - The Ingonyama ICICLE integration (`github.com/ingonyama-zk/icicle` + the halo2 wrapper).
   - What API does it expose? Which halo2 operations (MSM, FFT) are replaced by CUDA calls?
   - What CUDA version + GPU VRAM is required?
   - What halo2 version does it target? (Compatibility with our halo2_proofs 0.3.2.)
   - Document the findings.

2. **Scope the code changes:**
   - Which calls in `asset_orchard_circuit.rs` / `verify.rs` would be replaced by ICICLE calls?
   - How much of the circuit code stays the same (should be ALL — ICICLE is a backend swap)?
   - What new dependencies (CUDA bindings, ICICLE crate)?
   - Estimate the integration effort (LOC, complexity).

3. **Scope the Akash SDL:**
   - What would the prover container look like? (Docker image: CUDA + ICICLE + the postfiatl1v2 circuit code + the prover binary.)
   - What GPU spec? (RTX 3090/4090, 24GB VRAM — based on ICICLE's benchmarks.)
   - A minimal SDL template (deploy.yaml) for Akash.
   - How would StakeHub deploy it (the akash CLI commands)?

4. **Scope the StakeHub orchestration:**
   - How would StakeHub: lease the GPU (akash CLI) → deploy the container → feed the witness → collect the proof → submit the swap to PFTL → close the lease?
   - What new StakeHub code is needed (a "proving service" module)?
   - How would the witness be securely transmitted to the GPU instance (ephemeral, TEE-attested, or encrypted)?

5. **Write the scope** as `docs/status/icicle-gpu-prover-scope.md`. Include: the ICICLE compatibility assessment, the code-change estimate, the Akash SDL template, the StakeHub orchestration plan, + the expected GPU proving time (projected from ICICLE's published benchmarks).

### Contingency

- If ICICLE doesn't support our halo2 version → note the version gap + the migration path (which version to upgrade to).
- If the ICICLE API is too different from our proving path → note the adaptation effort.
- If the Akash SDL is complex → provide a minimal template + note the simplification options.
- This phase is SCOPING ONLY — no code changes, no circuit changes, no test changes. Just the design doc.

---

## Phase 6: Final benchmark report + blog update

**Goal:** produce the measured-evidence report + update the blog with real numbers.

### Steps

1. **Compile all benchmark results** into `docs/status/zk-prover-optimization-results.md`:
   - A table: baseline (stock serial) → multicore (if enabled) → fork (if migrated) → circuit-level (if optimized).
   - Each row: measured prove time (ms), verify time (ms), proof size (bytes), K, CPU utilization, speedup ratio vs baseline.
   - ALL numbers MEASURED on this hardware (32-core box). No projections in the results table.

2. **Update the blog post** (`heavy-zk-optimization-v2.md`):
   - Replace the projected speedup ranges with the REAL measured numbers from the results table.
   - The "Scope and evidence level" section can now say "measured on this circuit, on this hardware" (for the achieved tiers).
   - The GPU tier stays as "projected from ICICLE benchmarks" (not measured — no GPU).
   - Commit the updated blog.

3. **Commit** the results report + the updated blog.

### Contingency

- If no optimization improved the proving time (all phases failed or didn't help) → the report documents that honestly: "stock halo2 on 32 cores: X seconds. Multicore not available on stock 0.3. Fork migration incompatible. Circuit-level optimizations did not improve. GPU scoping: see design doc." That's still a valuable report (it answers the question empirically).

---

## Stop conditions (when to STOP the sprint)

1. **Soundness break at any phase** → STOP. Revert to the last green state. Soundness > speed. Document the break + do not proceed to further optimizations.
2. **Proving time already <5s** after Phase 2 or 3 → evaluate whether Phase 4 (circuit-level) is worth the risk. May skip to Phase 6 (document + publish).
3. **Phase 3 (fork migration) fundamentally incompatible** → STOP the fork path. Fall back to stock + Phase 4 (circuit-level on stock). Document the incompatibility.
4. **All phases exhausted** (nothing improved) → STOP. Write the honest report (Phase 6). The GPU path (Phase 5 scope) is the forward path.
5. **Any panic / crash during proving** → STOP. Investigate. A prover crash is a bug (consensus must never crash).

## Hygiene (non-negotiable)

- **Commit each unit** with a clear message + the benchmark delta.
- **Run the full test suite** after each optimization: `cargo test -p postfiat-privacy-orchard` + `cargo test -p postfiat-node shielded_swap` + `cargo test -p postfiat-privacy-orchard swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation --release -- --ignored`. ALL must pass.
- **Never mask a failing test** (`--skip`, commented-out, etc.) — fix root cause.
- **No sshpass, no keys in tree, no paper-over.**
- **VK pin constants MUST be updated** after ANY circuit change (K reduction, Sinsemilla/Poseidon optimization) — re-capture via the release metadata test (`swap_full_shape_key_metadata_is_pinned_and_consistent --release -- --ignored`).
- **Release-mode for all benchmarks** (debug is too slow + not representative of production).

## Success criteria (the sprint's deliverables)

1. ✅ **Measured baseline** proving time (Phase 1) — the evidence the TIH demanded.
2. ✅ **Multicore determination** (Phase 2) — is it a flag or a fork? Answered empirically.
3. ✅ **Fastest achievable CPU proving time** — after whatever optimizations worked (Phases 2/3/4).
4. ✅ **GPU path scoped** (Phase 5) — the ICICLE + Akash/io.net design doc.
5. ✅ **Results report** with all measured numbers (Phase 6).
6. ✅ **Blog updated** with real numbers replacing projections (Phase 6).
7. ✅ **No soundness test broken** at any phase.

The sprint is successful if the proving time is reduced AND the evidence is measured. Even if the proving time isn't reduced (e.g., multicore isn't available + the fork is incompatible), the MEASURED evidence + the GPU scope are still valuable deliverables — they answer the question "how fast can this get + what's the path?"

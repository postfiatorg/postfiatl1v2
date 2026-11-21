================================================================================
PEEL-BINIUS-SHIELDED: POST-QUANTUM GLOBAL-POOL CONFIDENTIAL PAYMENTS WITH
ZK-SHIELDED MINT/UNSHIELD, 0-ROUND ML-KEM-1024 STEALTH RECEIVE, 512-BIT-STATE
SINGLE-ORACLE QROM EXTRACTION, AND SUB-50ms DUAL-LANGUAGE VERIFICATION
================================================================================

Target System:  Post Fiat L1 (W-1 experimental budget: <= 149,130 B/tx archival
                ceiling [12 GiB/day at 1 tx/s], < 50 ms single-core validator
                full settlement verification, <= 1,048,576 B JSON envelope).
Artifact Date:  2026-11-21
Public Repo:    https://github.com/postfiatorg/postfiatl1v2
Branch:         feat/peel-binius-shielded-pq
Pinned Code:    https://github.com/postfiatorg/postfiatl1v2/tree/d76e5a8c61ec9741ffd6da59b00b7aee8ca0bb73/tools/pft-binius-shielded-pq
Code Commit:    d76e5a8c61ec9741ffd6da59b00b7aee8ca0bb73
Public CI Run:  https://github.com/postfiatorg/postfiatl1v2/actions/runs/44943485068 (7/7 PASS on GitHub ubuntu-latest, AMD EPYC 7763)
Spec Path:      docs/security/peel-binius-shielded-pq-payments.md
Container:      tools/pft-binius-shielded-pq/Dockerfile (pinned Rust 1.95.0, Go 1.25.1, z3-solver)

--------------------------------------------------------------------------------
ABSTRACT
--------------------------------------------------------------------------------
We specify, formally analyze, implement in Rust and an independent zero-
external-dependency Go 1.25.1 verifier inside postfiatl1v2
(tools/pft-binius-shielded-pq/, pinned at commit d76e5a8c6), and mechanically
audit Peel-Binius-Shielded (wire format PFSH02), a post-quantum confidential
payment protocol engineered to satisfy Post Fiat L1's 149,130 B/tx archival
ceiling and < 50 ms single-core validator settlement budget without fixed small
rings, interactive recipient rounds, transparent deposit-opening trapdoors, or
on-chain signatures on shielded-to-shielded transfers.

Core architectural, cryptographic, and empirical contributions:
  1. Unified ZK-Shielded Lifecycle Over a Depth-24 Global Accumulator
     (2^24 = 16,777,216 Coin Leaves):
     An append-only Depth-24 NIST FIPS-202 SHAKE256 Merkle tree is arithmetized
     inside the ZKP (56 Keccak-f[1600] permutations, 9,216 packed 2-bit Chi-mux
     swap rows, 6,144 64-row-aligned sibling rows, 448 native is_dummy gating
     rows, 258 66-bit exact ripple-carry conservation rows = 447,014 active
     rows). Every shielded lifecycle transition—ZK-Shielded Mint (v_mint > 0
     with is_dummy0 = is_dummy1 = 1, hiding recipient A and blinding R even at
     deposit, paired with outer-L1 depositor authorization over tx_ctx),
     2-in/2-out transfer, native 1-in+1-dummy transfer, and partial transparent
     Unshield exit (v_unshield > 0 to unshield_dest)—is enforced by one uniform
     66-bit integer conservation equation:
       v_in0 + v_in1 + v_mint == v_out0 + v_out1 + (fee + v_unshield)  in Z.
     At two output leaves per transaction, one Depth-24 tree holds 8,388,608
     transactions (~97.1 days at sustained 1 tx/s) prior to tree rollover.
  2. 0-Round Non-Interactive Stealth Receive & Context-Bound Non-Malleability:
     Each output seals (v_out || R_out) [86 B] under the recipient's static
     off-chain FIPS-203 ML-KEM-1024 encapsulation key + SHA-384 KDF +
     AES-256-GCM (AAD = coin_out) into a 1,670 B envelope while keeping A_out
     private inside the ZKP. Eliminating on-chain signatures on shielded spends
     yields a 3,648 B non-proof header; all header fields (including v_mint,
     v_unshield, unshield_dest, and env0/1) are committed via SHA-384 tx_ctx
     into Round 1 of the ZKP, preventing third-party relayer malleability.
  3. 512-Bit-State Single Fixed-Range Compiler Oracle & Disjoint Hash Families:
     Every compiler call (MT_1 48 B digests, 64 B internal transcript states,
     48 B E = GF(2^384) challenges, and Round-34 20-bit PoW + 52 x 21-bit query
     indices) is a fixed-prefix projection of ONE single fixed-range random
     oracle H: {0,1}^* -> {0,1}^{1536} instantiated via 3 x SHA-512 (NIST FIPS
     180-4 SHA-2), disjoint from in-circuit FIPS-202 SHAKE256. Widening only
     the never-transmitted internal Fiat-Shamir state from 384 to 512 bits adds
     zero wire bytes and zero verify latency while raising all 68 programmed
     queries to >= 512-bit conditional min-entropy.
     Conditional security targets (explicitly conditional on the unreviewed
     compressed-oracle bridge Theorem A.3 and conjecture-level SE wrapper
     Appendix A.4; published-only generic QROM-FS [DFMS20] gives a vacuous
     (Q+1)^68 multi-round bound while [CMS19] proves asymptotic QROM soundness
     for RBR-sound BCS-compiled IOPs without concrete 34-round or PoW
     constants): conditional extraction/SE advantage targets scale with total
     quantum queries Q as 2^-92.25 (ext/SE) at Q = 2^96 (matching 384-bit MT_1
     and SHAKE256 BHT sizing), 2^-137.55 (ext/SE) / 2^-150.60 (ZK) at Q = 2^80,
     and 2^-170.98 (ext) / 2^-170.97 (SE) / 2^-150.60 (ZK) at the reference
     sequential-query benchmark Q = 2^64 (P = 2^32, L = 2^16). End-to-end
     Auth and Privacy bounds additionally include explicit unquantified
     standing assumptions (SHAKE256 JointPRF/Hide, SHA-384 KDF, ML-KEM IK-CCA).
  4. Algebraic & Systems Optimizations:
     - Exact 3D-Karatsuba Tower-Reduced Ring-Switch Verifier (13,122 vs. 143,360
       E::muls, 11x fewer), mechanically differential-tested against the naive
       384 x 384 bilinear sum in both Rust and Go.
     - 64-Row-Aligned Factored Single-alpha Circuit MLE Verifier + 1-mul
       eqTable folding + 2x-unrolled MOVOU/PCLMULQDQ Go assembly kernel.
     - Streaming L1-Cache-Resident 1,024-Column Coset ANTT Prover (561.7 MiB
       peak prover RSS vs. 1.80 GiB naive).
  5. Measured Budgets Across Two Independent Hosts (Primary EPYC 16C + Public
     GitHub Actions CI ubuntu-latest AMD EPYC 7763 4 vCPU, pinned 1-core):
     - Wire size: 138,508 B median (137,500-139,180 B observed), 140,476 B
       deterministic worst-case (+8,654 B headroom under 149,130 B/tx).
     - Primary host — Rust 1C warm full settle (50t): 22.53 ms med / 25.15 ms
       p95 / 31.95 ms max (4.50 MiB RSS); cold-process wall (10 procs):
       29.98 ms med / 44.98 ms max.
     - Primary host — Go 1.25.1 1C warm full settle (50t, zero third-party
       deps, PCLMULQDQ): 36.32 ms med / 38.78 ms p95 / 40.40 ms max (+9.60 ms
       headroom, 5.25 MiB RSS); cold-process wall (10 procs): 44.79 ms med /
       47.06 ms max (-tags purego warm fallback: 71.50 ms med, exceeding 50 ms).
     - Second host (Public GitHub Actions Run 44943485068, clean runner):
       19/19 SHA256SUMS OK, 7/7 verify.sh PASS, Rust 1C cold wall 30.02 ms med
       / 30.70 ms max, Go 1C warm (50t) 40.37 ms med / 43.68 ms max, Go 1C cold
       wall (10 procs) 47.76 ms med / 48.34 ms max (purego 90.58 ms med).
     - Rust streaming prove: 2.89 s med (16C/32T, 50t), 26.73 s med (1C, 5t),
       561.7 MiB peak RSS. Go stdlib 1-core ML-KEM-1024 wallet scan: 211.77 us
       per non-matching output (4,722 outputs/s; ~36.6 s/core for a full day at
       1 tx/s).

================================================================================
SECTION 0. CLAIM SEGREGATION & PROOF-OBLIGATION AUDIT
================================================================================

Table 0 — Strict Separation of Claim Categories
--------------------------------------------------------------------------------
Category A — Empirically Measured & Publicly Replayable (7/7 verify.sh PASS):
  * Public repository commit d76e5a8c61ec9741ffd6da59b00b7aee8ca0bb73 in
    https://github.com/postfiatorg/postfiatl1v2 (branch
    feat/peel-binius-shielded-pq, path tools/pft-binius-shielded-pq/) includes
    all Rust/Go/Python source, pinned Cargo.lock, zero-dep go.mod, Dockerfile,
    SHA256SUMS, and one-command ./verify.sh replaying 4 lifecycle scenarios:
    (1) tx_mint.bin (138,508 B): ZK-Shielded Deposit (v_mint=1000, is_dummy0=
        is_dummy1=1 -> Alice 700 + Alice 290, fee 10; A and R never on-chain),
    (2) tx_hop1.bin (138,508 B): 2-real-input shielded transfer (Alice -> Bob
        650 + Alice 330, fee 10),
    (3) tx_dummy_1in.bin (137,788 B): native 1-real + 1-dummy spend,
    (4) tx_hop2.bin (138,268 B): multi-hop spend across 25 intervening deposits
        with partial transparent unshield (v_unshield=100 to unshield_dest),
    all verified in both Rust and Go 1.25.1.
  * Go 1.25.1 stdlib crypto/mlkem + crypto/aes + crypto/sha512 + crypto/sha3
    offline stealth scan recovering (v=650, R_78B) from Rust fips203 ciphertext
    in tx_hop1.bin and cleanly rejecting output 1.
  * 50-trial Rust 16C/1C & Go 1C warm benchmarks, 10-process cold-process 1C
    benchmarks (Rust & Go), 1-core ML-KEM-1024 wallet scan throughput,
    /usr/bin/time -v peak RSS, 35/35 Rust + 35/35 Go named adversarial controls
    (including Rule R4 zero-coin 0^48, Rule R6 2^24 capacity overflow, Rule R5
    mint/unshield mutual exclusion & overflow, unshield_dest malleability, live
    is_dummy=1/v_in=500 inflation ZKP attack, and forged-Merkle-path ZKP
    attack), and 150/150 structured byte-fuzz rejections.

Category B — Mechanically Verified Symbolic & Cross-Language Invariants:
  * check_math.py (exact Python Fraction arithmetic):
    - Binary tower GF(2) subset K_16 subset K_32 subset E = GF(2^384) =
      K_32[Y]/(Y^12 + Y^3 + Y + 0x00010002) Rabin irreducibility verified
      across all proper divisors d in {1, 2, 3, 4, 6}.
    - CFS17 Sumcheck-1 G_1 Jacobian (V_SC1 = 19): full rank 58/58 over K_32.
    - Sumcheck-2 Jacobian (V_SC2 = 8, 8 disjoint hypercube edges): full rank
      17/17 over K_32, determinant total degree <= 112.
    - Exact-rational RBR, BCIKS20 Thm 6.2, 512-bit-state DFMS22a/b QROM
      Ext/SE/ZK, Q_tot multi-tx degradation, and deterministic wire bounds.
  * verify_circuit_equivalence.py (Z3 5.4.1 + 3-way IR dump):
    - Z3 UNSAT: 66-bit exact ripple-carry conservation over Z across all 64-bit
      inputs (v_in0 + v_in1 + v_mint == v_out0 + v_out1 + pub_exit).
    - Z3 UNSAT: 2-bit packed Chi-monomial Merkle conditional-swap mux.
    - Z3 UNSAT: Native is_dummy gating (is_dummy=1 => v_in=0; is_dummy=0 =>
      root==anchor).
    - Z3 UNSAT: Exact 5-bit Keccak Chi-Iota row bijection.
    - Exhaustive 1,600-basis GF(2) verification of L_TRP = pi o rho o theta.
    - Complete 56-perm Keccak DAG bit-coverage check: all 56 x 24 x 1,600 =
      2,150,400 state bits uniquely indexed and constrained in Groups A & B.
    - 3-way bit-for-bit Group-C IR equality (42,444 constraints on tx_hop2.bin):
      Rust == Go == independent Python builder.
    - Cross-language Group A+B+C verifier MLE weights equality: Rust == Go.
  * Arithmetic & algebraic kernel differential tests:
    - Go: 10,000 random E384::mul (PCLMULQDQ == portable) + 1,000 random
      RingSwitchLinearMap (SSE2 ASM == portable) + exact 3D-Karatsuba
      evalBilinearRSIP == naive 384 x 384 bilinear sum.
    - Rust: 1,000 random E384::mul (PCLMULQDQ == portable) + streaming 1,024-pt
      ANTT == Novel Basis + exact 3D-Karatsuba eval_bilinear_rs_ip == naive
      384 x 384 bilinear sum.

Category C — Mathematical Security Proofs & Conditional Dependency Audit:
  Table 1 separates every theorem into (i) published anchor theorems (with
  exact venues and scope boundaries) and (ii) self-contained new lemmas/bridges
  proven in Appendices A-B that remain conditional proof obligations pending
  external peer review.

Table 1 — Theorem-by-Theorem Proof-Obligation & Review-Status Audit
--------------------------------------------------------------------------------
Component                | Published Anchor          | Conditional New Bridge
                         | (Exact Venue & Scope)     | (Unreviewed — App. A/B)
-------------------------+---------------------------+--------------------------
RS Parameterized Curve   | BCIKS20 (FOCS 2020 / JACM | Lemma 1: Peel(M) guard
Proximity (single call)  | 2023, ePrint 2020/654)    | |L_i|>2D, exact 4-line
                         | Thm 6.2 verbatim: m'>=3,  | telescoping for ALL n'>=0
                         | delta'<=1-(1+1/2m')*      | (0<=n'<=2D => Pr=0; n'>2D
                         | sqrt(rho'), any L_rem     | => single BCIKS20 call)
-------------------------+---------------------------+--------------------------
QROM BCS Straight-Line   | Published-only fallback:  | Standalone Theorem A.3
Extraction (Thm 2A)      | CMS19 (TCC 2019) proves   | (first-principles operator
                         | asymptotic QROM soundness | proof, unreviewed): exact
                         | for RBR-sound BCS IOPs    | rank-3 self-adjoint bound
                         | (no concrete 34-round/PoW | sqrt(8p)<sqrt(10p) (A.3.0),
                         | constant); DFMS20 generic | Tree-Invariance (A.3.1),
                         | FS gives vacuous (Q+1)^68 | block-diagonalization
                         | loss. Tools: Zha19,       | (A.3.2), both-direction
                         | CFHL21 (EUROCRYPT 2021),  | capacity (A.3.3), exact
                         | DFMS22a/b (EC/CRYPTO 22). | readout 2/|Y| (A.3.4).
-------------------------+---------------------------+--------------------------
Multi-Proof QROM ZK      | GHHM21 (ASIACRYPT 2021)   | Lemmas 1.1-1.2 & Sim_IOP:
(Thm 1B, P = 2^32)       | Thm 1 & AHU19 (CRYPTO 19) | 5,216b marginal O2H +
                         | Thm 3 (Semi-Classical O2H)| leaf-coll check, sequential
                         |                           | r=1..68 product-state reprog
-------------------------+---------------------------+--------------------------
Multi-Proof QROM SE      | NONE standalone published | Conjecture-level bridge
(Thm 2B, P=2^32, L=2^16) | (uses Zha19/CFHL21 +      | (App. A.4): unmodified-H
                         | GHHM21 Thm 1 as tools)    | wrapper B^H, (x*,pi*)
                         |                           | freshness, T_prog, A/B/C
-------------------------+---------------------------+--------------------------
0-Round Stealth Receive  | FIPS-203 ML-KEM-1024      | Single-user-reduced
(IND-CCA2, INT-CTXT,     | IND-CCA2; NIST SP 800-38D | Exp_GlobalPriv w/ active
 IK-CCA, SHA-384 KDF)    | AES-256-GCM INT-CTXT /    | O_Scan & O_Corrupt (2U /
                         | IND$-CPA; Kyber ANO-CCA:  | U(U-1) factor); standing
                         | GMP22 (EC22), MX23 (PKC23)| [NonStd] FIPS-203 IK-CCA &
                         |                           | SHA-384 key-in-middle KDF
-------------------------+---------------------------+--------------------------
In-Circuit Commitments   | Standard-model FIPS-202   | Adaptive multi-user games
& Depth-24 Merkle Tree   | SHAKE256 (c=512b sponge;  | Adv_JointPRF (allowing
                         | indiff. BDPV08 / QROM     | O_SN on other notes of
                         | Czajkowski+); classical   | same S), Adv_Hide, multi-
                         | CDMP05 prefix-free MD     | target Adv_MT-TPR (S4)
-------------------------+---------------------------+--------------------------

Category D — Explicit Non-Claims & Deployment Boundaries:
  1. Honest-Sender Envelope Non-Binding: The ZKP binds env0 and env1 into
     tx_ctx (preventing third-party relayer tampering), but does NOT prove
     in-circuit that env_c encrypts the valid opening of coin_c. A malicious
     sender can burn funds by submitting a valid coin with an undecryptable
     envelope (the same non-verifiable-encryption model as Zcash Orchard).
  2. Transparent-Mint Consensus Layer Boundary: Rules R1-R6 and the Rust/Go
     binaries enforce shielded-pool conservation against public v_mint and
     v_unshield. For v_mint > 0, authenticating and debiting the transparent
     L1 source account (Section 1.3 Rule R0-Auth: outer-L1 EUF-CMA signature
     over ("PFT-SH-MINT-AUTH-V2" || mint_src || tx_ctx)) is an L1 consensus
     layer obligation outside the standalone shielded-pool verifier binary.
  3. Functionality Scope vs. Production Shielded Protocols: This prototype does
     not implement incoming/full viewing keys, diversified addresses, variable-
     arity inputs/outputs, encrypted user memos, or fuzzy detection tags.
  4. Metadata, Timing, Fee, & Anchor-Choice Leakage: P2P IP metadata, mempool
     timing, public fee/v_mint/v_unshield values, recipient scanning timing
     side channels, and stale-anchor selection (narrowing effective anonymity
     set) are outside Exp_GlobalPriv. Exp_GlobalPriv also excludes subsequent
     spends of challenge output coins (single-challenge transaction privacy,
     not full multi-hop transaction-graph unlinkability). Production wallets
     must default to the latest finalized canonical anchor. Chain-reorg
     finality and mempool verification-DoS fee markets are L1 consensus
     concerns outside this proof-system layer.
  5. Tree Capacity & Rollover Implementation Status: One Depth-24 tree holds
     2^24 = 16,777,216 leaves (8,388,608 two-output transactions, ~97.1 days at
     sustained 1 tx/s). Rule R6 rejects appends beyond 2^24 leaves; Section 1.4
     specifies the Dual-Anchor Rolling Tree Rollover design (and explicitly
     notes the single-anchor cross-tree spend constraint), but only single-tree
     operation is implemented in the prototype binaries.
  6. Hardware Portability & Empirical Scope: Primary benchmarks are from one
     AMD EPYC 16C/32T x86_64 host (warm 50-trial + cold-process 10-trial),
     supplemented by independent public GitHub Actions CI replay on Ubuntu
     (Section 5.5). Pure-Go (-tags purego: 71.50 ms med) exceeds 50 ms,
     unpinned cold runs inside verify.sh immediately after cargo build showed
     scheduling variance up to 47.9 ms Rust / 62.0 ms Go, and ARM64 validators
     would require a PMULL assembly kernel.
  7. Provenance & Independent Review Status: Prover K_32/E_384 field ops use
     branchless zero-lookup PCLMULQDQ, but full constant-time auditing of OS
     and third-party libraries is outside scope. Both Rust and Go verifiers and
     the Python IR builder were authored from the same specification by the
     same team (so cross-language IR equality is a regression/implementation
     check, not third-party spec validation), and the new proofs in Appendices
     A-B have not yet undergone independent external peer review.

================================================================================
SECTION 1. PROTOCOL SPECIFICATION & FORMAL LEDGER SEMANTICS
================================================================================

1.1 Disjoint NIST Cryptographic Primitives (Zero Shared Permutation)
--------------------------------------------------------------------------------
Peel-Binius-Shielded separates the compiler QRO from in-circuit hashing across
distinct NIST hash families (SHA-2 Merkle-Damgard vs. Keccak-f[1600] sponge):

  (a) Compiler QRO (ONE Single Fixed-Range Oracle H: {0,1}^* -> {0,1}^{1536}):
      Instantiated with NIST FIPS 180-4 SHA-512 (64 B = 512-bit block):
        H(m) := SHA-512(0x00 || m) || SHA-512(0x01 || m) || SHA-512(0x02 || m).
      Every compiler call uses a unique 8-byte ASCII domain prefix inside m and
      has a strictly fixed-length (hence prefix-free) message payload m (plus
      the 1-byte counter prefix 0x00/0x01/0x02):
        - "PFS-LEAF" (m = 760 B: 8 B tag + 4 B col_idx + 652 B k_vals + 96 B
                      b_pcs/b_prox; 761 B with counter) -> H(m)[0..48]  (384 b)
        - "PFS-NODE" (m = 109 B: 8 B tag + 1 B lvl + 4 B pos + 96 B children;
                      110 B with counter)               -> H(m)[0..48]  (384 b)
        - "PFS-STMT" (m = 464 B; 465 B with counter)    -> H(m)[0..64]  (512 b)
        - "PFS-ABS " (fixed length per tag)             -> H(m)[0..64]  (512 b)
        - "PFS-CH  " (m = 76 B; 77 B with counter)      -> H(m)[0..48]  (E elem)
                                                        || H(m)[64..128](512 b state)
        - "PFS-C4  " (m = 120 B; 121 B with counter)    -> H(m)[0..192] (20 PoW
                                                           + 52x21 query bits)
      Prefix-free Merkle-Damgard note: Because every compiler domain tag has a
      fixed input length, the domain of SHA-512 is strictly prefix-free, where
      plain Merkle-Damgard is classically indifferentiable from a random oracle
      up to O(q^2/2^512) (Coron-Dodis-Malinaud-Puniya, CRYPTO 2005, Thm 1),
      ruling out classical length-extension attacks on untruncated 512-bit
      state outputs. Extending that classical indifferentiability heuristic to
      a quantum 1,536-bit QRO remains an explicit QROM modeling idealization.

  (b) In-Circuit Note Hashes & Depth-24 Accumulator (Standard-Model SHAKE256):
      NIST FIPS 202 SHAKE256 (Keccak-f[1600], rate r = 136 B, capacity c = 64 B
      = 512 bits), strictly 1 permutation per call (all inputs <= 135 B):
        A          = H_A(S)            := SHAKE256(0x00 || S[48])[0..48]                 [49 B]
        Coin       = H_Coin(A, v, R)   := SHAKE256(0x01 || A[48] || v[8] || R[78])[0..48][135 B]
        SN         = H_SN(S, Coin)     := SHAKE256(0x02 || S[48] || Coin[48])[0..48]     [97 B]
        MerkleNode = H_Node(L, R)      := SHAKE256(0x03 || L[48] || R[48])[0..48]        [97 B]
      Generic sponge capacity & query-scaling note: SHAKE256 has 512-bit
      capacity c, so generic sponge indifferentiability holds up to O(Q^2/2^512)
      classically (crossover Q = 2^256) and O(Q^3/2^512) in the QROM
      (crossover Q = 2^170.67; at fixed Q = 2^64, 32*Q^3/2^512 = 2^-315). For
      384-bit outputs, generic quantum collision (BHT) scales as O(Q^3/2^384)
      (crossover Q = 2^128; at Q = 2^64: 2^-187; at Q = 2^96: 2^-91), and
      generic single-target quantum preimage (Grover) scales as O(Q^2/2^384)
      (crossover Q = 2^192; at Q = 2^64: 2^-256). Note that these are black-box
      ideal-sponge/random-function sizing references (not standard-model proofs
      for fixed unkeyed SHAKE256); Theorems 3-5 retain explicit standard-model
      advantage terms for every SHAKE256 assumption.

  (c) 0-Round Stealth Receive (FIPS-203 ML-KEM-1024 + SHA-384 + AES-256-GCM):
      A recipient holds spend secret S <-$ {0,1}^{384} and ML-KEM-1024 keypair
      (ek, dk) <- ML-KEM-1024.KeyGen(d, z) (ek = 1,568 B, dk = 3,168 B). Static
      off-chain stealth address is pk_recv = (ek, A = H_A(S)); neither ek nor A
      is published on-chain (including during ZK-Shielded Mint). To pay v_out
      in [0, 2^64-1] to pk_recv:
        1. Sample R_out <-$ {0,1}^{624}; compute Coin_out = H_Coin(A, v_out, R_out).
        2. Run (ss, ct_kem) <- ML-KEM-1024.Encaps(ek) [ct_kem = 1,568 B, ss = 32 B].
        3. Derive:
             K_aead = SHA-384("PFT-SH-KEM-KEY-V1"   || ss || ct_kem || Coin_out)[0..32],
             N_aead = SHA-384("PFT-SH-KEM-NONCE-V1" || ss || ct_kem || Coin_out)[0..12].
        4. Encrypt pt = v_out_le64 || R_out [86 B] via AES-256-GCM(K_aead,
           N_aead, aad = Coin_out, pt) -> ct_aead [102 B].
        5. Publish env = ct_kem || ct_aead [1,670 B].
      Offline scanning (try_scan_output): Decapsulate ss' <- Decaps(dk, ct_kem)
      (FIPS-203 implicit rejection), derive (K', N'), verify AES-256-GCM tag
      over aad = Coin_out, and verify H_Coin(A, v_out, R_out) == Coin_out.

1.2 Canonical Wire Format PFSH02 & Context Binding (3,648 B Header)
--------------------------------------------------------------------------------
A shielded payload tx is serialized canonically as header(tx) || proof_len ||
proof, where header(tx) (3,644 B) + proof_len (4 B) = 3,648 B:
  magic         (8 B:  "PFSH02\0\0")
  anchor        (48 B: historical Depth-24 Merkle root)
  v_mint        (8 B:  uint64 LE — public transparent-to-shielded deposit)
  v_unshield    (8 B:  uint64 LE — public shielded-to-transparent exit)
  unshield_dest (32 B: transparent L1 recipient account hash; 0^32 if unused)
  fee           (8 B:  uint64 LE — public transaction fee)
  sn0, sn1      (96 B: 2 x 48 B nullifiers)
  coin0, coin1  (96 B: 2 x 48 B output coin commitments)
  env0, env1    (3,340 B: 2 x 1,670 B ML-KEM-1024 + AES-256-GCM envelopes)
  proof_len     (4 B:  uint32 LE)
  proof         (proof_len B: Peel-Binius-Shielded ZKP)

Context binding hash (zero extra wire bytes):
  tx_ctx = SHA-384("PFT-SH-TXCTX-V2" || header(tx))  [48 B].
The ZKP public statement is x = (anchor, sn0, sn1, coin0, coin1, v_mint,
v_unshield, fee, tx_ctx), absorbed into Round 1 (PFS-STMT). Any mutation to
header(tx) changes tx_ctx (up to SHA-384 CR) and invalidates the ZKP.

1.3 Canonical Ledger State, Outer-L1 Mint Auth, & Acceptance Rules (R0-R6)
--------------------------------------------------------------------------------
Public ledger state is S_t = (Bal_L1[], Coins[], Anchors, SpentSN,
Supply_minted, Unshielded_acc, Fees_acc), where Bal_L1[] maps transparent L1
accounts to balances, Coins[] is the append-only sequence of 48-byte leaf
commitments in a Depth-24 SHAKE256 binary Merkle tree (empty sentinel leaf
EMPTY[0] = 0^48, EMPTY[h+1] = H_Node(EMPTY[h], EMPTY[h])), Anchors is the set
of valid historical Depth-24 roots, SpentSN is the set of spent nullifiers, and
Supply_minted, Unshielded_acc, Fees_acc in Z_{>=0} track cumulative shielded
inflows, transparent exits, and burned fees (if consensus instead credits fees
to validator accounts in Bal_L1[], Fees_acc is folded into Bal_L1[] as noted in
Theorem 3b).

  R0 (Genesis & Two-Layer Transparent-to-Shielded Deposit Boundary):
     At genesis S_0: Coins = [], Anchors = {EMPTY[24]}, SpentSN = empty,
     Supply_minted = Unshielded_acc = Fees_acc = 0, and sum_a Bal_L1[a] =
     TotalSupply_0. ALL value enters the shielded pool via a PFSH02 transaction
     with v_mint > 0 (and is_dummy0 = is_dummy1 = 1 for a pure deposit), so
     recipient address A and blinding R are never published on-chain.
     Rule R0-Auth (Outer-L1 Consensus Binding for v_mint > 0):
       - When v_mint == 0: no L1 account signature is required; tx is authorized
         solely by the ZKP (fee paid from shielded inputs).
       - When v_mint > 0: the outer L1 transaction envelope supplies transparent
         depositor account mint_src and an L1 post-quantum EUF-CMA signature
         sig_mint (e.g., FIPS-204 ML-DSA-65 on Post Fiat L1) verified over
         ("PFT-SH-MINT-AUTH-V2" || mint_src || tx_ctx), requiring
         Bal_L1[mint_src] >= v_mint and atomically debiting
         Bal_L1[mint_src] -= v_mint. (Because tx_ctx commits to all of
         header(tx), sig_mint binds the depositor to exact v_mint, coin0/1, and
         env0/1. R0-Auth is specified for L1 consensus integration and modeled
         explicitly in Theorems 3b and 4 Mode 3; standalone shielded-pool
         binaries verify Rules R1-R6 — Category D.2.)

  A PFSH02 payload tx is accepted on state S_t iff ALL six rules R1-R6 hold
  (enforced identically in Rust payment.rs and Go main.go):
  R1 (Canonical Parse & Anchor Validity): Strict length check, magic ==
     "PFSH02\0\0", zero trailing bytes, and tx.anchor in S_t.Anchors.
  R2 (Intra-Tx Nullifier Distinctness): tx.sn0 != tx.sn1.
  R3 (Global Nullifier Freshness): tx.sn0 not in S_t.SpentSN and tx.sn1 not in
     S_t.SpentSN.
  R4 (Output Coin Uniqueness & Sentinel Exclusion): tx.coin0 != tx.coin1,
     tx.coin0 != 0^48, tx.coin1 != 0^48, and neither tx.coin0 nor tx.coin1
     appears in S_t.Coins.
  R5 (Public Flow Validity & ZKP Verification): Require !(tx.v_mint > 0 &&
     tx.v_unshield > 0), require tx.fee + tx.v_unshield < 2^64 (u64 non-
     overflow), recompute tx_ctx from header(tx), and verify
     Verify_Binius(x, tx.proof) == 1 (plus R0-Auth at L1 consensus if
     tx.v_mint > 0).
  R6 (Capacity Guard & Atomic State Transition): Require |S_t.Coins| + 2 <= 2^24.
     Insert {sn0, sn1} into SpentSN; append coin0 and coin1 to Coins[]; compute
     new Depth-24 root root_{t+1} and insert into Anchors; update
     Supply_minted += tx.v_mint, Unshielded_acc += tx.v_unshield,
     Fees_acc += tx.fee (and at L1 consensus: Bal_L1[unshield_dest] +=
     tx.v_unshield, plus validator fee credit if fees are not burned).

1.4 Dual-Anchor Rolling Tree Rollover Design (L1 Production Specification)
--------------------------------------------------------------------------------
When an active Depth-24 tree Tree_e reaches |Tree_e.Coins| = 2^24 (~97.1 days
at 1 tx/s), consensus freezes appends to Tree_e and opens Tree_{e+1} = [] while
retaining Tree_e.Anchors in Anchors_global:
  - Newly created outputs append exclusively to Tree_{e+1}; global SpentSN and
    Coins_global span all epochs, preserving Theorem 3 unchanged.
  - Single-anchor constraint: Because PFSH02 carries a single anchor field, a
    2-real-input transaction can only combine two coins residing in the SAME
    tree. Under single-anchor PFSH02, combining a legacy Tree_e coin with a
    Tree_{e+1} coin requires either (a) a 1-real + 1-dummy self-transfer of the
    Tree_e coin (placing the new output into Tree_{e+1}), or (b) extending
    PFSH02 with per-input anchors (anchor_0, anchor_1) (+48 B header, zero
    additional circuit permutations since C4 already gates each input's root
    independently). (Unimplemented in the single-tree prototype — Category D.5.)

================================================================================
SECTION 2. ARITHMETIZATION, CIRCUIT EQUIVALENCE, & ALGEBRAIC OPTIMIZATIONS
================================================================================

2.1 NP Relation R_pay & Five-Column Chi-Only Layout (447,014 Active Rows)
--------------------------------------------------------------------------------
A witness w = ((is_dummy_b, dirs_b[0..23], sibs_b[0..23], S_b, v_in_b,
R_in_b)_{b=0,1}, (A_out_c, v_out_c, R_out_c)_{c=0,1}) satisfies (x, w) in R_pay
iff (with pub_exit := fee + v_unshield < 2^64 enforced by R5):
  (1) For b in {0, 1}: A_b = H_A(S_b), Coin_in_b = H_Coin(A_b, v_in_b, R_in_b),
      and sn_b = H_SN(S_b, Coin_in_b).
  (2) For b in {0, 1}: Let root_b be the Depth-24 H_Node Merkle root computed
      from leaf Coin_in_b along (dirs_b, sibs_b). Then:
        is_dummy_b == 1  ==>  v_in_b == 0,
        is_dummy_b == 0  ==>  root_b == anchor.
  (3) For c in {0, 1}: coin_c = H_Coin(A_out_c, v_out_c, R_out_c) with A_out_c
      private inside w.
  (4) Exact 66-bit integer conservation over Z:
        v_in_0 + v_in_1 + v_mint == v_out_0 + v_out_1 + (fee + v_unshield) in Z
      (all operands in [0, 2^64-1], max side sum 3*(2^64-1) < 2^66, zero wrap).

Witness bits live in 5 bit-arrays B_0..B_4 in GF(2)^{2^19} (V_SC1 = 19, 524,288
virtual rows). Every quadratic gate uses ONLY the five Keccak Chi monomials
M_x(i) := (1 + B_{(x+1)%5}(i)) * B_{(x+2)%5}(i) (x in 0..4), with booleanity
enforced structurally by subfield-K_32 packing (Lemma 3):
  * 0 .. 430,079       (430,080 = 56 x 7,680): 56 Keccak-f[1600] perms
    (perms 0..7: A_0, Coin_in0, SN_0, A_1, Coin_in1, SN_1, Coin_out0, Coin_out1;
     perms 8..55: 2 x 24 Depth-24 Merkle path hashes).
  * 430,080 .. 439,295 (9,216 = 48 x 192, 64-row aligned): Packed 2-bit Merkle
    conditional-swap mux rows (M_4, M_1).
  * 439,296 .. 445,439 (6,144 = 48 x 128, 64-row aligned): Private Merkle
    siblings packed 3 bits/row (B_0, B_1, B_2).
  * 445,440 .. 445,503 (64 = 2 x 32): Native is_dummy value zero-gating.
  * 445,504 .. 445,887 (384 = 2 x 192): Native is_dummy anchor root-gating.
  * 445,888 .. 446,145 (258 = 64 LHS1 + 65 LHS + 64 MID + 65 RHS): Exact 66-bit
    ripple-carry conservation adders (v_in0+v_in1+v_mint == v_out0+v_out1+
    pub_exit; Z3 5.4.1 UNSAT over all 2^384 6-tuples).
  * 446,146 .. 447,013 (868): Remaining 4,338 private input bits packed 5/row.
  * 447,014 .. 524,287: Identically zero. Packed into K_COLS = 512 columns of
    32-bit K_32 elements (16,384 bits/row), only 28 subrows per polynomial
    (458,752 >= 447,014 bits) are active -> M_WIT_ROWS = 140.

2.2 Single-alpha Circuit Batching & Factored Verifier MLE
--------------------------------------------------------------------------------
Round 2 draws ONE single element alpha in E = GF(2^384) and assigns a distinct
monomial power alpha^c to every scalar constraint across:
  - Group A (56 x 23 x 1,600 = 2,060,800 transitions): alpha^0 .. alpha^{2,060,799}.
  - Group B (56 x 1,600 = 89,600 input bindings): alpha^{2,060,800} ..
    alpha^{2,150,399}. Mechanized DAG check verifies 1-to-1 coverage of all
    56 x 24 x 1,600 = 2,150,400 Keccak state bits across Groups A & B.
  - Group C (42,444 scalar constraints: 1,536 C1 pub-outs + 36,864 C2 muxes +
    384 C3 dummy-val + 2,304 C4 dummy-root + 1,356 C5 66-bit adders):
    alpha^{2,150,400} .. alpha^{2,192,843}.
Total constraints N_C = 2,192,844 (univariate degree in alpha <= 2,192,843).

2.3 Binary Field Tower, Streaming Coset ANTT Prover, & 3D-Karatsuba Verifier
--------------------------------------------------------------------------------
Tower (Rabin irreducibility verified in check_math.py across d in {1,2,3,4,6}):
  * K_16 = GF(2)[x] / (x^16 + x^5 + x^3 + x + 1)         (0x1002B, gen g = 3)
  * K_32 = K_16[Z]  / (Z^2 + Z + x^11)                   (c = 0x0800, Tr = 1)
  * E    = K_32[Y]  / (Y^12 + Y^3 + Y + (Z + x))         (alpha = 0x00010002)
  * Disjoint F_2-affine subspaces inside K_32:
      H = V_9 = {0..511}                                 (|H| = k = 512)
      L = 0x00200000 + V_21                              (|L| = n = 2,097,152)

Key engineering optimizations (all differentially verified against naive refs):
  1. 54-PCLMULQDQ Karatsuba + 2x-Unrolled MOVOU Go Assembly: Packing a_0 + a_1 Z
     into u64 = a_0 | (a_1 << 32) computes E::mul via 54 carryless multiplies
     (Rust _mm_clmulepi64_si128; Go Plan 9 amd64 2x-unrolled MOVOU + dual
     PCLMULQDQ $0x00/$0x11 clmulVec54 with runtime CPUID & -tags purego).
  2. 1-Mul eqTable Folding & Block-Factored C1/C4/B1 Verifier MLE: Computing
     p = val * ri, next[2i] = val ^ p, next[2i+1] = p halves eqTable E::muls
     (16,384 -> 8,192) in both Rust and Go; factoring eqHi blocks in Go
     B1/C1/C4 + basis stepping (mulByX/Z/Y) + paired log-lookup addMulKPair
     reduces Go 1-core warm verify to 36.32 ms med / 40.40 ms max (+9.60 ms
     headroom under 50 ms).
  3. Exact 3D-Karatsuba Tower-Reduced Ring-Switch Verifier: Reduces witness
     H-domain inner-product verification from 143,360 E::muls to 13,122 E::muls
     (11x fewer), verified bit-for-bit against the naive 384 x 384 bilinear sum
     on random inputs in both Rust (test_3d_karatsuba_rs_ip_matches_naive) and
     Go (TestPCLMULQDQEquivalence10000).
  4. Streaming L1-Cache-Resident 1,024-Column Coset ANTT Prover: Skips identity
     ANTT levels i = 20..10 for degree D = 565 < 1,024, streaming 2,048 blocks
     directly into MT_1 leaves at 561.7 MiB peak RSS and 2.89 s prove (16C).

================================================================================
SECTION 3. 34-ROUND IOP, 512-BIT-STATE SINGLE-ORACLE BCS, & SECURITY THEOREMS
================================================================================

3.1 Disjoint 256-Row Virtual Matrix & Single 384-Bit-Digest Merkle Tree MT_1
--------------------------------------------------------------------------------
VIRTUAL_ROWS = {0..255} (V_SC2 = 8) partitions disjointly into:
  - MASK2_PAIRS (8 disjoint hypercube edges flipping bit d = 0..7):
      (0,128), (1,65), (2,34), (3,19), (4,12), (16,20), (8,10), (6,7).
    MASK2_ROWS (16 rows): [0,1,2,3,4,6,7,8,10,12,16,19,20,34,65,128].
  - NON_MASK_ROWS (first 147 indices in {0..255} \ MASK2_ROWS):
      * 0..139   (140 rows): active witness B_0..B_4 (5 x 28 subrows x 512 cols).
      * 140..144 (5 rows):   5 x 32 Ring-Switch E-masks bit-transposed into
                             384 K_32 words/row (cols 0..383; 384..511 zero).
      * 145..146 (2 rows):   78 E-elements (G_1[76] + m_g1 + m_g2) = 936 K_32
                             words across 2 rows of 512 cols.
  - UNUSED_ZERO_ROWS (93 rows, omitted from MT_1).
All M_K_ROWS = 163 active K-rows (deg < D = 565, 53 uniform K_32 randomizers
vanishing on H) plus 2 uniform degree-< 565 E-rows (B_pcs, B_prox) are
committed in one height-21 Merkle tree MT_1 (n = 2^21, 48 B digests = H[0..48]).

3.2 34-Round IOP & 1-to-1 Atomic Single-Oracle BCS Schedule
--------------------------------------------------------------------------------
Every IOP challenge round k in {2..33b} invokes H("PFS-CH  " || tag || state)
once to produce (1 E-element = H[0..48], 512-bit next_state = H[64..128]):
  Round 1       (P->V): (MT_1 root, S_G1, S_mask2) -> STMT [64 B internal state]
  Round 2       (C02):  alpha in E (all 2,192,844 circuit constraints)
  Rounds 3..21  (C03..C21): 19 Sumcheck-1 rounds, deg 3, blinded by G_1
                        (verifier rejects if any r_1[d] == 0)
  Round 22      (C22):  (eval_c1, eval_c2) -> tau in E (Deg-2 hiding, deg 2;
                        verifier rejects if tau == 0)
  Round 23      (C23):  (eval_z[0..4], eval_zg) -> gamma in E (deg 4)
  Round 24      (C24):  rs_s_prime[0..31] -> rho_rs in E (deg 386; verifier
                        rejects if rho_rs == 0)
  Rounds 25..32 (C25..C32): 8 Sumcheck-2 rounds over 256 virtual rows, deg 2
  Round 33a     (C33P): b_eval -> lambda_p in E -> prover sends v_prox (deg < 565)
  Round 33b     (C33O): v_prox absorbed -> lambda_o in E -> prover sends v_open (deg < 565)
  Round 34      (H_C4): v_open absorbed -> 64 B tr_final; prover draws up to
                        M_pow = 2^27 uniform 48 B nonces until
                        H("PFS-C4  " || tr_final || pow_nonce)[0..192] satisfies
                        20-bit PoW, yielding 52 exact 21-bit query indices;
                        prover opens <= 52 columns & pruned 48 B Merkle path.
Total programmed oracle calls per proof: 34 absorbs + 33 challenges + 1 H_C4 =
68 calls, each with >= 512-bit hidden input min-entropy (conditioning proved in
Appendix B.2). At height h = 21 with t = 52 openings, the maximum number of
distinct internal MT_1 nodes verified is sum_{lvl=0}^{20} min(2^lvl, 52) =
1 + 2 + 4 + 8 + 16 + 32 + 15*52 = 63 + 780 = 843, yielding verifier query
budget kappa_v <= 68 + 52 + 843 = 963.

3.3 Theorem 1A (34-Round IOP RBR Knowledge Soundness & Statistical HVZK)
--------------------------------------------------------------------------------
Let M: L -> (K^{163} x E^2) cup {bot} be any (possibly partial) Round-1 oracle,
let P(M) subset RS[E, L, 565]^{165} be the information-theoretic greedy peeling
set defined by Peel(M) (Appendix A.1, stopping guard |L_i| > 2D, alpha =
sqrt(2), m' = 3), and let P_bad(M) := {u in P(M) : u does not encode a valid
witness w in R_pay(x)}. Then:
  (a) |P_bad(M)| <= |P(M)| <= L_max = floor(sqrt(2 / rho)) = 86
      (where rho = D / n = 565 / 2^21 = 0.000269413).
  (b) There exists an explicit 34-round pre-challenge RBR-KS state function
      State_KS(x, tr_k) in {0, 1} aligned 1-to-1 with each atomic challenge
      round k in {2..34} (defined strictly via pure algebraic discrepancy
      invariants in Appendix A.2, with zero sticky zero-challenge disjuncts)
      and a deterministic polynomial-time extractor Ext_IOP running in
      O(min(n, |supp(M)|) + 140 * 565^2) time such that whenever
      State_KS(x, tr_34) == 0 and V(x, tr_34) == 1, Ext_IOP(x, M, tr_34)
      outputs w* in R_pay(x) with probability 1, and per-round transition
      probabilities satisfy:

  Round(s)    | Atomic Challenge (Single E / H_C4)    | Pure IOP Bound eps_rbr_ks(k)
  ------------+---------------------------------------+------------------------------
  Round 1     | MT_1 M + S_G1 + S_mask2 (STMT)        | State_KS(x, tr_1) := 0
  Round 2     | C02: single alpha (deg 2,192,843)     | <= 86*2,192,843/2^384 = 2^-356.51
  Rds 3..21   | C03..C21: 19 SC1 rounds (deg <= 3)    | <= 86*3 / 2^384       = 2^-375.99
  Round 22    | C22: Deg-2 hiding tau (deg <= 2)      | <= 86*2 / 2^384       = 2^-376.57
  Round 23    | C23: single gamma (deg <= 4)          | <= 86*4 / 2^384       = 2^-375.57
  Round 24    | C24: single rho_rs (deg <= 386)       | <= 86*386 / 2^384     = 2^-368.98
  Rds 25..32  | C25..C32: 8 SC2 rounds (deg <= 2)     | <= 86*2 / 2^384       = 2^-376.57
  Round 33a   | C33P: lambda_p proximity curve:       |
              |  * Peel(M) + BCIKS20 Thm 6.2 (m'=3)   | <= 2^-305.79
              |  * A_both subset D_u pointwise union  | <= 86*164*2^21/2^384  = 2^-349.22
              |  * Round 33a total                    | <= 2^-305.79
  Round 33b   | C33O: lambda_o inner prod (L_max/|E|) | <= 86 / 2^384         = 2^-377.57
  Round 34    | Pure IOP queries (t=52, no PoW)       | <= (0.023212)^52      = 2^-282.31
  ------------+---------------------------------------+------------------------------
  BCS+PoW20   | Atomic H_C4 marking prob (Case 4)     | p_34 = 2^-20*2^-282.31 = 2^-302.31

  (c) Statistical HVZK (Appendix B.1): There exists an unconditional PPT
      simulator Sim_IOP(x) taking ONLY public statement x (operating
      identically on true OR false statements, outputting bot on the explicit
      exceptional challenge event E_exc of probability <= 133/2^384) whose
      transcript has statistical distance from the real prover on valid (x, w):
        Delta_HVZK <= (112 + 19 + 1 + 1) / 2^384 = 133 / 2^384 = 2^-376.94.

3.4 Lemmas 1.1-1.2 & Theorem 1B (Independent Punctured-G O2H, Capped PoW & ZK)
--------------------------------------------------------------------------------
Lemma 1.1 (Exact 5,216-Bit Marginal Unopened-Leaf Min-Entropy Under Independent G).
Fix any proof transcript, let V_alg denote the algebraic verifier view, and let
S_unop := {leaf_{x*} : x* in L \ Q_idx} be the n - |Q_idx| secret unopened leaf
preimages. Sample (a) an independent uniform random oracle G: {0,1}^* ->
{0,1}^{1536} (restricted to "PFS-LEAF" inputs) and (b) n - |Q_idx| fresh
independent uniform 48-byte digests d'_{x*}, constructing MT_1_sim from d'_{x*}.
Define H'(z) := d'_{x*} || G(z)[48..192] if z == leaf_{x*} in S_unop, and
H'(z) := G(z) otherwise. Because each 760-byte leaf payload m embeds its unique
4-byte column index col_idx in [0, 2^21-1], leaf_{x_1} != leaf_{x_2} holds
structurally for all x_1 != x_2 (Pr[E_leaf_coll] == 0; even ignoring col_idx,
B_pcs/B_prox uniformity caps pairwise collision at C(n, 2)*2^-768 <= 2^-727).
Then:
  (i)  H' has the exact distribution of a uniform oracle under which MT_1_sim
       is identically the real tree MT_1^H, and H'(z) == G(z) outside S_unop.
  (ii) Leakage accounting: All Sumcheck-1/Deg-2/Ring-Switch/Sumcheck-2 messages
       depend exclusively on U_r|_H (low 512 Novel Basis coeffs) and never
       touch the 53 independent uniform K_32 randomizers vanishing on H; for
       each fixed U, (B_pcs, B_prox) |-> (v_open, v_prox) is a bijection on
       E[X]_{<565}^2 leaking zero bits of U's 53 randomizers. Because |Q_idx| <=
       52 < 53, U(x*) is uniform over K_32^{163} conditioned on (V_alg,
       MT_1_sim, G), giving exact marginal conditional min-entropy
         H_inf(leaf_{x*} | V_alg, MT_1_sim, G) = 163 * 32 = 5,216 bits.
  (iii) By AHU19 Theorem 3 (Semi-Classical O2H), |Pr[A^{H'}=1] - Pr[A^G=1]| <=
       2 * Q * sqrt(P_find), where P_find is the probability that measuring ONE
       uniformly random query i <-$ {1..Q} of A^G(V_alg, MT_1_sim) yields an
       element of S_unop. By a union bound over the |S_unop| <= n - 1 marginals
       (requiring only marginal min-entropy, NOT joint independence):
         P_find <= sum_{x* in L \ Q_idx} Pr[q_i = leaf_{x*}] <= (n - 1) / 2^5216,
       hence |Pr[A^{H'}=1] - Pr[A^G=1]| <= 2 * Q * sqrt(n - 1) / 2^2608
       (check_math.py retains conservative factor 4 and adds C(n,2)*2^-768).

Lemma 1.2 (Hard-Capped Uniform-Nonce PoW Total-Variation Bound — App. B.2).
Let N = 2^384 (48 B nonce), q = 2^-20 (N*q = 2^364), and M_pow = 2^27
(= 128 * 2^20). Conditioned on freshly uniform 512-bit tr_final, comparing
Real_capped (up to M_pow i.i.d. uniform draws in {0,1}^384) against Ideal
(uniform nonce* programmed to a uniform passing output) yields:
  TV(Real_capped, Ideal) <= (1 - q)^{M_pow} + M_pow^2/(2N) + 1/(2*sqrt(Nq)) + e^{-Nq}
                         <= e^-128 + 2^-331 + 2^-183 + e^{-2^364} <= 2^-182.60.

Theorem 1B (Compiled Multi-Proof QROM Zero-Knowledge — Appendix B.2).
Against any adaptive quantum distinguisher making Q total QROM queries and
requesting P <= 2^32 compiled proofs from the hard-capped prover, Sim_BCS(x)
(taking ONLY public x, operating identically on true or false statements)
achieves distinguishing advantage:
  Adv_QROM-ZK(P, Q) <= P * (133/2^384) + P * TV(Real_capped, Ideal)
                       + 4 * P * Q * sqrt(n - 1) / 2^2608 + P * C(n,2)/2^768
                       + (3/2) * P * sqrt(Q) * (68 / 2^256).
With 512-bit hidden state min-entropy (proved in Appendix B.2), the GHHM21
reprogramming term is (3/2)*2^32*2^{32,40,48}*68/2^256 = 2^{-185.33, -177.33,
-169.33}, so Adv_QROM-ZK(P=2^32, Q) <= 2^-150.60 at Q = 2^64, 2^80, and 2^96
(dominated by the capped-PoW idealization artifact P * TV(Real_capped, Ideal)).

3.5 Theorems 2A & 2B (Conditional Concrete Single-Oracle Extraction & SE)
--------------------------------------------------------------------------------
Theorem 2A (Single-Oracle BCS Straight-Line QROM Extraction — Conditional on
Standalone Theorem A.3).
Compile the 34-round IOP of Theorem 1A using ONE single fixed-range random
oracle H: {0,1}^* -> {0,1}^{1536} (48 B MT_1 digests, 64 B internal states),
with kappa_v <= 963 verifier queries per proof.
  * Scope note on published precedent: CMS19 (TCC 2019) establishes asymptotic
    straight-line QROM extraction for BCS compilation of RBR-sound IOPs (not
    covering PoW-grinded queries or concrete constants for fixed parameters).
  * Conditional concrete bound (Theorem A.3, Appendix A.3): Combining CFHL21
    (EUROCRYPT 2021) and DFMS22a (EUROCRYPT 2022, ePrint 2021/280) compressed-
    oracle transition-capacity tools with our self-contained Lemmas A.3.1-A.3.3
    (Tree-Invariance & retroactive Peel(M) stability, order-independent chain
    uniqueness under neg(CL), and 4-case transition capacity), there exists a
    polynomial-time straight-line QROM extractor Ext_QROM whose extraction
    error for L simultaneous accepting proofs satisfies (with Q_v = Q + L*kappa_v):
      eps_Ext(Q, L) <= [ (2/3)*sqrt(30) * Q_v^{3/2} * 2^-192
                         + Q_v * sqrt(10 * max(eps_rbr(33a) + 2*Q_v/2^512, p_34)) ]^2
                       + 2 * L * kappa_v / 2^384,
    where (2/3)*sqrt(30) < 3.652, eps_rbr(33a) = 2^-305.79, p_34 = 2^-302.31,
    and 2*Q_v/2^512 <= 2^-415 for all Q <= 2^96 (hence max(...) == p_34).

Theorem 2B (Adaptive Multi-Proof QROM Simulation-Extractability — Conditional
Conjecture-Level Bridge in Appendix A.4; No Standalone Published Anchor).
Let A make Q quantum queries to H' and P <= 2^32 adaptive queries to Sim_BCS
returning {(x_i, pi_i)}_{i=1}^P, then output L <= 2^16 fresh accepting pairs
{(x*_j, pi*_j)}_{j=1}^L satisfying (x*_j, pi*_j) not in {(x_i, pi_i)}_{i=1}^P
for every j (freshness on statement-proof pairs). Conditional on Theorem A.3
and Appendix A.4 (wrapper B^H on unmodified H with classical overlay T_prog
disjoint from LEAF/NODE and pre-sampled event E_3), Ext_QROM extracts valid
witnesses w*_j in R_pay(x*_j) for all L fresh pairs except with probability:
  eps_SE(P, L, Q) <= eps_Ext(Q_eff, L)
                     + 10 * Q_eff^2 * P * (68/2^512 + (n - 1)/2^384 + (t/n)^t)
                     + (3/2) * P * sqrt(Q_eff) * (68 / 2^256),
where Q_eff = Q + P*(n - 1 + t) + L*kappa_v explicitly charges simulation
Merkle hashing and all L fresh-proof verifications.

Table 2 — Exact-Rational Conditional Compiler Targets vs. Published-Only & Ideal-Sponge References
--------------------------------------------------------------------------------
  [Part I: Conditional concrete QROM compiler targets (Thms 1B, 2A, 2B;
   recomputed in check_math.py) juxtaposed with the published-results-only
   fallback column. Part II: Black-box ideal-sponge/random-function quantum
   sizing references for SHAKE256 (not standard-model proofs). Ordered from
   high-query Q = 2^96 down to reference Q = 2^64.]
Component / Formula Reference          | Published-Only  | Q = 2^96  | Q = 2^80  | Q = 2^64 (ref)
---------------------------------------+-----------------+-----------+-----------+---------------
PART I — CONDITIONAL COMPILER TARGETS: |                 |           |           |
QROM Ext (Thm 2A, L=1, cond. Thm A.3)  | Asymptotic only | 2^-92.25  | 2^-137.55 | 2^-170.98
                                       | (CMS19; DFMS20  |           |           |
                                       | gives (Q+1)^68) |           |           |
QROM SE  (Thm 2B, P=2^32, L=2^16, A.4) | No pub. bound   | 2^-92.25  | 2^-137.55 | 2^-170.97
QROM ZK  (Thm 1B, P=2^32, M_pow=2^27)  | GHHM21 + AHU19  | 2^-150.60 | 2^-150.60 | 2^-150.60
                                       | (+ Lem 1.1-1.2) |           |           |
---------------------------------------+-----------------+-----------+-----------+---------------
PART II — IDEAL-SPONGE SIZING REFS:    |                 |           |           |
SHAKE256 384b BHT Coll (32 Q^3/2^384)  | Ideal-fn sizing | 2^-91.00  | 2^-139.00 | 2^-187.00
SHAKE256 c=512b Q-Cap  (32 Q^3/2^512)  | Ideal-sponge    | 2^-219.00 | 2^-267.00 | 2^-315.00
--------------------------------------------------------------------------------
Recomputation & regime-crossover check: In Thm 2A, b_term = Q*sqrt(10*2^-302.31)
dominates for Q < Q_cross ~ 2^81.28 (giving 2^-170.98 at Q = 2^64), whereas the
MT_1 collision term a_term = 3.652*Q^{3/2}*2^-192 dominates for Q > Q_cross
(giving 2^-92.25 at Q = 2^96, matching 384-bit BHT 2^-91.00). The conditional
bound reaches triviality (eps_Ext = 1) at Q_triv = 2^126.75 (triviality
threshold, not a security level).

================================================================================
SECTION 4. COMPOSITIONAL LEDGER SECURITY GAMES, REDUCTIONS, & SECURITY FLOOR
================================================================================

Model-composition note: The compiler random oracle H (3 x SHA-512) is analyzed
in the QROM, while in-circuit SHAKE256, ML-KEM-1024, SHA-384, and AES-256-GCM
are modeled as standard-model primitives against a quantum adversary (with no
oracle programming on SHAKE256, ML-KEM, or AES-GCM, and zero shared permutation
with H). In Theorems 3-5, each reduction simulates H for the quantum adversary
via standard QROM interfaces (compressed oracle or GHHM21 reprogramming).

4.1 Theorem 3 (Shielded-Pool & Joint Ledger Supply Conservation — Exp_Bal)
--------------------------------------------------------------------------------
Game Exp_Bal(A): Starting from empty genesis S_0 (Rule R0), adversary A (making
Q quantum queries to unmodified H) adaptively invokes:
  - O_Hon(in0, in1, out0, out1, v_mint, v_unshield, unshield_dest, fee):
    executes up to q_hon honest PFSH02 transactions (including ZK-Shielded
    Mints with v_mint > 0) using the real hard-capped prover on unmodified H
    (each taking <= Q_prover = 2n + M_pow + 68 classical H queries). The
    challenger maintains an internal table HonNotes recording (coin, S, v, R)
    for every honest-created note; for any adversary-deposited note targeting
    an honest address A_u, A supplies its explicit opening (v, R) when invoking
    O_Hon (or the challenger recovers (v, R) via dk_u if env decrypts cleanly
    and matches coin).
  - Submit(tx [, outer_auth]): submits arbitrary PFSH02 transactions validated
    atomically by Rules R1-R6 (plus R0-Auth for L1 consensus), producing q_tx
    accepted transactions and updating Supply_minted, Unshielded_acc, Fees_acc
    (and Bal_L1[]).
Winning condition (concrete witness-opening formulation): A outputs distinct
coins {c_1, ..., c_m} subset Coins[] together with explicit openings
{(S_j, v_j, R_j)}_{j=1}^m satisfying (i) c_j == H_Coin(H_A(S_j), v_j, R_j) with
v_j in [0, 2^64-1], (ii) H_SN(S_j, c_j) not in SpentSN, and:
  - (3a) Shielded-Pool Inflation: sum_{j=1}^m v_j + Unshielded_acc + Fees_acc
         > Supply_minted; OR
  - (3b) Joint Transparent+Shielded Inflation (at L1 consensus enforcing
         R0-Auth/R6): if fees are burned, sum_a Bal_L1[a] + sum_{j=1}^m v_j +
         Fees_acc > TotalSupply_0; if fees are credited to validator accounts
         in Bal_L1[], sum_a Bal_L1[a] + sum_{j=1}^m v_j > TotalSupply_0.

Theorem 3 (Unmodified-Oracle Extraction Only — Zero Simulation):
  Adv_Bal(A) <= eps_Ext(Q_tot, q_tx) + Adv_ZeroPreimage^{H_Coin,384}(A_0)
                + Adv_CR^{H_Coin,384}(A_1) + Adv_CR^{H_A,384}(A_2)
                + Adv_CR^{H_Node,384}(A_3),
where Q_tot = Q + q_hon * (2n + M_pow + 68) folds A and all q_hon real provers
into ONE unmodified-oracle quantum algorithm B^H outputting all q_tx accepted
transactions simultaneously.

Proof: Apply Theorem 2A with L = q_tx to B^H to extract valid witnesses w_i in
R_pay(x_i) for every accepted transaction i = 1..q_tx (including every ZK-
Shielded Mint; failure prob <= eps_Ext(Q_tot, q_tx)). Conditioned on extraction
of w_1, ..., w_{q_tx}, define inductively on t = 0..q_tx a canonical opening
map Open_L(c) = (A_c, val_L(c), R_c) for every c in Coins[] and live subset
Live_t subset Coins[]:
  - Base case t = 0: Coins = [], Live_0 = empty, Supply_minted = Unshielded_acc
    = Fees_acc = 0, so sum_{c in Live_0} val_L(c) + Unshielded_acc + Fees_acc
    = 0 = Supply_minted.
  - Inductive step (accepted tx_i with extracted witness w_i in R_pay(x_i)):
    1. Real vs. dummy inputs: For each b in {0, 1}:
       * If is_dummy_b == 1: Z3-verified C3 gating enforces v_{in,b} == 0.
       * If is_dummy_b == 0: Z3-verified C4 gating enforces root_b ==
         tx_i.anchor in Anchors (which by R1/R6 equals the Depth-24 root of a
         prefix Coins[0..m_i-1] padded with EMPTY[0] = 0^48). Walking down from
         root_b to extracted leaf c_{in,b} = H_Coin(A_b, v_{in,b}, R_{in,b}):
         either (a) c_{in,b} == Coins[idx] for a unique 0 <= idx < m_i (unique
         by R4), or (b) c_{in,b} == EMPTY[0] = 0^48 at an unused slot idx >= m_i
         (breaking Adv_ZeroPreimage^{H_Coin,384}), or (c) an H_Node collision
         occurred along the path (breaking Adv_CR^{H_Node,384}).
    2. Value binding & no double-spend: For each real input c_{in,b} in Coins[],
       compare extracted (A_b, v_{in,b}, R_{in,b}) against Open_L(c_{in,b}) =
       (A*, val_L(c_{in,b}), R*):
       * If (A_b, v_{in,b}, R_{in,b}) != (A*, val_L(c_{in,b}), R*): direct
         H_Coin collision (<= Adv_CR^{H_Coin,384}).
       * Otherwise A_b == A* and v_{in,b} == val_L(c_{in,b}). If c_{in,b} was
         already spent in an earlier tx_{i'} (or by slot 1-b of tx_i) with
         secret S': S_b != S' yields H_A(S_b) = A* = H_A(S') (Adv_CR^{H_A,384});
         S_b == S' yields H_SN(S_b, c_{in,b}) == H_SN(S', c_{in,b}) (rejected
         by Rule R2 or Rule R3).
       Thus every real input removes a distinct coin c_{in,b} from Live_{i-1}
       of exact canonical value val_L(c_{in,b}).
    3. Output registration & exact conservation: By Rule R4, coin_{out,0} and
       coin_{out,1} are distinct, non-zero, and fresh; record
       Open_L(coin_{out,c}) := (A_{out,c}, v_{out,c}, R_{out,c}) from w_i and
       insert both into Live_i. By Z3-verified 66-bit conservation:
         v_{in,0} + v_{in,1} + tx_i.v_mint
           == v_{out,0} + v_{out,1} + tx_i.fee + tx_i.v_unshield  in Z.
       Hence sum_{c in Live_i} val_L(c) + Unshielded_acc_i + Fees_acc_i ==
       Supply_minted_i at every step i = 1..q_tx.
  - Final contradiction for (3a): If A outputs distinct {c_1..c_m} subset Coins
    with valid unspent openings {(S_j, v_j, R_j)} whose sum + Unshielded_acc +
    Fees_acc > Supply_minted, then some c_j either has (H_A(S_j), v_j, R_j) !=
    Open_L(c_j) (Adv_CR^{H_Coin,384}), or is not in Live_{q_tx} (spent earlier
    with S' != S_j while H_A(S_j) == H_A(S'), breaking Adv_CR^{H_A,384}).
  - Joint conservation (3b): Under burned fees, consensus enforces
    sum_a Bal_L1[a] + Supply_minted - Unshielded_acc == TotalSupply_0 (adding
    (3a) yields sum_a Bal_L1[a] + sum v_j + Fees_acc <= TotalSupply_0); under
    validator-credited fees, consensus enforces sum_a Bal_L1[a] + Supply_minted
    - Unshielded_acc - Fees_acc == TotalSupply_0 (adding (3a) yields
    sum_a Bal_L1[a] + sum v_j <= TotalSupply_0). []

4.2 Theorem 4 (Spend Auth, Context Non-Malleability, & Mint Auth — Exp_Auth)
--------------------------------------------------------------------------------
Game Exp_Auth(A): Challenger samples honest shielded secret S* <-$ {0,1}^{384}
(A* = H_A(S*)) and honest transparent L1 signing keypair (pk_L1*, sk_L1*),
allows A to create notes under A* (supplying their explicit openings (v, R)
when later instructing O_Hon to spend them, or via valid ML-KEM envelopes under
ek* if registered), call O_Hon up to q_hon times to spend honest
notes owned by S* (producing {(tx_i, x_i, pi_i)}_{i=1}^{q_hon}), and call
O_MintSign on chosen honest deposits under pk_L1*. A wins in ANY of three modes:
  - Mode 1 (Unauthorized Shielded Spend): A outputs an accepted tx* spending an
    unspent coin Coin* owned by A* (not spent by any O_Hon call);
  - Mode 2 (Header / Context Malleability): A outputs an accepted tx* with
    header(tx*) != header(tx_i) (differing in anchor, v_mint, v_unshield,
    unshield_dest, fee, sn0/1, coin0/1, or env0/1; re-proving identical
    header(tx_i) has identical ledger effect and is excluded from Mode 2)
    sharing a nullifier sn* in {tx_i.sn0, tx_i.sn1} of some O_Hon tx_i,
    accepted on a pre-tx_i state where sn* not in SpentSN;
  - Mode 3 (Unauthorized Transparent Mint Debit — L1 Consensus Rule R0-Auth): A
    outputs an accepted outer-L1 mint transaction debiting mint_src = pk_L1*
    with (v_mint > 0) whose header(tx*) was never signed by O_MintSign.

Theorem 4 (Corrected Mode-2 Split & Multi-Target Target-Preimage Term):
  Adv_Auth(A) <= Adv_QROM-ZK(q_hon, Q) + Adv_JointPRF^{SHAKE256}(A_prf)
                 + eps_SE(q_hon, 1, Q) + Adv_CR^{SHA-384}(tx_ctx)
                 + Adv_MT-TPR^{H_SN,384}(q_hon; A_tpr) + Adv_OW^{H_A,384}(A_ow)
                 + Adv_CR^{H_Coin,384}(A_cr) + Adv_CR^{H_Node,384}(A_nd)
                 + Adv_ZeroPreimage^{H_Coin,384}(A_z)
                 + Adv_EUF-CMA^{L1-Sig}(A_sig).

Proof:
  - Mode 3: If header(tx*) != header(tx_j) for all O_MintSign calls j yet
    tx_ctx(tx*) == tx_ctx(tx_j), A breaks Adv_CR^{SHA-384}(tx_ctx); otherwise
    ("PFT-SH-MINT-AUTH-V2" || pk_L1* || tx_ctx(tx*)) is a fresh message forged
    under pk_L1*, breaking Adv_EUF-CMA^{L1-Sig}.
  - Modes 1 & 2 (Clean Two-Hop Hybrid Avoiding Coupled Assumptions):
    First, in Mode 2, header(tx*) != header(tx_i) implies EITHER tx_ctx(tx*) ==
    tx_ctx(tx_i) (direct SHA-384 collision on distinct headers, <=
    Adv_CR^{SHA-384}(tx_ctx)) OR x* != x_i (so (x*, pi*) not in {(x_i, pi_i)}
    is strictly fresh under Theorem 2B). In Mode 1, tx* spends Coin* not spent
    by O_Hon, so either (x*, pi*) is fresh or shares (x_i, pi_i) (which would
    share sn_i -> H_SN/H_A/H_Coin collision).
    * Hop 1 (H_0 -> H_1, QROM ZK): Replace all q_hon O_Hon proofs with
      Sim_BCS(x_i) via Theorem 1B (loss <= Adv_QROM-ZK(q_hon, Q)).
    * Hop 2 (H_1 -> H_2, Adaptive JointPRF): Replace all q_hon O_Hon nullifiers
      H_SN(S*, Coin_j) with independent uniform 48-byte targets U_1..U_{q_hon}
      via the single-user adaptive JointPRF^{SHAKE256} game (publishing real
      A* = H_A(0x00 || S*) and answering O_SN(c) vs. random; loss <=
      Adv_JointPRF^{SHAKE256}). Crucially, in Game H_2, O_Hon never uses S* at
      all—only the public challenge A* = H_A(S*) is needed to run H_2.
    * Extraction in H_2: Apply Theorem 2B (L = 1 fresh pair (x*, pi*)) to
      extract w* in R_pay(x*) (failure <= eps_SE(q_hon, 1, Q)):
        - If Mode 1 occurs in H_2: extracted real input opening of Coin* either
          collides in H_Node, H_Coin, or zero-preimage, or yields S' with
          H_A(S') == A*. Because H_2 runs without S*, this gives a standalone
          reduction to Adv_OW^{H_A,384} on challenge A*.
        - If Mode 2 occurs in H_2: extracted input (S', Coin') satisfies
          H_SN(0x02 || S' || Coin') in {U_1, ..., U_{q_hon}} for the q_hon
          independent uniform 48-byte targets sampled in H_2, directly breaking
          multi-target Target Preimage Resistance Adv_MT-TPR^{H_SN,384}(q_hon)
          (bounded generically in the ideal-function model by O(q_hon*Q^2/2^384),
          i.e., ~2^-224 at Q=2^64, q_hon=2^32 and ~2^-192 at Q=2^80, q_hon=2^32).
  (If honest users never reuse A*, O_Hon never touches S* and plain Theorem 2A
  suffices with zero simulation and zero JointPRF assumption.) []

4.3 Theorem 5 (Global-Pool & ZK-Mint Privacy With Active O_Scan & O_Corrupt)
--------------------------------------------------------------------------------
Single-User Standard-Model Primitive Interfaces (Avoiding Corruption Gaps):
  - Single-User Adaptive JointPRF^{SHAKE256}: Challenger samples one secret
    S* <-$ {0,1}^{384}, publishes A* = H_A(0x00 || S*), and grants A adaptive
    access to (i) O_SN(c) returning H_SN(0x02 || S* || c) for non-challenge
    coins c, and (ii) O_ChallSN(c*) (at most twice on distinct unqueried c*)
    returning H_SN(0x02 || S* || c*) if bit=0 or uniform U_{48} if bit=1.
    Advantage <= Adv_JointPRF^{SHAKE256}.
  - Single-User Adaptive Hide^{SHAKE256}: Given chosen (A_0, v_0, A_1, v_1),
    distinguishing H_Coin(0x01 || A_c || v_c || R_c) (R_c <-$ {0,1}^{624}) from
    uniform U_{48} has advantage <= Adv_Hide^{SHAKE256}.
  - Single-User ML-KEM-1024 IND-CCA2(q_scan) & IK-CCA(q_scan): Standard single-
    keyplate IND-CCA2 (one challenge keypair (ek*, dk*) with Decaps oracle) and
    two-key IK-CCA (challenge keys (ek_0*, ek_1*) with Decaps_0, Decaps_1
    oracles, distinguishing Encaps(ek_0*) from Encaps(ek_1*)).

Game Exp_GlobalPriv(b): Challenger maintains a Depth-24 ledger via R0-R6,
initializes U_users honest wallets via O_Gen, and grants A adaptive access to:
  - O_Mint(user, v_mint, fee): executes a ZK-Shielded Mint (PFSH02 tx with
    v_mint > 0, is_dummy0 = is_dummy1 = 1) to honest or corrupt user (counted
    in P total honest proofs);
  - O_Hon(in0, in1, dst0, v0, dst1, v1, v_unshield, unshield_dest, fee):
    executes honest shielded transfers (counted in P total honest proofs);
    PERMITTED both pre- and post-challenge to spend OTHER notes belonging to
    challenge input owners (excluding ONLY the specific challenge input coins
    I^(0), I^(1) and the challenge output coins);
  - O_Scan(u, coin, env): active recipient wallet-reaction oracle (up to q_scan
    total queries) running wallet_u.try_scan_output(coin, env) for ANY honest
    user u on ANY (coin, env) != (coin*_c, env*_c), AND ALSO for any non-
    challenge-recipient honest user u' not in {recv_0^(0), recv_1^(0),
    recv_0^(1), recv_1^(1)} even on (coin*_c, env*_c), returning 1 if decrypted
    & valid, 0 otherwise;
  - O_Corrupt(user): reveals (S_u, dk_u) of ANY honest user not in the
    challenge input owners or challenge recipients (enforced both pre- and
    post-challenge);
  - O_Challenge: invoked ONCE on fixed public anchor in Anchors and fixed
    public (v_mint, v_unshield, unshield_dest, fee) in EITHER of two modes:
    (a) Transfer mode (v_mint = 0): two candidate uncorrupted unspent input
        pairs I^(0), I^(1) drawn from ANY positions in the global Depth-24 tree
        up to anchor (each 2-real or 1-real+1-dummy of equal total value V_tot),
        two candidate uncorrupted honest recipient pairs (pk_0^(0), pk_1^(0)),
        (pk_0^(1), pk_1^(1)), and two output splits summing to V_tot - fee -
        v_unshield; OR
    (b) ZK-Shielded Mint mode (v_mint > 0, v_unshield = 0, is_dummy0 =
        is_dummy1 = 1): two candidate uncorrupted honest recipient pairs
        (pk_0^(0), pk_1^(0)), (pk_0^(1), pk_1^(1)) and two output value splits
        summing to v_mint - fee (hiding recipient addresses A_out0/1, split
        values v_out0/1, and blindings R_out0/1 despite public v_mint and fee).
    Challenger runs the real hard-capped prover for bit b and returns tx* (if
    the negligible R3/R4 random collision event occurs, returns bot; abort prob
    <= 2*(|Coins| + |SpentSN| + 4)/2^384 under uniform SN/Coin).

Theorem 5 (Oracle-Complete Corruption-Compatible Reduction — Zero Extraction):
  Adv_GlobalPriv(A) <= 2 * [ Adv_QROM-ZK(P, Q)
                             + 2 * U_users * (Adv_IND-CCA2^{ML-KEM-1024}(q_scan)
                                              + Adv_IK-CCA^{ML-KEM-1024}(q_scan))
                             + 2 * (Adv_PRF^{SHA-384-KDF}
                                    + Adv_INT-CTXT^{AES-256-GCM}(q_scan)
                                    + Adv_IND$-CPA^{AES-256-GCM})
                             + 2 * U_users * Adv_JointPRF^{SHAKE256}
                             + 2 * Adv_Hide^{SHAKE256}
                             + 2 * (|Coins| + |SpentSN| + 4) / 2^384 ].

ML-KEM-1024 Anonymity & SHA-384 KDF Provenance Notes:
  * Grubbs-Maram-Paterson (EUROCRYPT 2022) and Maram-Xagawa (PKC 2023, "Post-
    Quantum Anonymity of Kyber") proved post-quantum ANO-CCA / IK-CCA for
    Kyber's implicit-rejection FO transform. NIST FIPS 203 ML-KEM-1024 retains
    implicit rejection J(z || c) while simplifying ss = K (our KDF explicitly
    hashes ct_kem and coin_out via SHA-384). Because a FIPS-203-specific
    proof covering the exact FIPS-203 key-derivation delta is not imported
    verbatim, Adv_IK-CCA^{ML-KEM-1024} is tagged [NonStd] as a standing
    assumption in Table 1 and Table 3.
  * Because SHA-384("PFT-ENV-KDF" || ss || ct_kem || coin) places the secret ss
    inside an unkeyed Merkle-Damgard hash rather than HMAC-SHA-384,
    Adv_PRF^{SHA-384-KDF} is explicitly tagged [NonStd] (ROM-KDF assumption).

Proof (Oracle-Complete 5-Hop Hybrid With Exact O_Corrupt & O_Scan Simulation):
Every hybrid below specifies how ALL oracles (O_Mint, O_Hon, O_Scan, O_Corrupt,
O_Challenge) are answered without guessing-gap or missing-secret mismatches:
  - Game H_0: Real Exp_GlobalPriv(0) (both Transfer and ZK-Mint modes).
  - Game H_1 (Multi-Proof QROM ZK): Replace all P honest proofs (O_Mint, O_Hon,
    and challenge tx*) with Sim_BCS(x) via Theorem 1B. Sim_BCS(x) uses ONLY
    public x and runs identically on false statements; the reduction holds ALL
    U_users wallet keys (S_u, dk_u) and answers O_Corrupt and O_Scan directly.
    Loss <= Adv_QROM-ZK(P, Q).
  - Game H_2 (Active-Scan Stealth Envelope Confidentiality & Key Privacy,
    output slots c = 0, then c = 1): For each slot c in {0, 1}, replace env*_c
    via four sub-hops:
    (i)   ML-KEM-1024 IND-CCA2(q_scan) with single-user guess (factor U_users):
          The reduction guesses the challenge recipient index u*_c <-$
          {1..U_users}, embeds the single-user IND-CCA2 challenge key ek* as
          ek_{u*_c}, and generates (S_u, ek_u, dk_u) ITSELF for all u != u*_c
          (plus S_{u*_c} itself). Every O_Corrupt(u) for u != u*_c is answered
          directly; if A queries O_Corrupt(u*_c) or chooses recv_c^(0) != u*_c,
          the reduction aborts and outputs a random bit (correct guess occurs
          with exact probability 1/U_users, independent of A's view since all
          ek_u are identically distributed). Every O_Scan(u*_c, coin, env) with
          ct_kem != ct_kem^*_c is answered via the single-user IND-CCA2 Decaps
          oracle; every O_Scan(u', ...) for u' != u*_c (even on (coin*_c,
          env*_c)) is answered directly using dk_{u'} generated by the
          reduction. Replaces ss_c with uniform 32 B ss*_c. Loss <= U_users *
          Adv_IND-CCA2^{ML-KEM-1024}(q_scan).
    (ii)  SHA-384-KDF PRF [NonStd]: With ss*_c uniform and independent of all
          keys, replace (K_aead, N_aead) on (ct_kem^*_c, coin*_c) with uniform
          keys/nonces (and independent uniform keys on any O_Scan query with
          ct_kem == ct_kem^*_c, coin != coin*_c). All O_Corrupt and O_Scan
          queries are answered directly. Loss <= Adv_PRF^{SHA-384-KDF}.
    (iii) AES-256-GCM INT-CTXT(q_scan) + IND$-CPA: Reject all forged O_Scan
          queries sharing ct_kem^*_c with modified ct_aead or coin AAD (loss <=
          Adv_INT-CTXT^{AES-256-GCM}(q_scan), classically O(q_scan/2^128)), and
          replace ct_aead*_c with encryption of 0^86 (loss <= Adv_IND$-CPA).
    (iv)  ML-KEM-1024 IK-CCA(q_scan) with single-user guess (factor U_users):
          The reduction guesses u*_c <-$ {1..U_users}, embeds the IK-CCA
          challenge key ek_0* as ek_{u*_c} (with fresh dummy key ek_1* =
          ek_dummy), and generates ALL other (S_u, ek_u, dk_u) itself (plus
          S_{u*_c}). O_Corrupt(u) for u != u*_c is answered directly (aborting
          if u = u*_c or recv_c^(0) != u*_c; guess prob 1/U_users); O_Scan on
          u*_c with ct_kem != ct_kem^*_c uses the IK-CCA Decaps_0 oracle, and
          O_Scan on all u' != u*_c uses dk_{u'} held by the reduction. Replaces
          ct_kem^*_c with Encaps(ek_dummy). Loss <= U_users * Adv_IK-CCA(q_scan).
  - Game H_3 (Adaptive Single-User-Reduced Joint Nullifier Pseudorandomness,
    input slots a = 0, then a = 1):
    * In ZK-Shielded Mint mode (v_mint > 0, is_dummy0 = is_dummy1 = 1): both
      nullifiers sn0*, sn1* are ALREADY fresh uniform 48 B dummy nullifiers
      (H_SN on uniform fresh secrets independent of all users), so H_2 == H_3
      identically (zero loss).
    * In Transfer mode (v_mint = 0): for each real input slot a in {0, 1}
      (skipping dummy slots, whose nullifiers are already fresh uniform), the
      reduction guesses the challenge input owner u*_a <-$ {1..U_users}, embeds
      the single-user JointPRF challenge address A* as A_{u*_a}, and generates
      ALL other U_users - 1 users' keys (S_u, ek_u, dk_u) AND all U_users
      ML-KEM keys (ek_u, dk_u) itself. Every O_Corrupt(u) for u != u*_a is
      answered directly (aborting with a random bit if A corrupts u*_a or
      chooses input-a owner != u*_a; guess prob 1/U_users, independent of A's
      view); every pre- and post-challenge O_Hon spend of OTHER notes owned by
      u*_a uses O_SN(c) of JointPRF (proofs are already simulated via Sim_BCS
      in H_1, so S_{u*_a} is never needed); and sn_a* is replaced with
      uniform U_{48} via O_ChallSN(coin_a*). (If both inputs belong to the same
      user u*_0 == u*_1, O_ChallSN's two-query budget replaces both in one hop;
      otherwise the two sequential hops handle u*_0 then u*_1.) Total loss
      across both input slots <= 2 * U_users * Adv_JointPRF^{SHAKE256}.
  - Game H_4 (624-Bit Output Commitment Hiding & Abort Accounting): All secret
    keys are now decoupled from tx*; replace coin0*, coin1* with uniform 48 B
    strings via two single-user Hide^{SHAKE256} hops (loss <= 2 *
    Adv_Hide^{SHAKE256}). In H_4, the random R3/R4 collision abort occurs with
    probability <= 2*(|Coins| + |SpentSN| + 4)/2^384 (identical for b=0 and
    b=1). Thus H_4 is identically distributed for b = 0 and b = 1 in BOTH
    Transfer mode and ZK-Shielded Mint mode; reversing H_4 -> H_0 for b = 1
    yields the outer factor of 2. []

4.4 End-to-End Composed Security Audit & Q_tot Degradation
--------------------------------------------------------------------------------
Table 3 instantiates Theorems 3, 4, and 5 at Q = 2^96, Q = 2^80, and reference
Q = 2^64 for a full Depth-24 tree (q_hon = q_tx = 2^23 = 8,388,608 txs) and
multi-epoch history (q_hon = 2^32 = 4.29B txs), explicitly separating
published-only bounds, conditional targets (under unreviewed Theorem A.3 /
conjecture-level Appendix A.4), standing assumptions, and ideal-sponge sizing
references:
  [Pub] = Published theorem/anchor; [IdealRef] = Ideal-sponge/random-function
  sizing reference (NOT a standard-model proof); [Std] = Standard primitive
  assumption; [NonStd] = Standing nonstandard assumption (unquantified);
  [Unrev] = Conditional unreviewed bridge (App. A/B).

Table 3 — Composed Ledger Security Audit (Exact-Rational check_math.py)
--------------------------------------------------------------------------------
Theorem / Term                        | Tag        | Published-Only | Q = 2^96  | Q = 2^80  | Q = 2^64 (ref)
--------------------------------------+------------+----------------+-----------+-----------+---------------
Thm 3 (Balance — q_hon=q_tx=2^23):    |            |                |           |           |
  [Conditional Compiler Terms]        |            |                |           |           |
  * Q_tot = Q + 2^23 * 138,412,100    | [Exact]    | exact          | 2^96.0000 | 2^80.0000 | 2^64.0001
  * eps_Ext(Q_tot, 2^23) [full tree]  | [Unrev]    | CMS19 asymp.   | 2^-92.25  | 2^-137.55 | 2^-170.98
  * eps_Ext(Q_tot, 2^23) [q_hon=2^32] | [Unrev]    | CMS19 asymp.   | 2^-92.25  | 2^-137.55 | 2^-170.88
  [Ideal-Sponge Sizing References]    |            |                |           |           |
  * H_Coin/H_A/H_Node 384b BHT CR     | [IdealRef] | ideal-fn ref   | 2^-91.00  | 2^-139.00 | 2^-187.00
  * H_Coin 384b ZeroPreimage (Grover) | [IdealRef] | ideal-fn ref   | 2^-192.00 | 2^-224.00 | 2^-256.00
  * SHAKE256 c=512b sponge Q-cap      | [IdealRef] | ideal-sponge   | 2^-219.00 | 2^-267.00 | 2^-315.00
  --> Thm 3 Conditional Target (A.3): | [Cond]     | Asymp. only    | ~91b*     | ~137b*    | ~171b*
      (*conditional parameter calc. under Thm A.3 + IdealRef; NOT an unconditional security floor)
--------------------------------------+------------+----------------+-----------+-----------+---------------
Thm 4 (Auth — q_hon = 2^32 reused):   |            |                |           |           |
  [Conditional Compiler Terms]        |            |                |           |           |
  * Adv_QROM-ZK(2^32, Q)              | [Unrev]    | GHHM21+AHU19+L | 2^-150.60 | 2^-150.60 | 2^-150.60
  * eps_SE(2^32, 1, Q)                | [Unrev]    | No pub. bound  | 2^-92.25  | 2^-137.55 | 2^-170.97
  [Ideal-Sponge Sizing References]    |            |                |           |           |
  * Adv_CR^{SHA-384}(tx_ctx) (BHT)    | [IdealRef] | ideal-fn ref   | 2^-91.00  | 2^-139.00 | 2^-187.00
  * Adv_MT-TPR^{H_SN}(2^32) (Grover)  | [IdealRef] | ideal-fn ref   | 2^-160.00 | 2^-192.00 | 2^-224.00
  * Adv_OW^{H_A,384} (Grover)         | [IdealRef] | ideal-fn ref   | 2^-192.00 | 2^-224.00 | 2^-256.00
  [Unquantified Standing Terms]       |            |                |           |           |
  * Adv_JointPRF^{SHAKE256}           | [NonStd]   | unquantified   | (standing assumption)
  * Adv_EUF-CMA^{L1-Sig} (Mode 3)     | [Std]      | FIPS-204       | (ML-DSA-65/87 at L1)
  --> Thm 4 Conditional Target (A.3/4)| [Cond]     | No pub. bound  | ~91b*     | ~137b*    | ~150b*
      (*conditional on A.3 + Conj. A.4 + IdealRef + unquantified [NonStd] JointPRF + L1-Sig)
--------------------------------------+------------+----------------+-----------+-----------+---------------
Thm 5 (GlobalPriv — Transfer & Mint): |            |                |           |           |
  * 2 * Adv_QROM-ZK(2^32, Q)          | [Unrev]    | GHHM21+AHU19+L | 2^-149.60 | 2^-149.60 | 2^-149.60
  * 4*U*Adv_IND-CCA2^{ML-KEM}(q_scan) | [Pub/Std]  | FIPS-203 Cat-5 | (single-user reduced, factor 4U)
  * 4*U*Adv_IK-CCA^{ML-KEM}(q_scan)   | [NonStd]   | GMP22/MX23*    | (standing FIPS-203 assumption)
  * 4*Adv_INT-CTXT^{AES-GCM}(q_scan)  | [Std]      | SP 800-38D     | (O(q_scan / 2^128) forgery)
  * 4*Adv_IND$-CPA^{AES-256-GCM}      | [Std]      | SP 800-38D     | (AES-256 standard bound)
  * 4*Adv_PRF^{SHA-384-KDF}           | [NonStd]   | ROM-KDF        | (key-in-middle SHA-384)
  * 4*U*Adv_JointPRF + 4*Adv_Hide     | [NonStd]   | unquantified   | (standing SHAKE256; 0 for Mint)
  * R3/R4 abort conditioning (2^24)   | [Exact]    | exact          | 2^-358.00 | 2^-358.00 | 2^-358.00
  --> Thm 5 Status                    | [Cond]     | Parameterized by (U, q_scan) + [NonStd] (IK-CCA,
                                      |            | SHA-384-KDF, JointPRF, Hide); no single bit floor
--------------------------------------------------------------------------------
Note: At realistic full-tree throughput (q_hon = 2^23 = 8,388,608 txs), honest
prover queries inflate Q_tot by only 0.0001 bits at Q = 2^64 (0.0458 bits even
at q_hon = 2^32, degrading eps_Ext by only 0.10 bits).

================================================================================
SECTION 5. EMPIRICAL EVALUATION, WIRE ACCOUNTING, & PUBLIC REPRODUCIBILITY
================================================================================

5.1 Host Environment & Toolchain
--------------------------------------------------------------------------------
Single-host measurements: AMD EPYC Processor (16 physical cores / 32 threads,
pclmulqdq + avx2 + sse4.2, 122 GiB RAM), Linux 6.8.0-55-generic x86_64,
rustc 1.95.0 (59807616e 2026-04-14), go1.25.1 linux/amd64 (zero third-party Go
modules), Python 3.12.3, z3-solver reporting z3.get_version_string() = 5.4.1.

5.2 Exact Wire Byte Decomposition (PFSH02 vs. 149,130 B/tx W-1 Ceiling)
--------------------------------------------------------------------------------
Component                          | Expected (B) | Worst-Case (B) | Share
-----------------------------------+--------------+----------------+-------
0-Round Stealth Header PFSH02      |        3,648 |          3,648 |  2.6%
  (incl. v_mint, v_unshield, dest, |              |                |
   2 x 1,670 B ML-KEM-1024 envs)   |              |                |
Proof Fixed Fields (magic, root,   |       59,916 |         59,916 | 42.7%
  SC1[19], SC2[8], RS[32], v_prox  |              |                |
  [565], v_open[565], nonce[48])   |              |                |
Opened Columns (52 x 748 B)        |       38,896 |         38,896 | 27.7%
Pruned 48 B MT_1 Merkle Siblings   |       36,001 |         38,016 | 27.1%
  (exp 750.02 / wc 792 x 48 B)     |              |                |
-----------------------------------+--------------+----------------+-------
TOTAL WIRE SIZE                    |      138,461 |        140,476 | 100%
W-1 Archival Ceiling (12 GiB/day)  |      149,130 |        149,130 |
Deterministic Headroom             |      +10,669 |         +8,654 | PASS
--------------------------------------------------------------------------------
Quantization note: Proof sizes vary strictly in 48-byte increments (one 48 B
sibling hash); expected 750.02 siblings = 138,461 B quantizes to integer 751
siblings = 138,508 B (matching the measured 50-trial median 138,508 B and both
tx_mint.bin and tx_hop1.bin). Measured 50-trial range: 137,500 B min / 138,508 B
med / 139,180 B max. End-to-end fixtures: tx_mint.bin = 138,508 B, tx_hop1.bin
= 138,508 B, tx_dummy_1in.bin = 137,788 B (736 siblings), tx_hop2.bin =
138,268 B (746 siblings).

5.3 Measured Warm, Cold-Process, Worst-Case-Sibling, & Memory Benchmarks
--------------------------------------------------------------------------------
Metric (Pinned taskset -c 0 for 1C)        | Median    | p95       | Max       | Peak RSS
-------------------------------------------+-----------+-----------+-----------+----------
Rust 16C/32T Prove (50t, streaming ANTT)   | 2,892 ms  | 3,057 ms  | 3,073 ms  | 561.7 MiB
Rust 1-Core Prove (5t, streaming ANTT)     | 26,731 ms | 30,362 ms | 30,362 ms | 561.7 MiB
Rust 1C Warm Full Settle (50t, PCLMUL)     | 22.53 ms  | 25.15 ms  | 31.95 ms  | 4.50 MiB
Go 1.25.1 1C Warm Settle (50t, PCLMULQDQ)  | 36.32 ms  | 38.78 ms  | 40.40 ms  | 5.25 MiB
Go 1.25.1 1C Warm Settle (10t, purego)     | 71.50 ms  | 73.17 ms  | 73.17 ms  | —
Rust 1C Cold-Process (10 procs, verify)    | 25.40 ms  | —         | 39.29 ms  | 4.50 MiB
Rust 1C Cold-Process (10 procs, full wall) | 29.98 ms  | —         | 44.98 ms  | 4.50 MiB
Go 1C Cold-Process (10 procs, verify)      | 38.45 ms  | —         | 40.77 ms  | 5.25 MiB
Go 1C Cold-Process (10 procs, full wall)   | 44.79 ms  | —         | 47.06 ms  | 5.25 MiB
--------------------------------------------------------------------------------
Worst-case-sibling & scheduling note: Grinding PoW to maximize unpruned MT_1
siblings from 750 expected to the deterministic worst case of 792 adds strictly
+42 single-block SHA-512 calls (< 0.02 ms total). Unpinned cold runs inside
verify.sh immediately after cargo build showed OS scheduling variance up to
47.9 ms Rust / 62.0 ms Go; pinned validator-core execution stays <= 47.06 ms
even including full cold OS process startup, ledger JSON parsing, Depth-24 root
recomputation, and K-table initialization.

5.4 Offline Wallet Scanning Throughput & Detection-Tag Tradeoff
--------------------------------------------------------------------------------
Measured 1-core Go 1.25.1 stdlib crypto/mlkem + SHA-384 + AES-256-GCM trial
decapsulation on tx_hop1.bin (5,000 trials, pinned taskset -c 0):
  - Non-matching output (env1): 211.77 us/output (4,722 outputs/s = 2,361 tx/s)
  - Matching output     (env0): 211.21 us/output (4,735 outputs/s = 2,368 tx/s)
At sustained 1 tx/s (172,800 outputs/day), an offline wallet catching up on 24
hours of blocks scans all outputs in ~36.6 s on 1 core (~2.3 s on 16 cores)
without detection tags. Higher-throughput deployments or mobile light clients
would benefit from optional view tags or oblivious message retrieval (OMR).

5.5 Public Repository, Public GitHub Actions CI Replay, & 7/7 Manifest
--------------------------------------------------------------------------------
All source code, pinned Cargo.lock, zero-dependency go.mod, Dockerfile,
SHA256SUMS, fixtures, scripts, and CI workflow are publicly published in
postfiatl1v2:
  * Code tree: https://github.com/postfiatorg/postfiatl1v2/tree/d76e5a8c61ec9741ffd6da59b00b7aee8ca0bb73/tools/pft-binius-shielded-pq
  * CI workflow: .github/workflows/peel-binius-shielded-pq-ci.yml (commit bd05b2f7a)
  * Public third-party CI run (ubuntu-latest, AMD EPYC 7763 4 vCPU, fresh
    rustc 1.95.0, go 1.25.1, pip z3-solver 5.4.1):
    https://github.com/postfiatorg/postfiatl1v2/actions/runs/44943485068
    (Job 136965585163, conclusion: success, 2m14s total):
      - sha256sum -c SHA256SUMS: 19/19 OK
      - ./verify.sh: ALL 7/7 INDEPENDENT CHECKS PASSED (exit 0)
      - Rust 1C verify on all 4 fixtures: 26.30-26.40 ms
      - Rust 1C cold-process wall (10 procs): 30.02 ms med / 30.70 ms max
      - Go 1.25.1 1C warm settle (50t, PCLMULQDQ): 40.37 ms med / 41.53 ms p95 / 43.68 ms max
      - Go 1.25.1 1C cold-process wall (10 procs): 47.76 ms med / 48.34 ms max
      - Go 1.25.1 1C purego fallback (10t): 90.58 ms med / 91.89 ms max

Running sha256sum -c SHA256SUMS && ./verify.sh inside BOTH the local clean
postfiatl1v2 worktree and the public GitHub Actions runner passed all 7/7
checks (exit code 0):
  [1/7] Rust unit tests (SHAKE256 == sha3::Shake256, Depth-24 Merkle, E384
        1,000-pair PCLMULQDQ == portable, streaming 1,024-pt ANTT == Novel
        Basis, 3D-Karatsuba eval_bilinear_rs_ip == naive 384x384 sum, 4-mode
        56-perm circuit zero-sum + MLE [2-in, dummy, ZK-mint, unshield], Binius).
  [2/7] Rust release verification of tx_mint, tx_hop1, tx_dummy_1in, tx_hop2.
  [3/7] Rust adversarial suite: 35/35 named controls rejected + 150/150 fuzz.
  [4/7] Go 1.25.1 stdlib verifier (zero external deps): 10,000-pair PCLMULQDQ
        == portable + 1,000-pair ringSwitchApplyASM == portable + 3D-Karatsuba
        evalBilinearRSIP == naive 384x384 sum + 35/35 Go adversarial controls +
        all 4 lifecycle scenarios.
  [5/7] Go 1.25.1 stdlib FIPS-203 ML-KEM-1024 offline stealth scan of
        tx_hop1.bin recovering v=650, R_78B (cleanly rejecting output 1).
  [6/7] check_math.py exact-rational audit (tower irreducibility, 58/58 G_1 +
        17/17 SC2 Jacobians, RBR, 512b-state QROM, wire).
  [7/7] verify_circuit_equivalence.py (Z3 5.4.1 UNSAT x 4 incl. 66-bit
        mint/unshield conservation, 1,600-basis L_TRP, 56-perm 2,150,400-bit
        DAG coverage, 3-way Rust==Go==Python 42,444 Group-C IR, Rust==Go Group
        A+B+C weights).

Verified SHA-256 Manifest (tools/pft-binius-shielded-pq/SHA256SUMS):
  8fb7568d4cfc41af0158b1a7e5ce1ebbe8165f9e3022913dcc9ab82fd17860be  payments/tx_mint.bin
  a3a72c12af7fa43b17e36a1027342bc7296f9c342c4b32733741345d39528c4e  payments/tx_hop1.bin
  f38eb78aa05e93848dcb49e3e3deb33ea4cd20557f45b5eb0c107b798dbb3359  payments/tx_dummy_1in.bin
  dc1426ddab028ec87d1491cd3c7ad4ee8d2d309ffb9169f0c20cb04305f81050  payments/tx_hop2.bin
  1bd92fcd1b3f47f31d4ed09a8be90ed85438d7564391b5529c89792861d58b7c  payments/ledger_mint.json
  8b6823f65d2da5b6f5f5f3f2ee621867e258a030d3f2c7e56d60507c685272e6  payments/ledger_hop1.json
  779ba807a60a77e21f9748b5654461f4b0fcd47276aebb5fbee9329a814a6667  payments/ledger_dummy_1in.json
  47647e22ee2be0f4a5a1c5cc9ccb335d569f91e9aab306d750f639c9652f26c4  payments/ledger_hop2.json
  79e2c636a6eb9589e6b0a00ece578c8784bb713c022ed66f5fd83d3efea7402c  payments/bob_wallet.json
  8f692d951c2c1ab0c822546f4672426c4b87d7f61ffbe858fce5f8a8d57a0b1f  check_math.py
  5ac626722a2912559e44cf6d7eef6f4691f6ae4895cd06d662c926d4483e1cc2  verify_circuit_equivalence.py
  91e0e63caf9be908ef07a63853b795cf95140ec49a31c1573aa336973ba7fdf0  verify.sh
  7ace3085a427b6f557fe315e8e7f0555e2eed2fc0cbc1978b8b94672b0a9dd5c  math_bounds.json
  82fcacfe87df8a01f30488878b6fa5c2feb84d712ace5056abcf08d0b0cc77b4  circuit_equivalence.json
  15ffb76bfec7150a0e12a14733b982af8e56c9e0564b3d09d9c48a7409cc41db  bench_results.json
  ed0a9cbde19e62a6574b831663b6a4b93369dc8049096f640180595c7bc36598  rust_1core_bench.json
  77c13a826d86ab2c06f6848a5eb8dfb6379c637466ad929c6019bb486dd38f0c  go_bench.log
  57eb020213b6225fefe1eb4919c8bf1f6bc267840b4194d2192f42159cc73faf  go_purego_bench.log
  8fb1777a98b3ebbc4f00db6d3dc8125cd8187a2a00a23a57a245cf00afa08211  cold_bench.log

================================================================================
APPENDIX A. COMPLETE SELF-CONTAINED PROOFS: BCIKS20 + PEEL(M), 34-ROUND
            STATE_KS, STANDALONE THEOREM A.3 (DFMS22a), & MULTI-PROOF SE
================================================================================

A.1 Verbatim Imported BCIKS20 Theorem 6.2 & Self-Contained Lemmas 1-4
--------------------------------------------------------------------------------
Imported Anchor — BCIKS20 Theorem 6.2 (Ben-Sasson, Carmon, Ishai, Kopparty,
Saraf, FOCS 2020 / JACM 2023, ePrint 2020/654, Correlated Agreement for
Parameterized Curves up to the Johnson Bound):
  Hypotheses: Let F_q be a finite field, let D_eval subset F_q be ANY nonempty
  evaluation domain of size n' = |D_eval|, let V = RS[F_q, D_eval, rho'] be the
  Reed-Solomon code of dimension k' = rho' * n' (degree < k'), let u_0, u_1,
  ..., u_l : D_eval -> F_q be l + 1 functions, let integer m' >= 3, and let
  proximity parameter delta' satisfy:
    0 < delta' <= 1 - (1 + 1/(2*m')) * sqrt(rho').
  Conclusion: If
    Pr_{z <-$ F_q}[ Delta_{D_eval}(u_0 + z*u_1 + ... + z^l * u_l, V) <= delta' ]
      > l * ((m' + 1/2)^7 / (3 * (rho')^{3/2})) * (n')^2 / q,
  then there exist codewords v_0, ..., v_l in V and a subset S subset D_eval of
  size |S| >= (1 - delta') * n' such that u_i(x) == v_i(x) for all x in S and
  all i in {0, ..., l}.

Lemma 1 (Greedy Peeling Analysis Peel(M) & Exact 4-Line Telescoping for ALL n' >= 0).
Fix any (possibly partial) Round-1 oracle M: L -> (K^{163} x E^2) cup {bot},
and let L_0 := supp(M) = {x in L : M(x) != bot} (size n_0 := |L_0| <= n). Fix
alpha := sqrt(2), m' := 3, D = 565, n = 2,097,152, rho = D/n = 0.000269413.
Define P(M) information-theoretically (never executed by Ext_IOP) via Peel(M):
  Initialize L_0 := supp(M), P := empty, i := 0.
  While |L_i| > 2*D and there exists u in RS[E, L, D]^{165} whose remaining
  agreement set D^(i)(u) := {x in L_i : M(x) = u(x)} satisfies
  |D^(i)(u)| >= alpha * sqrt(D * |L_i|):
    Add the lexicographically first such u_{i+1} to P;
    Set L_{i+1} := L_i \ D^(i)(u_{i+1}); increment i.
  Halt with P(M) := P (size k := |P|) and L_rem := L_k (size n' := |L_rem|).

Then:
  (i)  k = |P(M)| <= L_max := floor(sqrt(2 / rho)) = 86.
  (ii) For uniform lp in E, define F_{lp}(M)(x) := B_prox(x) + lp*B_pcs(x) +
       sum_{r=0}^{162} lp^{r+2} M_r(x) for x in L_0, and Bad_peel(lp):
       exists v in RS[E,L,D] with |{x in L_0 : v(x) = F_{lp}(M)(x)}| >=
       sqrt(2*rho) * n and v not in {F_{lp}(u) : u in P(M)}. Then:
         Pr_{lp in E}[Bad_peel(lp)] <= eps_curve :=
           164 * ((3.5)^7 / (3 * rho^{3/2})) * n^2 / |E| = 2^-305.79.

Proof:
  (i) Let x_i := |L_i|. In every iteration i, x_i > 2D > 0, so d_i := x_i -
      x_{i+1} >= sqrt(2*D*x_i) > 2D > 0 (strictly decreasing). Since
      x_{i+1} < x_i, sqrt(x_i) + sqrt(x_{i+1}) < 2*sqrt(x_i), giving
        sqrt(x_i) - sqrt(x_{i+1}) = d_i / (sqrt(x_i) + sqrt(x_{i+1}))
                                  > sqrt(2*D*x_i) / (2*sqrt(x_i)) = sqrt(D/2).
      Summing i = 0..k-1 yields sqrt(n_0) - sqrt(n') >= k * sqrt(D/2) (strict
      when k >= 1), hence for ALL k >= 0 and ALL n' >= 0:
        k * D <= sqrt(2*D) * (sqrt(n_0) - sqrt(n')),
      so k <= floor(sqrt(2*n_0/D)) <= floor(sqrt(2/rho)) = 86.
  (ii) Fix any v in RS[E, L, D] not in {F_{lp}(u) : u in P(M)}, and set A :=
       {x in L_0 : v(x) = F_{lp}(M)(x)}. On each peeled slice D^(j-1)(u_j),
       v != F_{lp}(u_j) implies |A cap D^(j-1)(u_j)| <= D - 1, so:
         |A cap (L_0 \ L_rem)| <= k * (D - 1) <= k * D
                               <= sqrt(2*D) * (sqrt(n_0) - sqrt(n'))
       (strictly < whenever k >= 1). Partition ALL n' >= 0 into two cases:
       - Case 1 (0 <= n' <= 2*D, explicitly including n' = 0):
         Since (n')^2 <= 2*D*n', we have n' <= sqrt(2*D*n'). If k = 0, then
         n_0 = n' <= 2D = 1,130 < 48,680 = sqrt(2*rho)*n, so |A| < sqrt(2*rho)*n.
         If k >= 1, adding the strict peeled bound and |A cap L_rem| <= n' <=
         sqrt(2*D*n') gives |A| < sqrt(2*D)*(sqrt(n_0) - sqrt(n')) +
         sqrt(2*D*n') = sqrt(2*D*n_0) <= sqrt(2*rho)*n. Thus Pr[Bad_peel] = 0.
       - Case 2 (n' > 2*D):
         M is completely defined (no bot entries) on D_eval := L_rem of size
         n' > 2D, with rate rho' := D / n' < 1/2 strictly. Define E_rem(lp):
         exists v in RS[E, L, D] with |A cap L_rem| >= sqrt(2*D*n') =
         sqrt(2*rho')*n'. Checking ALL verbatim hypotheses of BCIKS20 Thm 6.2
         on (F_q = E, D_eval = L_rem, k' = D, rho' = D/n', l = 164, m' = 3):
           * D_eval = L_rem nonempty subset of E: satisfied (n' > 2D).
           * m' = 3 >= 3: satisfied.
           * delta' := 1 - sqrt(2*rho'): since rho' < 1/2, delta' > 0 strictly;
             since sqrt(2) = 1.4142 > 7/6 = 1 + 1/(2*3), delta' < 1 - (7/6)*
             sqrt(rho') strictly.
         By BCIKS20 Thm 6.2 on M|_{L_rem}, if Pr_{lp}[E_rem(lp)] > eps_curve(3,
         rho', n', 164), there exists u* in RS[E, L_rem, D]^{165} (uniquely
         extending to RS[E, L, D]^{165} since n' > 2D > D) agreeing with M on
         >= sqrt(2*D*n') points of L_rem, contradicting that Peel(M) halted
         while |L_rem| > 2D. Since (n')^2 / (rho')^{3/2} = (n')^{7/2}/D^{3/2}
         <= n^{7/2}/D^{3/2} = n^2 / rho^{3/2}, eps_curve(3, rho', n', 164) <=
         eps_curve = 2^-305.79. On neg(E_rem(lp)), |A cap L_rem| < sqrt(2*D*n'),
         so |A| < sqrt(2*D*n_0) <= sqrt(2*rho)*n. []

Lemma 2 (Pointwise Inclusion A_prox subset D_u & Dual-Polynomial Binding).
For each u in P(M), let D_u := {x in supp(M) : M(x) = u(x)}. Define
Bad_pt(lp): exists u in P(M) and x in supp(M) \ D_u with F_{lp}(M(x)) = F_{lp}(u(x)).
  (i)  Pr_{lp in E}[Bad_pt(lp)] <= L_max * n * 164 / |E| = 2^-349.22.
  (ii) Whenever neg(Bad_peel(lp)) and neg(Bad_pt(lp)), for any (v_prox, v_open)
       in RS[E,L,D]^2 whose joint agreement set A_both := {x in supp(M) :
       v_prox(x) = F_{lp}(M(x)) and v_open(x) = B_pcs(x) + lo*sum_r c_r M_r(x)}
       satisfies |A_both| >= sqrt(2*rho)*n (= 48,680 > D = 565):
         - v_prox == F_{lp}(u) for a unique u in P(M);
         - A_both subset A_prox := {x in supp(M) : v_prox(x) = F_{lp}(M(x))} subset D_u;
         - v_open == b_pcs(u) + lo * sum_{r=0}^{162} c_r u_r identically in E[X]_{<D}.
Proof: (i) For each fixed u in P(M) and x in supp(M) \ D_u, M(x) != u(x), so
F_Z(M(x)) - F_Z(u(x)) is a nonzero degree-<=164 polynomial in Z; union bound
over <= L_max * n pairs gives <= 86 * 2^21 * 164 / 2^384 = 2^-349.22.
(ii) Since |A_prox| >= |A_both| >= 48,680 and neg(Bad_peel), v_prox == F_{lp}(u)
for some u in P(M). For every x in A_prox, F_{lp}(M(x)) = v_prox(x) =
F_{lp}(u(x)); by neg(Bad_pt), x in D_u, giving A_both subset A_prox subset D_u.
Uniqueness of u: any distinct u' in P(M) agrees with u on <= D - 1 = 564 <
48,680 points. On all x in A_both subset D_u, M(x) = u(x), so v_open agrees
with b_pcs(u) + lo*sum_r c_r u_r on >= 48,680 > 565 points, forcing polynomial
identity in E[X]_{<D}. []

Lemma 3 (Subfield-K_32 Inheritance & Structural Booleanity).
Every u = (b_prox, b_pcs, u_0..u_{162}) in P(M) satisfies u_r(X) in K_32[X]_{<D}
for r = 0..162, hence u_r(H) subset K_32 = GF(2^32). Each K_32 element unpacks
uniquely into 32 GF(2) witness bits under the fixed tower basis, structurally
enforcing booleanity of B_0..B_4 without quadratic b(1-b)=0 gates.
Proof: Peel(M) only peels while |L_i| > 2D, so every peeled slice satisfies
|D^(i)(u)| >= sqrt(2D * 2D) = 2D > D points of supp(M) subset L subset K_32
where M_r(x) in K_32. Lagrange interpolation over any D such points yields
u_r in K_32[X]_{<D}. []

Lemma 4 (Single-Challenge Ring-Switching Algebra & Soundness).
Let {beta_b := 2^b}_{b=0..31} be the GF(2)-basis of K_32 and {e_{v,u} :=
Y^v * 2^u}_{v=0..11, u=0..31} be the 384-element GF(2)-basis of E = GF(2^384).
For any u in P(M), define L_{rho_rs}(z) := sum_{j=0}^{383} bit_j(z) * rho_rs^j
and eta_{1,2,3} := rho_rs^{384,385,386}.
  (i)  Transposition identity: By GF(2)-linearity of L_{rho_rs} over active
       witness rows (0..139; trailing zero subrows 28..31 contribute 0) and
       bit-transposed mask rows (140..144), L_{rho_rs}(sum_b eq(r_1[14..18], b)
       * s_comb_true[b](u)) == <U^(u)|_H, M_RS> identically.
  (ii) Soundness: If eval_z[x] != w_x(u) + tau*m_x(u) for some x in 0..4, then
       sum_{x=0}^4 gamma^x * (eval_z[x] - w_x(u) - tau*m_x(u)) is a nonzero
       univariate polynomial of degree <= 4 in gamma, vanishing with prob <=
       4 / |E|. Conditioned on nonzero difference, the Round-23 check forces
       s'_comb[b*] != s_comb_true[b*](u) for some b*, making the transposed
       difference Delta_hat_{v,u} != 0 for at least one j* = 32v* + u* in
       0..383. Then (RS_target - RS_target_true(u)) + eta_1*(eval_zg - g1_m(u))
       + eta_2*(S_G1 - S_G1^(u)) + eta_3*(S_mask2 - S_mask2^(u)) is a nonzero
       univariate polynomial of degree <= 386 in rho_rs (since rho_rs^0..
       rho_rs^{386} are pairwise distinct powers), vanishing with prob <=
       386 / |E|. []

A.2 Complete Pure-Algebraic 34-Round State_KS (No Sticky Zero-Challenge Flags)
--------------------------------------------------------------------------------
Fix statement x and any (possibly partial) Round-1 oracle M: L -> (K^{163} x
E^2) cup {bot}. For each u = (b_prox^(u), b_pcs^(u), u_0..u_{162}) in P(M),
define B_0^(u)..B_4^(u), G_1^(u), m_g1^(u), m_g2^(u), M_RS^(u), M_SC2^(u),
S_G1^(u), S_mask2^(u), and P_bad(M) := {u in P(M) : B^(u) violates at least one
of the N_C = 2,192,844 scalar constraints C_0..C_{N_C-1} of R_pay(x)} (|P_bad(M)|
<= 86).

Deterministic verifier-reject gate: Let ValidPrefix_k(tr_k) == 1 iff all
verifier checks evaluable on tr_k pass—specifically:
  - for k >= 3+d (d in 0..18): r_1[d] != 0;
  - for k >= 22: tau != 0;
  - for k >= 23: Round-22 algebraic check A(tau) == rhs_deg2(eval_z, eval_zg);
  - for k >= 24: rho_rs != 0 AND Round-23 check sum_b eq(r_1[14..18], b) *
    rs_s_prime[b] == sum_{x=0}^4 gamma^x * eval_z[x].
Notice that whenever ValidPrefix_k(tr_k) == 0, V(x, tr_34) == 0 deterministically
regardless of later messages; therefore we define State_KS(x, tr_k) := 0
whenever ValidPrefix_k(tr_k) == 0 (a rejected prefix can never cause extraction
failure). Because zero challenges r_1[d] == 0, tau == 0, rho_rs == 0 cause
ValidPrefix_k == 0 immediately, NO zero-challenge disjunct ever appears inside
Doomed_k(u; tr_k).

When ValidPrefix_k(tr_k) == 1, we define the pure algebraic discrepancy
predicate Doomed_k(u; tr_k) in {0, 1} for each u in P_bad(M) and set
State_KS(x, tr_k) := 1 iff exists u in P_bad(M) with Doomed_k(u; tr_k) == 0
(for k = 1..32; extended to 33a, 33b, 34 below):
  - Round 1 (STMT; tr_1 = (M, S_G1, S_mask2)):
    Set Doomed_1(u; tr_1) := 1 for all u in P_bad(M); hence State_KS(x, tr_1) := 0.
  - Round 2 (C02 -> alpha in E):
    Set claim1_0 := pub_rhs(x, alpha) + S_G1, and let F_0^(u)(0) + F_0^(u)(1)
    := sum_{c=0}^{N_C-1} alpha^c * eval_c(B^(u)) + S_G1^(u). Define:
      Doomed_2(u; tr_2) := 1 iff (claim1_0 != F_0^(u)(0) + F_0^(u)(1))
                                 OR (S_G1 != S_G1^(u)).
    Transition bound: If S_G1 != S_G1^(u), then Doomed_2(u) == 1 with prob 1.
    If S_G1 == S_G1^(u), then claim1_0 - (F_0^(u)(0) + F_0^(u)(1)) ==
    Q^circ_u(alpha) := sum_{c=0}^{N_C-1} alpha^c * (rhs_c(x) - eval_c(B^(u))).
    Because u in P_bad(M) violates at least one constraint c*, Q^circ_u(Z) is a
    nonzero univariate polynomial of degree <= N_C - 1 = 2,192,843, so
    Pr_alpha[Doomed_2(u) = 0] <= 2,192,843 / |E|. Union over <= 86 gives
    eps_rbr_ks(2) <= 86 * 2,192,843 / |E| = 2^-356.51.
  - Rounds 3..21 (Sumcheck 1: C03..C21 -> r_1[d] in E for d = 0..18):
    Let F_d^(u)(X) in E[X]_{<= 3} be the honest Round-(3+d) Sumcheck-1
    polynomial for u at r_1[0..d-1], and P_d(X) in E[X]_{<= 3} the prover's
    message with P_d(0) + P_d(1) == claim1_d and claim1_{d+1} := P_d(r_1[d]).
    Define:
      Doomed_{3+d}(u; tr_{3+d}) := 1 iff (claim1_{d+1} != F_d^(u)(r_1[d]))
                                         OR (S_G1 != S_G1^(u)).
    Exact conditioning: Suppose Doomed_{2+d}(u) == 1.
      * Case (i) S_G1 != S_G1^(u): persists identically -> Doomed_{3+d}(u) = 1
        with probability 1.
      * Case (ii) S_G1 == S_G1^(u): then necessarily claim1_d != F_d^(u)(0) +
        F_d^(u)(1) (for d=0 from Doomed_2; for d>=1 because F_{d-1}^(u)(r_1[d-1])
        == F_d^(u)(0) + F_d^(u)(1)), forcing P_d(X) != F_d^(u)(X) as degree-<=3
        polynomials -> Pr_{r_1[d]}[claim1_{d+1} == F_d^(u)(r_1[d])] <= 3/|E|.
    Union over <= 86 gives eps_rbr_ks(3+d) <= 86 * 3 / |E| = 2^-375.99.
  - Round 22 (Deg-2 Hiding: C22 -> tau in E, after absorbing eval_c1, eval_c2):
    Let A(T) := claim1_{19} + T*eval_c1 + T^2*eval_c2 and B_u(T) :=
    G_1^(u)(r_1) + T*m_g1^(u) + T^2*m_g2^(u) + sum_{x=0}^4 [wb_x(r_1)*(w_x^(u)
    + T*m_x^(u)) + wc_x(r_1)*(1 + w_{x+1}^(u) + T*m_{x+1}^(u))*(w_{x+2}^(u) +
    T*m_{x+2}^(u))]. Define:
      Doomed_22(u; tr_22) := 1 iff (A(tau) != B_u(tau)) OR (S_G1 != S_G1^(u)).
    Exact conditioning: Suppose Doomed_21(u) == 1.
      * Case (i) S_G1 != S_G1^(u): persists identically -> prob 1.
      * Case (ii) S_G1 == S_G1^(u): then necessarily A(0) = claim1_{19} !=
        F_{18}^(u)(r_1[18]) == B_u(0), so A(T) - B_u(T) is a nonzero deg-<=2
        polynomial -> Pr_tau[A(tau) == B_u(tau)] <= 2/|E|.
    Union over <= 86 gives eps_rbr_ks(22) <= 86 * 2 / |E| = 2^-376.57.
  - Round 23 (Ring-Switch Batch: C23 -> gamma in E, after absorbing eval_z[0..4],
    eval_zg):
    Set z_true^(u)[x] := w_x^(u) + tau*m_x^(u) and zg_true^(u) := G_1^(u)(r_1)
    + tau*m_g1^(u) + tau^2*m_g2^(u). Define:
      Doomed_23(u; tr_23) := 1 iff
        (sum_{x=0}^4 gamma^x*(eval_z[x] - z_true^(u)[x]) != 0)
        OR (eval_zg != zg_true^(u)) OR (S_G1 != S_G1^(u)).
    Exact conditioning (on ValidPrefix_23 == 1, so A(tau) == rhs_deg2):
    Suppose Doomed_22(u) == 1. If S_G1 != S_G1^(u), stays 1 with prob 1; else
    A(tau) != B_u(tau) together with A(tau) == rhs_deg2 forces (eval_z,
    eval_zg) != (z_true^(u), zg_true^(u)). If eval_zg != zg_true^(u), stays 1
    with prob 1; else some eval_z[x] != z_true^(u)[x], making the deg-<=4
    polynomial in gamma nonzero -> Pr_gamma <= 4/|E|. Union gives 86*4/|E|.
  - Round 24 (Ring-Switch Linearization: C24 -> rho_rs in E, after absorbing
    rs_s_prime[0..31]):
    Let claim2_0 := RS_target(rs_s_prime, rho_rs) + rho_rs^{384}*eval_zg +
    rho_rs^{385}*S_G1 + rho_rs^{386}*S_mask2, and claim2_true^(u) the analogous
    value for u. Define:
      Doomed_24(u; tr_24) := 1 iff (claim2_0 != claim2_true^(u)).
    Exact conditioning (on ValidPrefix_24 == 1): Suppose Doomed_23(u) == 1.
    Then AT LEAST ONE of the three disjuncts holds:
      (a) sum_x gamma^x*(eval_z[x] - z_true^(u)[x]) != 0 -> by ValidPrefix_24
          and Lemma 4(ii), at least one coefficient of rho_rs^j (j in 0..383)
          in claim2_0 - claim2_true^(u) is nonzero; OR
      (b) eval_zg != zg_true^(u) -> coefficient of rho_rs^{384} is nonzero; OR
      (c) S_G1 != S_G1^(u) -> coefficient of rho_rs^{385} is nonzero!
    In ALL three cases (discharging S_G1 != S_G1^(u) cleanly with zero lost
    invariant), claim2_0 - claim2_true^(u) is a NONZERO univariate polynomial
    of degree <= 386 in rho_rs -> Pr_{rho_rs}[Doomed_24(u) = 0] <= 386/|E|.
    Union over <= 86 gives eps_rbr_ks(24) <= 86 * 386 / |E| = 2^-368.98.
  - Rounds 25..32 (Sumcheck 2: C25..C32 -> r'_row[d] in E for d = 0..7):
    Define Doomed_{25+d}(u; tr_{25+d}) := 1 iff (claim2_{d+1} !=
    F_{2,d}^(u)(r'_row[d])). Conditioned on Doomed_{24+d}(u) == 1, claim2_d !=
    F_{2,d}^(u)(0) + F_{2,d}^(u)(1) forces P_{2,d}(X) != F_{2,d}^(u)(X) as
    deg-<=2 polynomials -> Pr <= 2/|E|. Union gives 86*2/|E| per round. At
    d = 7, F_{2,7}^(u)(r'_row[7]) == e*(u) := <u_comb(r'_row)|_H, m_col>.
  - Round 33a (C33P -> lambda_p): State_KS(x, tr_33a) := 1 iff ValidPrefix_32
    == 1 AND (State_KS(32) == 1 OR Bad_peel(lp) OR Bad_pt(lp)) -> Pr <=
    eps_curve + eps_pt = 2^-305.79.
  - Round 33b (C33O -> lambda_o): State_KS(x, tr_33b) := 1 iff ValidPrefix_32
    == 1 AND (State_KS(33a) == 1 OR exists u in P_bad(M) with b_eval^(u) +
    lo*e*(u) == b_eval + lo*claim2_8). Conditioned on State_KS(33a) == 0, every
    u in P_bad(M) has e*(u) != claim2_8 (from Doomed_32(u) = 1), so Pr <= 86/|E|.
  - Round 34 (H_C4 -> 52 indices): State_KS(x, tr_34) := 1 iff ValidPrefix_32
    == 1 AND (State_KS(33b) == 1 OR (|A_both| < sqrt(2*rho)*n AND all 52 drawn
    indices land in A_both)) -> Pr <= (sqrt(2*rho))^52 = 2^-282.31 (with 20-bit
    PoW: p_34 = 2^-302.31).

Terminal Extraction Proof (Including Partial-Oracle Case supp(M) != L):
Suppose State_KS(x, tr_34) == 0 AND V(x, tr_34) == 1. Since V == 1,
ValidPrefix_32 == 1 holds, so State_KS(34) == 0 forces |A_both| >= 48,680 > 565
and neg(Bad_peel) and neg(Bad_pt). By Lemma 2(ii), unique u* in P(M) satisfies
v_prox == F_{lp}(u*), A_both subset A_prox subset D_{u*} subset supp(M), and
v_open == b_pcs^(u*) + lo * sum_r c_r u*_r identically in E[X]_{<D}. The
verifier's H-domain check gives b_eval^(u*) + lo*e*(u*) == b_eval + lo*claim2_8,
so State_KS(33b) == 0 forces u* in P(M) \ P_bad(M), encoding valid w* in
R_pay(x). Ext_IOP interpolates the 140 witness rows over K_32 on the first 565
points of A_prox subset D_{u*} in O(min(n, |supp(M)|) + 140*565^2) time and
returns w* in R_pay(x) with probability 1. []

A.3 Standalone Formal Theorem A.3 (First-Principles Operator-Norm Extraction Proof)
--------------------------------------------------------------------------------
We present the self-contained compressed-oracle bridge (still tagged [Unrev] in
Table 1 pending external peer review) directly from first principles on
Zhandry's compressed-oracle unitary [Zha19] in the transition-capacity
formalism of CFHL21 (EUROCRYPT 2021) and DFMS22a (EUROCRYPT 2022), deriving
every constant (sqrt(8) < sqrt(10), 2/|Y| readout error, block-diagonal
superposition reduction, and both entry and exit/uncompute bounds) without
unverified black-box steps.

Compressed-Oracle Unitary & Phase-Basis Block Decomposition:
Let H: X -> Y = {0,1}^{1536} (N := |Y| = 2^{1536}) be the single fixed-range
random oracle. In Zhandry's compressed-oracle representation [Zha19], the
oracle state lies in the tensor product over x in X of registers D(x) with
orthonormal basis {|bot>} cup {|u> : u in Y}; a classical database D is a
finite-support partial function X -> Y cup {bot} (size |D| <= s denoted
D in SZ_{<= s}). Applying the Hadamard transform on the adversary's response
register places the oracle query unitary O in the phase basis |x, z, D>
(z in Y), where O commutes with |x>, |z>, and all registers D_{-x} := {D(x') :
x' != x}:
  O = sum_{x in X, z in Y, D_{-x}} |x, z, D_{-x}><x, z, D_{-x}| (x) O_{x,z},
with O_{x,0} = I and, for every z != 0, O_{x,z} := ST_x Z_z ST_x acting ONLY
on the single register D(x). Here:
  * |phi> := N^{-1/2} sum_{u in Y} |u>,
  * |psi_z> := Z_z |phi> = N^{-1/2} sum_{u in Y} (-1)^{<z,u>} |u> (<phi|psi_z>=0),
  * Z_z |bot> = |bot>, Z_z |u> = (-1)^{<z,u>} |u>,
  * ST_x := I - (|bot> - |phi>)(<bot| - <phi|) swaps |bot> <-> |phi> and acts
    as identity on span{|bot>, |phi>}^perp.

Lemma A.3.0 (Exact Rank-3 Self-Adjoint Operator & Both-Directions Norm Bound).
For every z != 0, O_{x,z} satisfies the exact self-adjoint identity:
  O_{x,z} = Z_z + (|psi_z> - |bot>)<bot| + (|phi> - |psi_z>)<phi|
            + (|bot> - |phi>)<psi_z|,   O_{x,z}^\dagger = O_{x,z}.
Consequently, for EVERY subset S subset Y NOT containing bot (p := |S|/|Y|),
with projector P_S on S and complement projector P_{not S} = I - P_S on
(Y \ S) cup {bot}, BOTH entry and exit/uncompute norms satisfy identically:
  || P_S O_{x,z} P_{not S} ||_2 = || P_{not S} O_{x,z} P_S ||_2
                                <= 2*sqrt(2)*sqrt(p) = sqrt(8*p) < sqrt(10*p).
Proof: Verify O_{x,z} = ST_x Z_z ST_x directly on the orthonormal decomposition
span{|bot>, |phi>, |psi_z>} (+) perp:
  - O_{x,z}|bot>   = ST_x Z_z |phi>   = ST_x |psi_z> = |psi_z>  (vs Z_z|bot> = |bot>);
  - O_{x,z}|phi>   = ST_x Z_z |bot>   = ST_x |bot>   = |phi>    (vs Z_z|phi> = |psi_z>);
  - O_{x,z}|psi_z> = ST_x Z_z |psi_z> = ST_x |phi>   = |bot>    (vs Z_z|psi_z> = |phi>);
  - on span{|bot>, |phi>, |psi_z>}^perp: ST_x = I, so O_{x,z} = Z_z.
Subtracting Z_z yields the exact rank-3 formula, which is manifestly self-
adjoint (since ST_x = ST_x^\dagger and Z_z = Z_z^\dagger). Now multiply by P_S
on the left and P_{not S} on the right: Z_z is diagonal in {|bot>} cup {|u>},
so P_S Z_z P_{not S} = 0; since bot not in S, P_S |bot> = 0. Collecting the
remaining terms gives:
  P_S O_{x,z} P_{not S}
    = (P_S |psi_z>)(<bot| - <phi|)P_{not S}
      + (P_S |phi>)(<phi| - <psi_z>)P_{not S}.
By || |a><b| ||_2 = ||a||_2 * ||b||_2, ||P_S |phi>||_2 = ||P_S |psi_z>||_2 =
sqrt(|S|/N) = sqrt(p), and || |bot> - |phi> ||_2 = || |phi> - |psi_z> ||_2 =
sqrt(2) (since P_{not S} is a contraction), the triangle inequality gives
|| P_S O_{x,z} P_{not S} ||_2 <= 2*sqrt(2)*sqrt(p) = sqrt(8*p) < sqrt(10*p).
Self-adjointness yields || P_{not S} O_{x,z} P_S ||_2 = || (P_S O_{x,z} P_{not S})^\dagger ||_2
= || P_S O_{x,z} P_{not S} ||_2 <= sqrt(8*p) < sqrt(10*p). (Retaining sqrt(10*p)
in check_math.py is strictly conservative and aligns with CFHL21 Thm 5.17.) []

Formal Database Predicates & Projectors on Unordered Databases D:
  - R(D) := {root in {0,1}^384 : exists "PFS-STMT" input in Dom(D) embedding root};
  - C(D) := {h in {0,1}^384 : exists "PFS-NODE" input in Dom(D) embedding h as
             left or right child};
  - S(D) := R(D) cup C(D) (each entry contributes <= 1 root or <= 2 children,
            so |S(D)| <= 2*|D|);
  - MRoot^{-1}_D(root): the partial Round-1 oracle M: L -> (K^{163} x E^2) cup
    {bot} obtained by walking down uniquely matched "PFS-NODE" and "PFS-LEAF"
    preimages in D from root (returning bot at any leaf lacking a unique valid
    height-21 path in D);
  - CL(D): holds iff D contains (i) distinct x != x' in Dom(D) whose 384-bit
    MT_1 outputs H[0..48] collide (for LEAF/NODE), OR (ii) a LEAF/NODE entry
    with H[0..48] == 0^48, OR (iii) distinct x != x' whose 512-bit internal
    states collide (H[0..64] for STMT/ABS; H[64..128] for CH);
  - SUC(D) := union_{k=2}^{34} S_k(D), where S_k(D) holds iff D contains a
    2k-step directed chain (x_STMT, x_CH2, ..., x_CHk) linked by exact 512-bit
    state equality (out_state(x_j) == in_state(x_{j+1})) such that, for
    M := MRoot^{-1}_D(root(x_STMT)), State_KS(x, tr_{k-1}; M) == 0 and
    State_KS(x, tr_k; M) == 1;
  - P' := CL cup SUC, P_s := SZ_{<= s} \ P', with diagonal database projectors
    Pi_{P'} := sum_{D in P'} |D><D| and Pi_{P_s} := sum_{D in P_s} |D><D|.

Lemma A.3.1 (Tree-Invariance & Retroactive Peel(M) Stability).
For any D in SZ_{<= s}, any LEAF or NODE input x with D(x) = bot, and any u in
{0,1}^{1536} with u[0..48] not in S(D):
  MRoot^{-1}_{D[x->u]}(root) == MRoot^{-1}_D(root)  for EVERY root in R(D).
Consequently, Peel(MRoot^{-1}_{D[x->u]}(root)) == Peel(MRoot^{-1}_D(root))
identically for every root in R(D) whenever u[0..48] not in S(D) (and
Pr_{u <-$ Y}[u[0..48] in S(D)] = |S(D)|/2^384 <= 2s/2^384).
Proof: Walking down from any root in R(D) queries only digests belonging to
S(D) = R(D) cup C(D). Since u[0..48] not in S(D), the new entry (x, u) cannot
match root or any reachable child digest, leaving MRoot^{-1}_D(root) and its
deterministic function Peel(M) bit-for-bit identical. []

Lemma A.3.2 (Block-Diagonal Superposition Reduction & Chain Uniqueness).
  (a) Because O commutes with |x, z, D_{-x}> and Pi_{P'}, Pi_{P_s} are diagonal
      in D, the quantum transition capacity across arbitrary superposed query
      inputs x and superposed databases D reduces POINTWISE to classical pairs:
        [[P_s -> P']] := || Pi_{P'} O Pi_{P_s} ||_2
                      <= sup_{x in X, z != 0, |D_{-x}| <= s}
                           || Pi_{P'}^{x, D_{-x}} O_{x,z} Pi_{not P'}^{x, D_{-x}} ||_2,
      where Pi_{P'}^{x, D_{-x}} projects onto {v in Y cup {bot} : D_{-x}[x->v] in P'}.
  (b) CL(D) and SUC(D) are pure set-properties of unordered D. Whenever
      D_{-x} not in CL, every x in X (STMT, ABS, CH, or C4) has AT MOST ONE
      directed ancestor chain in D_{-x} back to a unique STMT entry and unique
      partial oracle M = MRoot^{-1}_{D_{-x}}(root), regardless of how many
      interleaved partial chains coexist in D_{-x}.
Proof: (a) Direct-sum decomposition of operator norm over orthogonal subspaces
indexed by (x, z, D_{-x}) (for z = 0, O_{x,0} = I and Pi_{P'} I Pi_{P_s} = 0).
(b) Under neg(CL(D_{-x})), every 512-bit state and 384-bit MT_1 digest has at
most one preimage in Dom(D_{-x}), so predecessor pointers are unique. []

Lemma A.3.3 (Exhaustive Both-Directions Capacity for Every Classical (x, D_{-x})).
Fix ANY classical (x, z != 0, D_{-x}) with |D_{-x}| <= s (where D_{-x}(x) = bot).
Exactly ONE of the following two mutually exclusive cases holds:

  * Direction A — Exit / Uncompute / Overwrite (D_{-x} in P', so bot in P'^{x,D_{-x}}):
    Here P'^{x, D_{-x}} contains bot, so its complement on D(x) is strictly a
    subset G_- := {u in Y : D_{-x}[x->u] not in P'} subset Y (bot not in G_-).
    By Lemma A.3.0 (self-adjoint exit bound):
      || Pi_{P'}^{x, D_{-x}} O_{x,z} Pi_{not P'}^{x, D_{-x}} ||_2
        = || P_{not G_-} O_{x,z} P_{G_-} ||_2 <= sqrt(8 * |G_-| / |Y|).
    Bounding |G_-|/|Y| (even with arbitrarily many coexisting SUC witness chains):
      - If D_{-x} in CL: CL is monotone upward -> D_{-x}[x->u] in CL subset P'
        for ALL u in Y -> G_- = empty (|G_-| = 0).
      - If D_{-x} in SUC \ CL: fix ANY ONE witness chain (root*, X_chain) in
        D_{-x} (x not in X_chain since D_{-x}(x) = bot).
        (i)  x in {STMT, ABS, CH, C4}: adding x->u leaves X_chain and
             MRoot^{-1}_{D_{-x}}(root*) bit-for-bit identical (and any collision
             places D_{-x}[x->u] in CL subset P') -> G_- = empty (|G_-| = 0).
        (ii) x in {LEAF, NODE}: leaving P' requires destroying EVERY coexisting
             SUC chain in D_{-x}, which in particular requires changing
             MRoot^{-1}_{D_{-x}}(root*); by Lemma A.3.1 this occurs ONLY if
             u[0..48] in S(D_{-x}) -> |G_-|/|Y| <= 2s / 2^384.

  * Direction B — Entry / Overwrite (D_{-x} not in P', so bot not in P'^{x,D_{-x}}):
    Here D_{-x} has ZERO collisions and ZERO SUC chains, and P'^{x, D_{-x}} is
    strictly G_+ := {u in Y : D_{-x}[x->u] in P'} subset Y (bot not in G_+).
    By Lemma A.3.0:
      || Pi_{P'}^{x, D_{-x}} O_{x,z} Pi_{not P'}^{x, D_{-x}} ||_2
        = || P_{G_+} O_{x,z} P_{not G_+} ||_2 <= sqrt(8 * |G_+| / |Y|).
    Bounding |G_+|/|Y| by x's fixed domain tag:
      - x in {LEAF, NODE}: D_{-x}[x->u] enters CL ONLY if u[0..48] hits <= s
        existing digests or 0^48 (<= (s+1)/2^384), and enters SUC ONLY if
        MRoot^{-1}_{D_{-x}}(root) changes for some root in R(D_{-x}), requiring
        u[0..48] in S(D_{-x}) by Lemma A.3.1 (<= 2s/2^384) ->
        |G_+|/|Y| <= (3s + 1) / 2^384 <= 3(s + 1) / 2^384.
      - x in {STMT, ABS}: enters CL or links forward into an existing state in
        D_{-x} ONLY if 512-bit u[0..64] hits <= 2s states -> <= 2s / 2^512.
      - x in {CH, C4}: By Lemma A.3.2(b), since D_{-x} not in CL, the SINGLE
        classical input x determines AT MOST ONE domain-tagged round
        k in {2..34}, ONE unique prefix chain (root, tr_{k-1}) in D_{-x}, and
        ONE fixed partial oracle M = MRoot^{-1}_{D_{-x}}(root) BEFORE u is
        sampled:
        (i)  CH round k in {2..33b}: u[0..48] (chall_k) and u[64..128]
             (next_state) are disjoint independent uniform slices of u in Y.
             If next_state neither collides with <= s existing output states
             nor matches <= s existing input states (prob <= 2s/2^512), then
             (x, u) completes ONLY the single Round-k transcript tr_k and cannot
             complete any longer chain k' > k. Hence it enters SUC ONLY if
             chall_k flips State_KS(k-1; M)=0 to State_KS(k; M)=1 (prob <=
             eps_rbr_ks(k) <= eps_rbr_ks(33a) = 2^-305.79) ->
             |G_+|/|Y| <= eps_rbr_ks(33a) + 2s / 2^512.
        (ii) C4 (Round 34): enters SUC ONLY if x's unique prefix chain has
             State_KS(33b; M)=0 and |A_both| < sqrt(2*rho)*n, AND u satisfies
             the 20-bit PoW (2^-20) and 52 independent 21-bit indices in A_both
             -> |G_+|/|Y| <= p_34 = 2^-302.31.

Combining Directions A & B over all (x, z != 0, |D_{-x}| <= s) via
sqrt(a + b) <= sqrt(a) + sqrt(b) and sqrt(8) < sqrt(10) gives:
  [[P_s -> P']] <= sqrt(30*(s + 1)) * 2^-192
                   + sqrt(10 * max(eps_rbr(33a) + 2s/2^512, p_34)). []

Lemma A.3.4 (Telescoping Induction & First-Principles Populated-Database Readout).
Run A (Q queries) followed by classical verification V on L output proofs
(<= L*kappa_v queries; total Q_v = Q + L*kappa_v queries to compressed oracle O).
  (a) Telescoping: Since |psi_0> has D = empty not in P' (||Pi_{P'}|psi_0>||_2
      = 0) and adversary unitaries U_q commute with Pi_{P'}, each step q = 1..Q_v
      satisfies ||Pi_{P'} O |psi_{q-1}>||_2 <= ||Pi_{P'}|psi_{q-1}>||_2 +
      [[P_{q-1} -> P']]. Summing via sum_{q=1}^{Q_v} sqrt(q) <= (2/3)*(Q_v+1)^{3/2}
      (using 3.652 > (2/3)*sqrt(30) = 3.65148...) and squaring yields:
        Pr[D_end in P'] = || Pi_{P'} |psi_{Q_v}> ||_2^2
          <= [ 3.652 * Q_v^{3/2} * 2^-192
               + Q_v * sqrt(10 * max(eps_rbr(33a) + 2*Q_v/2^512, p_34)) ]^2.
  (b) Exact Readout Proof: Whenever classical V queries x_i inside O and
      measures response y_i in Y (N = 2^{1536}), in the decompressed basis
      register D(x_i) collapses strictly to |y_i> (reachable states have zero
      |bot> support in the decompressed basis). Recompressing via ST_{x_i}
      produces:
        ST_{x_i} |y_i> = (1 - 1/N)|y_i> + N^{-1/2}|bot> - N^{-1} sum_{u != y_i} |u>,
      so Pr[D_end(x_i) != y_i] = 1 - (1 - 1/N)^2 = 2/N - 1/N^2 < 2/2^{1536}
      (< 2/2^384). Union bound over all <= L*kappa_v verifier queries gives
      Pr[exists i : D_end(x_i) != y_i] <= 2*L*kappa_v/2^{1536} < 2*L*kappa_v/2^384.
  (c) Completing Extraction: On the event {all L proofs accepted} cap
      {D_end(x_i) == y_i for all i} cap {D_end not in CL cup SUC}, for each
      proof j in {1..L}: (i) every opened leaf and height-21 Merkle path of
      pi_j is present in D_end with matching digests, so under neg(CL) walking
      down from root_j in D_end uniquely recovers all 52 opened columns inside
      M_end^{(j)} := MRoot^{-1}_{D_end}(root_j); (ii) every STMT/ABS/CH/C4
      challenge of pi_j is present in D_end; (iii) since State_KS(1; M_end^{(j)})
      == 0 and D_end not in SUC, induction on k = 2..34 forces
      State_KS(34; M_end^{(j)}) == 0. By Appendix A.2's Terminal Extraction
      Proof, V(x_j, tr_34) == 1 and State_KS(34; M_end^{(j)}) == 0 guarantee
      that Ext_IOP(x_j, M_end^{(j)}, tr_34) outputs w*_j in R_pay(x_j) with
      probability 1. Combining (a) and (b) yields Theorem 2A.

Why This Avoids DFMS20's (Q+1)^{2*mu} Multi-Round Blowup:
General multi-round Fiat-Shamir (DFMS20) has no Round-1 witness commitment and
must rewind/measure-and-reprogram across all mu rounds (incurring (Q+1)^{2*mu}
loss). In BCS compilation, M = MRoot^{-1}_D(root) is committed in Round 1 and
extracted straight-line from D_end with zero rewinding; Lemma A.3.2(a) reduces
superposed queries pointwise to classical (x, D_{-x}), where each query targets
at most one domain-tagged round k, so [[P_s -> P']] takes max_k eps_rbr_ks(k)
rather than a product over rounds. (Still tagged [Unrev] in Table 1 pending
independent external peer review.) []

A.4 Proof of Theorem 2B (Adaptive Multi-Proof QROM SE Wrapper B^H — Unreviewed)
--------------------------------------------------------------------------------
Pre-sample T_aux = (T_tr, T_leaf, T_qidx) independently of H before A starts:
|T_tr| <= 68*P in {0,1}^{512} (512-bit simulated transcript states), |T_leaf| <=
P*(n-1) in {0,1}^{384} (unopened simulated leaf digests), and |T_qidx| <= P
subsets of size <= 52. Define monotone event E_3(D): D contains a STMT/ABS/CH
query whose 512-bit state hits T_tr, or a LEAF query whose 384-bit digest hits
T_leaf, or a C4 query whose 52 indices lie inside some Q_{idx,i} in T_qidx. By
CFHL21 Lem 5.6 + DFMS22a:
  Pr[D in E_3] <= 10 * Q_eff^2 * P * (68/2^512 + (n - 1)/2^384 + (t/n)^t).

Construct wrapper B^H interacting solely with unmodified compressed oracle H:
  1. On Sim_BCS call i in 1..P: run Sim_IOP(x_i) with i-th pre-sampled tape,
     hash the <= 52 opened columns via unmodified H("PFS-LEAF"||..), place
     pre-sampled T_leaf digests on the <= n - 1 unopened leaves, compute all
     n - 1 internal nodes via unmodified H("PFS-NODE"||..), and store the 68
     programmed STMT/ABS/CH/C4 entries in local classical overlay T_prog
     (placing ZERO LEAF/NODE entries in T_prog).
  2. On A's quantum query |x>|y> to H': controlled check x in Dom(T_prog) ->
     XOR T_prog[x] locally without querying H; else forward to unmodified H.
  3. View equivalence (GHHM21 Thm 1): B^H's overlay matches real Sim_BCS
     reprogrammed oracle H' except for pre-call query weight on Dom(T_prog)
     <= (3/2) * P * sqrt(Q_eff) * (68 / 2^256).
  4. Exhaustive Cases A/B/C for any fresh accepted pair (x*, pi*) not in
     {(x_i, pi_i)}_{i=1}^P (under neg(CL), neg(SUC), neg(E_3), and H == D):
       - Case A ((x*, root*, s_g1*, s_mask2*) != all simulated i — covering
         EVERY mauled statement x* != x_i): STMT misses Dom(T_prog) and runs on
         unmodified H, producing a 512-bit st_0 not in T_tr (by neg(E_3)). By
         induction across all 68 calls, every ABS/CH/C4 input avoids Dom(T_prog)
         and runs on unmodified H -> V_BCS^H(x*, pi*) == 1 -> Ext_IOP extracts
         w* by Theorem 2A.
       - Case B (same Round-1 tuple (x*, root*, s_g1*, s_mask2*) as some
         simulated proof i, but at least one later message m_r* or pow_nonce*
         differs): At the first differing index r*, the next ABS/C4 input
         (tag || state_{r*-1} || m_{r*}*) differs from proof i's entry in
         Dom(T_prog) and from every other simulated proof j != i's entry (up to
         512-bit pre-sampled state birthday collision <= (68*P)^2/2^513 <<
         2^-430). Thus it executes on unmodified H, producing a 512-bit state
         outside T_tr (by neg(E_3)). By induction through H_C4, the 52 drawn
         indices are NOT a subset of Q_{idx,i} (by neg(E_3)), forcing V_BCS to
         verify at least one leaf j* not in Q_{idx,i} against root_i entirely
         on unmodified H -> contradicts neg(CL) or neg(E_3).
       - Case C (all statement, transcript, and pow_nonce fields match proof i,
         only opened columns/siblings differ): direct LEAF/NODE collision in
         unmodified H, contradicting neg(CL).
     Hence all L fresh accepted pairs extract valid witnesses w*_j in R_pay(x*_j). []

================================================================================
APPENDIX B. COMPLETE HVZK SIMULATOR SIM_IOP, CAPPED PoW, & QROM ZK PROOF
================================================================================

B.1 Complete Written Simulator Sim_IOP(x) (Works on True OR False Statements x)
--------------------------------------------------------------------------------
Given ANY public statement x (without any witness w) and uniform challenges
(alpha, r_1[0..18], tau, gamma, rho_rs, r'_row[0..7], lp, lo, j_1..j_52):
Explicit Exceptional-Challenge Abort Event E_exc: If any r_1[d] == 0 (prob <=
19/2^384), tau == 0 (<= 1/2^384), rho_rs == 0 (<= 1/2^384), or the 17x17 minor
of the Sumcheck-2 Jacobian vanishes at r'_row (total deg <= 112, full rank 17
mechanically verified at concrete K_32^8 point in check_math.py -> Schwartz-
Zippel prob <= 112/2^384), Sim_IOP outputs bot. Total Pr[E_exc] <= 133/2^384.
Conditioned on neg(E_exc) (every inverse below is strictly well-defined):
  1. Sumcheck 1 (CFS17 Mask G_1, V_SC1 = 19, rank 58/58 verified): Sample
     uniform S_G1 and sc1_rounds[0..18] in (E^3)^19, setting c_{d,1} =
     claim1_d + c_{d,2} + c_{d,3} (starting from claim1_0 = pub_rhs(x, alpha) +
     S_G1). Triangular 58x58 submatrix det = prod_{k=0}^{17} r_1[k]^{3*(18-k)}
     != 0 on neg(E_exc).
  2. Degree-2 Evaluation Hiding (tau != 0 on neg(E_exc)): Sample uniform
     eval_z[0..4], eval_zg, eval_c2 in E^7 and solve uniquely for eval_c1 =
     tau^-1 * (rhs_deg2 - claim1_19 - tau^2 * eval_c2) using
     eval_verifier_weights_at_r1(x, alpha, r_1).
  3. Compressed Ring-Switching: Pick any b* with eq(r_1[14..18], b*) != 0
     (always exists since sum_b eq(r, b) = 1 != 0); sample uniform
     rs_s_prime[b] (b != b*) and solve uniquely for rs_s_prime[b*] so
     sum_b eq(r_1[14..18], b)*rs_s_prime[b] == sum_x gamma^x * eval_z[x].
  4. Sumcheck 2 (16-Row Full-Rank Masking scaled by eta_3 = rho_rs^386 != 0):
     Sample uniform S_mask2 and sc2_rounds[0..7] (17 E-elements). Full rank 17
     of the 17x32 Jacobian on neg(E_exc) matches the real distribution exactly.
  5. Dual PCS & Opened Columns: Sample uniform b_eval in E, uniform opened
     columns at the <= 52 queried points L[j_q], uniform v_prox in E[X]_{<565}
     matching those <= 52 prox evaluations, and uniform v_open in E[X]_{<565}
     matching those <= 52 open evaluations plus <v_open|_H, m_col> == b_eval +
     lo*claim2 (exact match since 53 > 52 vanishing-on-H randomizers + uniform
     B_prox, B_pcs).
Total statistical distance on valid (x, w) is <= Pr[E_exc] <= 133/2^384 =
2^-376.94; on false statements x, Sim_IOP(x) outputs a verifier-accepting
transcript whenever neg(E_exc) (prob >= 1 - 133/2^384). []

B.2 Proofs of Lemma 1.2, 512-Bit Reprogramming Conditioning, & Theorem 1B
--------------------------------------------------------------------------------
Proof of Lemma 1.2: Couple Real_capped with Real_unbounded by sharing the first
M_pow = 2^27 i.i.d. uniform draws u_1..u_{M_pow} in {0,1}^384: they agree
whenever at least one draw lands in S_pass (|S_pass| = A ~ Bin(N, q), N=2^384,
q=2^-20). Birthday collision among M_pow draws has prob <= M_pow^2/(2N) = 2^-331.
Conditioned on no collision, Pr[all M_pow miss S_pass] <= (1-q)^{M_pow} <=
e^-128 = 2^-184.66. Comparing Real_unbounded vs. Ideal gives TV <=
(1/2)*E[|A/(Nq) - 1|] + (1-q)^N <= 1/(2*sqrt(Nq)) + e^{-Nq} <= 2^-183.0.
Triangle inequality yields TV(Real_capped, Ideal) <= 2^-182.60. []

Sequential Product-State Quantum Reprogramming Conditioning (GHHM21 Thm 1):
Rather than assuming predecessor locations were classically "unqueried by
induction," we replace the 68 programmed calls of each proof i in {1..P} via
68 sequential single-point hybrids H_{i,0} -> H_{i,1} -> ... -> H_{i,68}
(performed before proof i is returned to A, so A's pre-proof-i quantum state
|psi_{pre-i}> is completely independent of proof i's fresh simulator randomness):
  - Step r = 1 (H_{i,0} -> H_{i,1}, PFS-STMT): z_{i,1} embeds fresh uniform
    (s_G1, s_mask2) <-$ E^2 (768 bits), sampled in product with |psi_{pre-i}>
    and H -> p_max(z_{i,1} | view before proof i) <= 2^-768 <= 2^-512.
    Reprogramming z_{i,1} sets its output state st_{i,1} to a FRESH independent
    uniform 512-bit string in product with |psi_{pre-i}> and H.
  - Step r in {2..67} (H_{i,r-1} -> H_{i,r}, PFS-ABS / PFS-CH): In H_{i,r-1},
    z_{i,r-1} was ALREADY reprogrammed to output a fresh uniform 512-bit state
    st_{i,r-1} independent of H and in product with |psi_{pre-i}> (proof i has
    not yet been returned to A). Because z_{i,r} embeds st_{i,r-1}, every
    string x has Pr[z_{i,r} = x | A's prior view] <= 2^-512 directly (with ZERO
    assumption that A had zero quantum query amplitude on z_{i,r-1}!). Applying
    GHHM21 Theorem 1 (single-point adaptive reprogramming on Q quantum queries)
    bounds |Pr[H_{i,r-1}] - Pr[H_{i,r}]| <= (3/2)*sqrt(Q)*2^-256, and sets
    st_{i,r} freshly uniform for step r + 1.
  - Step r = 68 (H_{i,67} -> H_{i,68}, PFS-C4): In H_{i,67}, 512-bit tr_final
    (= st_{i,67}) is freshly uniform in product with |psi_{pre-i}>; swapping
    the M_pow = 2^27 capped PoW draw for uniform 384-bit nonce* (Lemma 1.2:
    TV <= 2^-182.60) gives z_{i,68} = ("PFS-C4  " || tr_final || nonce*) with
    p_max(z_{i,68}) <= 2^-896 <= 2^-512, adding (3/2)*sqrt(Q)*2^-256.

Proof of Theorem 1B (4-Stage QROM ZK across P <= 2^32 proofs):
  - Stage 1 (67-Point STMT/ABS/CH + 1-Point H_C4 = 68-Point Reprogramming +
    Capped PoW Swap): Executing H_{i,0} -> H_{i,68} across all i = 1..P proofs
    yields exact total reprogramming + PoW loss:
      <= P * 2^-182.60 + (3/2) * P * sqrt(Q) * (67 + 1) * 2^-256
      =  P * 2^-182.60 + (3/2) * P * sqrt(Q) * 68 * 2^-256.
  - Stage 2 (Independent Punctured-Oracle Leaf O2H): Apply Lemma 1.1 + AHU19
    Thm 3 across P proofs -> loss <= 4*P*Q*sqrt(n-1)/2^2608 + P*C(n,2)/2^768.
  - Stage 3 (Algebraic HVZK Replacement): Replace real algebraic view with
    Sim_IOP(x_i) (Appendix B.1) -> loss <= P * 133 / 2^384.
Summing Stages 1-3 yields Adv_QROM-ZK(P=2^32, Q) <= 2^-150.60 at Q = 2^64,
2^80, and 2^96. []

================================================================================
APPENDIX C. PUBLIC INDEPENDENT REPLAY COMMANDS (GIT & DOCKER)
================================================================================
Option 1 (Direct clone at pinned commit):
  git clone --branch feat/peel-binius-shielded-pq \
    https://github.com/postfiatorg/postfiatl1v2.git
  cd postfiatl1v2
  git checkout d76e5a8c61ec9741ffd6da59b00b7aee8ca0bb73
  cd tools/pft-binius-shielded-pq
  sha256sum -c SHA256SUMS
  ./verify.sh

Option 2 (Pinned container with Rust 1.95.0, Go 1.25.1, z3-solver):
  cd tools/pft-binius-shielded-pq
  docker build -t pft-binius-shielded-pq .
  docker run --rm pft-binius-shielded-pq
================================================================================

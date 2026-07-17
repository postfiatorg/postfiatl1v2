# Vault-Bridge Deposit-Credit Evidence Spec

Status: Stage 0 spec — what Stage 2 builds against. Author: Snaga/Burzum. Date: 2026-07-05.

Governing plan: `orc_directives/NAVCOIN-BRIDGE-VERIFICATION-BUILD-PLAN.md` (Rev 2), Stage 0.
Stage 2 gate this spec is measured against:
`orc_directives/bridge_verification_stage2_GATE-CRITERIA-DRAFT.md`.

This document is descriptive of code that already exists. It invents no field names.
Source of truth:
- Struct + derivations + validation:
  `crates/types/src/lib_parts/ledger_assets_parts/part_01.rs:591` (`VaultBridgeDepositEvidence`).
- Constants: `crates/types/src/lib_parts/core_chain.rs`.
- Admission: `crates/execution/src/lib_parts/nft_escrow_asset_state_parts/part_02.rs`
  (`apply_vault_bridge_deposit_propose`, `apply_vault_bridge_deposit_finalize`,
  `apply_vault_bridge_deposit_claim`).

> **Deviations from the dispatch brief (declared, code-verified).**
> 1. `source_domain` prefix is `erc20_bridge_vault`, **not** `evm_bridge`
>    (`VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX = "erc20_bridge_vault"`, core_chain.rs:102).
> 2. `source_tx_or_attestation` prefix is `erc20_bridge_deposit`, **not** `evm_deposit`
>    (`VAULT_BRIDGE_DEPOSIT_SOURCE_TX_PREFIX = "erc20_bridge_deposit"`, core_chain.rs:103).
> 3. The bytes32 fields (`block_hash`, `tx_hash`, `nonce`, `deposit_id`,
>    `pftl_recipient_hash`) are stored on-ledger as **64 lowercase hex chars with NO
>    `0x` prefix** (`validate_lower_hex_len(.., 64)`), not "0x-prefixed". Only the EVM
>    *address* fields carry `0x`. The `0x…`-prefixed hash form seen in the gate pack's
>    `source-finality-audit-*.json` is the off-chain RPC/display form, not the
>    on-ledger canonical form.
> `finality_ref` and `vault_id` prefixes in the brief were correct.

---

## 1. Field schema

Canonical struct (`part_01.rs:591`), field order preserved:

```rust
pub struct VaultBridgeDepositEvidence {
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub depositor: String,
    pub pftl_recipient: String,
    pub pftl_recipient_hash: String,
    pub amount_atoms: u64,
    pub nonce: String,
    pub deposit_id: String,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
}
```

Format constraints are enforced by `VaultBridgeDepositEvidence::validate()` (part_01.rs)
and by `vault_bridge_deposit_id()`. Address rule: `validate_evm_address_text` — length
42, `0x`-prefixed, lowercase hex, non-zero address. Bytes32 rule: `validate_lower_hex_len(.., 64)`
— exactly 64 lowercase hex chars, no `0x`.

| Field | Canonical type | Format constraint (enforced) | Example (gate pack, deposit `4b425fcb…`) |
| --- | --- | --- | --- |
| `source_chain_id` | `u64` | nonzero decimal (`source_chain_id == 0` → reject) | `42161` |
| `vault_address` | `String` | 0x-prefixed lowercase 20-byte hex, 42 chars, non-zero | `0x6a700337663d7c4143e26a3a172077415d90e7d7` |
| `token_address` | `String` | 0x-prefixed lowercase 20-byte hex, 42 chars, non-zero | `0xaf88d065e77c8cc2239327c5edb3a432268e5831` |
| `depositor` | `String` | 0x-prefixed lowercase 20-byte hex, 42 chars, non-zero | `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0` |
| `pftl_recipient` | `String` | PFTL account-id text (`validate_text_field`); credited account | `pf323cfe884291b17844024b43ac44962c468b51b4` |
| `pftl_recipient_hash` | `String` | 64-char lowercase hex (bytes32), no 0x; MUST equal `keccak256(pftl_recipient bytes)` lower-hex | `keccak256("pf323…")` |
| `amount_atoms` | `u64` | must be `> 0` (`amount_atoms == 0` → reject); 6-dp USDC atoms (`VAULT_BRIDGE_UNIT = 1_000_000`) | `100000000` (100 pfUSDC) |
| `nonce` | `String` | 64-char lowercase hex (bytes32), no 0x | `…` |
| `deposit_id` | `String` | 64-char lowercase hex (bytes32), no 0x; MUST equal `vault_bridge_deposit_id(evidence)` | `4b425fcb159e132a1b19cba8d9d8318cbe0154a8e3e302fb9f27f545da266506` |
| `block_hash` | `String` | 64-char lowercase hex (bytes32), no 0x | `298760125bf64a0785ccb00ea5e3fec8a3a6fe3fa90e2eadd70de883cab7f74c` |
| `tx_hash` | `String` | 64-char lowercase hex (bytes32), no 0x | `0683aed92b4289086244d064766b96ce9e8a947528d8277f80df50812d4f63d4` |
| `log_index` | `u64` | decimal | `8` |

Two fields are *self-referential* and re-derived at validation, so they cannot be forged
independently of the rest of the evidence:
- `pftl_recipient_hash` must equal `vault_bridge_pftl_recipient_hash(pftl_recipient)` =
  `keccak256(pftl_recipient.as_bytes())` as lowercase hex.
- `deposit_id` must equal `vault_bridge_deposit_id(evidence)` = `keccak256` over the
  ABI-encoded preimage `(offset, source_chain_id, vault, token, depositor, amount_atoms,
  recipient_hash, nonce, "postfiat.erc20_bridge.deposit.v1")`. This binds the ERC20BridgeVault
  `depositId` preimage to the on-ledger row.

## 2. Derived fields (verbatim constructors, `part_01.rs`)

```rust
pub fn source_domain(&self) -> String {
    format!("{}:{}:{}:{}",
        VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX, // "erc20_bridge_vault"
        self.source_chain_id, self.vault_address, self.token_address)
}
pub fn source_asset_ref(&self) -> String {
    format!("erc20:{}:{}", self.source_chain_id, self.token_address)
}
pub fn source_tx_or_attestation(&self) -> String {
    format!("{}:{}",
        VAULT_BRIDGE_DEPOSIT_SOURCE_TX_PREFIX, // "erc20_bridge_deposit"
        self.deposit_id)
}
pub fn finality_ref(&self) -> String {
    format!("evm_log:{}:{}:{}:{}",
        self.source_chain_id, self.block_hash, self.tx_hash, self.log_index)
}
pub fn vault_id(&self) -> String {
    format!("evm:{}:{}:{}",
        self.source_chain_id, self.vault_address, self.token_address)
}
```

| Derived value | Canonical format | Gate-pack example |
| --- | --- | --- |
| `source_domain` | `erc20_bridge_vault:<chain_id>:<vault>:<token>` | `erc20_bridge_vault:42161:0x6a70…e7d7:0xaf88…5831` |
| `source_asset_ref` | `erc20:<chain_id>:<token>` | `erc20:42161:0xaf88…5831` |
| `source_tx_or_attestation` | `erc20_bridge_deposit:<deposit_id>` | `erc20_bridge_deposit:4b425fcb…` |
| `finality_ref` | `evm_log:<chain_id>:<block_hash>:<tx_hash>:<log_index>` | `evm_log:42161:298760125…:0683aed9…:8` |
| `vault_id` | `evm:<chain_id>:<vault>:<token>` | `evm:42161:0x6a70…e7d7:0xaf88…5831` |

`evidence_root` (not a struct field): `vault_bridge_deposit_evidence_root(evidence)` =
`hash_hex_domain(VAULT_BRIDGE_DEPOSIT_EVIDENCE_ROOT_DOMAIN, canonical_preimage)` — the 96-hex
content address admission recomputes and matches against the submitted `evidence_root`.
`canonical_preimage()` is the newline-delimited dump of all twelve fields plus the four
derived strings, so the root binds every field including `finality_ref`.

## 3. Per-tier admission requirements

Three tiers from the plan. Only **attested** exists in code today.

| Tier | Status | What it requires |
| --- | --- | --- |
| **attested** | **enforced today** | Issuer/operator-attested credit. Deterministic hygiene on the evidence + a signed attestation quorum (`min_attestations`, 1-of-1 on devnet). See §3.1. |
| **observed** | **Stage 2 — not yet enforced** | N-of-M registered, identity-bearing observers each independently attest: tx exists, receipt status `0x1`, log at `log_index` matches (`vault`, `token`, `amount`, `depositor`), `block_hash`, and `confirmation_depth ≥ K`. Exact-equality comparison (no tolerance band). See §3.2. |
| **proven** | **Stage 4 — not yet enforced** | Deposit carries an MPT receipt-inclusion proof under the block `receiptsRoot`; admission verifies it deterministically (keccak + RLP + Merkle-Patricia walk). Observer role shrinks to header attestation. See §3.3. |

### 3.1 attested tier — exactly what admission checks today

Two consensus ops gate a credit before mint; a third mints. All are pure functions of
`(transaction, ledger)` — no live I/O.

**`apply_vault_bridge_deposit_propose` (part_02.rs:877):**
1. Asset is a registered NAV asset; a `vault_bridge:<source_domain>` proof profile exists.
2. `ensure_vault_bridge_source_policy` — `source_domain()` equals the profile's
   `source_class` suffix, **and** `policy_hash` equals the profile `valuation_policy_hash`.
3. Expiry: `expires_at_height > block_height`.
4. Evidence-root recomputation: `vault_bridge_deposit_evidence_root(evidence)` must equal the
   submitted `evidence_root` (`vault_bridge_deposit_evidence_root_mismatch` otherwise).
5. `ensure_vault_bridge_deposit_source_proof` — source-proof-kind must match the profile
   verifier: `multi_fetch` profiles must carry **empty** SP1 fields; `sp1-groth16` profiles
   must carry a proof whose `source_public_values_hash` binds the exact evidence.
6. Duplicate evidence-root rejection: an existing deposit for `(asset_id, evidence_root)` →
   `duplicate_vault_bridge_deposit`.

**`apply_vault_bridge_deposit_finalize` (part_02.rs:1131):**
1. Record exists and is `pending`; not past `expires_at_height`.
2. Source policy + source proof re-checked (as propose).
3. Attestation mechanics by `profile.verifier_kind`:
   - `multi_fetch`: **zero failing attestations** (`fail_count == 0`) and
     `pass_count ≥ profile.min_attestations`; else
     `vault_bridge_deposit_failed_attestations_present` /
     `vault_bridge_deposit_attestation_quorum_not_met`.
   - `sp1-groth16`: proof already bound at propose/source-proof step.
4. Challenge window: `block_height ≥ submitted_at_height + challenge_window_blocks`.
5. Snapshot age: `block_height ≤ submitted_at_height + max_snapshot_age_blocks`.
6. Marks the record `finalized`.

**`apply_vault_bridge_deposit_claim` (part_02.rs:1265):** mints on the finalized record.
1. `ensure_vault_bridge_asset_policy` — operator is the NAV asset issuer or reserve operator;
   the issued asset's issuer matches the NAV asset issuer.
2. `recipient == evidence.pftl_recipient`; `amount_atoms == evidence.amount_atoms`.
3. Recipient must be a holder trustline, not the issuer (`unsupported_issuer_mint`).
4. **Mint:** `issued_supply.checked_add(claim_amount)` guarded by `issued_supply_overflow` /
   `issued_supply_cap_exceeded` (part_02.rs:1367). Receipt is reconstructed and every field
   (amount, `finality_ref`, `vault_id`, `policy_hash`, `bucket_id`, evidence) must match.

**What is NOT checked (the hole).** Admission recomputes `evidence_root` — a hash over the
*claimed* fields — and, for `multi_fetch`, counts attestation signatures. **Nothing verifies
`block_hash`, `tx_hash`, or `log_index` against a real Arbitrum chain.** A fabricated
`block_hash`/`tx_hash` yields a perfectly valid `evidence_root` and a perfectly valid attestor
signature. The chain cannot distinguish a real block from an invented one; it verified a
*signature*, not the *claim*. This is well-formed lying, not malformed input — Stage 1 hygiene
will not catch it.

Incident proof (W6, gate pack `source-finality-audit-after-run4.json`): deposit
`bed939f29493f25895b6b0c27dc0b6ed1d258f17d30bab6148d49028f025f641`, block
`0x88a720e1028d2720694246bcc74ff1a1951b574bd5f5b39d685804734495ec06`, `tx`
`0x906c0952…`, `log_index 28676`, 20000000 atoms (20 pfUSDC) — finalized at height 95 with a
single (1-of-1) pass attestation despite `evm_block_exists=false`, `evm_tx_receipt_exists=false`.

**Exit-side has the same gap.** Withdrawal/redemption settlement
(`apply_vault_bridge_redeem_settle`, part_02.rs:3622) records a withdrawal packet and settles
bucket accounting without any consensus verification that the Arbitrum withdrawal actually
executed. The exit side is *not* independently verified today; Stage 2 criterion 4 closes it
symmetrically (withdrawal execution observation-confirmed before bucket release accounting).
Do not claim the exit side is verified.

### 3.2 observed tier — Stage 2 target (not yet enforced)

Reuses the Phase-1 NavAttestor registry chassis
(`part_01.rs:399-448`). A credit becomes **consensus-invalid** unless it carries N-of-M
attestations from registered, identity-bearing observers, each independently attesting:
`tx exists`, `receipt status == 0x1`, log at `log_index` matches (`vault`, `token`, `amount`,
`depositor`), `block_hash`, and `confirmation_depth ≥ K`. Comparison is **exact equality** —
EVM facts are discrete, so there is no tolerance band. (Contrast: the NAV venue-equity lane
*does* use a tolerance band because venue NAV is a continuous quantity; bridge-deposit facts
are discrete and must match bit-for-bit.) The comparison rule is content-addressed in the
proof profile. `K` and the observer set are profile/ledger state, changed only by governance
(determinism rule — validators never do live I/O). Under this tier the h94 vector flips GREEN:
observers cannot attest a nonexistent block (Stage 2 criterion 1).

### 3.3 proven tier — Stage 4 target (not yet enforced)

Deposit evidence additionally carries an MPT inclusion proof of the receipt under the block's
`receiptsRoot`; admission verifies it deterministically in `crates/execution`. Observers can
then no longer lie about contents — only about header canonicality — which Stage 5 addresses.
Unlocks `quantity_evidence: "cryptographic"` for the EVM leg.

## 4. `confirmation_depth` field

**Not present on the current struct.** `VaultBridgeDepositEvidence` (part_01.rs:591) has no
`confirmation_depth` field today; Stage 1 does not add one.

**Stage 2 adds it** (Stage 2 gate criterion 3). Planned semantics — recorded here only to
reserve the slot and fix the direction so Stage 2 cannot contradict this spec:
- `confirmation_depth` = the count of confirmed source-chain blocks above the deposit's block,
  independently attested by observers.
- Admission requires `confirmation_depth ≥ K`, where `K` is profile/ledger state changed only
  by governance (determinism rule).
- Comparison is exact (no tolerance band), consistent with §3.2.

This spec deliberately does **not** fix the field's Rust type or a default `K` — Stage 2 owns
that choice.

## 5. Summary of the Stage-0 finding

The evidence schema, its derivations, and the attested-tier hygiene are already sound against
*malformed* input. They are structurally blind to *well-formed lies*: any consistent set of
`block_hash`/`tx_hash`/`log_index` passes, because consensus never touches Arbitrum. The
permanent negative vector is h94 (`bed939f2…`). Stage 2's observer quorum is the first tier
that makes the claim — not merely the signature — a validity rule.

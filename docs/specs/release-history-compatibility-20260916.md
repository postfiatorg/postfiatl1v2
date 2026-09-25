# PR41: preserve chain history and reproduce withdrawal proofs

Proposed repair specification, 16 September 2026. The PR41 implementer owns implementation and qualification under accepted Task Node task `task_0747d04c6cffb93874ae122c17faeae6`. The operator retains deployment and live-governance authority.

This extends the locked [FastPay recovery specification](fastpay-payment-recovery-v1.md) and [release qualification decision](../governance/signing-fix-deploy-decision-20260909.md). It preserves their original files. The three failures in [PR41](https://github.com/postfiatorg/postfiatl1v2/pull/41) have distinct causes:

| Failure | Cause | Repair |
|---|---|---|
| Historical FastPay records fail verification | Commit `0a1216c3` changed existing v1 committee-root and retained-certificate encodings. | Restore exact v1 bytes; introduce governed v2 commitments. |
| Replay fails the supply check at block 1011 | Ordinary archive replay drops Orchard balances supplied by live execution. | Pass identical supply inputs through the existing activated executor. |
| Withdrawal builds differ from their pins | CI compares mutable source with a historical Arc identity; the later pfETH ELF also embeds different build paths. | Reproduce each retained identity from its own immutable source and build environment. |

## 1. FastPay: preserve v1 and govern v2

### Exact encoding rules

`crates/types/src/fastpay_recovery_types.rs` at `0a1216c3^` defines the historical v1 encoder. Restore those bytes exactly. The committee's v1 root omits admission heights; its state encoding and signed governance payload already contain them. V1 reveal and fence commitments retain their historical digest fields. Apply and replay continue to verify complete certificate signatures.

Add schema `postfiat-fastpay-recovery-committee-v2`. The existing Rust record and governance envelope remain wire carriers; their explicit schema selects the encoding. Constructors create v2. Deserialization preserves the supplied schema. Unknown schemas fail before hashing; hash matching never chooses a version.

Let `T(s) = U64BE(byte_length(s)) || UTF8(s)`. Integers below use unsigned big-endian encoding. The v2 root preimage is:

```text
UTF8("postfiat.fastpay.recovery-committee.root.v2") || 0x00
|| T(chain_id) || T(genesis_hash) || U32BE(protocol_version)
|| U64BE(committee_epoch) || U64BE(valid_from_height)
|| U64BE(new_orders_through_height) || U64BE(validator_count)
|| for each validator in ascending validator_id order:
     T(validator_id) || T(algorithm_id) || T(public_key_hex)
```

The root is lowercase hex of SHA3-384 over the root domain, one zero byte, and that preimage. Thus both inner and outer domain framing match the existing hash helper. Preserve existing size, height, algorithm, key, ordering and quorum validation. V2 committee state bytes retain the v1 field order, with the v2 schema, root preimage, root and `postfiat.fastpay.recovery-committee.state.v2` domain.

For reveals and fences, preserve every historical field and its order. Replace the state-domain suffix `.v1` with `.v2`, then append `U64BE(certificate_byte_length) || certificate_bytes` when a certificate exists. A reveal always carries a certificate; a confirmed fence requires one; a cancelled fence forbids one. `FastPayCertificateV1::canonical_bytes()` remains the sole certificate encoder. It includes the signed order and canonical validator votes. No JSON serialization becomes a commitment encoder.

V1 application acknowledgements retain their historical fence preimage and terminal-state domain. Signed v3 orders, owner authorizations, certificate digests, cancellation, recovery windows and terminal object versions retain their existing definitions.

### Activation and compatibility

The existing signed, ordered bootstrap/rotation envelope authorizes installation. Its payload binds the schema and complete committee state bytes. Preserve distinct active-registry ML-DSA authorization, exact chain/genesis/protocol, unchanged policy, next epoch, contiguous future admission interval and atomic rejection. Retain historical committees. Reject a v1 successor after any installed v2 committee; reject an unknown schema or inconsistent committee ordering.

The first v2 committee's **ordered installation** activates v2 state commitments. The ledger immediately commits every retained reveal and fence using v2, including records from earlier epochs. Its future admission height separately controls new signing authority.

The governance executor applies the update atomically before computing the installing block's final state root. That root is the first certified v2 root. Subsequent transactions in the same ordered batch observe the installed committee list. Failed installation leaves the prior state intact. Replay, restart and snapshot loading derive the encoding from the same retained, committed committee list; no machine setting participates.

This release can verify the current v1 chain before live governance installs v2. Qualification exercises installation only on disposable copies. Before activation, rollback restores the exact old executable and pre-upgrade data. After a live v2 installation, reverting to v1 would discard finalized commitments; recovery instead requires a compatible corrected binary. Operational approval must account for that boundary.

### Required evidence

The retained height-1020 committee root must remain `b1ef4b05277587af46dc231391c1216f47e3f13acae7553791b4a15f95da99ea382b9d0e2ef36eb06566d9172f739517`. A separate synthetic vector independently reconstructed from the old encoder uses chain `fastpay-recovery-types`, genesis `11` repeated 48 bytes, protocol 3, epoch 7, heights 100–120, and validators `validator-0` through `validator-3`, each with algorithm `ML-DSA-65` and public-key text `aa` repeated 32 bytes. Its v1 root is `7994495faefce27215cfae8bbb4fb34fee43c5219da4fab2e693551f4b28fa6bcc9a197982b5f9cfdd1aa2da20399a3d`.

Tests must preserve the old vector, distinguish explicit versions, reject unknown versions and downgrade, detect changes to both v2 heights and retained signatures, and preserve old-certificate recovery. Compare roots across governed installation, including pre-existing fences, then replay and restart. Measure the resulting commitment size and verification time on the captured state; retain existing resource bounds. No historical root or expected signature changes to accommodate the repair.

## 2. Supply replay: use the live execution inputs

Block 1011 contains a vault-bridge deposit claim. Its stored state records a proof-bounded NAV checkpoint of 313700595. Replay produces 304700595 while global supply is 313700595.

In `crates/node/src/execution_actions.rs`, live transparent execution calls `execute_asset_transaction_with_compatibility_and_orchard`. The ordinary archive branch instead calls the wrapper that supplies an empty Orchard balance list, even though replay has already obtained those balances.

Pass the supplied balances through the same Orchard-aware executor. Preserve the governed activation and narrowly pinned legacy exceptions. The captured governance activation is height 906. Below activation, the executor retains the historical perimeter; at and above activation, both paths include transparent and shielded family supply. The existing claim transition in `crates/execution/src/nav_vault_asset_execution.rs` derives cap growth from finalized, unclaimed, route-bound SP1 backing. Keep its checked arithmetic, backing ceiling, receipt validation and supply checks intact.

The regression executes one signed claim through both paths and compares complete ledger state and receipts before, at and after activation. It includes nonzero Orchard supply, duplicate claims and rejection without mutation; existing execution tests cover proof-backed cap growth. Captured-chain replay must reproduce block 1011 and every later certified root through 1020. Changing a root or relaxing the cap fails qualification.

## 3. Withdrawal identities: reproduce immutable releases

The Arc and pfETH releases have different verifier pins. Preserve both.

| Input | Arc v2 | pfETH v1 |
|---|---|---|
| Source revision | `b945bbb321eb4fdc5a57998fadcf0b353f1acb86` | `c9407b1c2176cf7f812ba886f8053ae7bb5cdeb0` |
| Retained ELF revision | `65df4263746566c90d0e46db8c90a46c23576265` | `c9407b1c2176cf7f812ba886f8053ae7bb5cdeb0` |
| SP1 | 6.3.1 | 6.3.1 |
| Build layout | Docker, `/root/program`, Cargo home `/root/.cargo` | Native, `/home/postfiat/repos/postfiatl1v2`, Cargo home `/home/postfiat/.cargo` |
| ELF SHA-256 | `8b0f266a035a432ef3c0e4233672d8a15b913cefd4c85ff07f91f008601bb744` | `0ea4dbabc8a36b44824c861cf2e1ae90454df2a6e4bb8a357f9d3a8672696d83` |
| Verifying key | `0x0036cbe7d36bbfe1118a3c544eeba74f3791d2a19bf5ec59b972f72d36416852` | `0x00140f09ca6f1b917e3999a806df5ef4ac4468bd65b85e8580cbc16f832dfe47` |

`programs/pfusdc-egress/releases.json` records the complete source revisions, lockfile hashes, build layouts and pinned Docker digest. Existing Arc evidence and the pfETH deployment manifest supply the expected hashes and keys. Source/layout identification is a reproduction hypothesis until two clean builds establish equality; a retained hash alone proves no source provenance.

Run `.github/workflows/arc-proof-identities.yml` for both releases. Each gets two clean source trees and independent build outputs. Require both outputs to match each other and the retained ELF byte for byte, then derive and compare the verifying key. Record toolchain, lockfile and image identities. Reproduce embedded paths as build inputs; never patch the resulting ELF or refresh an expected hash.

The prover exposes an explicit release choice for withdrawals and checkpoint proofs. It checks the ELF against that release's retained digest before execution and checks the derived key before proving. Cross-release substitution must fail. Current-source development and native proof tests remain separate from historical reproduction; a new guest requires a new identity and deployment process.

## 4. Qualification and rollback

Use isolated PR41 source and disposable copies of the six captured nodes. Keep keys, full state and witnesses outside Git. Preserve the earlier failure packet; publish a new packet with exact commands, source and binary hashes, sanitized results and raw logs.

The [existing verification commands](https://github.com/postfiatorg/postfiatl1v2/blob/12721f618999dcb175ff4e4397743ece0682220b/deployments/combined-release-20260915/verification-commands.md) and `deployments/signing-fix-qualification-20260909/run_local_service_gate.py` define the operator interface. The service gate requires six loopback peers and alternates RPC-first and transport-first startup. It exercises a certified continuation, compares all roots, stops and restarts the services, then compares the recovered tip.

Pass these gates before closing the milestone:

1. Focused FastPay, governance, supply, commitment and proof-identity regressions; exact captured-history replay through 1020 on all six copies.
2. Two identical clean node builds and the full workspace suite once, at the final milestone boundary, because the repairs cross Orchard accounting and state commitments.
3. Both service startup orders, certified continuation and restart; governed v2 installation and replay on disposable state.
4. Restore the exact deployed executable with the schema-compatible pre-upgrade data, verify its finalized checkpoint and test local service restart. The repaired verifier must independently pass full-history replay of that same pre-upgrade history.
5. Update PR41, document working operator commands, publish the qualification packet and complete Task Node verification.

The old executable's full replay bug remains recorded against its exact hash; an altered executable cannot stand in for it. Checkpoint verification establishes its ability to authenticate and serve the restored tip, while the repaired verifier establishes historical execution. Failure of either obligation keeps rollback qualification open. Live deployment and live v2 activation require separate operator action.

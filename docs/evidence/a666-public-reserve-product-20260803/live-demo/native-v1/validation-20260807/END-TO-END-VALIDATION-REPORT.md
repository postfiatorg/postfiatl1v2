# A666 End-to-End Validation Report

Validation target: origin/main at afd6e5cfe64ea94377f7cf33ce166e2a14a6f71a  
Validation date: 2026-08-07  
Scope: capability validation only. No live broadcast, no fleet mutation, and no key values were used. The validation worktree is /home/postfiat/repos/pftl-validation-20260807. Live fleet state was left untouched at h778.

All PFTL and Ethereum token amounts in this report use six-decimal atoms. Every token figure is written as N atoms (H.HHHHHH UNIT). Hashes, commit identifiers, block heights, ports, test counts, and exit codes are identifiers or counts, not token amounts. PFTL supply of 20,000,000 atoms is 20.000000 pfUSDC.

## 1. Stage table

| Validation stage | Verdict | Observed result | Evidence |
|---|---|---|---|
| Worktree and remote | PASS | target origin/main afd6e5cfe64ea94377f7cf33ce166e2a14a6f71a | build-test/remote.txt; build-test/worktree-head.txt (execution provenance: /tmp/krimp-val/remote.txt) |
| Source build | PASS with declared nondeterminism | validation build sha256 9adce9a474644af31b5e02bcdfb7d1fb1b8a51f11e6bf3f1c3a4a0f279d036ed; production orchard release sha256 25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4 | build-test/build-shas.txt; packet/determinism-ruling.txt |
| Cargo check | PASS | exit code 0 | build-test/check.txt; build-test/check.code |
| Native execution tests | PASS | 13 passed, 0 failed | build-test/exec-tests.txt; build-test/exec-tests.code |
| Python suites | PASS | 84 passed, 1 skipped | build-test/pytests.txt (execution provenance: /tmp/krimp-val/pytests.txt) |
| Node library tests | PASS | 267 passed; 0 failed; 2 ignored; 0 measured; exit code 0; finished in 2496.82 seconds | build-test/node-lib.txt; build-test/node-lib.code (execution provenance: /tmp/krimp-val/node-lib.txt; /tmp/krimp-val/node-lib.code) |
| Node full tests | PASS | 424 passed, 0 failed across 13 test binaries; exit code 0; runtime 45m17s | build-test/node-full.txt; build-test/node-full.code (execution provenance: /tmp/krimp-val/node-full.txt; /tmp/krimp-val/node-full.code) |
| Packet hashes | PASS | 16 of 16 packet hashes match binding | packet/packet-hashes.txt |
| Binding | PASS | binding cc1bd291543e45e59fa2ff89df7e5c041c8ed101d6f93fb7e0eac57dd134bf9; all 16 entries match | packet/binding.txt |
| Executable linter | PASS | exit code 0; all executable surfaces classified, staged exemptions printed | packet/linter.txt |
| Resolver self-check | PASS | exit code 0 | packet/resolver-selfcheck.txt |
| Resolver full render | PASS | exit code 0 | packet/resolver-full.txt |
| Determinism | PASS with D1 audit residual | 14 of 16 packets byte-identical after output-root normalization; leg1 difference is the disclosed provenance gap | packet/determinism-ruling.txt; packet/determinism.txt |
| Temporary references | PASS | zero binary references; 94 references classified as 37 scratch data-artifact references and 57 provenance/log references | packet/fix-round-acceptance.txt; packet/tmp-refs-classified.txt |
| Secret scan | PASS | value-bearing secret hits 0 | staged-secret-scan.txt; packet/secret.txt |
| Diff check | PASS | exit code 0 | packet/diffcheck.txt |
| StakeHub boundary | PASS | 25 of 25 references are custody/signing locations; control-plane rows 0 | packet/stakehub-classified.txt; packet/stakehub-counts.txt |
| Fleet state | PASS read-only | 6 of 6 validators at block height 778, common state root b287451679a9d4d95c36a10f54bfab6cf0ea76cf86e4dd32faeee672520273df6bcb91fb58d67a56c29f187223a0d463 | live/fleet.txt |
| Holder state | PASS read-only | 1,358,493 atoms (1.358493 pfUSDC) | live/pftl-state-summary.txt |
| Route state | PASS read-only | deposits verified 422,210,781 atoms (422.210781 pfUSDC), claims minted 412,210,781 atoms (412.210781 pfUSDC), finalized unclaimed 10,000,000 atoms (10.000000 pfUSDC), route live, route epoch 6 | live/route-status.txt; live/pftl-state-summary.txt |
| NAV cap | PASS read-only pre-claim state | 287,859,297 atoms (287.859297 pfUSDC), epoch 45 pre-claim (reserve summary records finalized epoch 44), pricing epoch 5 | live/vault-summary.txt; live/vault-arithmetic.txt |
| EVM wallet and deposit | PASS read-only | wallet 74,161,443 atoms (74.161443 USDC), nonce 304; deposit block 25,698,310, status 1, amount 10,000,000 atoms (10.000000 USDC) | live/evm-state.txt |
| Protected wA666 baseline | PASS read-only | 103,000,000 atoms (103.000000 wA666), no packet executor debit path touches it | live/wa666-baseline.txt |
| Conservation arithmetic | PASS for static cells and executed receipt cells | residuals 0 where values are static or fork-observed; later receipt-chained cells remain explicitly unresolved | live/arithmetic.txt; live/done-check.txt |
| Budget caps | PASS | prior 501.024845 USDC + principal 10.000000 USDC + gas ceilings 0.9220 USDC = 511.946845 USDC, within 530.000000 USDC; GPU packet references 0 | live/caps.txt |
| Stage-zero staging | PASS | orchard release sha matches 25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4; signed manifest binds all 6 validators and 2 services per validator | build-test/stage0-sha.txt; build-test/stage0-manifest.txt |
| Leg1 claim admission | PASS capability check | orchardfix batch-only exit code 0 on a fresh clone; this is admission validation, not a live claim | build-test/batch-only.txt (execution provenance: /tmp/krimp-val/batch-only.txt) |
| Local EVM fork legs | PASS for 3c, 3d, 3e, 3f, 3g, and 3h | every executed receipt status 0x1; 3e delta -11,027,135 atoms (-11.027135 wA666) and +8,057,858 atoms (+8.057858 USDC); 3h delta -8,057,858 atoms (-8.057858 USDC) and +11,013,374 atoms (+11.013374 wA666) | fork/fork2/fork2-summary.txt; fork/fork2/leg3c.json; fork/fork2/leg3e.json; fork/fork2/leg3h.json |
| Leg4 fork phase one | UNVALIDATED | amount resolved to 11,013,374 atoms (11.013374 wA666); constrained-signer socket intentionally not invoked | fork/fork2/leg4.json; fork/fork2/leg4-rebuild.txt |
| Leg5b fork withdrawal | UNVALIDATED | leg5a receipt, burn report, withdrawal signatures, and resolvable calldata were unavailable | fork/fork2/leg5b.json |

The executed forward and reverse fork path paid a measured round-trip fee of 13,761 atoms (0.013761 wA666). The local fork used Anvil impersonation and cast only. No executed fork step required StakeHub.

## 2. Authoritative packet hashes

The following SHA-256 values were computed from packets-S1 and matched the binding:

| Packet | SHA-256 |
|---|---|
| native-leg0-proxy-verify.json | 2df8068c0b9e6db360eb828959314467bfa1d34763f7eda8f1a11339c2bb8ced |
| native-leg1-bridge-in.json | b6ae7b845d7a03d888af1ed8975f4e8d0edacd66884da7863fab2cce694a927a |
| native-leg2a-order-reserve.json | fc356139e9950bd6ff3056a03bdc3351271ec6c24a2c60e4676928b886671714 |
| native-leg2b-primary-subscribe.json | ef88d977d02c874df4bcbca1879ab595265085cd26e40a7fce8b0ad20163bf42 |
| native-leg3a-export-debit.json | b3271f2fc7da6c91a67744f736d3e3ef926a40beabd260a9929df6ba2e76bf3a |
| native-leg3b-accept-mint.json | 14b0a36cd1e4768608fb4a0a97d3d9fc25cc7722be2a74bdb0c44715d15f848b |
| native-leg3b0-signer-funding.json | a3447f98eb15d255df3622cb1912ffdbbca7c719b8316b6c9d260617e14de064 |
| native-leg3c-approve-wa666-permit2.json | af67f1b1d89a25f790508e77b15321c019bf7899cdc3e2b7f936960ae48f7c1a |
| native-leg3d-permit2-wa666-router.json | e80e8092958824ba53074a5107a117acad55491010be2438d9c4069d0a1f85a3 |
| native-leg3e-swap-wa666-usdc.json | f171c982c1a37a436ff9989809cf53d23b84d45a2a7d1095e5fcdd41f6c8e50f |
| native-leg3f-approve-usdc-permit2.json | cc99af4f4eea906aaba05d0eb0ea67eb82ac5740404402d40898c8a5f1c81cbe |
| native-leg3g-permit2-usdc-router.json | 3251954991274136d678ed946a3c7fd431d30a80c5dc65a80dfcd2d53595586c |
| native-leg3h-swap-usdc-wa666.json | 4c9a598f27dbd3818f8afb9dd43a5c95d666689d827b385bea270caf213656d9 |
| native-leg4-return-burn-import.json | 46ac60b2d0fca313fa333425a8788697adad2abb94c2800e66bbe77664391c9c |
| native-leg5a-primary-redeem.json | c3affa77469799f2094346927b3933a44b97efded4aab027e60ed271ddb343e9 |
| native-leg5b-bridge-out.json | 5dc9fa5425c86d9904dfda3b4ebbd50291eba4af19464644bcb254a023d44185 |

The authoritative binding artifacts are binding-S1.json sha256 cc1bd291543e45e59fa2ff89df7e5c041c8ed101d6f93fb7e0eac57dd134bf9c, values-S1.json sha256 9d6c226217c0b72ff881ca5004060492649057098e957ec519c8b6aa88b18a33, resolution-rules.md sha256 54a7056568a8f863bb8c64634bf963580feac9588c25fac989ed1b761c538293, and authorization-native-fire-20260806.json sha256 f0679ac76e5194527cc62aded34e778dfdc23089eab16efcb803c45fe74588d5.

## 3. Defects and residuals

### 3.1 Orchardfix binary lineage

The orchardfix binary used for claim admission has sha256 25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4 and was built from commit 540b2c1c according to BUILD-INFO.json. Commit 16621fa9 is reachable from main, with git merge-base --is-ancestor 16621fa9 afd6e5c exiting 0, and is code-identical to 540b2c1c: the diff-of-diffs residual is exactly commit hash, date, and message metadata, with zero code-line differences. Therefore the binary semantics are reachable from main; binary-hash nondeterminism is declared, not identity. Evidence: build-test/lineage-proof.txt.

### D1. leg1 resolver provenance gap

The leg1 committed packet changed abort_conditions from 5 to 9 entries, changed custody_signing_leaf text, and changed evidence_output_paths from 3 to 22 entries after resolution without corresponding resolved_fields entries. The executable integrity remains intact: binding-S1 pins the committed bytes, the hardened packet is the correct artifact, and the older same-batch finalize-and-claim render is invalid. resolution-rules.md makes HELD source packets immutable, so the changes cannot be back-propagated. Disposition: disclose this documentation defect and use an S2 stage supplement.

### D2. leg3e S1-pinned calldata stale by design

The S1 deadline 0x6a74c23a is stale on a current fork and produces TransactionDeadlinePassed. The packet policy recomputes a fresh deadline and fresh minimum output as floor(0.97 times the fresh fill). Regenerated calldata executed with receipt status 0x1 and a fresh fill of 8,057,858 atoms (8.057858 USDC), producing minimum output 7,816,122 atoms (7.816122 USDC). This is the specified fire-time mechanism, not an infrastructure defect.

### D3. PFTL legs 2a–5a batch-only UNVALIDATED

Admission for legs 2a through 5a requires post-leg1 pre-state. That state would include holder balance 11,358,493 atoms (11.358493 pfUSDC) and cap 297,859,297 atoms (297.859297 pfUSDC), which cannot be created on a read-only pre-claim clone without ledger surgery. The operations are fully specified in each ops_file_template. The earlier description of these operations as having zero executor commands was inaccurate: they are HELD placeholders, not malformed operations. The money-critical leg1 claim batch-only admission did pass with exit code 0.

### D4. leg4 phase-1 burn UNVALIDATED on fork

The fork reconstruction resolved the receipt-chained burn amount to 11,013,374 atoms (11.013374 wA666). Execution was intentionally withheld because the phase one command requires a constrained-signer socket outside the impersonation-only boundary. No custody request was sent.

### D5. leg5b legacy-packet/successor-main split

The packets bind fleet-lineage nav-roundtrip-live-demo, which is absent from main after the b19ce4c decoupling. Main carries the functional successors vault-bridge-burn-to-redeem-bundle, vault-bridge-withdrawal-plan, and vault-bridge-withdrawal-signature-bundle. The binary-binding split is deliberate: ledger semantics bind to the orchard release while EVM bundle tooling binds to the production client. Leg5b remains unvalidated until its successor command surface and post-leg5a receipt values are resolved.

## 4. Conservation table

| Leg | In / debit | Out / credit | Acceptance and residual |
|---|---|---|---|
| 1 claim | 1,358,493 atoms (1.358493 pfUSDC) holder balance plus 10,000,000 atoms (10.000000 pfUSDC) claim | 11,358,493 atoms (11.358493 pfUSDC) holder balance; cap 297,859,297 atoms (297.859297 pfUSDC); claims minted 422,210,781 atoms (422.210781 pfUSDC) | Static arithmetic residual 0; live execution held |
| 2a reserve | 9,950,248 atoms (9.950248 pfUSDC) base settlement; due ceiling 10,000,000 atoms (10.000000 pfUSDC) | 11,027,135 atoms (11.027135 A666) mint | Packet residual 0; post-claim PFTL state required |
| 2b subscribe | 10,000,000 atoms (10.000000 pfUSDC) settlement | 11,027,135 atoms (11.027135 A666) mint | Packet residual 0; post-claim PFTL state required |
| 3a export | 11,027,135 atoms (11.027135 A666) | 11,027,135 atoms (11.027135 wA666) EVM-side amount | Packet residual 0; receipt witness required |
| 3b accept mint | 11,027,135 atoms (11.027135 wA666) expected mint | 11,027,135 atoms (11.027135 wA666) wallet delta | Packet acceptance exact; PFTL export state required |
| 3b0 funding | 10,000,000,000,000,000 wei native gas funding; ceiling 0.0201 USDC | signer gas balance increase; principal unchanged | Gas-only; no token residual |
| 3c approve | 11,027,135 atoms (11.027135 wA666) allowance | allowance 11,027,135 atoms (11.027135 wA666) | Fork receipt status 0x1; residual 0 |
| 3d Permit2 | 11,027,135 atoms (11.027135 wA666) allowance | router allowance 11,027,135 atoms (11.027135 wA666) | Fork receipt status 0x1; residual 0 |
| 3e forward swap | 11,027,135 atoms (11.027135 wA666) | fresh output 8,057,858 atoms (8.057858 USDC); minimum 7,816,122 atoms (7.816122 USDC) | Fork receipt status 0x1; residual 0 against packet expectation |
| 3f approve | receipt-chained 8,057,858 atoms (8.057858 USDC) | allowance 8,057,858 atoms (8.057858 USDC) | Fork receipt status 0x1; residual 0 |
| 3g Permit2 | receipt-chained 8,057,858 atoms (8.057858 USDC) | router allowance 8,057,858 atoms (8.057858 USDC) | Fork receipt status 0x1; residual 0 |
| 3h reverse swap | receipt-chained 8,057,858 atoms (8.057858 USDC) | fresh output 11,013,374 atoms (11.013374 wA666); minimum 10,682,972 atoms (10.682972 wA666) | Fork receipt status 0x1; residual 0 |
| 4 return burn/import | receipt-chained 11,013,374 atoms (11.013374 wA666) | expected PFTL import 11,013,374 atoms (11.013374 A666) | Fork UNVALIDATED because signer leaf is outside boundary |
| 5a primary redeem | receipt-chained imported A666 amount; exact amount pending | receipt-chained pfUSDC payout; exact amount pending | PFTL batch-only UNVALIDATED |
| 5b bridge-out | receipt-chained pfUSDC payout; exact amount pending | external USDC payout; exact amount pending | PFTL and EVM sequence UNVALIDATED |

Static and executed cells have zero residuals. Receipt-chained cells are explicitly pending predecessor receipts rather than silently substituted.

## 5. Rerunnable validation commands

Commands below are reviewable validation commands. They are not an authorization to run a live mutation.

Worktree and Rust checks:

    cd /home/postfiat/repos/pftl-validation-20260807
    cargo check --workspace
    cargo test -p postfiat-node --lib
    cargo test -p postfiat-node
    cargo test -p postfiat-execution unit_tests

Python validation:

    cd /home/postfiat/repos/a666-eth-fast-lane-combined-20260724
    python3 -m pytest scripts/test_native_campaign_driver.py scripts/test_native_prover_leaf.py scripts/test_native_rpc_finality_submit.py -q

Resolver self-check and full render:

    cd /home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806
    python3 resolve.py --held-dir /home/postfiat/repos/pftl-validation-20260807/docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1 --values /home/postfiat/repos/pftl-validation-20260807/docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806/values-S1.json --self-check
    python3 resolve.py --held-dir /home/postfiat/repos/pftl-validation-20260807/docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1 --values /home/postfiat/repos/pftl-validation-20260807/docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806/values-S1.json --output-root /home/postfiat/repos/pftl-validation-20260807/docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806/resolved-validation-tmp

Local fork setup and read-only simulation:

    anvil --fork-url https://ethereum-rpc.publicnode.com --port 8547 --chain-id 1
    cast rpc anvil_impersonateAccount 0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0 --rpc-url http://127.0.0.1:8547
    cast rpc anvil_setBalance 0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0 0x1000000000000000000 --rpc-url http://127.0.0.1:8547

The fork deal and each EVM calldata template are defined by packets-S1 and the evidence under fork/fork2. Never use a live signer or agentd socket in the local fork. The exact rebuilt fields, receipts, and deltas are recorded beside each leg artifact.

HISTORICAL COMMAND (not self-contained after scratch cleanup)

    /home/postfiat/repos/a666-eth-fast-lane-combined-20260724/deployments/pnok-private-fix-20260801/pnok-private-fix-2246d25-orchard1/build/postfiat-node pftl-submit-certified-asset-ops --batch-only --ops-file /tmp/krimp-exec-fire20260806/leg-1/relay/claim-only.ops.json --data-dir /tmp/krimp-u21/h776-clone/validator-1 --topology /tmp/krimp-u21/h776-clone/topology-live.json --key-file /nonexistent-placeholder-validator-key-must-never-be-dereferenced.json --artifact-dir /tmp/validation-claim-artifacts

claim-only.ops.json is derived from the leg1 relay bundle via the packet-bound command chain in packets-S1/native-leg1-bridge-in.json: bundle build, pftl-certified-asset-ops-from-bundle, then jq claim-only extraction. The intermediate artifacts lived at /tmp/krimp-exec-fire20260806 and are ephemeral. The clone at /tmp/krimp-u21/h776-clone is refreshable read-only via rsync from validator-1 using the command in that packet chain.

This is an offline clone admission check only. The claimed exit code 0 is recorded in build-test/batch-only.txt (execution provenance: /tmp/krimp-val/batch-only.txt). It does not submit to the live fleet. The Python scripts were verified byte-identical to main at afd6e5cfe64ea94377f7cf33ce166e2a14a6f71a by diff -q.

## 6. Final verdict

PARTIAL PASS — production capability validated for tested surfaces; complete end-to-end loop NOT validated and NOT yet proven working.

The blockers to an E2E PASS are: (a) PFTL legs 2a–5a batch-only admission, because post-leg1 prestate is unavailable on the read-only clone; (b) the leg4 constrained-signer burn; and (c) the leg5b EVM withdrawal. Node full is terminally green with 424 passed and 0 failed across 13 test binaries, including the 267 library tests, with exit code 0 and runtime 45m17s. The honest scope is capability validated, live loop not executed. Leg1 claim and all subsequent economic mutations remain held behind their authorization and STOP-no-retry gates.

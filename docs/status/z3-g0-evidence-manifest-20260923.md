# Z3 G0 evidence baseline manifest — 2026-09-23

**133 files frozen: 23 in-repo blobs and 110 server-only files. Secret and forbidden-field scan: no secret, key material or plan/spec-named forbidden field found.** This closes the two open [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md#gate-g0-lock-the-read-only-baseline) G0 items.

Read-only work: no RPC call, SSH, transaction, signature, key or password-file access, or Task Node action. Main was `ad7ea8ed48a0ed489e67537455fe822b4fb354b6` when hashing finished at `2026-09-23T10:00:44Z`.

## Sources

Only files that the plan and the Z3 records cite were included:

- **Plan baseline:** the 2026-09-02 [deposit, mint, burn and release links](../plans/active/z3-navcoin-roundtrip-plan.md#evidence-baseline-and-reconciliation) and the deployment record. The plan does not link the "retained replay record"; the only replay file in that packet is `egress/withdraw.replay.txt` (row 5).
- **[G2 verdict](../review/z3-g2-route-compatibility-20260917.md#identities-read-back):** its frozen A666, Ethereum-comparator and Arc files, plus the release-repair proof-identity record. Source-code files it read, and the release-repair plan it cites for a commit, are not fixtures.
- **[Preflight](z3-preflight-20260917.md):** cites no fixture file. Its values are read-only observations recorded inline; its links point to documents.
- **[Dry runs](z3-dry-run-20260921.md):** the release-branch deployment input, both retained capture directories in full, and the three hash-pinned reference executables.

The preflight and dry-run records are in `docs/status/`, not `docs/review/`.

## Hash method

```bash
# In-repo blob at the cited commit
git cat-file -s COMMIT:PATH
git cat-file blob COMMIT:PATH | sha256sum
# Server-only file
stat -c %s FILE
sha256sum FILE
```

| Short | Commit | Why |
| --- | --- | --- |
| `38e626e9` | `38e626e95682f79639a83aba5cf617807e960c8e` | Plan's Arc evidence lineage |
| `484a0feb` | `484a0feb524f6526b8f96f8ce4a79a74205bbf62` | Plan's deployment-record link |
| `15126ac3` | `15126ac3408f3afe58cf4e4ed4c31d00efa04366` | G2 candidate |
| `b1201bc9` | `b1201bc9f2d30822359ce5610b2a465f120d4b96` | Dry-run release checkout |

Each in-repo blob is byte-identical at every one of these commits and main that contains it. Rows 7–11 and 13 are also on main; the others are not. Recorded SHA-256 values in the dry-run record match rows 70, 85, 127, 129 and 131–133. Row 133 is an untracked build output outside `~/.cache`; it is also server-only. All paths are regular files, not symlinks.

## Manifest

Scan codes are defined in the [scan section](#secret-and-forbidden-field-scan).

### 2026-09-02 Arc bridge packet and deployment record (plan G0)

| # | File | Hashed at | Bytes | SHA-256 | Cited by | Scan |
| ---: | --- | --- | ---: | --- | --- | :---: |
| 1 | `docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/deposit.receipt.json` | `38e626e9` | 5270 | `e6deb76a96b4d729ae38b9f434e08743b0fe49b889ccbf5911786471c86bc1b3` | Plan baseline (deposit); G2 | C |
| 2 | `docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/arc-finalize-claim.report.json` | `38e626e9` | 3144 | `36fbe21597545004785779328b7a8ffd1a6859183224653a7083810a2a74f4b1` | Plan baseline (mint); G2 | C |
| 3 | `docs/evidence/arc-mvp-20260828/devnet-20260902/egress/egress-witness.json` | `38e626e9` | 3722241 | `e4d5202be88f8411671c9a97228f7f307aa3b2078f96a0806bab673c6fd5975a` | Plan baseline (burn); G2 | C |
| 4 | `docs/evidence/arc-mvp-20260828/devnet-20260902/egress/withdraw.receipt.json` | `38e626e9` | 4497 | `ebd463220fae74ba946669efb0825a9803eb19e6ab012179adb8157fc2b70ce7` | Plan baseline (release); G2 | C |
| 5 | `docs/evidence/arc-mvp-20260828/devnet-20260902/egress/withdraw.replay.txt` | `38e626e9` | 163 | `4f1787c3c9c816cff331a58322d60507db020f4d1eff3ad03a39d3d36ee17a5b` | Plan baseline (replay record, unlinked) | C |
| 6 | `docs/evidence/arc-mvp-20260828/deployments.md` | `484a0feb` | 10251 | `eee99cfec9d52efcdab6403606fd50e717c7affd605df34cedae53dd9c493d36` | Plan baseline (deployment record); G2 | R |

### G2 frozen fixtures

| # | File | Hashed at | Bytes | SHA-256 | Cited by | Scan |
| ---: | --- | --- | ---: | --- | --- | :---: |
| 7 | `deployments/a666-mainnet-20260727/09-production-route-config.json` | `15126ac3` | 1235 | `dc8785cecc816aa1e3085d312c439f11e3d850ee7d78972abe7c390e68711624` | G2 | C |
| 8 | `deployments/a666-mainnet-20260727/09-production-route-config-digest.json` | `15126ac3` | 451 | `888c5186f463f5f4acac2ed010caa9062fc96194debc249b212c1e218cc791c2` | G2 | C |
| 9 | `deployments/a666-mainnet-20260727/10-production-route-activate.ops.json` | `15126ac3` | 1722 | `ebe58a7c9aeb8a52eac4ff20b30a3f9ee091adffcc5517f02ac733a621580ab9` | G2 | P |
| 10 | `deployments/a666-mainnet-20260727/opening-nav-proof-manifest.json` | `15126ac3` | 1148 | `2fee970be097360408391e83b542bd91b2399eb201a2f71cf33731c2028cebdc` | G2 | C |
| 11 | `deployments/a666-mainnet-20260727/11a-nav-valuation-unit-fix.ops.json` | `15126ac3` | 1132 | `0a71e331cfb8573699b297e5f297a706f67a4045995474f7763e96f31da8b55c` | G2 | P |
| 12 | `deployments/a666-source-route-20260907/canary-rpc.json` | `15126ac3` | 9941 | `e5e46e7ded4f8e676a5948fe2ceb5b8d6b31fbf20318cf9e1284ffe68227a882` | G2 | C |
| 13 | `deployments/pfusdc-eth-mainnet-20260809-epoch6-successor/route-profile.mainnet-epoch6.json` | `15126ac3` | 1676 | `ae66bd270500be2e44f39136f8d136506262e17f0b094ca38c41927f7bbabe80` | G2 | C |
| 14 | `docs/evidence/arc-mvp-20260828/route-profile.json` | `15126ac3` | 1244 | `6dc1049abfd253a4367ec8fb2f89a1a1c1b4361fb42e8fca3aa37ad677b75dcc` | G2 | C |
| 15 | `docs/evidence/arc-mvp-20260828/route-profile-info.json` | `15126ac3` | 326 | `f37ad92e2e599ebd35ea20737db9bd2ca1f84c710d91430ba838b3b0b87e4511` | G2 | C |
| 16 | `docs/evidence/arc-mvp-20260828/program-info.json` | `15126ac3` | 673 | `584fad7cd091a6061723c0e31c1bc16f5d0895c7af1d435d37a12cc7148f6027` | G2 | C |
| 17 | `docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/route-profile.v2.json` | `15126ac3` | 1244 | `544b0d7284610bfdf09290cfe62d354eb8ae8fc069d680e6aceb19e67c0d1c3f` | G2 | C |
| 18 | `docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/route-profile-info.v2.json` | `15126ac3` | 367 | `769464faf3231ab7d0e28453a88e684581607768d98f590ea49876375f500f7a` | G2 | C |
| 19 | `docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/factory.env` | `15126ac3` | 179 | `4fd10167df4c32de72d7cab3f7e947ccde0ef603d462fb6510b4b42cb2674ba2` | G2 | C |
| 20 | `docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/relay-bundle.report.json` | `15126ac3` | 18399 | `44aaba6511b55bba6f22039f07b02e4f23fbe50a4982fb09f5d1a2801d854d89` | G2 | R |
| 21 | `docs/evidence/arc-mvp-20260828/program-info.current-v2-docker-20260902.json` | `15126ac3` | 674 | `38b5a0128ced50cb3b7a1bc0473deda50d2870912c6c1337407d55dc8c0e2ee1` | G2 | C |
| 22 | `deployments/release-repair-20260916/proof-identities.json` | `15126ac3` | 1725 | `fe93ad2594ab6219b971f53ecccbfc59ea13b646b42bc6a5562a58f33c59e065` | G2 | C |

### Dry-run deployment input

| # | File | Hashed at | Bytes | SHA-256 | Cited by | Scan |
| ---: | --- | --- | ---: | --- | --- | :---: |
| 23 | `deployments/combined-devnet-20260921/manifest-input.unsigned.json` | `b1201bc9` | 5250 | `1aa78cf38e5308f97a1025c8fe4d23390c4fc520f4f86d35085c3ad564df96aa` | Dry runs (node identity) | R |

### Dry-run captures, first run — server-only

| # | File | Hashed at | Bytes | SHA-256 | Cited by | Scan |
| ---: | --- | --- | ---: | --- | --- | :---: |
| 24 | `~/.cache/z3-dry-run-20260921/audit.py` | server-only | 3235 | `b47e4df8e17b08215c695812819351a0bd996eae5a7a8495f64597bd3005808a` | Dry run 1 | P |
| 25 | `~/.cache/z3-dry-run-20260921/captures.json` | server-only | 6976 | `2cb51fc40f09e578b2023167a93809a6c84cfe42b56ca1350746d5ad9b2943cd` | Dry run 1 | P |
| 26 | `~/.cache/z3-dry-run-20260921/dry-run.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 27 | `~/.cache/z3-dry-run-20260921/dry-run.stdout` | server-only | 61 | `3e3e2bf04e01d45bcd82c3b6018129c0b650963c043d8e9c79a29e1283db5b0b` | Dry run 1 | C |
| 28 | `~/.cache/z3-dry-run-20260921/help-build-issue.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 29 | `~/.cache/z3-dry-run-20260921/help-build-issue.stdout` | server-only | 1326 | `386cdbdc830535a745261345adc613134efb1b43484bde5ed0200f42e873009f` | Dry run 1 | H |
| 30 | `~/.cache/z3-dry-run-20260921/help-build-redeem.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 31 | `~/.cache/z3-dry-run-20260921/help-build-redeem.stdout` | server-only | 1203 | `acaecd64a0fd81dbc478396b6c8835f60511a1bb63274a3de80592f37132e263` | Dry run 1 | H |
| 32 | `~/.cache/z3-dry-run-20260921/help-cast-call.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 33 | `~/.cache/z3-dry-run-20260921/help-cast-call.stdout` | server-only | 17940 | `fe824aa6b8b65293a29a239e6bf5bb67981917a5b08b5b0359fabbb0b8f75051` | Dry run 1 | H |
| 34 | `~/.cache/z3-dry-run-20260921/help-cast-send.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 35 | `~/.cache/z3-dry-run-20260921/help-cast-send.stdout` | server-only | 16335 | `3ed4d0561d8a2967c46848b3f2bfdb815bab62c0568e294bd14ad127ee76a8dd` | Dry run 1 | H |
| 36 | `~/.cache/z3-dry-run-20260921/help-cycle-build.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 37 | `~/.cache/z3-dry-run-20260921/help-cycle-build.stdout` | server-only | 252 | `2e69a8871a76700bb029c004fe54e869f279f143a5252e90395560592ca110cd` | Dry run 1 | C |
| 38 | `~/.cache/z3-dry-run-20260921/help-cycle-verify.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 39 | `~/.cache/z3-dry-run-20260921/help-cycle-verify.stdout` | server-only | 151 | `0ff01ed6b03183d5b8e287cd39c552892e619c662610221bcf59428e02eb9b77` | Dry run 1 | C |
| 40 | `~/.cache/z3-dry-run-20260921/help-node-pfusdc-egress-witness.stderr` | server-only | 50427 | `b61dbc4431570b1944c01d0b50a085f783e8a0ffedb615f37bec96a66943945a` | Dry run 1 | H |
| 41 | `~/.cache/z3-dry-run-20260921/help-node-pfusdc-egress-witness.stdout` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 42 | `~/.cache/z3-dry-run-20260921/help-node-vault-bridge-burn-to-redeem-bundle.stderr` | server-only | 50419 | `51f1addf09b63945c58279acf9a707e8aca0070c83a19b6ac50068f255afab3a` | Dry run 1 | H |
| 43 | `~/.cache/z3-dry-run-20260921/help-node-vault-bridge-burn-to-redeem-bundle.stdout` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 44 | `~/.cache/z3-dry-run-20260921/help-node-vault-bridge-deposit-relay-bundle.stderr` | server-only | 50440 | `bb8caa501d96b271e4de572abe3f6f4ec0fa665fb384627c7c35f8f33fc66b16` | Dry run 1 | H |
| 45 | `~/.cache/z3-dry-run-20260921/help-node-vault-bridge-deposit-relay-bundle.stdout` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 46 | `~/.cache/z3-dry-run-20260921/help-preflight.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 47 | `~/.cache/z3-dry-run-20260921/help-preflight.stdout` | server-only | 241 | `17ff76f39f1ac09da982a3f4c25c5e6e91b80594f6c6d2b30c916bcf612a192e` | Dry run 1 | C |
| 48 | `~/.cache/z3-dry-run-20260921/help-prepare-nav.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 49 | `~/.cache/z3-dry-run-20260921/help-prepare-nav.stdout` | server-only | 1259 | `231c321463c4eada46013aa243ca3788faa45e29e7890fcf7fb5fee7e30e7ad8` | Dry run 1 | H |
| 50 | `~/.cache/z3-dry-run-20260921/help-prepare-route-epoch.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 51 | `~/.cache/z3-dry-run-20260921/help-prepare-route-epoch.stdout` | server-only | 867 | `11dd14c84178bdd353aa87f902ea089f588e44f072e48a84b0e21c1ef3f10a4c` | Dry run 1 | H |
| 52 | `~/.cache/z3-dry-run-20260921/help-prover-arc-ingress-capture.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 53 | `~/.cache/z3-dry-run-20260921/help-prover-arc-ingress-capture.stdout` | server-only | 900 | `16ecda22ed71241bc0e9ed42f7b1ce088dba3bca37d6e2197cd9044c289c7e3f` | Dry run 1 | C |
| 54 | `~/.cache/z3-dry-run-20260921/help-prover-arc-ingress.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 55 | `~/.cache/z3-dry-run-20260921/help-prover-arc-ingress.stdout` | server-only | 309 | `93d0dd850a8d9d6ddd01e612e248304c8e87e672ac61715be37a3f0fce15725b` | Dry run 1 | C |
| 56 | `~/.cache/z3-dry-run-20260921/help-prover-egress.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 57 | `~/.cache/z3-dry-run-20260921/help-prover-egress.stdout` | server-only | 567 | `f0ace149cc25e3b472fb37e931239fa734a8203e45a8a9d04ffde7113dfef665` | Dry run 1 | C |
| 58 | `~/.cache/z3-dry-run-20260921/help-remote-submit.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 59 | `~/.cache/z3-dry-run-20260921/help-remote-submit.stdout` | server-only | 1565 | `afe02497aca3719cf77b52a6eda7b838a581908ab069ca72b1dcba7ca38a5105` | Dry run 1 | H |
| 60 | `~/.cache/z3-dry-run-20260921/help-route-switch.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 61 | `~/.cache/z3-dry-run-20260921/help-route-switch.stdout` | server-only | 437 | `a614bf37a6ed88e6f7aefe347561c1d87556aba8ee34602155226b76c7a26bda` | Dry run 1 | C |
| 62 | `~/.cache/z3-dry-run-20260921/help-verify-issue.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 63 | `~/.cache/z3-dry-run-20260921/help-verify-issue.stdout` | server-only | 1461 | `422dcc2e16efcbdfc0a4da4a219d1a99ce1dcc2934ddcf1318e9e203e501f212` | Dry run 1 | C |
| 64 | `~/.cache/z3-dry-run-20260921/help-verify-redeem.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 65 | `~/.cache/z3-dry-run-20260921/help-verify-redeem.stdout` | server-only | 952 | `d59015f9c556412483b27b4086338bf9fceb868ff3893557cb0167470e4117ca` | Dry run 1 | C |
| 66 | `~/.cache/z3-dry-run-20260921/help-wrap-operation.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 67 | `~/.cache/z3-dry-run-20260921/help-wrap-operation.stdout` | server-only | 407 | `fad582a0018b584bd48054b23fbc594ce108c0c3d7e377d76572c9c5c7099d7f` | Dry run 1 | C |
| 68 | `~/.cache/z3-dry-run-20260921/help-wrapper-run.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 69 | `~/.cache/z3-dry-run-20260921/help-wrapper-run.stdout` | server-only | 1383 | `8908358a9c9912278824bf4e330705a95ac1283fc88790d2a2b8fd38607ddde4` | Dry run 1 | H |
| 70 | `~/.cache/z3-dry-run-20260921/inputs-reviewed.json` | server-only | 5806 | `d6e17640c304b04983cc5c9e5ba12ac6687d8af91e12faaa547cb2429559e80f` | Dry run 1 | C |
| 71 | `~/.cache/z3-dry-run-20260921/inputs.json` | server-only | 5767 | `03932c60e6d548054f158aae167efc21770978c964952ef1d6c74738598f7661` | Dry run 1 | C |
| 72 | `~/.cache/z3-dry-run-20260921/invocation.txt` | server-only | 858 | `43b7e2d10a571a585d79fdb03774f6e6f461f26d3b03764ccde6020610808e21` | Dry run 1 | P |
| 73 | `~/.cache/z3-dry-run-20260921/mkdocs-final.log` | server-only | 22370 | `72954737c88cdaf10c26424bdb10697545884fe5587c5b4ca3691db16c2fc949` | Dry run 1 | C |
| 74 | `~/.cache/z3-dry-run-20260921/mkdocs.log` | server-only | 22370 | `6321918aa5e34666109a44ca9e76317e18c8335715914dd71d19d629484d55b0` | Dry run 1 | C |
| 75 | `~/.cache/z3-dry-run-20260921/public-doc-links-final.log` | server-only | 40 | `a4bcfe01577f9a5d98ab528b7058f4a639f823fb27f3f8c6edbfcc115f94477c` | Dry run 1 | C |
| 76 | `~/.cache/z3-dry-run-20260921/public-doc-links.log` | server-only | 40 | `a4bcfe01577f9a5d98ab528b7058f4a639f823fb27f3f8c6edbfcc115f94477c` | Dry run 1 | C |
| 77 | `~/.cache/z3-dry-run-20260921/public-secret-scan-final.log` | server-only | 44 | `a0f42a415b4d46673441340f7f32729983cdc4ba9b5c35a7f7da9554e663e725` | Dry run 1 | L |
| 78 | `~/.cache/z3-dry-run-20260921/public-secret-scan.log` | server-only | 44 | `a0f42a415b4d46673441340f7f32729983cdc4ba9b5c35a7f7da9554e663e725` | Dry run 1 | L |
| 79 | `~/.cache/z3-dry-run-20260921/resolution.json` | server-only | 11747 | `9e374fe8eaed05d613724dedeea59eeef25974db9917ddf306a1c5aa0b07e6a5` | Dry run 1 | P |
| 80 | `~/.cache/z3-dry-run-20260921/resolve.py` | server-only | 5986 | `d740d18861e99dab613285b7235af19db890af869af8565725961f6e13d310e9` | Dry run 1 | H |
| 81 | `~/.cache/z3-dry-run-20260921/reviewed-dry-run.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 1 | C |
| 82 | `~/.cache/z3-dry-run-20260921/reviewed-dry-run.stdout` | server-only | 61 | `3e3e2bf04e01d45bcd82c3b6018129c0b650963c043d8e9c79a29e1283db5b0b` | Dry run 1 | C |
| 83 | `~/.cache/z3-dry-run-20260921/reviewed-invocation.txt` | server-only | 867 | `8360a89930d21f6394300c24e8912f6a723eb8c7745b233adee56549760f0d73` | Dry run 1 | P |

### Dry-run captures, second run — server-only

| # | File | Hashed at | Bytes | SHA-256 | Cited by | Scan |
| ---: | --- | --- | ---: | --- | --- | :---: |
| 84 | `~/.cache/z3-dry-run-fix-20260921/dry-run.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 85 | `~/.cache/z3-dry-run-fix-20260921/dry-run.stdout` | server-only | 22573 | `bf859e8f4908135ae47a12ce4713945a1f2d0e90810e761137fde8d0598d6e71` | Dry run 2 | P |
| 86 | `~/.cache/z3-dry-run-fix-20260921/help-1.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 87 | `~/.cache/z3-dry-run-fix-20260921/help-1.stdout` | server-only | 241 | `17ff76f39f1ac09da982a3f4c25c5e6e91b80594f6c6d2b30c916bcf612a192e` | Dry run 2 | C |
| 88 | `~/.cache/z3-dry-run-fix-20260921/help-10.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 89 | `~/.cache/z3-dry-run-fix-20260921/help-10.stdout` | server-only | 1259 | `231c321463c4eada46013aa243ca3788faa45e29e7890fcf7fb5fee7e30e7ad8` | Dry run 2 | H |
| 90 | `~/.cache/z3-dry-run-fix-20260921/help-11.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 91 | `~/.cache/z3-dry-run-fix-20260921/help-11.stdout` | server-only | 437 | `a614bf37a6ed88e6f7aefe347561c1d87556aba8ee34602155226b76c7a26bda` | Dry run 2 | C |
| 92 | `~/.cache/z3-dry-run-fix-20260921/help-12.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 93 | `~/.cache/z3-dry-run-fix-20260921/help-12.stdout` | server-only | 867 | `11dd14c84178bdd353aa87f902ea089f588e44f072e48a84b0e21c1ef3f10a4c` | Dry run 2 | H |
| 94 | `~/.cache/z3-dry-run-fix-20260921/help-13.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 95 | `~/.cache/z3-dry-run-fix-20260921/help-13.stdout` | server-only | 1203 | `acaecd64a0fd81dbc478396b6c8835f60511a1bb63274a3de80592f37132e263` | Dry run 2 | H |
| 96 | `~/.cache/z3-dry-run-fix-20260921/help-14.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 97 | `~/.cache/z3-dry-run-fix-20260921/help-14.stdout` | server-only | 952 | `d59015f9c556412483b27b4086338bf9fceb868ff3893557cb0167470e4117ca` | Dry run 2 | C |
| 98 | `~/.cache/z3-dry-run-fix-20260921/help-15.stderr` | server-only | 50419 | `51f1addf09b63945c58279acf9a707e8aca0070c83a19b6ac50068f255afab3a` | Dry run 2 | H |
| 99 | `~/.cache/z3-dry-run-fix-20260921/help-15.stdout` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 100 | `~/.cache/z3-dry-run-fix-20260921/help-16.stderr` | server-only | 50427 | `b61dbc4431570b1944c01d0b50a085f783e8a0ffedb615f37bec96a66943945a` | Dry run 2 | H |
| 101 | `~/.cache/z3-dry-run-fix-20260921/help-16.stdout` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 102 | `~/.cache/z3-dry-run-fix-20260921/help-17.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 103 | `~/.cache/z3-dry-run-fix-20260921/help-17.stdout` | server-only | 567 | `f0ace149cc25e3b472fb37e931239fa734a8203e45a8a9d04ffde7113dfef665` | Dry run 2 | C |
| 104 | `~/.cache/z3-dry-run-fix-20260921/help-18.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 105 | `~/.cache/z3-dry-run-fix-20260921/help-18.stdout` | server-only | 17940 | `fe824aa6b8b65293a29a239e6bf5bb67981917a5b08b5b0359fabbb0b8f75051` | Dry run 2 | H |
| 106 | `~/.cache/z3-dry-run-fix-20260921/help-19.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 107 | `~/.cache/z3-dry-run-fix-20260921/help-19.stdout` | server-only | 252 | `2e69a8871a76700bb029c004fe54e869f279f143a5252e90395560592ca110cd` | Dry run 2 | C |
| 108 | `~/.cache/z3-dry-run-fix-20260921/help-2.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 109 | `~/.cache/z3-dry-run-fix-20260921/help-2.stdout` | server-only | 16335 | `3ed4d0561d8a2967c46848b3f2bfdb815bab62c0568e294bd14ad127ee76a8dd` | Dry run 2 | H |
| 110 | `~/.cache/z3-dry-run-fix-20260921/help-20.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 111 | `~/.cache/z3-dry-run-fix-20260921/help-20.stdout` | server-only | 151 | `0ff01ed6b03183d5b8e287cd39c552892e619c662610221bcf59428e02eb9b77` | Dry run 2 | C |
| 112 | `~/.cache/z3-dry-run-fix-20260921/help-3.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 113 | `~/.cache/z3-dry-run-fix-20260921/help-3.stdout` | server-only | 900 | `16ecda22ed71241bc0e9ed42f7b1ce088dba3bca37d6e2197cd9044c289c7e3f` | Dry run 2 | C |
| 114 | `~/.cache/z3-dry-run-fix-20260921/help-4.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 115 | `~/.cache/z3-dry-run-fix-20260921/help-4.stdout` | server-only | 309 | `93d0dd850a8d9d6ddd01e612e248304c8e87e672ac61715be37a3f0fce15725b` | Dry run 2 | C |
| 116 | `~/.cache/z3-dry-run-fix-20260921/help-5.stderr` | server-only | 50440 | `bb8caa501d96b271e4de572abe3f6f4ec0fa665fb384627c7c35f8f33fc66b16` | Dry run 2 | H |
| 117 | `~/.cache/z3-dry-run-fix-20260921/help-5.stdout` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 118 | `~/.cache/z3-dry-run-fix-20260921/help-6.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 119 | `~/.cache/z3-dry-run-fix-20260921/help-6.stdout` | server-only | 407 | `fad582a0018b584bd48054b23fbc594ce108c0c3d7e377d76572c9c5c7099d7f` | Dry run 2 | C |
| 120 | `~/.cache/z3-dry-run-fix-20260921/help-7.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 121 | `~/.cache/z3-dry-run-fix-20260921/help-7.stdout` | server-only | 1565 | `afe02497aca3719cf77b52a6eda7b838a581908ab069ca72b1dcba7ca38a5105` | Dry run 2 | H |
| 122 | `~/.cache/z3-dry-run-fix-20260921/help-8.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 123 | `~/.cache/z3-dry-run-fix-20260921/help-8.stdout` | server-only | 1326 | `386cdbdc830535a745261345adc613134efb1b43484bde5ed0200f42e873009f` | Dry run 2 | H |
| 124 | `~/.cache/z3-dry-run-fix-20260921/help-9.stderr` | server-only | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Dry run 2 | C |
| 125 | `~/.cache/z3-dry-run-fix-20260921/help-9.stdout` | server-only | 1461 | `422dcc2e16efcbdfc0a4da4a219d1a99ce1dcc2934ddcf1318e9e203e501f212` | Dry run 2 | C |
| 126 | `~/.cache/z3-dry-run-fix-20260921/help-index.json` | server-only | 204371 | `4da2b2654f1e50b8fb7ae068cbdecf93b0337473afe3328819825d27d29bc7c7` | Dry run 2 | H |
| 127 | `~/.cache/z3-dry-run-fix-20260921/inputs-reviewed.json` | server-only | 5818 | `5cd8d89ae9dc9ed159cbff61e4cacafc2f1a9747c89986f6c68ae35d457497b7` | Dry run 2 | C |
| 128 | `~/.cache/z3-dry-run-fix-20260921/invocation.txt` | server-only | 879 | `eaafdde813c57921876d045976aa5ad0bca7c5e278e68e4acc28dffd3ee16d74` | Dry run 2 | P |
| 129 | `~/.cache/z3-dry-run-fix-20260921/packet/cycle.skeleton.json` | server-only | 7844 | `8779e2b2273c9099790891428fa39554f4c29308ae38faf72dd2b9cb5d67ec15` | Dry run 2 | C |
| 130 | `~/.cache/z3-dry-run-fix-20260921/resolution.json` | server-only | 43954 | `fdd38749597c8b4b1388bbd165f49fa5b85f94e6e4dbd520484233c6257d8003` | Dry run 2 | P |

### Dry-run reference executables — server-only

| # | File | Hashed at | Bytes | SHA-256 | Cited by | Scan |
| ---: | --- | --- | ---: | --- | --- | :---: |
| 131 | `~/.cache/qualify-fix-20260918/binaries/candidate-1` | server-only | 62014976 | `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d` | Dry runs (node reference executable) | B |
| 132 | `~/.cache/release-repair-20260916/arc-identity-20260902/pfusdc-egress-program` | server-only | 5183608 | `8b0f266a035a432ef3c0e4233672d8a15b913cefd4c85ff07f91f008601bb744` | Dry runs (Arc egress ELF); G2 (egress ELF SHA-256) | B |
| 133 | `~/repos/postfiatl1v2-arcusdc-current/tools/pfusdc-tier4-prover/target/release/pfusdc-tier4-prover` | server-only | 77366688 | `1372ad0c0cd5c4364247c7891e17949a6855c08c33757ffddad3219db10334e1` | Dry runs (prover reference executable) | B |

## Secret and forbidden-field scan

**Forbidden-field list.** The plan names no field list. It requires redaction-safe artifacts and bars private keys and key material from evidence. The [NRRS specification](../deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md#234-privacy) requires a "private evidence forbidden-field scan" in section 23.4 but names no fields. The operational list used was the Z3 cycle verifier's `SECRET` field-name rule in `python/postfiat_rpc/z3_cycle.py`. It covers private key, secret key, password, mnemonic, seed phrase, keystore, ciphertext, crypto and key file.

**Method.**

1. `scripts/public-secret-scan`, unchanged. The 130 non-executable files were placed in a scratch Git index at `/tmp/z3-g0-20260923/scan1`: in-repo blobs through `git cat-file blob`, server-only files by copy. Then `POSTFIAT_SECRET_SCAN_ROOT=/tmp/z3-g0-20260923/scan1 scripts/public-secret-scan` printed `public secret scan passed mode=tracked-tree` with zero findings.
2. A targeted, case-insensitive byte scan over all 133 files: `-----BEGIN … PRIVATE KEY-----`, `private[_ .-]?key`, `secret`, `mnemonic|seed[_ .-]?phrase|master[_-]?seed|\bseed\b`, `passw(or)?d|passphrase`, `keystore|ciphertext|kdfparams|"crypto":`, `api[_-]?key|access[_-]?token|auth[_-]?token|bearer|authorization:|xprv|xpriv`, `spending/signing key` and `key.?file`. Each hit was classified by the token after it: literal value, `$VARIABLE`, path, placeholder or flag text. Only masked excerpts were displayed.
3. The executables that the tool skips were searched for PEM private-key headers and for header plus base64 key blocks.

The password file and keystore were not opened. Their contents were not compared by value. Instead, the scan confirmed that no file has keystore JSON structure (`ciphertext`, `kdfparams`, `crypto`) or a password value. Every password hit is a flag name or the path `~/.postfiat/arc-testnet-server.password`.

**Result: no literal secret value, key material, seed or mnemonic, passphrase or password value, token, or keystore content in any of the 133 files.**

| Code | Files | Meaning |
| --- | ---: | --- |
| C | 92 | No hit |
| R | 3 | `$VARIABLE`, placeholder or instruction text only (rows 6, 20, 23) |
| P | 10 | Signer, keystore or password-file path argument, or `key_file` path field; no contents (rows 9, 11, 24, 25, 72, 79, 83, 85, 128, 130) |
| H | 23 | CLI help text or flag names only |
| L | 2 | The tool name in a `public-secret-scan` log line |
| B | 3 | Executable; no PEM private-key header or key block |

Rows 9 and 11 carry a `key_file` field whose value is a path. Row 23 carries `publisher_key_file` with a placeholder value. The Z3 verifier would reject those field names inside a future public cycle packet. These baseline fixtures are not cycle packets, and neither the plan nor the spec forbids path fields. The P-coded server-only captures repeat the Arc keystore, password-file and placeholder signer paths already published in the dry-run record.

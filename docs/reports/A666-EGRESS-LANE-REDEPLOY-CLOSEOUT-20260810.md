# A666 Egress Lane Redeploy Closeout — 2026-08-10

Status: **PASS / CLOSED** at 2026-08-10 03:35:45Z.

This closes the execution plan in
`docs/plans/A666-EGRESS-LANE-REDEPLOY-PLAN-20260809.md`. The old proof lane remains paused
and economically written down. A distinct governed successor lane completed a fresh,
proof-native 10,000,000-atom USDC round trip and returned both user balances and the new vault
to their exact starting values.

## 1. Final identities

| Item | Final value |
|---|---|
| PFTL asset | `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b` |
| Successor verifier | `0xA53926F0F7453ad9f8dCa592A076991eC627838C` |
| Successor vault | `0x4939a45caa85Da31Fb26D7DBe6477B45F7f08688` |
| Egress vkey | `0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87` |
| Egress ELF SHA-256 | `4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0` |
| Ingress vkey | `0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e` |
| Route profile hash | `f088876e4bc7f611fdf7199237f241a1bb91ffc1850a8b65cd50a4852cab2ec40a2fae18c6dbf0ee5dd4934b22107f1a` |
| Enriched manifest SHA-256 | `b213a7462ba5495977a6795376dada0a56d48ec58991b1821f2cb2463e7532be` |

The first epoch-6 deployment (`0xd219…1734` verifier, `0x2604…69Ea` vault) was quarantined
after its one governed registration attempt rejected the exact route-profile mismatch at h793.
No retry was made. The identities above are the distinct successor that passed registration and
the live round trip.

## 2. Host-versus-guest finding

The old h544-to-h608 witness is SHA-256
`f4e1d7528605900a608f6091af0159fcb6af2bf966a3006a639dba2d25db26c1` and native current-code
verification passes it. The deployed verifier still cannot accept it because its immutable vkey
`0x0026a156…a02ca89b` corresponds to the pinned ELF
`ea0d3ef37ade9e2413646c8051b58f8e8123516e75da0937a8d47d4d9586f2fe`, while the fresh build
produces ELF `4d5f8449…761c67e0` and vkey `0x0015b046…a9d87`.

The earlier log's deployed-vkey line was a measurement of the checked-in/pinned ELF, not the
fresh build. This refutes the stale-host-only theory and makes the old lane permanently unusable
past h544. The source log remains at
`/tmp/a666-owner-20260809/egress-guest-rebuild.log` (SHA-256
`65f962544780e88b9727a0a3375ad347ca7b7e94c7df4b5ec0badbc6f2747bdf`); the recheck verdict is
`/tmp/a666-owner-20260809/vkey-recheck/RESULT.txt` (SHA-256
`055b09db08628f308160f6a6c2d79ba16510a171dc84a062ffa99146dad186e2`).

## 3. Old-lane accounting closure

- Impairment tx: `c2fefc40598fc9169fe2b1dead1e0283de55e89ad65eea85d88f854257582170ee23cadf230c145a292d3adb42ee84f8`.
- Finalized: h792, six-validator convergence.
- Old bucket `5d5abc04…3665c536`: counted value 195,031,396 -> 0 atoms;
  impairment factor 10,000 -> 0 bps.
- Old vault `0xaaa78fda…bc8183`: remains paused with 195,031,396 USDC atoms stranded.
- Old redemption `b3651dd4…3a931d5b`: 9,932,863 atoms, settled 0, permanent pending tombstone.

This is an explicit 195,031,396-atom write-down, not a claim that the inaccessible USDC moved.

## 4. Fresh round-trip receipts

| Leg | Receipt |
|---|---|
| Route governance | h795; amendment `47c26875…d69f`; certificate `ae33f790…65e346` |
| Ethereum approval | `0x0b49da948044e8e3607b5f5fb1fb511d486290aaf94a41a2bccffe98d72db6ac`, block 25,721,709 |
| Ethereum deposit | `0xb19ca77a83e6c329beaa44bc68403a3f58c37a24966eb456be386bcf6d2dd60f`, block 25,721,711 |
| Deposit ID | `0x86bbc86df9fda5d9ae4858469a9403b8174327e259965a99ee9567d5679e9b74` |
| PFTL ingress | propose h796, finalize h797, claim h798; +10,000,000 pfUSDC atoms |
| PFTL burn | `dc4bb0686ee97c999d718894f8ef4e089bb6e6cc33960797f63f7411710d01d61675a5ca2ecb8cfae62a1c0ff6e70026`, h799 |
| Ethereum withdrawal | `0xf0003bda41670668bfc6be8d17f7dd13d12d5bb8209313a6a56ffa1f3badb5fc`, block 25,721,902; +10,000,000 USDC atoms |
| PFTL settlement | `1094c18cf1f4985545da9c8ba7b45e6dcd9703ec2cc4367bec6d2cbaf6480e33b17a8a2d06b5c27553cd2966f019c1ce`, h800 |
| Settlement receipt | `c1adbb035928f70be7eeb16ffa5e7906373736830cf4e4f1bf4837da9ed3b5917a87bda8614787f442009103ee296251` |

The withdrawal, burn, and proof nullifiers are consumed. A read-only replay call rejected.

## 5. A6 conservation

### Atom ledger

| Conservation surface | Before | Movement | After | Residual |
|---|---:|---:|---:|---:|
| Wallet USDC | 74,161,443 | -10,000,000 + 10,000,000 | 74,161,443 | 0 |
| Successor vault USDC | 0 | +10,000,000 - 10,000,000 | 0 | 0 |
| Successor obligations | 0 | +10,000,000 - 10,000,000 | 0 | 0 |
| Joe pfUSDC | 1,358,493 | +10,000,000 - 10,000,000 | 1,358,493 | 0 |
| Successor bucket counted value | 0 | +10,000,000 - 10,000,000 | 0 | 0 |
| Successor redemption queue | 0 | +10,000,000 - 10,000,000 | 0 | 0 |
| Protected wallet wA666 | 103,000,000 | 0 | 103,000,000 | 0 |
| Old bucket truthful accounting | 195,031,396 counted | -195,031,396 write-down | 0 counted | 0 |

The old-lane physical vault balance and write-down are deliberately separate: the inaccessible
195,031,396 atoms remain at the paused old vault while counted backing decreased by the exact same
195,031,396 atoms.

### Wei and provider ledger

| Item | Exact cost |
|---|---:|
| Successor verifier deployment gas | 292,933,145,500,918 wei |
| Successor vault deployment gas | 116,260,225,441,696 wei |
| Deployment gas total | 409,193,370,942,614 wei ($0.76703297383193 at the recorded price basis) |
| Round-trip approval gas | 5,382,249,808,572 wei |
| Round-trip deposit gas | 19,162,763,443,200 wei |
| Round-trip withdrawal gas | 50,321,347,514,490 wei |
| Round-trip gas total | 74,866,360,766,262 wei |
| Wallet ETH during round trip | 278,901,454,238,316,062 -> 278,826,587,877,549,800 wei; delta exactly -74,866,360,766,262 wei |
| Successor binary-build GPU | $0.202118827 |
| Round-trip proof GPU | $0.4973481481; instance 47326743 destroyed |

## 6. Terminal reconciliation

- PFTL: all six validators at h800, state root
  `6f2919796b22b1f39c67556cb041de83010d53fc122d174b6e491fcccd351f82d11751e6e0b1a19db6700bbf92e9cb6c`,
  tip `f6e97f10028fd635791d1076b214ae9937024c5e3755899e3bcc33005d5b2c31318f5a560c2977b5d34278b31d83c969`,
  mempool 0/0/0/0/0/0.
- Ethereum: PublicNode and dRPC independently report successor finalized height 799, wallet
  USDC 74,161,443, successor vault USDC 0, and wallet wA666 103,000,000.
- Proofs: ingress witness SHA-256
  `e73f863daa4a83a320fd01be02151b9b577a4d891d6517ef88802d956e950718`; egress witness SHA-256
  `86bb388234061802226f7ab9a018231a5d138732e860c9aa0ea7a0915929f154`; CUDA prover binary
  SHA-256 `42a72b06e1a7c763400942392ff9099a62acffbafc56d2bbc4758afc99454d43`.

## 7. Custody and continuous-auth correction

The live agent was already unlocked. The two exact successor calls used bounded launch sessions;
there was no new passphrase, no global-policy mutation, no service restart, and no relock.

The source defect was that unlock decrypted the vault but discarded the derived encryption
capability, forcing `set_policy` and Hyperliquid agent approval to ask for the passphrase again.
The fix retains the derived key and salt only while unlocked, zeroizes both on lock/wipe, and
persists policy through that live capability. The CLI no longer prompts for the policy command.
`pytest -q tests/test_agent.py` passes 25 tests. The running service was intentionally not
restarted, so its existing unlocked lifetime was not disturbed.

## 8. Prevention controls

1. Checkpoint freshness is an automated, fail-closed money-path alarm:
   `scripts/pfusdc-mainnet-latency-gate.py` rejects predeposit execution if the Ethereum
   verifier trails the PFTL tip by more than one block. Its focused test suite is part of the
   closeout checks.
2. Every future bridge campaign must record a day-zero matrix of each money-path contract,
   runtime code hash, immutable verifier/vkey, route epoch/profile hash, and current chain
   software/block-ID domain before any deposit is enabled.
3. Any change to block-ID derivation is bridge-affecting. It must ship with a handover plan
   containing a new route epoch, current-state-seeded verifier, new vault when immutable
   bindings require it, proof rehearsal, and pause/drain/write-down disposition for the old
   lane before money flows.

## 9. Evidence retention

The raw deployment, governance, round-trip, reconciliation, prover, and custody
receipt tree was removed from the working documentation tree. Git history
retains the exact closeout artifacts; this report is the curated summary.

Task Node task: `task_0f8d57dcc1dab7228ce8ff8792b50fe3`.

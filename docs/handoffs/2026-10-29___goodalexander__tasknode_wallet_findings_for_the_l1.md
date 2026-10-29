# Task Node wallet findings for the L1

- **Operator:** Alex (`goodalexander`), Task Node wallet build
- **Date:** 2026-10-29 UTC

## BLUF

The Task Node Mac app and its PFTL v2 wallet are being built in the private repo `postfiatorg/tasknode-apple`. That work changed no code in this repository. The app pins this repository at `48ecefe` and reads from it. Four findings need the L1 team:

1. The live ingress driver pins the hash of the ingress permit file, so changing the permit breaks bridge readiness.
2. One USDC deposit from the wallet's live test sits in the Ethereum vault and has not been credited on ce22.
3. Two gateway changes, W-1 and W-2, are needed before the app can run private swaps and native sessions on ce22.
4. There is a proposed issue on Asset-Orchard secret lifetimes.

None of these blocks the exchange page or the Base token work.

## Current state

**1. The ingress permit is pinned by the driver.** The live bridge ingress for `ethereum-mainnet-usdc-v1` runs `wallet-proxy/eth-fast-lane-driver.js` from the `navcoin-p0-d55-native-quorum` worktree at `299be0600`, configured by `navcoin-p0-native-quorum-runtime-D55/configs/ingress-driver.json`. That config's `pinned_files` (checked at `eth-fast-lane-driver.js:129–131`) include `navcoin-p0-relay-runtime-D55/configs/ingress-authorization.json` at SHA256 `123f5018…cdc223`.

- Any change to the permit file, even a correct one, makes `readiness` exit 2 ("terminal driver failure"). The proxy then reports `trustless_ingress_unavailable`, and `POST /api/bridge/jobs` returns 503 after the action has been durably admitted.
- This happened from 00:00 to 00:47 UTC on 2026-10-29. A planned test swapped the permit to admit the test wallet. Restoring the exact original bytes returned readiness to `ready: true` within 20 seconds.
- No other ingress was attempted in that window. The permit is back at `123f5018…`, mode 0600.
- Changing which wallets may deposit therefore also needs a matching change to the driver's pin set, or a permit mechanism outside the pinned bytes.

**2. One deposit is not yet credited.**

- **Deposit:** `depositV2` tx `0x6998541288e57b85490a4843901134c2c8399f0a86b56fedbaa5250153d8884e`, finalized in Ethereum block 26294764.
- **Amount and route:** 1 USDC (1,000,000 atoms) from `0x64c514851927b019aaaf14dbeff7544f662c3fab` to `pfbfe576f09313158a62c74dd50bc93d209ed1caa3`, route epoch 10.
- **Deposit id:** `0x338850111c8ffc6879e16e774eefc295834113761899935a6f10bf2bbd16c163`.

It is in the vault's `depositRecords` with `depositSeen = true`. The deployed `ERC20BridgeVaultL1` has no expiry, delete or refund path, so the deposit can be proven and claimed later without another transfer.

Two things block the claim today:

- The proxy's admission record for it has no result: `~/.local/state/postfiat-wallet-relay-security/actions/39b21bec….json` on the operator host. A resubmit returns 409 `relay_action_already_admitted` until that record is handled.
- The test wallet is not in the ingress permit (finding 1).

The wallet team is preparing a reviewed recovery plan (`HOLD-tnapp-v2-d15b`) and will not touch the record or the permit without it.

**3. Gateway changes needed for the app on ce22.** These are W-1 and W-2 in the wallet spec.

- **W-1:** shielded rounds must accept user-proven A666 and pfUSDC actions without the custodial `pftl_swapd` wallet or a proxy-held validator key. Today rounds are certified by a proxy holding a validator key (`navswap-config-bridge.js:780–835`).
- **W-2:** the gateway needs a session API for native clients, with a per-install token and the same caps as the browser relay. Today the session is loopback- and IP-based, and remote native sessions get 403.

The app side of private swaps is built. On a six-validator owned network, real Halo2 proofs ran on an Apple-silicon Mac: shield, then a private A666 swap, then unshield, all accepted on all six validators. A corrupted proof was rejected before proposal. Proof time was about 30 s after a 4-minute prewarm, with about 2.1 GB peak memory. This is evidence from a local test network, not ce22.

Separately, the wallet proxy refuses `/eth-rpc` for wallet sessions (`relay-security.js:135`). So wallet clients broadcast their own Ethereum transactions to public RPCs. The app does this, and it is the expected design.

**4. Asset-Orchard secret lifetimes.** The issue text is in `tasknode-apple` `docs/upstream-issues.md`. In brief:

- `build_asset_orchard_swap_action` takes output seeds as owned `String`s.
- `AssetOrchardPublicNoteOpening` derives `Debug`/`Clone` and holds custody material without a zeroizing owner.
- Some witness and curve intermediates are plain values.
- `verify.rs:2648–2652` builds a `Copy` orchard `SpendingKey`.

These are source-level lifetime gaps, not a demonstrated exploit. The existing wire formats should stay exactly as they are.

## Next decision or action

- Decide how operators should change the ingress allowlist without breaking the driver pin: pin a separate permit file, or move the permit out of `pinned_files`.
- Agree the recovery of deposit `0x3388…c163`: who clears the admission record and how. The wallet team's D15b plan will propose exact steps for review.
- Schedule W-1 and W-2. The app's private-swap and native-session paths are ready to integrate once they exist.
- File or reject the Asset-Orchard lifetime issue.

## References

- Wallet spec (operator host, `~/repos/orc_directives/`): `TASKNODE-PFTL-V2-WALLET-SPEC-20261028.md`, section 6 (W-1, W-2).
- Decisions D19 (direct Ethereum broadcast) and D20 (ingress pin, stopped run), operator host: `orc_directives/NAZGUL-DECISIONS-tnapp.md`.
- Stopped-state verification on two validators and two Ethereum providers: `orc_directives/TNAPP-V2-D20-VERIFICATION-20261029.json` (SHA256 `168bc106…a30b`).
- Private-swap evidence: `orc_directives/TNAPP-V3-HTTP-5c6db86-ACCEPTANCE.json`; CI run 40789295056 in `postfiatorg/tasknode-apple`.
- Upstream issue text: `postfiatorg/tasknode-apple` `docs/upstream-issues.md`.
- Driver: `wallet-proxy/eth-fast-lane-driver.js` (branch `navcoin-p0-d55-native-quorum`, `299be0600`).

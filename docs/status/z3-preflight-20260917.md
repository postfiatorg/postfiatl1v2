# Z3 preflight — 2026-09-17

Read-only fact sheet for the operator’s first integrated-cycle decision under the [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md). **G1/G4 authorization remains open; Z3 remains OPEN.** No transaction, signature, restart, deployment, host-file write, chain mutation, or Task Node action was performed. Neither wallet secret file was opened. Local documentation and Git publication are the authorized writes.

Capture times below are exact UTC response/command completion times: RPC times use the collecting server’s clock; systemd and `/proc` times use each validator host’s clock. The fleet observations span `2026-09-17T11:16:49.908219Z`–`2026-09-17T11:32:07.540468Z`. Arc state reads are pinned to block `62561207` (`0x3ba9bb7`), obtained at `2026-09-17T11:18:24.190209Z`. These observations do not replace the fresh route, NAV, capacity, proof-key, and balance checks required before an authorized cycle.

## Fleet

All six agree on chain, genesis, height, tip, state root, and empty mempools. All six probes pass. All 12 validator/RPC processes are active/running and match the September 14 [fleet baseline](chain-state-current.md); height has advanced by zero blocks.

| Common identity | Read-back value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Release | `a666-source-route-20260907` |
| Executable on all 12 processes | `/opt/postfiat/releases/a666-source-route-20260907/postfiat-node` |
| Executable SHA-256 on all 12 processes | `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83` |
| RPC-reported build revision | `707e006f` (reported metadata; not the operator-selected qualified lineage) |

| Validator | SSH host / remote loopback RPC | Height | Tip | State root | Mempool pending |
| --- | --- | --- | --- | --- | --- |
| validator-0 | `64.176.220.75` / `127.0.0.1:27650` | 1020 | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` | 0 |
| validator-1 | `95.179.184.122` / `127.0.0.1:27651` | 1020 | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` | 0 |
| validator-2 | `66.42.48.39` / `127.0.0.1:27652` | 1020 | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` | 0 |
| validator-3 | `149.28.63.106` / `127.0.0.1:27653` | 1020 | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` | 0 |
| validator-4 | `95.179.179.206` / `127.0.0.1:27654` | 1020 | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` | 0 |
| validator-5 | `45.32.110.170` / `127.0.0.1:27655` | 1020 | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` | 0 |

The probe uses the existing Python `postfiat_rpc.rpc_probe` implementation on main, with an eight-second timeout, through an authenticated SSH forward to the listed loopback endpoint. Round-trip milliseconds include forwarding and RPC processing; SSH setup is outside the probe measurement. Each probe returned height `1020` and tip prefix `9d02b8eecb78`. The health methods were only `status`, `server_info`, and `mempool_status`.

| Validator | status UTC | server_info UTC | mempool_status UTC | Probe UTC | Probe result / round-trip ms |
| --- | --- | --- | --- | --- | --- |
| validator-0 | `2026-09-17T11:16:55.198346Z` | `2026-09-17T11:17:01.204520Z` | `2026-09-17T11:17:01.417221Z` | `2026-09-17T11:16:54.966078Z` | PASS / 737 |
| validator-1 | `2026-09-17T11:20:49.035070Z` | `2026-09-17T11:20:55.057378Z` | `2026-09-17T11:20:55.149767Z` | `2026-09-17T11:20:48.967052Z` | PASS / 543 |
| validator-2 | `2026-09-17T11:20:51.003072Z` | `2026-09-17T11:20:58.615953Z` | `2026-09-17T11:20:59.397483Z` | `2026-09-17T11:20:49.703375Z` | PASS / 1276 |
| validator-3 | `2026-09-17T11:20:49.361453Z` | `2026-09-17T11:20:55.725750Z` | `2026-09-17T11:20:56.040644Z` | `2026-09-17T11:20:49.129198Z` | PASS / 702 |
| validator-4 | `2026-09-17T11:20:49.015472Z` | `2026-09-17T11:20:55.146778Z` | `2026-09-17T11:20:55.238815Z` | `2026-09-17T11:20:48.947959Z` | PASS / 521 |
| validator-5 | `2026-09-17T11:20:50.018028Z` | `2026-09-17T11:32:07.540468Z` | `2026-09-17T11:20:58.526842Z` | `2026-09-17T11:20:49.554167Z` | PASS / 1127 |

Service identity reads used `systemctl show` for `Id`, `MainPID`, `ActiveState`, `SubState`, and `ExecMainStartTimestamp`; `readlink` of `/proc/PID/exe`; and `sha256sum /proc/PID/exe`. Every row below resolved to the common executable and hash above.

| Service | PID | State | Process started UTC | systemctl show UTC | /proc exe UTC | sha256sum UTC |
| --- | --- | --- | --- | --- | --- | --- |
| `postfiat-validator-0.service` | 3611286 | active/running | `2026-09-11T06:16:08Z` | `2026-09-17T11:16:50.813762Z` | `2026-09-17T11:16:50.813842Z` | `2026-09-17T11:16:50.980084Z` |
| `postfiat-validator-0-rpc.service` | 3611291 | active/running | `2026-09-11T06:16:08Z` | `2026-09-17T11:16:50.994802Z` | `2026-09-17T11:16:50.994882Z` | `2026-09-17T11:16:51.165429Z` |
| `postfiat-validator-1.service` | 346281 | active/running | `2026-09-11T06:26:49Z` | `2026-09-17T11:16:49.908219Z` | `2026-09-17T11:16:49.908306Z` | `2026-09-17T11:16:50.098357Z` |
| `postfiat-validator-1-rpc.service` | 346285 | active/running | `2026-09-11T06:26:50Z` | `2026-09-17T11:16:50.113930Z` | `2026-09-17T11:16:50.114006Z` | `2026-09-17T11:16:50.305405Z` |
| `postfiat-validator-2.service` | 889864 | active/running | `2026-09-11T06:01:49Z` | `2026-09-17T11:16:52.553755Z` | `2026-09-17T11:16:52.553850Z` | `2026-09-17T11:16:52.752265Z` |
| `postfiat-validator-2-rpc.service` | 889868 | active/running | `2026-09-11T06:01:50Z` | `2026-09-17T11:16:52.772297Z` | `2026-09-17T11:16:52.772411Z` | `2026-09-17T11:16:52.977458Z` |
| `postfiat-validator-3.service` | 2047345 | active/running | `2026-09-11T06:46:43Z` | `2026-09-17T11:16:50.719396Z` | `2026-09-17T11:16:50.719489Z` | `2026-09-17T11:16:50.925491Z` |
| `postfiat-validator-3-rpc.service` | 2047349 | active/running | `2026-09-11T06:46:44Z` | `2026-09-17T11:16:50.943591Z` | `2026-09-17T11:16:50.943714Z` | `2026-09-17T11:16:51.142255Z` |
| `postfiat-validator-4.service` | 2183058 | active/running | `2026-09-11T06:25:16Z` | `2026-09-17T11:16:50.021077Z` | `2026-09-17T11:16:50.021156Z` | `2026-09-17T11:16:50.295966Z` |
| `postfiat-validator-4-rpc.service` | 2183064 | active/running | `2026-09-11T06:25:17Z` | `2026-09-17T11:16:50.320662Z` | `2026-09-17T11:16:50.320741Z` | `2026-09-17T11:16:50.526093Z` |
| `postfiat-validator-5.service` | 2464290 | active/running | `2026-09-11T06:26:17Z` | `2026-09-17T11:16:51.753611Z` | `2026-09-17T11:16:51.753719Z` | `2026-09-17T11:16:52.051493Z` |
| `postfiat-validator-5-rpc.service` | 2464294 | active/running | `2026-09-11T06:26:18Z` | `2026-09-17T11:16:52.073209Z` | `2026-09-17T11:16:52.073313Z` | `2026-09-17T11:16:52.287764Z` |

The initial forwards incorrectly reused validator-0’s port `27650` on validators 1–5 and were refused; they do not establish service failure. A read of each RPC process’s `/proc/PID/cmdline` selected only its public port argument: validator-0 `27650` at `2026-09-17T11:18:57.943031Z`, validator-1 `27651` at `2026-09-17T11:18:57.164461Z`, validator-2 `27652` at `2026-09-17T11:19:00.198325Z`, validator-3 `27653` at `2026-09-17T11:18:58.039969Z`, validator-4 `27654` at `2026-09-17T11:18:57.211149Z`, and validator-5 `27655` at `2026-09-17T11:18:59.045192Z`. Corrected reads are tabulated above. Validator-5’s retained `server_info` attempt timed out at `2026-09-17T11:20:58.026736Z` after eight seconds; the sole subsequent retry, with a 15-second timeout, succeeded at `2026-09-17T11:32:07.540468Z` and returned the same ledger and empty mempool.

## Arc chain and contract pairs

| Read / fixed endpoint | Value | Capture UTC |
| --- | --- | --- |
| RPC | https://rpc.testnet.arc.network | Supplied endpoint |
| Explorer | https://explorer.testnet.arc.io | Supplied endpoint; not visited |
| eth_chainId | `5042002` / `0x4cef52` | `2026-09-17T11:18:24.037237Z` |
| eth_blockNumber | `62561207` / `0x3ba9bb7` | `2026-09-17T11:18:24.190209Z` |
| USDC system contract | `0x3600000000000000000000000000000000000000` | Supplied identity |
| USDC decimals() via eth_call | `6` | `2026-09-17T11:18:24.283959Z` |

All `eth_getCode`, `eth_call`, `eth_getBalance`, and `eth_getTransactionCount` state reads used block tag `0x3ba9bb7`. Code size is the decoded byte length of `eth_getCode`, excluding `0x`. Every call below succeeded.

| Pair | Contract | Full address | Code exists / bytes | eth_getCode capture UTC |
| --- | --- | --- | --- | --- |
| epoch-7 | anchor | `0x661D558a818A07002C7D5da4A3179c4672FEf124` | yes / 3084 | `2026-09-17T11:18:24.375721Z` |
| epoch-7 | vault | `0xe88fb9ab4890f513261f0aca4ff13bfba3e14862` | yes / 5284 | `2026-09-17T11:18:24.484499Z` |
| current-v2 | anchor | `0x92390D3a2102CB74E4746c05B4d91F61093475D0` | yes / 3084 | `2026-09-17T11:18:25.266274Z` |
| current-v2 | vault | `0x160307f3efead79b6a3629c4b8d90e8301fc250f` | yes / 5284 | `2026-09-17T11:18:25.366704Z` |

The getter target matters: `owner()` and `directIngress()` belong to the vault; `routeEpoch()` and the egress `programVKey()` belong to the address returned by its `finalityVerifier()`; `governedRouteBinding()` belongs to the anchor. Vault USDC uses `balanceOf(vault)` on the system token. Addresses below preserve the full G2 pair identities; getter-returned addresses are lowercase.

| Pair | Getter / target | Read-back value | eth_call capture UTC |
| --- | --- | --- | --- |
| epoch-7 | owner() / vault | `0xdb9b78c87f76054b204188109b35ce4614d03814` | `2026-09-17T11:18:24.578502Z` |
| epoch-7 | directIngress() / vault | `true` | `2026-09-17T11:18:24.702819Z` |
| epoch-7 | finalityVerifier() / vault | `0xc59ebed2a65b26e203f14c445b904dcf5f1b686b` | `2026-09-17T11:18:24.793680Z` |
| epoch-7 | routeEpoch() / returned verifier | `7` | `2026-09-17T11:18:24.883230Z` |
| epoch-7 | programVKey() / returned verifier | `0x00c8d744e19bc828d1b3fb19709d36863d8c5aba14af0ca939eb85fc806f868f` | `2026-09-17T11:18:24.978966Z` |
| epoch-7 | governedRouteBinding() / anchor | `0x6edfa31c57cfeec8955572fd9cdb81b22222beb1dbac432ff7c7f0fc7ad9c520` | `2026-09-17T11:18:25.110372Z` |
| epoch-7 | balanceOf(vault) / USDC | `1000000` atoms = **1.000000 USDC** | `2026-09-17T11:18:25.182161Z` |
| current-v2 | owner() / vault | `0x0995876e5a97c036c0fda8846f8f47b57b6d2bfc` | `2026-09-17T11:18:25.456607Z` |
| current-v2 | directIngress() / vault | `true` | `2026-09-17T11:18:25.564957Z` |
| current-v2 | finalityVerifier() / vault | `0x1d436908516d15e3c55a936899b47a885e047f27` | `2026-09-17T11:18:25.654234Z` |
| current-v2 | routeEpoch() / returned verifier | `9` | `2026-09-17T11:18:25.766005Z` |
| current-v2 | programVKey() / returned verifier | `0x0036cbe7d36bbfe1118a3c544eeba74f3791d2a19bf5ec59b972f72d36416852` | `2026-09-17T11:18:25.861880Z` |
| current-v2 | governedRouteBinding() / anchor | `0xd9e0cd409c5d1e118d65c78ee059adcbba937616353e9675e350d52ee8d498b2` | `2026-09-17T11:18:25.950820Z` |
| current-v2 | balanceOf(vault) / USDC | `1000000` atoms = **1.000000 USDC** | `2026-09-17T11:18:26.042912Z` |

The epochs (`7` and `9`), egress keys, and anchor route bindings match [G2’s identities read back](../review/z3-g2-route-compatibility-20260917.md#identities-read-back). The **2026-09-02 evidence used current-v2, epoch 9**, anchor `0x92390D3a2102CB74E4746c05B4d91F61093475D0` and vault `0x160307f3efead79b6a3629c4b8d90e8301fc250f`. This is the existing G2 historical identification, read at `2026-09-17T11:19:51.649547Z`; it does not select the future Z3 pair. Code presence and vault balances do not establish usable A666 settlement capacity.

## Wallet

Public address: `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB`.

| Read | Value | Capture UTC |
| --- | --- | --- |
| eth_getBalance(address, block) | `20000000000000000000` native units (`0x1158e460913d00000`) = **20.000000 USDC** at 18 native decimals | `2026-09-17T11:18:26.252646Z` |
| USDC balanceOf(address) via eth_call | `20000000` six-decimal USDC atoms = **20.000000 USDC** | `2026-09-17T11:18:26.339514Z` |
| eth_getTransactionCount(address, block) | **0** (`0x0`); confirmed nonce at the pinned block | `2026-09-17T11:18:26.441618Z` |
| eth_getTransactionReceipt(faucet transaction) | **status 1** (`0x1`), block **62549207** (`0x3ba6cd7`) | `2026-09-17T11:18:26.546444Z` |
| Faucet rate limit documented in repository | No numeric rate limit found in main or the cited candidate’s Arc documentation; unknown. The MVP spec asks that limits be recorded. | `2026-09-17T11:19:51.711071Z`–`2026-09-17T11:19:51.812468Z` |

Faucet transaction: `0x96ef5060b64f2811b531a0b3cb0f8cedab59caee7cd76fd0a49ecd9c8ed95c75`. Its receipt contains both the native `20000000000000000000` and system-token `20000000` transfer representations to this wallet. They represent the same 20 USDC, not additive funds; native gas expenditure draws on that balance. The read-back agrees with the user’s 09:58Z balance. The faucet was not visited or called. Repository limit reference: [Arc MVP specification, USDC conformance](../specs/pfusdc-arc-mvp-testnet-spec-20260828.md).

## Operator inputs still missing for G1/G4

No value below is selected or authorized by this fact sheet. The operator’s dated go must name these values and explicitly authorize one Arc-testnet/PFTL-devnet integrated cycle.

| Input | Value the operator must provide |
| --- | --- |
| [ ] Qualified lineage | Choose candidate `15126ac3408f3afe58cf4e4ed4c31d00efa04366` or another explicitly qualified source commit; name its release ID, binary SHA-256, and qualification/deployment evidence. The observed running `a666-source-route-20260907` identity is recorded above. |
| [ ] Arc pair | Choose **7** or **current-v2 (epoch 9)** and confirm the exact anchor/vault addresses and corresponding verifier, egress key, and route binding above. |
| [ ] A666 route confirmation | Confirm `pftl-a666-ethereum-wA666-usdc-v1 / primary_pftl_mint`, or name an existing-operation-only testnet configuration. Confirm the selected Arc source series is authorized for that route; pin its source custody, profile, policy, and fresh NAV as required by G2/G4. |
| [ ] Per-cycle value cap | An explicit positive integer in six-decimal **USDC atoms** (`1000000` atoms = 1 USDC), with an exact cycle amount within that cap and room for USDC gas. The observed `20000000`-atom wallet balance is not an authorized spend cap. |
| [ ] Seven-day window | Exact UTC start and end timestamps, seven days apart. No campaign dates are selected here; a G4 go authorizes only one cycle, while sustained repetition requires G6 authorization. |
| [ ] Testnet-only confirmation | Explicit scope: Arc testnet chain `5042002` and PFTL controlled devnet `postfiat-wan-devnet-2`; no mainnet or production value. |
| [ ] Signer custody statement | Name who controls signing for public wallet `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB` and the required PFTL accounts, and the operator-controlled signing/confirmation flow. Supply no private key, password, or secret contents. |

This limited preflight establishes fleet health/identity and the listed Arc/wallet facts. It does not tick G1, G2’s selected-state readback, or any G4 box.

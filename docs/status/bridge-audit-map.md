# Bridge audit map

!!! note "2026-11-25: what changed since this audit"

    This page is the audit of 21 November 2026 (Post Fiat height 1739); its tables are unchanged.
    Since then:

    - On 25 November the validators ran build `04600868` (second Solana exit release, rules from
      block 1890), which is on `main` since `26acd4639` (#234). From the evening of 25 November
      they ran `sol-exit-r3` (`2356caf1`, a commit on `main`); since the evening of 26 November
      they run `pq-r3` (`9dcb689e`, a commit of #280's branch, not of `main`); see
      [Master State](master-state.md).
    - The Solana egress verifier was initialised on 23 November and upgraded and migrated on
      24 November. A read-only finalized read on 25 November (slot 473711314) found 127,168 bytes of
      code with SHA-256 `3406becc6db7ba7c250f8db55dc43965656093bb4ceb3e330d2f452edd465fdf`, last
      deployed at slot 473433318. Its first 123,152 bytes have SHA-256
      `4091ee16db29154e7aae642b637320c0a0d0cdee110da88d07416ab7b1c15b21`, the build of the
      verifier source now on `main` recorded in the
      [24 November handoff](../handoffs/2026-11-24___dravlic__second_withdrawal_release_prepared_and_main_checks_green.md);
      the rest is zero fill. This review did not rebuild it. Its `egress-light` account now exists.
    - The vault and recorder code read at slot 473711310 is unchanged from the table below.
    - Not every bridge service runs unmodified `main`; see
      [Master State](master-state.md#where-the-bridges-run).

On 21 November 2026, two clean builds from public `main` reproduced all three deployed Solana programs byte for byte. Public source also reproduced all four Ethereum contracts on the pfUSDC Groth16 call path. The five current Solana ingress guest sources and their verification keys are on `main`. Their release binaries are distributed separately.

The public source baseline is `166c609e0c5bac8ac80541d0ef50c12213597d01`. The Post Fiat governance and anchor values below are the historical read at height **1739** on `postfiat-wan-devnet-2`. The Solana and Ethereum tables give their own finalized read points.

## Where the bridge services run

At this audit, every bridge service listed in [Master State](master-state.md#where-the-bridges-run) ran from one checkout of `main` at `7e67fc292`. This covers Solana deposits, the Ethereum deposit bridge and wallet proxy, withdrawals and the faucet. The Solana relay, capture and proof host are in `tools/pfusdc-sol-relay`, `tools/pfusdc-sol-capture` and `tools/pfusdc-sol-ingress-host`. The observer and prover launch scripts are in `ops/sol-live`. The Agave observer patch is in `ops/sol-public-node/agave-4.4.1-observer.patch`. The Ethereum proxy release files are in `deployments/ethereum-usdc-proxy`. Withdrawal and faucet code is in `ops/withdraw`. At this audit the six validators ran source `674dbb18a`, which is on `main`.

Two other processes used different checkouts during this audit. They were the loopback instant-advance pilot and a separate wallet proxy. Their role in these mainnet routes was not verified.

## Solana mainnet programs

The first address is the executable program. Its loader account points to the ProgramData address below it. SHA-256 covers **every byte after the 45-byte ProgramData header**, including any allocation padding. All three accounts had an upgrade authority at the time of the read, and still had one on 25 November.

| Artifact and addresses | SHA-256 of deployed code | Bytes / last deploy slot / finalized read slot | Source and rebuild |
| --- | --- | --- | --- |
| Vault `GFkpeTibgdRAAAndgUMYsJnSTck91tUbrpuWBWan6z1G`<br>ProgramData `ETTLit4rUBtkYdjYfV9LNAz6GwT9yPHj32PRVpWZ7szB` | `75371e9a74f6ac2b5462f9e286f11396e3dee9f9f621cba1ec5cc55fee3a95aa` | 85,784 / 471290952 / 472294460 | Main, `solana/pfusdc-vault/src/lib.rs` and `solana/pfusdc-egress-verifier/src/vault.rs`. Exact match. |
| Recorder `2fWk8a86FXqGAWCuypnEuQnhuKBUUaw4vxv7FX8Haem6`<br>ProgramData `73bHxuEQUjM9EsyG61YtbbT59CfRFoS1PE3SgAAZsuBi` | `d6400718d5e329052d72c0ee686f471427db346d32ca0124b7b35ec26d39577c` | 66,488 / 471291424 / 472294460 | Main, `solana/pfusdc-recorder/src/lib.rs`. Exact match. |
| Egress verifier `8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF`<br>ProgramData `D7YLTGrEHo3aLeREBtX5ZBXwSaWKyndPXzuJYQ5HDFmN` | `3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c` | 116,928 / 471292217 / 472294460 | Main, `solana/pfusdc-egress-verifier/src/`. Exact match. The vault pins this verifier ID. This image, replaced on 24 November (note above), bound one bucket in its route; exit setup and rehearsal refuse it (`VERIFIER_UNPINNED` in `ops/sol-live/exit_setup.py`) until the rebuilt custody-route verifier's hash is pinned. |

To read an image, set `SOL_RPC` to a public Solana mainnet RPC and `PROGRAM` to the program address. This read uses Python and `solders`. It validates the loader, account types, and ELF magic. It prints the full ProgramData SHA-256 separately. For the vault, the full-account digest was `f7a1bf3de2960391cd73bd949204891ba248b6f38c1a09138f16c572008d6f47`.

```python
import base64, hashlib, json, os, urllib.request
from pathlib import Path
from solders.pubkey import Pubkey

loader = "BPFLoaderUpgradeab1e11111111111111111111111"
def account(address, minimum_slot=0):
    options = {"encoding": "base64", "commitment": "finalized", "minContextSlot": minimum_slot}
    body = {"jsonrpc": "2.0", "id": 1, "method": "getAccountInfo", "params": [address, options]}
    req = urllib.request.Request(os.environ["SOL_RPC"], data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as response:
        result = json.load(response)["result"]
    value = result["value"]
    assert value["owner"] == loader
    return result["context"]["slot"], value["executable"], base64.b64decode(value["data"][0], validate=True)

slot, executable, p = account(os.environ["PROGRAM"])
assert executable and len(p) == 36 and int.from_bytes(p[:4], "little") == 2
address = str(Pubkey.from_bytes(p[4:]))
slot, executable, data = account(address, slot)
assert not executable and int.from_bytes(data[:4], "little") == 3 and data[45:49] == b"\x7fELF"
print(address, slot, "full", hashlib.sha256(data).hexdigest(), "code", hashlib.sha256(data[45:]).hexdigest())
Path(os.environ["PROGRAM"] + ".so").write_bytes(data[45:])
```

Use Anza platform-tools v1.58 in a fresh shell. The binaries embed the Cargo registry path. Set `SBF_PLATFORM_TOOLS` to the unpacked toolchain. From the public repository root, save all three images with the read above and export `VAULT_IMAGE` as the path to the vault dump. The wrapper maps the local Cargo directory to the prefix found in that image.

```sh
set -eu
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export BUILD_CARGO_HOME="$(python3 -c 'import os,re; from pathlib import Path; print(re.search(rb"(/[A-Za-z0-9_./-]+)/registry/src/index[.]crates[.]io-",Path(os.environ["VAULT_IMAGE"]).read_bytes()).group(1).decode())')"
export RUSTC_WRAPPER="$(mktemp)"
cat > "$RUSTC_WRAPPER" <<'SH'
#!/bin/sh
exec "$@" "--remap-path-prefix=$CARGO_HOME=$BUILD_CARGO_HOME"
SH
chmod +x "$RUSTC_WRAPPER"
export SOLANA_BRIDGE_IMAGES="$(mktemp -d)"
bash solana/deploy/build.sh 166c609e0c5bac8ac80541d0ef50c12213597d01
cmp "$VAULT_IMAGE" "$SOLANA_BRIDGE_IMAGES/pfusdc_vault.so"
cmp 2fWk8a86FXqGAWCuypnEuQnhuKBUUaw4vxv7FX8Haem6.so "$SOLANA_BRIDGE_IMAGES/pfusdc_recorder.so"
cmp 8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF.so "$SOLANA_BRIDGE_IMAGES/pfusdc_egress_verifier.so"
```

The vault config PDA `JByQZxACTnFXQDGLBUKAfTysSuqtpoLHoAWqUh4ri6WG` held mint `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` and the same route binding as the Post Fiat ledger. Custody was `AhmUaCijypvcbYMDxeXVbUPqjPX1tiHfLH6NWkYnQTCi`. Derive them with seeds `config` and `custody` under the vault ID. The config stores the mint at bytes 0–31 and binding at 32–63. At finalized slot 472294460 the verifier's `egress-light` PDA `FQe34dCQg9m6yEW6NjbjoR8LfWwKGLX2hBvTdY2Lu6s1` did not exist. Deployment of the verifier alone therefore does not establish an initialized withdrawal route. The PDA exists since the initialisation of 23 November (read at slot 473711317).

## Solana proof programs

All five source paths below are relative to `programs/pfusdc-sol-mainnet-ingress/` on public `main`. Its `src/` tree, Cargo manifest, lockfile and `program-info.json` are byte-identical to the release at `72bb66ac8`. The manifest publishes each ELF digest and SP1 program verification key (vkey). The matching ELFs are absent from public `main` history. The private tag `archive/20261121/private/sol/live` retains them at that revision under `programs/pfusdc-sol-ingress/elf/`. Revision 7 accepts an aggregate for a deposit. That guest verifies the shard proofs internally. The ledger also accepts a handoff proof under its own key.

| Guest and source | Program vkey | Retained ELF SHA-256 |
| --- | --- | --- |
| Deposit aggregate, `src/aggregate_guest.rs` | `0x00503a8b7322a326ebd7a347b388641c9916e573328ba7f665808d7627a932f6` | `84127cfa76de2854e1bea408a8e7910092515d24647f4a8683e7935ca585d6cb` |
| Delta shard, `src/shard_guest.rs` | `0x002d18dc66bb6a40224ebd4d057c4d157413ef43a6a92b3aa66a4fba0bb0a956` | `5fd57ea8b9432c14f94da218e96eb20b600fea94c5e1adeef073f547edecad83` |
| Epoch handoff, `src/handoff_guest.rs` | `0x008adba6be08c9546fccc181f6808538b63778c7c5f0388bb4d848958c82298d` | `665fe185b830602a8f3fb9906d43f45ad0f9dda4deae7990ca2da69ca44253ce` |
| Monolithic deposit, `src/main.rs`. Separate build from the admitted aggregate. | `0x00d29f5e1ef34f0bcb12cc5a31cbc31dff20db652d6aab1c50babaedca16c867` | `78619aeae5aa823940cf3c194ea7ce256f405fa814bc2c81fd2bb2cf425a6217` |
| Key-table registration, `src/key_table_guest.rs`. The revision 7 profile has no standalone key-table field. | `0x00c3a3949e0f57e9c10eea9b49e2a6ed1c04efbe7846496f369189d5f412d9e9` | `bc4105e3d3995f96a85fd7e342128cc8e60039360ca161b23c2bd43758cc3cbd` |

This audit derived all five keys from the retained ELFs with SP1 6.3.1. The five ELFs staged for the service also matched the public digests. The release stager also pins their hashes in `ops/sol-live/runtime-files.sha256`. For an independent build, use the public source baseline above. The manifest records the Docker image digest but names an older source commit, `b4852dd4d`, and records no byte-exact replay. A clean guest build from public source was not run in this refresh.

```sh
cd programs/pfusdc-sol-mainnet-ingress
cargo prove build --docker --tag v6.3.1 --locked --features guest --output-directory rebuilt
```

From the same directory, run this check with `ELF_DIR=rebuilt` for build output or set `ELF_DIR` to a directory of release ELFs. It checks all five entries, including any missing output.

```sh
set -eu
ELF_DIR=${ELF_DIR:-rebuilt}
for name in $(jq -r '.programs[].program' program-info.json); do
  digest=$(jq -er --arg n "$name" '.programs[] | select(.program == $n).elf_sha256' program-info.json)
  key=$(jq -er --arg n "$name" '.programs[] | select(.program == $n).program_vkey' program-info.json)
  printf '%s  %s\n' "$digest" "$ELF_DIR/$name" | sha256sum -c -
  derived=$(cargo prove vkey --elf "$ELF_DIR/$name" 2>&1)
  printf '%s\n' "$derived" | grep -Fx "$key"
done
```

To check the recursive link, run `pfusdc-sol-ingress-host vkey-u32 "$ELF_DIR/pfusdc-sol-delta-shard"`. Compare its eight words to `LEAF_KEY` in `src/aggregate_guest.rs`. All eight words matched. The host command is in `tools/pfusdc-sol-ingress-host/src/main.rs`.

### Earlier accepted Solana keys

Public `programs/pfusdc-sol-ingress/` still contains a separate, older three-guest build. Its deposit, key-table and handoff keys differ from revision 7. For the table below, **archive** means the private tag named above. The exact source trees, manifests and release ELFs for `8c303c3db`, `e6e700950` and `e7b49ad6a` are absent from public `main` history. Each has a different `src/lib.rs` and `src/aggregate_guest.rs`. **Current** refers to the public source and keys in the five-guest table.

| Route revision / accepted height | Deposit aggregate key / source | Handoff key / source |
| --- | --- | --- |
| 1 / 1678 | `0x0082ff60ee6ad74b00234b4ea57c0fb199197a082b16ea454e1a4d851493d82f` at archive `8c303c3db` | `0x00fb0992728268d7b86e208e57d6e8eb2f3dac9730c7ac6f3d54c5a6ef7a5a78` at archive `8c303c3db` |
| 2 / 1695 | `0x004afc3cc58a1c582d4145d73b95d3936d058b344005007a38262e28ed2f6a37` at archive `e6e700950` | Same as revision 1 |
| 3 / 1700 | `0x0041726330ef23ce4d4d625b77963740432a0145fc2c25bf52fddd1f87ecbc02` at archive `e7b49ad6a` | Same as revision 1 |
| 4 / 1705 | Same as revision 3 | `0x0079d959e6d9a7567429ae1b0e11062b04bed139a588297754fc8dafec853827` at archive `e7b49ad6a` |
| 5 / 1709 | Current aggregate, public main | Same as revision 4 |
| 6 / 1713 and 7 / 1731 | Current aggregate, public main | Current handoff, public main |

All five distinct earlier keys in the table matched fresh SP1 derivations from the archived ELFs. To repeat a historical check with archive access, set `rev` to the revision for that key and `guest` to `pfusdc-sol-deposit-aggregate` or `pfusdc-sol-handoff-proof`. Compare both outputs with the manifest entry for the named guest. Use its accepted height to check the ledger receipt below.

```sh
dir=programs/pfusdc-sol-ingress
git show "$rev:$dir/program-info.json" | jq --arg g "$guest" '.programs[] | select(.program == $g)'
git show "$rev:$dir/elf/$guest" > "$guest.elf"
sha256sum "$guest.elf"
cargo prove vkey --elf "$guest.elf"
```

## Ethereum mainnet

At finalized Ethereum block **26466176**, hash `0x0c65b3056fb25912bef8104cd4019f7c98d62aa134f12cfdfde4c41a3ebfbece`, the vault's `finalityVerifier()` led to the finality verifier, then `sp1Verifier()` led to Succinct's gateway. The gateway dispatches on the first four proof bytes. For this Groth16 release, `routes(0x4388a21c)` returned the v6.1.0 verifier with `frozen = false`. Other gateway selectors are shared infrastructure.

| Artifact / address | Keccak-256 of full runtime | Source on main |
| --- | --- | --- |
| USDC vault `0xe7a432a28b20621a70e845c7f202004cf98e7a2c` | `0xc6dbb722c23bfc841624bb909fcb54d84a65a2ea6ece96e2a28bf61d5dea6d05` | `crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol` and imported `ERC20BridgeVaultV2.sol` at public ancestor `087e3d157`. Tip changes to the import change the metadata hash. |
| Finality verifier `0xc398a26bd997168c3c966763b0a97829d14db655` | `0xbe4f974fe001792e00ed6c5a95bb1433e7212fbaa239c970a440a7646f649ee4` | `crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol` at `087e3d157` and at the audited tip. |
| SP1 gateway `0x397a5f7f3dbd538f23de225b51f532c34448da9b` | `0x11612dc6695484a4dfcac5ae1a7bcb093f891eb93d8aa9ddeca7f24a1f3b7d57` | Public submodule `external/sp1-contracts` at `2ac5ecbbe473421a963d67e55f182e9a36576f7c`, file `contracts/src/SP1VerifierGateway.sol`. Its OpenZeppelin submodule is pinned at `dbb6104ce834628e473d2173bbc9d47f81a9eec3`. |
| SP1 Groth16 verifier `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2` | `0xcceb864cd8a5a36b2073a8f2b32a773835cd2dd2c78a56f8e6fdb942feff04dd` | Same public submodule, `contracts/src/v6.1.0/SP1VerifierGroth16.sol` and `Groth16Verifier.sol`. Full verifier selector hash `0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696`. |

To repeat **all four** exact builds, use a full clone, Python with Web3.py, and solc 0.8.24 and 0.8.20. Set `ETH_RPC_URL` to an Ethereum mainnet read endpoint. Set `SOLC_024` and `SOLC_020` to the respective compiler executables if needed.

```sh
git submodule update --init external/sp1-contracts
git -C external/sp1-contracts submodule update --init contracts/lib/openzeppelin-contracts
scripts/bridge-audit-ethereum-runtime
```

The check reads both creation transactions, compares their init code to the compiler output and runs the constructors via `eth_call` with their original arguments. It compares all runtime bytes, including immutables and metadata. The two upstream contracts are compiled with their deployment settings. All four comparisons passed.

| Ethereum guest | Deployed vkey / retained ELF SHA-256 | Source and check |
| --- | --- | --- |
| Ethereum → Post Fiat deposit | `0x00d8cbf06d5640adde0fef863b3c689c4ab3442b0dc91d28ac4cc5f9c9b2ccbb` / `b6a06cb90ee81477a33c9a5902458db5b379cc40b765988cebb0ec46d08766fe` | Main, `programs/pfusdc-eth-mainnet-ingress/src/{main,lib}.rs`. The matching ELF entered main history at `6739c2e3b`. Run `sha256sum programs/pfusdc-eth-mainnet-ingress/elf/*` and derive the key with `cargo prove vkey --elf` on that file. Both passed. |
| Post Fiat → Ethereum withdrawal and checkpoint | `0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87` / `4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0` | Guest family on main, `programs/pfusdc-egress/src/main.rs` and `crates/pfusdc_proofs/`. The epoch-6 successor manifest names public ancestor `9d14fdcc9` and these pins. `ops/withdraw/proof_host.rs` checks the same pins. The Solana egress verifier also pins this key in `solana/pfusdc-egress-verifier/src/groth16.rs`. The matching raw ELF and its archived gzip are absent from public Git history. The private tag retains the gzip. Its uncompressed digest and derived key matched. |

To repeat the frozen egress check with archive access, run the following from the repository root.

```sh
git show archive/20261121/private/sol/live:solana/pfusdc-egress-verifier/guest/pfusdc-egress.elf.gz | gzip -dc > frozen-egress.elf
sha256sum frozen-egress.elf
cargo prove vkey --elf frozen-egress.elf
```

For either Ethereum guest, a candidate build is `cargo prove build --locked --output-directory rebuilt` in the respective guest directory at the indicated public revision, with the SP1 6.3.1 toolchain. Compare the built ELF to the digest above, then derive its key. Neither guest was rebuilt here. The current main egress ELF hashes to `0ea4dbabc8a36b44824c861cf2e1ae90454df2a6e4bb8a357f9d3a8672696d83`.

## Governed routes and anchors

| Ledger artifact | Accepted identity at height 1739 | Public source |
| --- | --- | --- |
| Solana ingress profile | `solana-mainnet-usdc-v1`, revision 7, accepted at 1731. Profile SHA3-384 `475af4cc18945cf1301833f0a1007e0adfabafdebd8ebd55218a2f5a5058c7835ac0551f527ff2068f3291465b66a17c`. Amendment `cbd372449dbd5177b5ea989a51475642dd91f8407d7b82fafe0d88c3227a126e809bb77e0548c4bddf34948aca83d008`. Stable vault binding `e34b1b03a04b54eab4fbc229c3bc75051ed8ce7374939c968e44c04050370628`. | Main, `crates/types/src/account_owned_asset_types.rs` and `crates/node/src/execution_actions.rs`. Profile values come from the governance batch. The profile leaves the Solana vault runtime hash empty. |
| Solana governed anchor | Solana epoch **1093**, slot floor **472176000**, set hash `b6e4a26423f9972f4322a322d73db2c5b17e168d7a7300dd47849c6a59fb6f80`, total stake **428866760183055897**, known-key-table hash `3fbafd2b5a53a8893728e23833e4295e8afa2c2e8025f5c7150d17aadd94e4fe`. Shred version 50093. | Main, `crates/types/src/solana_bridge.rs`. Read `profile.solana.anchor_slot` and `profile.solana.anchor` in the accepted amendment. |
| Ethereum ingress profile | `ethereum-mainnet-usdc-v1`, epoch **11**. Effective height **1254**, accepted in governance block **1256**. Profile `29c91daa1c8d707ea3245db091976219266c5deca82b2d4571fafb221fd2dd092eb8833a28f4422bf0b4bbde44069e28`. Binding `95f0dcbf204825b0789384769dddfa12e4be09aa1af8e841fe4a56bec423927d`. Amendment `e0d08b2fe1cee91d5c7474a3b511c81d8d518b4704351486a5be167cb5ddcf9c6860c29930acbc2fc5013bf055299f3b`. | Main, `crates/node/src/market_bridge.rs`, `crates/types/src/account_owned_asset_types.rs`. The exit verifier's immutable `routeEpoch()` was **10**. Its key verifies withdrawals. |
| Ethereum beacon anchor | Finalized slot **15683872**, beacon root `207a86cc96c633bb86f4343ccbe2e86c4856058ab6f613803704282f6f14ae8c`, current sync committee root `fac594af4f828096f111564110b92bcc06996a79746102011f0a26a9f51f6d6e`. | Main, `crates/node/src/b23_ingress_route_report.rs` and `crates/types/src/b23_ingress_anchor.rs`. Read `beacon_finality_anchor` and compare its profile hash, epoch and binding. |

Public fixtures for the Ethereum activation and its accepted receipt are in `programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1256-{governance,receipt}.json`. The beacon anchor in the table comes from the later height-1739 route read.

Use a historical Post Fiat RPC reader that supports the line-JSON protocol. Set `PFT_RPC_HOST` and `PFT_RPC_PORT` for that reader. The family lookup returned the Ethereum profile, so use the accepted governance batch for Solana. These commands read the ledger.

```sh
rpc() {
  jq -cn --arg m "$1" --argjson p "$2" \
    '{version:"postfiat-local-rpc-v1",id:"audit",method:$m,params:$p}' |
    nc -q1 "$PFT_RPC_HOST" "$PFT_RPC_PORT" |
    jq -e 'if .ok then .result else error(.error|tostring) end'
}
asset=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
rpc status '{}' | jq '{chain_id,block_height,build_git_revision,state_root,binary:.deployment_runtime_artifacts.binary_sha256}'
rpc vault_bridge_route "$(jq -nc --arg a "$asset" '{asset_id:$a}')"
h=1731                         # use 1256 for the Ethereum activation
batch=$(rpc blocks "{\"from_height\":$h,\"limit\":1}" | jq -er '.[0].header | select(.batch_kind=="governance").batch_id')
rpc batch_archive "$(jq -nc --arg b "$batch" '{batch_kind:"governance",batch_id:$b,limit:1}')" |
  jq -r '.[0].payload_json' > governance.json
jq '.vault_bridge_route_profile_activations[] | {amendment,profile}' governance.json
tx=$(jq -er '.vault_bridge_route_profile_activations[0].amendment.amendment_id' governance.json)
rpc receipts "$(jq -nc --arg t "$tx" '{tx_id:$t,limit:1}')" |
  jq -e '.[0] | select(.accepted and .message=="vault bridge route profile activated")'
```

For Solana, the archived payload preserves the canonical field order. The following reproduced the profile hash in the amendment. Repeat the batch and receipt checks at each listed revision. Height 1731 was the last governance batch through the historical read at height 1739.

```python
import hashlib, json
rows = json.load(open("governance.json"))["vault_bridge_route_profile_activations"]
row, = [r for r in rows if r["profile"]["route_id"] == "solana-mainnet-usdc-v1"]
preimage = json.dumps(row["profile"], separators=(",", ":")).encode()
digest = hashlib.sha3_384(b"postfiat.vault_bridge.route_profile_hash.v1\0" + preimage).hexdigest()
assert row["amendment"]["kind"].endswith(":" + digest)
print(digest)
```

## Where the Post Fiat ledger verifies and credits

| Entry point on main | What it checks |
| --- | --- |
| `crates/execution/src/solana_bridge.rs`, `apply_proof` → `credit_verified` | Selects the active profile and its deposit or handoff key. Verifies Groth16. Checks the deposit journal against the installed client, slot, vault and route binding. Rejects duplicate receipts and credits the source series. A handoff updates the light client. |
| `crates/execution/src/nav_vault_asset_execution.rs`, `apply_vault_bridge_deposit_propose_with_genesis` → `ensure_vault_bridge_deposit_source_proof` | Checks proof and public-value commitments, verifies the Ethereum proof and matches its public values to the route and deposit. The anchored path advances the stored beacon state. Finalization uses the committed proposal. The accepted claim flows through `apply_vault_bridge_deposit_claim_with_orchard` to `commit_bridge_receipt_credit`. |
| `crates/execution/src/nav_sp1_verifier.rs`, `verify_bounded_sp1_groth16_with_config` | Enforces the proof kind and byte bounds. Calls `sp1_verifier::Groth16Verifier` with the authorized program key and committed public values. |

At height 1739 the reader reported build `674dbb18`. `git diff --exit-code 674dbb18a 166c609e0 -- crates` passed. The reader reported binary SHA-256 `95d5b237c991f57a9ea122e110f6a2d0c017bf9ca0a85315b2359ecba3b017d1`, matching the six-validator release in [Master State](master-state.md). At the release revision, the source build command is `cargo build --locked --release -p postfiat-node --bin postfiat-node`. Compare `sha256sum target/release/postfiat-node` to that digest. A byte-identical rebuild of the release binary was not tested.

Focused regression commands are `cargo test --locked -p postfiat-node real_groth16_credit_replay_and_handoff_converge_on_six_replicas` and `cargo test --locked -p postfiat-execution --test b23_ingress_cpu_proof -- --ignored`. The latter needs a real proof directory in `B23_INGRESS_PROOF_DIR`. Neither Rust suite was run for this map.

## Not auditable from main today

- **Solana deployment state.** The three binaries reproduce from public source. The programs are upgradeable, and the ledger profile leaves the vault runtime hash empty. A later audit needs a fresh finalized account read.
- **Solana guest binaries.** Main has all five revision 7 sources and expected identities. It omits their five release ELFs. A clean build that reproduces those digests remains unverified. The historical source versions and ELFs for the earlier keys marked “archive” above are still missing from main.
- **Ethereum guest builds.** Main has the deposit source and matching ELF. The withdrawal source and release pins are public, but the matching frozen ELF is only retained in the private archive. Clean builds for both deployed guest identities remain unverified. `scripts/test-proof-public-input-inventory` still fails on the stale digest for `crates/execution/src/nav_vault_asset_execution.rs`.
- **Running binaries.** The reader and the six-validator release record agree on the node digest. An independent byte-for-byte build is still open. The public service source and release hashes do not attest the code loaded by a running observer or prover.
- **Ledger history.** Independent acceptance checks need the governance batches and receipts. Reconstructing the initial Solana stake and key commitments also needs historical Solana validator and stake data. Main supplies the verification rules.

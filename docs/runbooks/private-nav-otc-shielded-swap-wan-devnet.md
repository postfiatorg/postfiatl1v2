# Private NAV OTC Shielded Swap on WAN Devnet

Status: controlled-devnet runbook and execution record
Date: 2026-06-20
Audience: protocol engineering, validator operators, bridge operators

## Purpose

This document records the first end-to-end private NAV OTC swap run on the live
WAN devnet:

- fund the Arbitrum `ERC20BridgeVault` with real USDC;
- relay that deposit into PFTL as bridge-backed `pfUSDC`;
- ingress transparent `pfUSDC` and transparent `a651` into the
  asset-typed Orchard pool;
- execute a real Halo2-backed `ShieldedSwap` between the two private notes;
- verify that public chain state reveals nullifiers, commitments, proof
  material, and pool metadata, but not the swapped asset ids, amounts, owners,
  recipients, or counterparties;
- verify that bridge custody and NAV accounting remain publicly auditable at
  the transparent boundaries.

This is not a generic production operating policy. It is the exact controlled
WAN-devnet procedure and evidence record for the 2026-06-20 proof run.
It proves the private Asset-Orchard middle, not private egress or private
bridge-out. Boundary ingress is public, and current Asset-Orchard egress is
disclosed.

Current topology note: this document preserves the historical 2026-06-20 run
only as an evidence record. Its mixed Hetzner/Vultr validator access pattern is
retired and must not be reused. New and resumed live runs must use the
all-Vultr topology in
`$POSTFIAT_STATE/live-e2e-20260621T015853Z/all-vultr-remote-topology.json`
and must not use Hetzner hosts.

## Security Rules Used

The live run followed these rules:

- Do not use raw EVM private keys for Arbitrum signing. Use the StakeHub
  `agentd` signing path with a bounded launch session.
- Do not use `sshpass` or command-line plaintext passwords.
- For the active topology, use only Vultr validators with key-based SSH via
  `~/.ssh/id_ed25519`.
- Do not use Hetzner validators for current or future testnet runs.
- Do not restart or roll validators for this run; the fix-pass binary was
  already deployed.
- Treat every shielded batch as consensus-affecting. Confirm height and
  state-root convergence across all validators after each live mutation.
- If a local mirror certifies a batch as `validator-3`, explicitly check the
  real validator-3 process. The local apply may advance the mirror, not the
  remote validator.

## Live Constants

WAN devnet:

```text
chain_id             postfiat-wan-devnet
genesis_hash         231b1cfb63439c23bdcc3f7ea2f7f3ce7a53f9abffef8f720f47421b575f16e7f2d9ad5e61298207be2e9ce08743f870
protocol_version     1
validator_count      6
```

EVM / bridge:

```text
source_chain_id      42161
stakehub_wallet      0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
erc20_bridge_vault   0x1A15e6103D6Af4e88924F748e13B829D3948DEa9
arbitrum_usdc        0xaf88d065e77c8cC2239327C5EDb3A432268e5831
withdrawal_verifier  0x70C259Bf8D65AF76DfcB4991ccB26E88b2C2188E
```

PFTL assets and accounts:

```text
a651 asset_id        dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5
pfUSDC v2 asset_id   8751c2d04b993eb54f751b0f130c420fdb089548ec2f2a53837d11d1c397a1252e74bcc24616527e9c79b968635fae90
bridge policy_hash   853b8c0478fbfdf488a48f48ca58c4dde5decef53a68c260aa44ebdc44eeb9fffdee81431370c2b091d9819042655daa
buyer account        pf07381735ddb7de134e8be8402b465c9cd8ec7546
holder account       pf65c9783ceafc0f519a74195e78cc7909f92429c3
issuer account       pff3e396f771a8f490ca330e1720472d473bcfcb6d
asset pool_id        asset-orchard-v1
```

Local run root:

```text
$POSTFIAT_STATE/shielded-pfusdc-wan-20260620T
```

Operator key files used by the local CLI:

```text
buyer key            $POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/keys/buyer.key.json
holder key           $POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/keys/holder.key.json
certifier keys       $POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/certifier-keys
```

The key-file paths are operator inputs. The runbook intentionally does not
embed private key material.

## Validator Access Pattern

The historical 2026-06-20 run used mixed infrastructure. That pattern is now
retired. The active WAN devnet validator set for resumed and future runs is
all Vultr, and every validator uses root key authentication with
`~/.ssh/id_ed25519`.

```text
validator-0  Vultr  192.0.2.10    rpc 27650  data /var/lib/postfiat/validator-0
validator-1  Vultr  192.0.2.11   rpc 27651  data /var/lib/postfiat/validator-1
validator-2  Vultr  192.0.2.12      rpc 27652  data /var/lib/postfiat/validator-2
validator-3  Vultr  192.0.2.13    rpc 27653  data /var/lib/postfiat/validator-3
validator-4  Vultr  192.0.2.14   rpc 27654  data /var/lib/postfiat/validator-4
validator-5  Vultr  192.0.2.15    rpc 27655  data /var/lib/postfiat/validator-5
```

Use:

```bash
ssh -i ~/.ssh/id_ed25519 -o BatchMode=yes root@<ip> \
  'postfiat-node status --data-dir <data-dir>'
```

## Preflight

Set the common environment.

```bash
export RUN_ROOT=$POSTFIAT_STATE/shielded-pfusdc-wan-20260620T
export DATA="$RUN_ROOT/data"
export BIN=$POSTFIAT_REPO/target/release/postfiat-node
export TOPO="$DATA/remote-topology.json"
export CAST=$FOUNDRY_HOME/bin/cast

export BUYER=pf07381735ddb7de134e8be8402b465c9cd8ec7546
export HOLDER=pf65c9783ceafc0f519a74195e78cc7909f92429c3
export BUYER_KEY=$POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/keys/buyer.key.json
export HOLDER_KEY=$POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/keys/holder.key.json
export CERT_KEYS=$POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/certifier-keys

export A651=dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5
export PFUSDC=8751c2d04b993eb54f751b0f130c420fdb089548ec2f2a53837d11d1c397a1252e74bcc24616527e9c79b968635fae90
export POLICY_HASH=853b8c0478fbfdf488a48f48ca58c4dde5decef53a68c260aa44ebdc44eeb9fffdee81431370c2b091d9819042655daa

export ARB_CHAIN_ID=42161
export VAULT=0x1A15e6103D6Af4e88924F748e13B829D3948DEa9
export USDC=0xaf88d065e77c8cC2239327C5EDb3A432268e5831
export STAKEHUB_WALLET=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
export AMOUNT=1000000
```

Mirror a live validator into the local run root before building local batches.
The run used validator-3 as the mirror.

```bash
test ! -e "$DATA"
scp -r \
  -i $SSH_KEY_FILE -o BatchMode=yes \
  root@192.0.2.13:/var/lib/postfiat/validator-3 "$DATA"

"$BIN" status --data-dir "$DATA"
```

Before the first live mutation, all six validators were at height `76` with
state root:

```text
e95db8c089052e2def0a892a3cec7e7d4db95b36bb0e4b779658317c7e8d5e27ea1433d45cbd85cdccc46e6708af320b
```

## Step 1: Deposit Real USDC into the Arbitrum Vault

The EVM deposit must be signed by StakeHub `agentd`, not by exporting a raw
private key. The launch session should:

- be on Arbitrum chain id `42161`;
- allowlist the StakeHub wallet, the USDC contract, and the bridge vault;
- cap USDC spend to the exact amount being deposited;
- be closed after the approve/deposit sequence.

Build the two calldata payloads:

```bash
export NONCE=0x5ce1dfc7d8030b1b39098e74ddb586102335b4118269b29e95a3494e4d54de3a
export APPROVE_DATA=$("$CAST" calldata 'approve(address,uint256)' "$VAULT" "$AMOUNT")
export DEPOSIT_DATA=$("$CAST" calldata \
  'deposit(uint256,string,bytes32)' "$AMOUNT" "$BUYER" "$NONCE")
```

Then submit both through `agentd` using `op=evm_contract_tx`:

- approval target: `USDC`;
- deposit target: `VAULT`;
- session id: `pfusdc-shielded-swap-20260620`;
- session actions: distinct labels such as `approve_pfusdc_vault` and
  `deposit_pfusdc_vault`;
- RPC URL: Arbitrum RPC.

The current `agentd` policy requires an active launch session for
`evm_contract_tx`. If that policy still requires an `expected_deploys` entry to
open a launch session, include an inert expected deploy and close the session
manually after the deposit. Do not deploy anything for this flow.

Live result:

```text
approve_tx        2e0d03fb8a28bd7ca5f3a6e0e054219b0c0d48caccb4bbd4292fe7f14fdb8943
approve_gas       55,835
deposit_tx        440616cd1b890ace17ee2f606ee466343b66e4f2ad0a7297155f7250f8e53c9e
deposit_gas       87,475
amount_atoms      1,000,000
pftl_recipient    pf07381735ddb7de134e8be8402b465c9cd8ec7546
nonce             0x5ce1dfc7d8030b1b39098e74ddb586102335b4118269b29e95a3494e4d54de3a
report            $RUN_ROOT/flow1-evm-deposit.json
```

## Step 2: Relay the Deposit to PFTL and Mint pfUSDC

Build the relay bundle from the Arbitrum receipt.

```bash
export BUNDLE="$RUN_ROOT/bundles/flow2-deposit-relay"

"$BIN" vault-bridge-deposit-relay-rpc-bundle \
  --source-rpc-url "$ARBITRUM_RPC_URL" \
  --tx-hash 0x440616cd1b890ace17ee2f606ee466343b66e4f2ad0a7297155f7250f8e53c9e \
  --cast-bin "$CAST" \
  --vault-address "$VAULT" \
  --token-address "$USDC" \
  --asset-id "$PFUSDC" \
  --policy-hash "$POLICY_HASH" \
  --proposer "$HOLDER" \
  --attestor "$HOLDER" \
  --finalizer "$HOLDER" \
  --claimer "$BUYER" \
  --expires-at-height 1000000 \
  --bundle "$BUNDLE" \
  --overwrite
```

The generated bundle includes `commands.sh`, operation JSON files, quotes, and
signed transaction files after signing. The command script signs and submits
four PFTL asset operations:

1. `propose`
2. `attest`
3. `finalize`
4. `claim`

Use the holder key for proposer, attestor, and finalizer. Use the buyer key for
claimer.

```bash
export PFTL_DATA_DIR="$DATA"
export PROPOSER_KEY_FILE="$HOLDER_KEY"
export ATTESTOR_KEY_FILE="$HOLDER_KEY"
export FINALIZER_KEY_FILE="$HOLDER_KEY"
export CLAIMER_KEY_FILE="$BUYER_KEY"

bash "$BUNDLE/commands.sh"
```

After each mempool submission, finalize the mempool with a peer-certified
round. The live run advanced one operation per height:

```text
propose    height 77
attest     height 78
finalize   height 79
claim      height 80
```

The relay evidence was:

```text
deposit_id       1d6f13ad112e09204949098d69cad7c8f8d3ff8dbd59eef88f4870625e14ad9a
evidence_root    0cf280e6db6b0bc68c1c534891b6ddba2d67789b580e1284c1a1fdb0369766e45ef62b5534d0cb1a2804cb6933f7359e
block_hash       a5d66b52b8fdd2b37bb09201558200d091c5b3df6d107b2a343517526468f6c5
tx_hash          440616cd1b890ace17ee2f606ee466343b66e4f2ad0a7297155f7250f8e53c9e
log_index        2
amount_atoms     1,000,000
status           finalized
submitted_height 77
finalized_height 79
claim_height     80
```

After claim, the buyer had `1,000,000` transparent v2 pfUSDC atoms.

## Step 3: Ingress pfUSDC into the Asset-Orchard Pool

The ingress action burns transparent issued asset units and inserts an
asset-typed note commitment into `asset-orchard-v1`. This is the public boundary
where bridge-backed pfUSDC enters the private pool.

```bash
export PFUSDC_ING="$RUN_ROOT/ingress/pfusdc"
mkdir -p "$PFUSDC_ING"
export PFUSDC_NOTE_SEED=$(printf \
  'asset-orchard-pfusdc-v2-live-ingress-20260620-height81' | sha256sum | awk '{print $1}')

"$BIN" asset-orchard-ingress-create \
  --data-dir "$DATA" \
  --key-file "$BUYER_KEY" \
  --asset-id "$PFUSDC" \
  --amount 1000000 \
  --note-seed-hex "$PFUSDC_NOTE_SEED" \
  --ingress-file "$PFUSDC_ING/ingress.json" \
  --note-file "$PFUSDC_ING/note.json" \
  --overwrite > "$PFUSDC_ING/create-report.json"

"$BIN" shield-batch-asset-orchard-ingress \
  --data-dir "$DATA" \
  --ingress-file "$PFUSDC_ING/ingress.json" \
  --batch-file "$PFUSDC_ING/batch.json"
```

Certify the shielded batch at the next height:

```bash
"$BIN" transport-peer-certified-batch-round \
  --data-dir "$DATA" \
  --topology "$TOPO" \
  --batch-kind shielded \
  --batch-file "$PFUSDC_ING/batch.json" \
  --key-file "$DATA/validator_keys.json" \
  --artifact-dir "$PFUSDC_ING/artifacts" \
  --height 81 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 1000 \
  --quorum-early-full-propagation > "$PFUSDC_ING/round.json"
```

Live result:

```text
batch_id          5c400814dca77f7bdfb1cedd2f61ffa9589f80ab07395864e6a1b68c3fa4ea9aaf11c5a2dfa6a4ae970adc4935e3eea5
height            81
vote_count        5
state_root        e3326ea9b745d7719ff8f996ac70c62709ed88787e2eb89bdee24371fbd733436b782aeecfabf33835594c16f15e417f
burn_tx_id        6053fbed3bcbcd5f3b65baa7166ac9e02343b32524a3adf75c27d8ce5e6ed36cf7c3bb9a30fc34176f4710339d3d6ce4
burn_fee          22
output_commitment 4bc0fbcf521ed66055dd6aa5fe215a18bb9b0f18a147876b5d12123787ed8427
note_file         $PFUSDC_ING/note.json
```

Operational note: the local mirror had node id `validator-3`. The certified
send path advanced the other validators, but the real validator-3 still needed
the certified shielded batch applied directly. Use `apply-shield-batch`, not
generic `apply-batch`.

```bash
scp -i ~/.ssh/id_ed25519 "$PFUSDC_ING/batch.json" \
  root@192.0.2.13:/tmp/pfusdc-ingress-batch.json
scp -i ~/.ssh/id_ed25519 "$PFUSDC_ING/artifacts/block-certificate.json" \
  root@192.0.2.13:/tmp/pfusdc-ingress-cert.json

ssh -i ~/.ssh/id_ed25519 root@192.0.2.13 \
  'postfiat-node apply-shield-batch \
     --data-dir /var/lib/postfiat/validator-3 \
     --batch-file /tmp/pfusdc-ingress-batch.json \
     --certificate-file /tmp/pfusdc-ingress-cert.json'
```

Then verify all six validators agree at height `81`.

## Step 4: Ingress a651 into the Asset-Orchard Pool

The run used a minimal live-value a651 note: `1` a651 atom. This proves the
typed-asset path without increasing live-value risk.

```bash
export A651_ING="$RUN_ROOT/ingress/a651"
mkdir -p "$A651_ING"
export A651_NOTE_SEED=$(printf \
  'asset-orchard-a651-live-ingress-20260620-height82' | sha256sum | awk '{print $1}')

"$BIN" asset-orchard-ingress-create \
  --data-dir "$DATA" \
  --key-file "$BUYER_KEY" \
  --asset-id "$A651" \
  --amount 1 \
  --note-seed-hex "$A651_NOTE_SEED" \
  --ingress-file "$A651_ING/ingress.json" \
  --note-file "$A651_ING/note.json" \
  --overwrite > "$A651_ING/create-report.json"

"$BIN" shield-batch-asset-orchard-ingress \
  --data-dir "$DATA" \
  --ingress-file "$A651_ING/ingress.json" \
  --batch-file "$A651_ING/batch.json"
```

Height `82` proposer was `validator-4`, so the certification command used the
validator-4 proposal key.

```bash
"$BIN" transport-peer-certified-batch-round \
  --data-dir "$DATA" \
  --topology "$TOPO" \
  --batch-kind shielded \
  --batch-file "$A651_ING/batch.json" \
  --key-file "$DATA/validator_keys.json" \
  --proposal-key-file "$CERT_KEYS/validator-4.validator_keys.json" \
  --artifact-dir "$A651_ING/artifacts" \
  --height 82 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 1000 \
  --quorum-early-full-propagation > "$A651_ING/round.json"
```

Live result:

```text
batch_id          3a62e8f88170c5d70e396d39328e8257ff83ff56fd0458ebf7257e1b0e849ab6a793fa6f799e5633320d40169653c1c8
height            82
vote_count        5
state_root        3bdbc78313bb6e227145ebb27be6f93c5ce99903580b74f372e5d2575bd32215bcf3574581d1d204b2e9a615abf2b27f
burn_tx_id        2b4fc7570e1398a9e39cc39bbae8ecf2f05c467e58b04189b27fa7381d7d53d0824219f61d5c7329f94a6fd3fbae2d1d
burn_fee          22
output_commitment e980eda1233e9062cd7cc4564e8ca4916ae400aa8fc59a2c610316f1a9be3e27
note_file         $A651_ING/note.json
```

Again, apply the certified shielded batch directly to real validator-3 if the
local mirror advanced but the remote validator did not.

```bash
scp -i ~/.ssh/id_ed25519 "$A651_ING/batch.json" \
  root@192.0.2.13:/tmp/a651-ingress-batch.json
scp -i ~/.ssh/id_ed25519 "$A651_ING/artifacts/block-certificate.json" \
  root@192.0.2.13:/tmp/a651-ingress-cert.json

ssh -i ~/.ssh/id_ed25519 root@192.0.2.13 \
  'postfiat-node apply-shield-batch \
     --data-dir /var/lib/postfiat/validator-3 \
     --batch-file /tmp/a651-ingress-batch.json \
     --certificate-file /tmp/a651-ingress-cert.json'
```

Then verify all six validators agree at height `82`.

## Step 5: Build the SNARK-Backed ShieldedSwap

The swap action consumes the two private asset-typed notes and creates two new
private asset-typed output notes. The public action contains nullifiers and
commitments, not the asset ids or amounts.

```bash
export SWAP="$RUN_ROOT/swap"
mkdir -p "$SWAP"
export SWAP_OUT_A_SEED=$(printf \
  'asset-orchard-swap-live-output-a-20260620-height83' | sha256sum | awk '{print $1}')
export SWAP_OUT_B_SEED=$(printf \
  'asset-orchard-swap-live-output-b-20260620-height83' | sha256sum | awk '{print $1}')

"$BIN" asset-orchard-swap-create \
  --data-dir "$DATA" \
  --input-note-file-a "$PFUSDC_ING/note.json" \
  --input-note-file-b "$A651_ING/note.json" \
  --output-note-seed-hex-a "$SWAP_OUT_A_SEED" \
  --output-note-seed-hex-b "$SWAP_OUT_B_SEED" \
  --action-file "$SWAP/action.json" \
  --output-note-file-a "$SWAP/output-note-a.json" \
  --output-note-file-b "$SWAP/output-note-b.json" \
  --overwrite > "$SWAP/create-report.json"

"$BIN" shield-batch-swap \
  --data-dir "$DATA" \
  --swap-file "$SWAP/action.json" \
  --batch-file "$SWAP/batch.json"
```

Live action summary:

```text
schema             postfiat-asset-orchard-swap-action-v1
pool_id            asset-orchard-v1
proof_system_id    postfiat.privacy.asset-orchard-halo2.v1
circuit_id         asset_orchard.swap.v1
anchor             a495102b88732f193c303e66380c167cd67fe3e9d2c18519c97cb4d78edf5021
swap_binding_hash  97e6d214be7f3978e59fadbfef08fa369dab2544fd3cbe06b926a9aaf129822e162cadac40f4afa6fbde7cd1a2c96b0f0bec06053f7bf2d470fdb1d4e570ce3f
proof_bytes        6,880
signature_count    2
```

Public nullifiers:

```text
590697301ced7958117469a321f0e54b47afc6a63094b0b1b89ccce2191c4d09
a0b23e5d16e58c80c26446d170ec96ad83dd63f947cb8eff6bbe49475a850310
```

Public output commitments:

```text
c53ca6390808b4af9e09bf5a9fbcd064eaa2d066dcd25e4904c8ae4a3791ab2b
896efdef189be41c548582165fd77103531cb15ff4127a613c733ac7cf8b2f1c
```

The proof generation and batch packaging steps are CPU-heavy. In this run the
local prover/batch-packaging phase took several minutes and produced no
intermediate stdout. Do not interrupt it solely because it is quiet; check CPU
activity first.

## Step 6: Certify and Apply the ShieldedSwap

Height `83` proposer was `validator-5`, so the swap certification used the
validator-5 proposal key. The timeout was set high because each validator must
verify the SNARK-backed action.

```bash
"$BIN" transport-peer-certified-batch-round \
  --data-dir "$DATA" \
  --topology "$TOPO" \
  --batch-kind shielded \
  --batch-file "$SWAP/batch.json" \
  --key-file "$DATA/validator_keys.json" \
  --proposal-key-file "$CERT_KEYS/validator-5.validator_keys.json" \
  --artifact-dir "$SWAP/artifacts" \
  --height 83 \
  --timeout-ms 900000 \
  --send-retries 3 \
  --retry-backoff-ms 1000 \
  --quorum-early-full-propagation > "$SWAP/round.json"
```

Live result:

```text
round_ok          true
proposal_proposer validator-5
vote_count        5
batch_id          cd815198d73b82f5de7b995fdc4941e744537fce42983ba2f37e93ca304d7632f3794980a38d7e01e7e672077ebcb92d
height            83
state_root        ecac90725d9a5e29e8371a7dff0fc03a79346a70ab8ea9beaaed825ec4c2dd7518d40d1f940e573503602fceb65c573c
block_tip_hash    d6f15bdc1e0c17a54dbe7e25b134862900fde387482d7a5d39895b56744daf91a48a168b00783d8e8eb5f7544778b5a2
client_finality   711,953 ms
```

The certified swap receipt on real validator-3 after direct catch-up was:

```text
tx_id    1f4cb1576a8981ea369f1d11f3dd465df086154fbfdf2bc2bf24dcc910c1349d8780878d2b2cadcc255b37e3caa2c8b5
code     accepted
message  asset-orchard swap verified and public pool state updated
```

If real validator-3 is behind because the local mirror applied locally, copy
the certified batch and certificate to validator-3 and use
`apply-shield-batch`.

```bash
scp -i ~/.ssh/id_ed25519 "$SWAP/batch.json" \
  root@192.0.2.13:/tmp/swap-batch.json
scp -i ~/.ssh/id_ed25519 "$SWAP/artifacts/block-certificate.json" \
  root@192.0.2.13:/tmp/swap-cert.json

ssh -i ~/.ssh/id_ed25519 root@192.0.2.13 \
  'postfiat-node apply-shield-batch \
     --data-dir /var/lib/postfiat/validator-3 \
     --batch-file /tmp/swap-batch.json \
     --certificate-file /tmp/swap-cert.json'
```

Final six-validator consensus:

```text
validator-0 height=83 root=ecac90725d9a5e29 tip=d6f15bdc1e0c17a5 mempool=0
validator-1 height=83 root=ecac90725d9a5e29 tip=d6f15bdc1e0c17a5 mempool=0
validator-2 height=83 root=ecac90725d9a5e29 tip=d6f15bdc1e0c17a5 mempool=0
validator-3 height=83 root=ecac90725d9a5e29 tip=d6f15bdc1e0c17a5 mempool=0
validator-4 height=83 root=ecac90725d9a5e29 tip=d6f15bdc1e0c17a5 mempool=0
validator-5 height=83 root=ecac90725d9a5e29 tip=d6f15bdc1e0c17a5 mempool=0
```

## Step 7: Privacy Verification

The public action keys in `$SWAP/action.json` were:

```text
anchor
circuit_id
encrypted_outputs
fee
nullifiers
output_commitments
pool_domain
pool_id
proof
proof_system_id
randomized_verification_keys
schema
spend_authorization_signatures
swap_binding_hash
version
```

The public action had:

```text
pool_id                         asset-orchard-v1
proof_system_id                 postfiat.privacy.asset-orchard-halo2.v1
circuit_id                      asset_orchard.swap.v1
nullifier_count                 2
randomized_verification_keys    2
output_commitment_count         2
encrypted_output_count          2
proof_bytes                     6,880
spend_authorization_signatures  2
```

It did not contain these fields:

```text
asset_id
amount
owner
recipient
```

Run this leakage scan against the public action and batch:

```bash
rg -n "$A651|$PFUSDC|$BUYER|$HOLDER|1000000|\"amount\"|\"asset_id\"|\"owner\"|\"recipient\"" \
  "$SWAP/action.json" "$SWAP/batch.json"
```

Expected result:

```text
no public matches for asset ids/accounts/amount fields in swap action or batch
```

Interpretation:

- Observers can see that an `asset-orchard-v1` swap occurred.
- Observers can see two nullifiers, two output commitments, encrypted outputs,
  proof bytes, spend authorization signatures, and the action binding hash.
- Observers cannot identify from the public action whether the swap involved
  a651, pfUSDC, another admitted asset, what values moved, or who received the
  output notes.
- Boundary actions are still public. The two ingress actions reveal asset ids
  and amounts because they burn transparent balances into the shielded pool.

## Step 8: Accounting and Reserve Verification

After the run, transparent buyer balances showed that the boundary assets had
been burned into the shielded pool:

```text
buyer pfUSDC v2 transparent balance  0
buyer a651 transparent balance       898
```

The buyer a651 balance was `899` before the a651 ingress and `898` after
burning `1` atom into the shielded note.

`pfUSDC` v2 asset state:

```text
asset_id            8751c2d04b993eb54f751b0f130c420fdb089548ec2f2a53837d11d1c397a1252e74bcc24616527e9c79b968635fae90
code                PFUSDC
version             2
precision           6
display_name        pfUSDC-WAN-v2
outstanding_supply  0
trustline_count     2
holder_count        0
```

`a651` asset state:

```text
asset_id            dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5
code                a651
version             1
precision           6
display_name        NAVCoin a651 (canonical interim)
outstanding_supply  1998
trustline_count     2
holder_count        2
```

Vault bridge status remained auditable:

```text
issued_supply_atoms                  0
counted_value_atoms                  3,916,365
unallocated_counted_capacity_atoms   916,365
receipt_count                        3
bridge_deposit_count                 3
allocation_count                     4
redemption_count                     2
gross_receipt_atoms                  9,000,000
outstanding_vault_bridge_atoms       1,000,000
redemption_queue_atoms               2,000,000
last_updated_height                  80
```

Fresh deposit status:

```text
deposit_id          1d6f13ad112e09204949098d69cad7c8f8d3ff8dbd59eef88f4870625e14ad9a
evidence_root       0cf280e6db6b0bc68c1c534891b6ddba2d67789b580e1284c1a1fdb0369766e45ef62b5534d0cb1a2804cb6933f7359e
amount_atoms        1,000,000
status              finalized
submitted_height    77
finalized_height    79
pftl_recipient      pf07381735ddb7de134e8be8402b465c9cd8ec7546
```

The shielded swap does not mint, redeem, or recompute NAV reserves. It consumes
already-ingressed private notes and creates replacement private notes with
per-asset conservation enforced by the Halo2 proof. The auditable reserve
boundary remains the bridge/NAV transparent accounting:

- Arbitrum custody is evidenced by the bridge deposit receipt.
- pfUSDC mint/claim is evidenced by PFTL vault-bridge state.
- pfUSDC ingress burns transparent pfUSDC supply into a private note.
- a651 ingress burns transparent a651 into a private note.
- the internal swap updates nullifiers and commitments only.

## Step 9: Shielded Pool Verification

Run:

```bash
"$BIN" verify-shielded --data-dir "$DATA"
"$BIN" orchard-pool-report --data-dir "$DATA"
```

Live result:

```text
verify-shielded.verified       true
orchard_pool_id                asset-orchard-v1
orchard_nullifier_count        4
orchard_output_count           8
orchard_anchor_count           2
orchard_root_count             7
orchard_latest_root            40d6930e80f480e04da12447229e9d26f3870a677f7567135db0f0a778f8420c
orchard_value_balance_total    0
```

`orchard-pool-report` passed and reported:

```text
pool_initialized                         true
output_count                             8
nullifier_count                          4
retained_root_count                      7
accepted_anchor_count                    2
latest_retained_root                     40d6930e80f480e04da12447229e9d26f3870a677f7567135db0f0a778f8420c
exact_active_note_count_publicly_available false
conservative_public_floor                4
public_upper_bound                       8
state_verified                           true
no_private_material_fields               true
passed                                   true
```

## Step 10: Six-Validator Status Check

Use this status check pattern after each live height. It checks the six public
all-Vultr RPC endpoints directly and avoids host-login dependencies.

```bash
python3 - <<'PY'
import json, socket

peers = [
    ('validator-0', '192.0.2.10', 27650),
    ('validator-1', '192.0.2.11', 27651),
    ('validator-2', '192.0.2.12', 27652),
    ('validator-3', '192.0.2.13', 27653),
    ('validator-4', '192.0.2.14', 27654),
    ('validator-5', '192.0.2.15', 27655),
]

rows = []
for node, host, port in peers:
    with socket.create_connection((host, port), timeout=10) as s:
        request = {
            'version': 'postfiat-local-rpc-v1',
            'id': node,
            'method': 'server_info',
            'params': {},
        }
        s.sendall((json.dumps(request) + '\n').encode())
        data = b''
        while not data.endswith(b'\n'):
            data += s.recv(65536)
    result = json.loads(data.decode())['result']
    ledger = result['ledger']
    rows.append((node, ledger['height'], ledger['state_root'],
                 ledger['hash'], result['mempool']['pending']))

for r in rows:
    print('%s height=%s root=%s tip=%s mempool=%s' %
          (r[0], r[1], r[2][:16], r[3][:16], r[4]))
print('unique_count', len({(r[1], r[2], r[3]) for r in rows}))
PY
```

Final expected result for this run:

```text
unique_count 1
height       83
root         ecac90725d9a5e29...
tip          d6f15bdc1e0c17a5...
mempool      0 on all validators
```

## Operational Lessons

Use `apply-shield-batch` for certified shielded batches.

The generic `apply-batch` path produced a batch-id mismatch on one shielded
batch because it interpreted the archived payload through the wrong path. The
shielded-specific apply command accepted the same certified batch.

Do not rely on `rpc-catch-up` for this specific historical window.

`rpc-catch-up` failed on validator-3 because an old archived transparent payload
could not be reconstructed:

```text
block 9 archived transparent payload invalid: batch id mismatch
```

That was unrelated to the height-81 to height-83 shielded batches. Direct
certified shielded apply was the correct repair path for this run.

Expect long proof and verification times.

Observed timings:

```text
asset-orchard-swap-create      several minutes, CPU active
shield-batch-swap              several minutes, CPU active
height-83 certification        711,953 ms total
local apply inside round       18,005 ms
validator-3 direct apply       about 8 minutes, CPU active
```

The command may be quiet while CPU is active. Check `ps` before assuming a
hang.

## What This Proves

This controlled-devnet run proves:

- a fresh real-USDC Arbitrum deposit can be relayed into the real WAN devnet as
  v2 pfUSDC;
- transparent pfUSDC can be burned into an asset-typed Orchard note;
- transparent a651 can be burned into an asset-typed Orchard note;
- the live WAN validators accept a real Halo2-backed `shielded_swap_v1`;
- the swap public payload hides asset identity, value, owner, recipient, and
  counterparty;
- public bridge/NAV accounting remains available at transparent boundaries;
- all six validators converged on the same height-83 state root after the swap.

This run does not prove:

- production anonymity-set size;
- post-quantum privacy for the Halo2/Orchard proof path;
- fully unattended public bridge operation;
- a finalized market-ops envelope for a651 at the swap height.

Those are separate production-readiness gates.

# WAN Devnet Full Live End-to-End Run

Status: operator runbook
Audience: PFTL operator, StakeHub operator, protocol engineer
Date: 2026-06-21

This runbook describes the next correct live-value WAN devnet run. It is not a
local/no-value demo. It starts with real Arbitrum USDC, enters the real PFTL
WAN devnet, touches real `pfUSDC` and real NAVCoin accounting, exercises the
private swap path, swaps back, exits to Arbitrum USDC, and verifies public
validator convergence and NAV accounting at every economic boundary.

## What End-to-End Means

End-to-end means the value completes a full round trip through the actual
system:

1. real USDC leaves the StakeHub Arbitrum wallet;
2. that USDC is deposited into the Arbitrum `ERC20BridgeVault`;
3. the deposit is relayed to PFTL and mints bridge-backed `pfUSDC`;
4. `pfUSDC` is used against the real NAVCoin path, not a local test asset;
5. NAV is checked after money enters, proving verified net assets changed by
   the expected amount;
6. `a651` and `pfUSDC` are shielded into the asset-typed Orchard pool;
7. a real SNARK-backed private swap executes;
8. the position is swapped back or otherwise unwound so the value can exit;
9. NAV is checked after money leaves, proving verified net assets returned or
   changed by the expected exit amount;
10. `pfUSDC` is burned/redeemed through the bridge;
11. USDC is finalized and claimed back on Arbitrum;
12. final bridge custody, PFTL accounting, NAV accounting, and all six public
    validator roots are verified.

If the value only goes in but does not come back out, the run is not
end-to-end. If NAV is not checked after money-in and after money-out, the run is
not end-to-end. If the run only succeeds on quorum while public validators are
stale, the run is not end-to-end.

## Claim Boundary

This runbook can be used for two classes of live run:

| Class | Requirement | Claim allowed |
| --- | --- | --- |
| Controlled launch | Existing controlled-launch contracts and short challenge windows are acceptable if explicitly recorded in the run manifest. | Controlled WAN-devnet live-value proof. |
| Fixed bridge | F-01/F-02/F-03/F-04 Arbitrum fixes are redeployed, verified, and used by the run. | Stronger bridge-security proof against the fixed contracts. |

Do not blur these. If the run uses the old controlled-launch Arbitrum
contracts, label it as controlled launch. If the run uses fixed contracts,
record the fixed contract addresses and the verification evidence.

## Hard Stop Rules

Stop before moving funds if any of these are true:

- any public validator is unreachable;
- any public validator reports a different chain id, genesis hash, height, tip,
  or state root;
- the local operator data directory has a `node_id` that collides with a public
  validator identity;
- the runner is configured for quorum-only success or degraded peer handling;
- the run manifest is missing;
- StakeHub `agentd` is not unlocked or cannot sign via the approved path;
- Arbitrum wallet USDC/gas balances are insufficient;
- the bridge contract class is unknown;
- the NAV baseline cannot be read;
- the expected money-in and money-out accounting deltas cannot be computed
  before the deposit.

Stop during the run if any of these are true:

- any certified send is skipped;
- any public validator fails to reach the expected height/root after a PFTL
  stage;
- the NAV checkpoint after money-in does not increase verified net assets by
  the expected settlement amount;
- the NAV checkpoint after money-out does not reduce or restore verified net
  assets by the expected exit amount;
- Arbitrum vault/wallet deltas do not match the deposit or withdrawal amount;
- the shielded swap action reveals asset id, amount, owner, recipient, or
  counterparty on chain;
- bridge custody accounting disagrees with actual vault USDC balance.

## Required Artifacts

Every run must write a timestamped run root:

```text
$POSTFIAT_STATE/live-e2e-YYYYMMDDTHHMMSSZ
```

Required files:

```text
run-manifest.json
preflight-public-validators-before.json
preflight-wallets-and-contracts.json
nav-baseline-before-money-in.json
flow-01-evm-deposit.json
flow-02-deposit-relay.json
flow-03-primary-mint.json
flow-04-nav-after-money-in.json
flow-05-shield-ingress.json
flow-06-private-swap-forward.json
flow-07-private-swap-back.json
flow-08-nav-exit.json
flow-09-nav-after-money-out.json
flow-10-burn-to-redeem.json
flow-11-evm-withdrawal-claim.json
flow-12-pftl-redeem-settle.json
final-public-validator-convergence.json
final-bridge-custody.json
final-summary.json
```

The run is not green unless `final-summary.json` references every artifact
above and all checks are green.

## Known Live Constants

Update these before the run if any address or asset id has changed.

```text
chain_id             postfiat-wan-devnet
genesis_hash         231b1cfb63439c23bdcc3f7ea2f7f3ce7a53f9abffef8f720f47421b575f16e7f2d9ad5e61298207be2e9ce08743f870
protocol_version     1
validator_count      6

source_chain_id      42161
stakehub_wallet      0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
arbitrum_usdc        0xaf88d065e77c8cC2239327C5EDb3A432268e5831

a651 asset_id        dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5
pfUSDC asset_id      8751c2d04b993eb54f751b0f130c420fdb089548ec2f2a53837d11d1c397a1252e74bcc24616527e9c79b968635fae90
asset_orchard_pool   asset-orchard-v1
```

## Validator Topology

Use public RPC evidence as the source of truth. Do not substitute a local data
directory for a public validator.

The active testnet topology is **all Vultr**. Hetzner hosts are no longer part
of the active validator set for this run and must not appear in scripts,
topology files, status checks, or recovery procedures.

The replacement plan is to run the old validator-0/1/2 slots on three Vultr
instances and keep validator-3/4/5 on the existing Vultr instances. There is no
Hetzner fallback path. If any of the new Vultr replacements are unavailable,
stop the run and repair or replace that Vultr host; do not move the slot back to
Hetzner.

Topology source of truth:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/all-vultr-remote-topology.json
```

Topology id:

```text
3e09754e96dd78df121e4a2806f7bb07e9e8f3dd2386152ed029386aaa579a08ebb717ca404de3dfabffefc6b330070d
```

```text
validator-0 192.0.2.10:27650
validator-1 192.0.2.11:27651
validator-2 192.0.2.12:27652
validator-3 192.0.2.13:27653
validator-4 192.0.2.14:27654
validator-5 192.0.2.15:27655
```

Authentication hygiene:

- All validators use Vultr root SSH key auth with `~/.ssh/id_ed25519`.
- Do not use `sshpass`.
- Do not put plaintext passwords on a command line.
- Do not use Hetzner boxes for this run.

### Vultr Replacement Procedure

Use this procedure when replacing a validator slot or rebuilding the testnet
fleet. It exists to prevent the old mixed Hetzner/Vultr plan from reappearing.

1. Provision three Vultr instances for validator-0/1/2.
2. Install the release binary and system service on each new Vultr host.
3. Initialize or restore the validator data directory from an approved local
   genesis/snapshot source or a healthy Vultr peer. Do not copy live state from
   Hetzner.
4. Stage only the intended validator identity/key material for that slot. Never
   run the same validator identity on two hosts at once.
5. Update the topology JSON so validator-0/1/2 point to the Vultr IPs.
6. Start one replacement at a time and verify public RPC chain id, genesis
   hash, height, tip, and state root across all six validators before starting
   the next replacement.
7. Record the topology id and public RPC convergence table in the run
   manifest.

The current Vultr replacements are:

```text
validator-0 192.0.2.10:27650   host wan-vultr-validator-0-repl
validator-1 192.0.2.11:27651  host wan-vultr-validator-1-repl
validator-2 192.0.2.12:27652     host wan-vultr-validator-2-repl
```

These replacements are live on the testnet and reachable with
`ssh -i ~/.ssh/id_ed25519 root@<ip>`. Their validator and RPC services must be
`active` before any live-value run starts.

### 2026-06-21 All-Vultr Live-Value Result

The replacement topology above was used for a real Arbitrum USDC round trip on
2026-06-21. The run reused the existing StakeHub-signed deposit artifact and did
not use Hetzner hosts, password auth, or raw EVM private keys.

Run artifacts:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/roundtrip-live-20260621T063208Z
```

Result:

```text
final_summary_ok                 true
bridge_class                     controlled_launch_existing_contracts
final_height                     146
final_state_root                 cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
final_mempool_pending            0
final_validator_consensus_ok     true
```

Money-in evidence:

```text
deposit_tx                       b4f1fa881eb3a586e64fd2cfcf87839c656785df2712caf75fafa44db509ae2c
deposit_amount_atoms             5082364
wallet_usdc_atoms                102005183 -> 96922819
vault_usdc_atoms                 6994916 -> 12077280
deposit_delta_ok                 true
expected_money_in_vna_delta      508236400
actual_money_in_vna_delta        508236400
nav_money_in_delta_ok            true
```

Money-out evidence:

```text
claim_withdrawal_tx              a53398c82e916ff7cf799d02aa23ce96bb8e5c94674c21841a945648d2cf1a55
withdrawal_amount_atoms          5083635
wallet_usdc_atoms                96922819 -> 102006454
vault_usdc_atoms                 12077280 -> 6993645
withdrawal_delta_ok              true
expected_money_out_vna_delta     -508236400
actual_money_out_vna_delta       -508236400
nav_money_out_delta_ok           true
```

Bridge accounting after PFTL settlement:

```text
redemption_state                 pending -> settled
redemption_queue_atoms           7083635 -> 2000000
counted_value_atoms              14077280 -> 8993645
accounting_ok                    true
```

Fresh public RPC convergence after the run:

```text
validator-0 192.0.2.10:27650  height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28 pending=0
validator-1 192.0.2.11:27651 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28 pending=0
validator-2 192.0.2.12:27652    height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28 pending=0
validator-3 192.0.2.13:27653  height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28 pending=0
validator-4 192.0.2.14:27654 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28 pending=0
validator-5 192.0.2.15:27655  height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28 pending=0
unique_height_root_tip_count      1
```

Shielded-state check on the final local data mirror:

```text
verify-shielded.verified         true
orchard_pool_id                  asset-orchard-v1
orchard_nullifier_count          10
orchard_output_count             16
orchard_anchor_count             5
```

## Phase 0: Make The System Correct

### 0.1 Build and Test the Local Binary

Run from `$POSTFIAT_REPO`:

```bash
cargo fmt --check
cargo test -p postfiat-node nav_roundtrip -- --nocapture
cargo test -p postfiat-node wan_devnet_legacy -- --nocapture
cargo test -p postfiat-node rpc_catch_up_rejects_zero_max_blocks_before_work_dir_mutation -- --nocapture
cargo build --release -p postfiat-node
openssl dgst -sha3-384 target/release/postfiat-node
```

Required result:

- formatting green;
- live runner tests green;
- WAN legacy replay tests green;
- catch-up preflight test green;
- release binary built;
- release hash recorded in `run-manifest.json`.

### 0.2 Decide and Record Bridge Class

Record one of:

```text
bridge_class=controlled_launch_existing_contracts
bridge_class=fixed_contracts_redeployed
```

For `fixed_contracts_redeployed`, record:

- `ERC20BridgeVault` address;
- `PFTLWithdrawalVerifier` address;
- verifier membership/threshold;
- challenge-window settings;
- deployed bytecode hash;
- Foundry verification result;
- a fresh small bridge battery against those addresses.

For `controlled_launch_existing_contracts`, record:

- the existing vault and verifier addresses;
- the known limitation that F-01/F-02/F-03/F-04 are fixed in source but not
  necessarily live at those addresses;
- operator acceptance that this is a controlled-launch demo, not a public
  trustless bridge claim.

### 0.3 Roll the Fixed PFTL Binary or Explicitly Waive

The clean path is a rolling binary deployment to all six Vultr validators:

1. stage `target/release/postfiat-node` on validator-0;
2. stop that validator and RPC service only;
3. replace the service binary;
4. start services;
5. verify public RPC height/root convergence across all six;
6. repeat for validator-1 through validator-5.

Stop if any validator rejoins at a different height/root.

If the operator waives the binary roll, record:

- the service binary hash on each host;
- the local runner binary hash;
- the staged recovery binary hash, if any;
- why the live run is allowed without replacing service binaries.

### 0.4 Public Fleet Preflight

Query all six public RPC endpoints.

Required:

```text
chain_id == postfiat-wan-devnet
genesis_hash == 231b1cfb63439c23bdcc3f7ea2f7f3ce7a53f9abffef8f720f47421b575f16e7f2d9ad5e61298207be2e9ce08743f870
height equal across all six
tip hash equal across all six
state root equal across all six
unique_height_root_tip_count == 1
```

Write `preflight-public-validators-before.json`.

### 0.5 Local Operator Identity Guard

If the local data directory has a validator `node_id`, it must not silently
stand in for a public validator.

Required:

- local state is labeled `operator_local_state`; or
- the run is executed on the matching validator host; or
- the run stops.

No local `validator-3` mirror may be counted as public `validator-3`.

### 0.6 StakeHub and Arbitrum Preflight

Required:

- StakeHub `agentd` is unlocked;
- no raw EVM private key is used;
- Arbitrum RPC is reachable;
- StakeHub wallet has enough USDC and gas ETH;
- vault has expected pre-run USDC balance;
- USDC allowance is either zero or exactly controlled by the run;
- bridge vault bytecode exists at the configured address;
- withdrawal verifier bytecode exists at the configured address.

Write `preflight-wallets-and-contracts.json`.

### 0.7 NAV Baseline

Before funds move, record:

- a651 profile id;
- a651 supply;
- a651 verified net assets;
- a651 NAV floor;
- pfUSDC bridge bucket status;
- pfUSDC unallocated capacity;
- actual Arbitrum vault USDC balance;
- expected money-in VNA delta;
- expected money-out VNA delta.

Write `nav-baseline-before-money-in.json`.

## Phase 1: Full Live End-to-End Run

### Flow 1: Deposit Real USDC into the Arbitrum Vault

Use StakeHub `agentd` signing only.

Required checks:

- StakeHub wallet USDC decreases by the deposit amount;
- vault USDC increases by the deposit amount;
- deposit event binds the PFTL recipient;
- deposit event binds the nonce;
- transaction hash and receipt are archived.

Write `flow-01-evm-deposit.json`.

### Flow 2: Relay Deposit to PFTL and Mint pfUSDC

Use the vault-bridge deposit relay path against the real WAN devnet.

Required checks:

- EVM receipt proof is included;
- deposit cannot be relayed twice;
- pfUSDC balance increases for the intended PFTL account;
- bridge bucket counted value and custody view match the relay;
- all six validators converge after certification.

Write `flow-02-deposit-relay.json`.

### Flow 3: Subscribe pfUSDC into a651 as Primary Mint

Use the real a651 profile and the real pfUSDC asset. Do not bootstrap a new
NAVCoin or a new local pfUSDC.

Required checks:

- pfUSDC allocation is consumed or locked according to the NAV mint path;
- a651 supply changes by the expected amount;
- settlement amount is computed from the live NAV formula;
- all six validators converge after certification.

Write `flow-03-primary-mint.json`.

### Flow 4: NAV Checkpoint After Money-In

Submit and finalize the next a651 reserve/NAV epoch with the pfUSDC reserve leg
counted correctly.

Required checks:

```text
verified_net_assets_after - verified_net_assets_before == expected_money_in_delta
```

The expected delta must be derived from the actual pfUSDC settlement atoms, not
from a hardcoded nominal example.

If verified net assets do not rise by the expected amount, stop. This is a real
accounting failure.

Write `flow-04-nav-after-money-in.json`.

### Flow 5: Shield Ingress for pfUSDC and a651

Ingress transparent issued assets into the asset-typed Orchard pool.

Required checks:

- transparent pfUSDC is locked or burned so it cannot be double-spent;
- transparent a651 is locked or burned so it cannot be double-spent;
- asset-typed note commitments are inserted into the asset Orchard tree;
- note witnesses are available to the swap builder;
- public action does not reveal private recipient details beyond the designed
  transparent ingress boundary;
- all six validators converge after certification.

Write `flow-05-shield-ingress.json`.

### Flow 6: Private Swap Forward

Execute the real SNARK-backed `ShieldedSwap`.

Required checks:

- proof verifies at consensus;
- input nullifiers are unique and newly spent;
- output commitments are unique and inserted;
- action binding hash matches the signed/proved action;
- public chain data reveals nullifiers, commitments, proof bytes, and pool
  metadata only;
- public chain data does not reveal asset ids, amounts, owners, recipients, or
  counterparties;
- all six validators converge after certification.

Write `flow-06-private-swap-forward.json`.

### Flow 7: Private Swap Back

Execute the reverse private swap or equivalent unwind so the value can exit the
system back through pfUSDC.

Required checks:

- proof verifies at consensus;
- the forward-swap output note is spendable;
- the reverse-swap output note gives the operator an exitable pfUSDC position;
- no public asset id, amount, owner, recipient, or counterparty is revealed;
- all six validators converge after certification.

Write `flow-07-private-swap-back.json`.

### Flow 8: Exit a651 Back to pfUSDC

Use the NAV redeem path to exit a651 into pfUSDC.

If the selected exit path starts from a shielded Asset-Orchard a651 note, the
current implementation first uses disclosed Asset-Orchard egress to return the
note to a public a651 balance. That disclosed egress reveals the note opening,
asset id, amount, nullifier, and destination. Do not claim this step is private
egress or private cash-out.

Required checks:

- a651 balance or shielded exit position is consumed according to the selected
  exit path;
- if disclosed egress is used, its receipt and revealed fields are archived in
  the run artifacts;
- pfUSDC redemption allocation is created or settled;
- a651 supply and redemption queue update correctly;
- all six validators converge after certification.

Write `flow-08-nav-exit.json`.

### Flow 9: NAV Checkpoint After Money-Out

Finalize the next a651 reserve/NAV epoch after the exit.

Required checks:

```text
verified_net_assets_after_exit == expected_after_exit_vna
```

The expected value must account for the exact exit settlement and any remaining
inventory/reserve treatment. If money was fully round-tripped out, verified net
assets should return by the expected money-out delta. If some value remains as
inventory, the run must explain and quantify it.

If NAV does not move as expected, stop before bridge redemption.

Write `flow-09-nav-after-money-out.json`.

### Flow 10: Burn pfUSDC to Redeem

Burn or lock pfUSDC on PFTL to request Arbitrum USDC withdrawal.

Required checks:

- burn packet binds recipient, vault, token, source chain, amount, nonce, and
  redemption id;
- redemption cannot be replayed;
- redemption queue and counted vault value update consistently;
- unallocated bridge capacity equals actual available vault balance after
  accounting for pending redemptions;
- all six validators converge after certification.

Write `flow-10-burn-to-redeem.json`.

### Flow 11: EVM Withdrawal Proof, Finalize, Submit, Claim

Use the configured withdrawal verifier and bridge vault.

Required checks:

- proof digest binds the exact packet/hash pair;
- signer set satisfies threshold and membership;
- signers are sorted and unique;
- low-s signatures are enforced;
- challenge/finality windows are respected;
- withdrawal cannot be replayed;
- recipient cannot be substituted;
- StakeHub wallet USDC increases by the withdrawal amount;
- vault USDC decreases by the withdrawal amount.

Write `flow-11-evm-withdrawal-claim.json`.

### Flow 12: PFTL Redemption Settlement

Settle the PFTL redemption after the Arbitrum claim.

Required checks:

- redemption status is closed;
- redemption queue decreases;
- counted vault value decreases with the queue;
- unallocated capacity equals actual vault USDC balance;
- all six validators converge after certification.

Write `flow-12-pftl-redeem-settle.json`.

## Phase 2: Final Verification

### 2.1 Public Validator Convergence

Query all six public RPC endpoints.

Required:

```text
unique_height_root_tip_count == 1
```

Write `final-public-validator-convergence.json`.

### 2.2 Bridge Custody

Compare:

- actual Arbitrum vault USDC balance;
- PFTL counted vault value;
- PFTL redemption queue;
- PFTL unallocated bridge capacity;
- total minted pfUSDC;
- total burned/redeemed pfUSDC.

Required:

```text
unallocated_capacity == actual_available_vault_balance
pending_redemptions == redemption_queue
minted_minus_burned == circulating_or_locked_pfusdc
```

Write `final-bridge-custody.json`.

### 2.3 NAV Accounting

Compare:

- baseline VNA before money-in;
- VNA after primary mint;
- VNA after exit;
- expected deltas from actual settlement atoms.

Required:

```text
money_in_delta_ok == true
money_out_delta_ok == true
```

Write final NAV evidence into `final-summary.json`.

### 2.4 Privacy Boundary

Inspect the public shielded actions from Flow 6 and Flow 7.

Allowed public data:

- nullifiers;
- note commitments;
- proof bytes;
- public pool/domain metadata;
- action hashes;
- encrypted payload commitments or ciphertext hashes.

Forbidden public data:

- asset id;
- amount;
- owner;
- recipient;
- counterparty;
- swap direction.

Write privacy evidence into `final-summary.json`.

## Final Green Criteria

The run is green only if all are true:

- all required artifacts exist;
- real Arbitrum USDC moved in and then back out;
- bridge deposit, relay, burn, proof, finalize, claim, and settlement all
  completed;
- a651 NAV after money-in rose by the expected amount;
- a651 NAV after money-out moved by the expected exit amount;
- the private swap forward and swap-back were verified by the real consensus
  SNARK verifier;
- public shielded actions did not reveal asset, amount, owner, recipient, or
  counterparty;
- bridge custody equals PFTL accounting;
- all six public validators report the same final height, tip hash, and state
  root;
- no local validator mirror was counted as a public validator;
- final summary labels the bridge class accurately.

## Failure Handling

If a failure occurs before the Arbitrum deposit, fix the preflight and restart
with a new run root.

If a failure occurs after funds enter the vault:

1. stop submitting new state transitions;
2. query and archive all six public validator states;
3. archive Arbitrum wallet/vault balances;
4. archive bridge bucket status;
5. archive NAV profile/supply/reserve status;
6. write `run-failure.json` with the exact last green flow;
7. do not continue from a guessed state.

If validators diverge:

1. stop the live run;
2. do not run another bridge or shielded action;
3. preserve every validator data dir;
4. use `wan-devnet-structural-fix.md` repair protocol;
5. repair one validator at a time only after replay/catch-up preflight passes.

## Related Documents

- `docs/runbooks/wan-devnet-structural-fix.md`
- `docs/runbooks/nav-roundtrip-speedup-plan.md`
- `docs/runbooks/private-nav-otc-shielded-swap-wan-devnet.md`

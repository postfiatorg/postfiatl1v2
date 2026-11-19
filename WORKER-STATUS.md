BLOCKED

Release 6 candidate `3725330d` was built and rehearsed locally. Hold the rollout. The copy and transaction gates below are independent. All SSH activity in this task read existing files. No live transaction was submitted.

The [rehearsal and host checklist](docs/review/release6-solana-route-rehearsal.md) gives the commands, paths and deployment sequence. In the commands below, `E=/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-release6`, `R5=/home/postfiat/var/b24-release5-20261117/bin/postfiat-node`, `R6=$E/bin/3725330d/postfiat-node` and `LOG=$E/evidence`.

| Gate command | Actual output |
| --- | --- |
| `cargo build --locked --release -p postfiat-node --bin postfiat-node` | `Finished release profile [optimized]`, 217.44 s wall time. |
| `sha256sum "$R5" "$R6"` | R5 `69de5ce01bbda92ac5507565388d9e9655457be9bdd37582efd5d38f969bcd18`. R6 `5238d4a486c5e84cecaa41c4cb4a3bd093c903f5debd9cd709aea3ac7d12f70c`. |
| `cargo test --locked -p postfiat-execution --lib` | `237 passed; 0 failed; 0 ignored`. Verified the release-pinned epoch 6 and 7 Groth16 handoffs. |
| `cargo test --locked -p postfiat-node --lib vault_bridge -- --skip solana_bridge` | `30 passed; 0 failed; 2 ignored`. |
| `cargo test --locked -p postfiat-node --bin postfiat-node transport` | `51 passed; 0 failed; 0 ignored`. |
| `cargo test --locked -p postfiat-node --test b23_facility_offer_finality` | `6 passed; 0 failed; 0 ignored`. |
| `cargo test --locked -p postfiat-node --lib b23_release4_public_state_copy_root_and_route_readback -- --ignored --nocapture` | `public state root unchanged at 1257: 050adc8eaa1c5c420870b01e29504fd003b0fb9eec30fc34c48b5c85037b5bbd71104dfbfc12e0d06ed3dfab9387c7b7; epoch-11 route read succeeded`. |
| `cargo test --locked -p postfiat-node --lib b23_legacy_live_disk_copies_deserialize -- --ignored --nocapture` | `17 public live-disk legacy inputs decode and remain inadmissible`. |
| `cargo test --locked -p postfiat-node --lib governed_route_real_anvil_deposit_withdrawal_roundtrip -- --ignored` | Clean R5 and R6 both rejected `deposit-claim` with `pfusdc_source_series_not_active` when running Anvil with immediate finality. |
| `ops/release6/copy-state.sh "$E/copies" "$n"`, for validators 0–5 | Six transfers completed, 1,791,193,304–1,794,885,596 bytes per host. |
| `ops/release6/probe-copy.sh "$binary" "$E/copies/validator-$n" status`, both binaries × six directories | All 12 returned exit 1, `integrity MAC mismatch in genesis` and `refusing to load possibly tampered state`. |
| `"$R5" snapshot-import-finalized-checkpoint --snapshot-dir "$E/portable" --data-dir "$E/restore/validator-$n" --node-id validator-$n` | Six successful imports of the public height-1161 checkpoint. |
| `python3 ops/release6/catch-up.py "$R5" STORE WORK PEER 1453` | At 1209 each source reached the same `current_sync_committee_root must be 64 lowercase hex characters` rejection. The signed archive requires its historical execution rules. |
| `ops/release6/continue-replay.sh "$E" "$n" 1453` | The binary recorded for height 1209 verified its certificate and produced root `e101f848597ed4b58cccc8f5b93cc2048beb1d900b73f3a6e2c5e02753183bc128cc9b4c7d94b898fb74e4c83ee8f19c`. Release 5 resumed from 1210. |
| `python -B /run-check.py --deposit-fixture /fixture --fleet-public-registry /fleet-registry` in the documented loopback-only container | Six candidate validators finalized 100,000 atoms at height 1. Fresh keys were distinct from the fleet registry. They activated the route through signed governance and credited 5,000,000 atoms from the slot-24 proof at height 8. All six verified root `3666b55aeff51f34a4b2ff347077f10d585da510e7222c057d4bb5b61684422009a97013cf199a3e68acf1cdcf939e2d`. |

The physical directories cannot authenticate without their original integrity keys. The portable checkpoint plus independently sourced certified deltas provides a way to reconstruct the public chain. A fresh committee with the same genesis finalizes transfers and deposits using its own keys. The copied fleet committee still needs its own signers to certify a new block.

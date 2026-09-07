# Hyperliquid and Lighter funding activation review

Implementation is in progress. This package does not authorize transfers.

Working local commands (from StakeHub):

```bash
.venv/bin/python -m stakehub.shielded_exit_cover_cli policy
.venv/bin/python -m stakehub.shielded_exit_cover_cli simulate --amount-eth 0.005
.venv/bin/python -m stakehub.shielded_exit_cover_cli serve
```

Open gates:

- [ ] operator_budget: manager-selected isolated wallet, exact capital, native ETH/PFT and proof/quote budgets, pilot holding horizon
- [ ] route_verification: fresh chain pins, review of vault/verifier/route/Inbox and Lighter proxy/implementation/additional implementation, certified ingress/burn/release conformance
- [ ] archive_importer: complete finalized Ethereum/PFTL/Arbitrum/venue history importer, ownership/sponsor/behavior evidence and source lineage; normalized JSON alone is insufficient
- [ ] production_driver: integrate local note creation, public-state synchronization, ingress proof relay, shared sponsorship, finalized burn/release and venue credit into the common stage runner
- [ ] funded_probe: source-to-Hyperliquid and source-to-Lighter functional runs at an explicitly selected affordable denomination; exact bridge delivery where applicable and independently verified venue credit
- [ ] cover_privacy: actual funded cover history passes every selected model; separate operator result and later disclosure sensitivity
- [ ] operating_capacity: measured fees, inventory, finite venue holding capacity, restart/recovery and disclosure behavior

# Operator scripts written during the 23 November go-live

These ran against the live system. They are kept here so nothing lives only on the relay host.

- `settle.py`, `settler.sh`: verify a finalized Solana payout against its burn and attest it as paid with six signers. `settler.sh` runs in tmux session `settler` on the relay host; its installed successor is `exit_settler.py` (RUNBOOK-LIVE.md, "Settlement loop as a service").
- `clearance.py`: writes the six rotation clearance files for the current parent.
- `advance_checkpoint.py`: advances the Solana light checkpoint with the setup payer.
- `manual_payout.py`: pays one burn with a witness taken from the validator CLI. Used once, before the witness assembly fix.
- `send_test_deposit.py`: sends a small real USDC deposit from the setup payer to the b23-bob test wallet and waits for six-validator credit.
- `cutover-mixed.py`: switched the deposit prover to instance 62235352 while keeping the original observer.
- `reanchor-page-size.patch`: applied to the live checkout so the re-anchor reads eight blocks per request.
- `copy-exit-prover.sh`: copied the withdrawal proof host to the four-GPU machine under `/workspace/exit` (not yet in use).

Known gaps: the settlement loop applies one operator's checks for all six signers; the prover key entry on the withdrawal prover is also in `authorized_keys2` because Vast rewrote `authorized_keys`; the validator-0 RPC timeout was raised to 120 s by a systemd drop-in.

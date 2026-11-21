# Solana pfUSDC relay

The mainnet custody watcher and wallet status unit create the relay configuration from the governed profile. See [the local service cutover](../../ops/sol-live/CUTOVER.md).

Run the offline unit suite with `python3 -m unittest discover -s tools/pfusdc-sol-relay -p 'test_*.py'`. Run `python3 tools/pfusdc-sol-relay/check_fast_delivery.py` for the delivery checks.

The end-to-end stub gate accepts a disposable run directory and a separately supplied captured source directory containing its signature, archive and route metadata. Run `tools/pfusdc-sol-relay/gate.sh RUN_DIRECTORY CAPTURE_DIRECTORY` from the root. The recorder replay accepts `check_recorder.py STUB_CONFIG --evidence PROOF_DIRECTORY`. The historical capture, proofs and GPU setup are external inputs. Keep all stub state separate from service state.

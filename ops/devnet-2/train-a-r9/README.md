# train-a-r9: the six devnet-2 validators on `main`

Rolled 2026-11-28. Binary: release `v0.5.0-devnet2`, `main` `9f04bf2a22c7b0e74ce927d20c91e9d18def6c16`, SHA-256 `318a18b32195913e3b8ef477b4a30aa1043fe31ae6ad75bcf41224fb7c418e89` (the workflow build, not a local one). Previous: `train-a-r8`, built from the then-unmerged FastPay branch.

- `env`, `stage.sh`: stage the release directory, units and the signed deployment manifest. Units and env files are the previous release's with the name changed.
- `roll.sh N...`: one validator at a time; ships the binary, verifies the manifest, restarts, checks the running binary's hash, then requires all six to agree on height, tip and state root.
- `manifest-fields.json`: the signed deployment manifest without key and signature.

Order used: validator-5 first, then 4, 3, 2, 1, 0, with one small transfer after each to make a block. Blocks 2036 to 2041 were made during the roll; all six agreed after each.

Roll back one validator: put back the units saved in `/etc/postfiat/releases/train-a-r8/rollback-units/`, `systemctl daemon-reload`, restart the validator service and then the RPC service. The `train-a-r8` binary starts on data written by this build (checked on a four-validator test chain, not on the fleet).

Not exercised after the roll: a FastPay payment, and the FastPay speed benchmark of `train-a-r8`.

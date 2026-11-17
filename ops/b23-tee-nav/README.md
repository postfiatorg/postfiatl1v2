# B23 daily Nitro NAV

These files are committed but **not installed or enabled**. The first daily
observation is **2026-11-14 at 18:15:00 UTC**, epoch 4. The timer starts preparation
at 17:55 UTC; the measured collector waits for the exact 18:15 observation.
The original 60-second read window, 65-second attestation bound, account,
PCR0/1/2, proof key and epoch schedule remain pinned. A missed day is not
backdated, and an unexpected next epoch requires operator reconciliation.

The script collects through enclave-owned Schwab TLS, verifies the encrypted
archive and native attestation, proves on existing Vast **53579074**, independently
verifies the Groth16 proof, and canaries the next packet against a fresh read-only
live export. It then takes `chain.lock=TEE` and submits only reserve submit,
finalize and route pricing for the already registered profile. It derives the
reserve overlay and composite root again from live state at submission time.
It never rebinds the asset, mints, trades, buys or cashes in. Literal receipts and
six-validator reads are required for completion. A changed state or failed check
stops the run; there is no automatic retry.

## Spend bounds and measured cost

- Hard daily wrapper cap: **$9.00 per attempt** and **$250.00 per UTC calendar
  month**, also stated in the service/timer. Configuration may lower these caps;
  raising them requires a code change.
- One m6i.xlarge collector, one 40-GiB delete-on-termination disk, at most two
  hours, and an independent teardown guard. The reused qualification runner
  retains its legacy $25 ceiling; the daily wrapper additionally checks its
  complete resource/time reservation against the stricter $9 cap before launch.
- Current conservative reservation is **$8.04 per reading**, rounded up from
  $8.033333333333333. This includes the $6 ancillary/prior reserve and a
  conservative $1/hour compute ceiling. Reservations persist even on failure.
  Thirty-one attempts reserve **$249.24**, within the $250 monthly cap.
- Successful Phase-2 reading: **18.6277725 measured machine minutes ×
  $0.192/hour ÷ 60 = $0.0596089 EC2 compute**. Runtime is measured from launch to
  confirmed termination, so it is an upper elapsed estimate, not an AWS invoice.
  Disk/KMS/network charges are additional and covered by the conservative
  reservation. Evidence: `evidence/tee/phase2/opening3-run/final-inventory.json`
  in the coordination workspace.
- Real proof took **135.377 seconds** on the already rented GPU, with no new AWS
  prover instance. The shared bridge lock is respected; no other Vast is used.
- Daily budgets use `evidence/tee/daily/monthly-budget.json`; Phase-2's exhausted
  reservation ledger is neither edited nor reset. No daily spending is authorized
  or triggered by the local `--check` command.

## Local check and exact enable command

Run the local-only schedule check (zero AWS calls and chain writes):

```bash
cd /home/postfiat/repos/postfiatl1v2-b23-tee-nav
python3 ops/b23-tee-nav/daily.py --check --at 2026-11-14T17:55:00Z
```

After the operator chooses to enable daily execution, run exactly:

```bash
install -d -m 700 /home/postfiat/.config/systemd/user
install -m 644 /home/postfiat/repos/postfiatl1v2-b23-tee-nav/ops/b23-tee-nav/b23-tee-nav-daily.service /home/postfiat/.config/systemd/user/
install -m 644 /home/postfiat/repos/postfiatl1v2-b23-tee-nav/ops/b23-tee-nav/b23-tee-nav-daily.timer /home/postfiat/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now b23-tee-nav-daily.timer
```

`Persistent=false` prevents a late startup from inventing a missed observation.
The code rejects dates before 2026-11-14 and prohibits `--at` with `--execute`.
The service has no restart policy. Inspect the journal and
`evidence/tee/daily/YYYYMMDD/` after any failure before further action.

## Pinned deployment inputs

This installation uses the existing NAV worktree at
`/home/postfiat/repos/navstrategies-b23-tee-nav`, fixed collector revision
`f6d6be7f14cbb46452cc44d886f68a2e829cbfb0`, its venv, and the accepted
`evidence/tee/phase2/opening3-run/` manifest/profile. The retained guest is
`evidence/tee/phase2/b23-nav.elf`; both proof and canary binaries are the paths in
`daily.py`. The release-4 signer is checked against its published SHA256 by the
live writer. The newly built EIF must reproduce the registered profile before
KMS secret release; drift causes teardown instead of a profile change.

AWS credentials remain `/home/postfiat/aws.txt`. Schwab credentials remain on
`pfrpc@productionrpc` at `/home/pfrpc/repos/schwab.txt` and
`/home/pfrpc/.config/navstrategies/schwab_tokens.json`. Recovery uses the owner-only
key `/home/postfiat/.local/state/b23-tee-nav/recovery-20261113.pem`. Do not copy or
print these credentials. OAuth rotation is preserved only after authenticated
archive recovery and a compare-and-swap check of the original credential store.
If capture fails after rotation, its retained ciphertext remains available for
owner recovery; the script never blindly overwrites a newer token.

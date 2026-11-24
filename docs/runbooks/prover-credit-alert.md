# Prover Credit Alert

The three rented GPU proving hosts are Vast.ai instances. They stop when the Vast account runs
out of credit. `ops/b24-live/prover_credit.py` reports before that happens, through the same hook
as the daily B24 reading (`B24_READING_NOTIFY`, sent by `ops/b24-live/reading_alert.py`).

Every 15 minutes it reads the account credit (`vastai show user --raw`) and the hourly cost of the
instances (`vastai show instances --raw`: `dph_total` of each running instance, which includes its
storage, plus `storage_total_cost` of each stopped one), and divides one by the other.
It reports once when the hours left fall under 24, and once more, marked URGENT, under 6. A top-up
above 24 hours re-arms both. If the credit cannot be read, it reports that, again every 6 hours
while it stays unreadable, and exits with status 1 until a read succeeds. A reply of an
unexpected shape counts the same: no `credit` field (`balance` is not taken in its place), a
running instance without a positive `dph_total`, or an instance list that is not a list. A report
whose hook fails is retried on the next run.

## Install

On the machine that runs the bridge services, as the user that owns them (`postfiat`), from the
live checkout of `main`:

```bash
cd ~/repos/postfiatl1v2-live
install -m 0644 ops/b24-live/prover-credit-alert.service ops/b24-live/prover-credit-alert-failed.service \
  ops/b24-live/prover-credit-alert.timer ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user start prover-credit-alert.service   # one check now
journalctl --user -u prover-credit-alert.service -n 5
systemctl --user enable --now prover-credit-alert.timer
```

Nothing runs until the timer is installed. A user timer runs only while the user has a session,
unless lingering is on; check once with `loginctl show-user postfiat -p Linger` and, if it says
`no`, ask root for `sudo loginctl enable-linger postfiat`.

A failed run shows in `systemctl --user --failed`. `prover-credit-alert-failed.service` (started by
`OnFailure=`) also reports a run that ended without its own report, such as a crash or a timeout.

The unit needs:

- the Vast CLI at `~/.local/bin/vastai`, the one `ops/sol-live/prover-tunnel.py` uses. The CLI reads
  its own key file (`~/.config/vastai/vast_api_key`). The key is never passed as an argument. The
  script reads that file only to strip the key from what it prints or sends;
- `B24_READING_NOTIFY` in `~/.config/b24-tee-nav/notify.env`, already used by the B24 reading.
  Without it a report that is due is not sent: the run fails with "the alert address is not set:
  nothing was sent", and the report is sent by the first run after the address is set.

## Thresholds

Create `~/.config/prover-credit-alert/thresholds.env`:

```bash
PROVER_CREDIT_WARN_HOURS=24
PROVER_CREDIT_URGENT_HOURS=6
```

The urgent value must be below the warning value. The last reported level is kept in
`~/.local/state/prover-credit-alert/state.json`. Delete that file to report the current level again.

## Reports

```text
Proving hosts stop in about 11.6 h: Vast credit 40.50 USD at 3.50 USD/h for 2 running instance(s), under the 24 h limit. Top up the Vast account (2026-11-23T07:47:50Z)
URGENT: proving hosts stop in about 5.0 h: Vast credit 30.00 USD at 6.00 USD/h for 3 running instance(s), under the 6 h limit. Top up the Vast account (…)
Proving hosts: Vast credit could not be read (vastai user exited 1: …); the reading is not usable and the low-credit alert is blind until it can (…)
```

A stopped instance is billed for its disk until it is destroyed, so it counts with its
`storage_total_cost`; all instances stopped with the credit nearly gone still reports. A stopped
instance without that field makes the reading not usable.
Credit at or below zero counts as zero hours left.

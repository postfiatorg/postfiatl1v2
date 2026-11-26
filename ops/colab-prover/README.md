# Colab proving window

One Google Colab GPU session a day does the proving that two Vast machines did around the clock:
Solana deposits, Solana and Ethereum withdrawals, and the B24 daily proof.

- Window: 17:00 to 19:00 UTC. The session starts at 16:15 (`colab-prover-open.timer`) and stops at
  19:00 (`colab-prover-close.timer`). Outside the window deposits and withdrawals report closed.
- `colab-prover.sh up deposits` gets a machine (G4 first), uploads the prover files, lays them out as
  the Vast provers had them, warms up, and then redirects the services. `down` undoes it.
- The services keep their own commands. While a window is open an ssh_config stanza sends
  `ssh8.vast.ai` to the session over one multiplexed connection (Colab allows one SSH connection per
  runtime). The Solana withdrawal service runs as its own user: `/etc/hosts` points that name at a
  local port that is forwarded to a second SSH daemon on the session, which has a fixed host key and
  admits only that service's key with its forced command (`colab-sol-withdraw.sh`, run once with sudo).
- `colab-prover.sh down` attempts every close action (state files, the connection, the stanza and socket, the
  session stop) and logs "window closed" only when the session is stopped and no longer listed; otherwise it
  logs and alerts once what failed and exits non-zero. Logging is best-effort everywhere: an unwritable
  `window.log` never keeps an action (the session stop included) from running.
- `up`, `down` and `ensure` hold one lock (`window.lock` beside `window.mode`). A second `up` exits at once
  ("nothing changed"); `ensure` skips its run quietly while `up` or `down` holds it; `down` waits for it up to
  150 s (`COLAB_PROVER_LOCK_WAIT`) and then closes anyway. An `up` that finds today's open window (dated record,
  stanza, mode, and the session listed) changes nothing. A session listing that fails or times out
  (`COLAB_PROVER_LIST_SECONDS`, 15 s and a 2 s kill) is never taken as "no session": `up` asks again five
  times and then alerts and changes nothing; `check` alerts. A failed `up` removes only the state files it
  wrote and stops only a session it created.
- `colab-prover-check.timer` (17:05 UTC, not enabled by default) runs `colab-prover.sh check`, which sends
  the B24 alert unless today's open is recorded (`window.opened`, written by a successful `up`), the stanza
  and `window.mode` (`deposits` or `none`) are in place and the session is listed; a listing that fails or
  times out counts as not open, and its 17 s deadline leaves the unit's minute for the alert. Before it asks
  for a machine, `up` removes the state of a window that is not open (a record from another day, an open
  that did not finish, or a session the listing says is gone).
- `colab-prover.sh ensure` (every two minutes) puts back the connection, the withdrawal door and the
  deposit socket, each step whatever the others did and each command bounded with a 2 s kill grace
  (`COLAB_PROVER_STEP_SECONDS`, 15 s, for the connection, the forwards and the tunnel stop;
  `COLAB_PROVER_DOOR_SECONDS`, 30 s, for the door's self-test, an SSH login and two file tests that take a
  few seconds when healthy). With the defaults a run takes at most 125 s (five step commands, the door
  check and the 8 s socket probe), inside the unit's three minutes; larger overrides must keep that sum
  under the unit's limit. A failing step of an open window is counted, never torn down. A run counts as failed
  until it succeeds, so one the unit's limit cuts short is counted too; after three failed runs in a row in
  an open window it sends the B24 alert once, and once more when a run works again
  (`window.ensure-failures` and `window.ensure-alerted` beside `window.mode`).
- A failed open alerts once, also when the failure is inside a command substitution or when logging or
  cleanup itself fails; the handler keeps the original status.
- The prover files are not in this repository. They are kept on the relay host in
  `~/var/colab-prover/bundle` (506 MB); `bundle.SHA256SUMS` lists them. The session's fixed host key
  and the launcher's self-test key are in `~/var/colab-prover/solexit` on that host only.

Measured on 25 and 26 November 2026 (G4: RTX PRO 6000 Blackwell 96 GB, 48 cores):

| Proof, warm | Colab G4 | Vast |
| --- | ---: | ---: |
| Solana deposit | 66 to 73 s | 62 s (4x RTX 5090) |
| Withdrawal | 23 to 41 s | 122 s (1x RTX 5090) |

Warm-up from request to ready was 43 minutes (the circuit files are downloaded each session). The
session uses about 11.2 compute units an hour: about 31 a day for this window.

The circuit files (6.2 GB) are fetched on the session with 16 parallel ranges and checked against a
pinned hash (`fetch-circuits.py`, 3 min 43 s). One connection to that bucket gives about 5 MB/s, and
the prover's own download once took over 40 minutes and failed a window.

Colab's supply varies. On 26 November a G4 came in 5 seconds at 23:40 UTC, a fallback A100 after 12
minutes at 01:13, nothing at all for 43 minutes from 02:26, and a G4 after 3 minutes at 03:13. The
launcher keeps asking, every shape in turn, until 18:30 UTC, and raises the B24 alert if it never
opens. The two stopped Vast provers (`62235352`, `53579074`) are the manual fallback.

The Solana observer moved on 26 November from Vast `61511840` (2x RTX PRO 5000, $2.24/h) to Vast
`62617699` (Threadripper PRO 5975WX, 251 GB, 1.1 TB, $0.71/h). `mainnet-capture.py` and
`mainnet-go.py` name the new host. It has its own node identity and caught up 32 minutes after
starting from an empty disk. A real deposit (`Sm1Mo4BP…`) was captured by it, proved on the session
in 96 s and credited on six validators 151 s after sending.

Not done: the Task Node does not yet show the window's hours; a session that dies inside a window is
not restarted; a real Ethereum withdrawal and a real Solana withdrawal have not been made through it.

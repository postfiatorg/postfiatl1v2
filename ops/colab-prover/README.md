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

Not done: the Task Node does not yet show the window's hours; a session that dies inside a window is
not restarted; a real Ethereum withdrawal and a real Solana withdrawal have not been made through it.

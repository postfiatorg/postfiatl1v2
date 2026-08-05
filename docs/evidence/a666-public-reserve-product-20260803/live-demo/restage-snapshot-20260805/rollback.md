# Git-free service rollback commands

These commands are recorded only. The snapshot did not execute them.

```bash
set -euo pipefail
SNAPSHOT=/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/live-demo/restage-snapshot-20260805

install -D -m 0664 "$SNAPSHOT/unit-files/stakehub-pfusdc-wallet-agent.service" /home/postfiat/.config/systemd/user/stakehub-pfusdc-wallet-agent.service
install -D -m 0644 "$SNAPSHOT/unit-files/stakehub-private-swap-asset-orchard.service" /home/postfiat/.config/systemd/user/stakehub-private-swap-asset-orchard.service
install -D -m 0644 "$SNAPSHOT/unit-files/stakehub-private-swap-dashboard-bfinal.service" /run/user/1000/systemd/transient/stakehub-private-swap-dashboard-bfinal.service
install -D -m 0644 "$SNAPSHOT/unit-files/stakehub-private-swap-dashboard.service" /home/postfiat/.config/systemd/user/stakehub-private-swap-dashboard.service
install -D -m 0644 "$SNAPSHOT/unit-files/stakehub-private-swap-wallet-agent.service" /home/postfiat/.config/systemd/user/stakehub-private-swap-wallet-agent.service
install -D -m 0664 "$SNAPSHOT/unit-files/postfiat-pftl-pnok-prover.service" /home/postfiat/.config/systemd/user/postfiat-pftl-pnok-prover.service
install -D -m 0644 "$SNAPSHOT/unit-files/postfiat-pftl-high-core-prover.service" /run/user/1000/systemd/transient/postfiat-pftl-high-core-prover.service

systemctl --user daemon-reload
systemctl --user restart stakehub-pfusdc-wallet-agent.service stakehub-private-swap-asset-orchard.service stakehub-private-swap-dashboard-bfinal.service stakehub-private-swap-dashboard.service stakehub-private-swap-wallet-agent.service postfiat-pftl-pnok-prover.service
systemctl --user stop postfiat-pftl-high-core-prover.service
```

The high-core prover was inactive in this snapshot, so `stop` restores its
recorded service state after the daemon reload. No git command is required.

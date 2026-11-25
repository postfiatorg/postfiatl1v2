#!/bin/bash
# Installs the faster payout service (second Solana exit release + speed-up). Asks for the sudo password.
sudo /usr/bin/systemctl stop pft-sol-mainnet-withdraw.service
sudo /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root /bin/bash -ceu 'cd /; [[ $1 =~ ^[0-9a-f]{64}$ && $2 == /* && $3 =~ ^[1-9][0-9]{0,17}$ ]]; umask 077; d=$(mktemp -d /var/lib/pf-sol-install.XXXXXXXX); trap '\''rm -rf -- "$d"'\'' EXIT; cd "$d"; timeout --kill-after=5s 120s runuser -u postfiat -- /usr/bin/head -c "$(($3 + 1))" -- "$2" > bundle.tar; [[ $(stat -c %s bundle.tar) == "$3" ]]; printf "%s  bundle.tar\n" "$1" | sha256sum --strict -c -; umask 022; tar -xf bundle.tar --no-same-owner --no-same-permissions; /bin/bash ./install-exit-system.sh' _ 9fac1ea70a29be5aabca92be7d2f7f4cccbf32ad33254eece0d4a47dca1d7613 /home/postfiat/var/sol-exit-r1/sol-exit-install-c033278b.tar 90224640
echo "exit code: $?"

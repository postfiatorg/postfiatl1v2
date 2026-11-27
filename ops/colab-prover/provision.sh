#!/bin/bash
# Runs ON the Colab VM after the bundle is unpacked in /content/bundle. Lays the files out as the
# two Vast provers have them, so the services' existing commands work unchanged.
set -euo pipefail
B=/content/bundle
( cd $B && sha256sum -c --quiet SHA256SUMS )
# CUDA 12 runtime and the driver library, without per-command environment.
rt=$(dirname "$(find /usr/local/lib -name 'libcudart.so.12' | head -1)")
[ -n "$rt" ] || { echo "no libcudart.so.12 on this VM"; exit 1; }
printf '%s\n%s\n' /usr/lib64-nvidia "$rt" > /etc/ld.so.conf.d/colab-prover.conf && ldconfig
# sp1-gpu-server refuses to start without CUDA_VISIBLE_DEVICES; a wrapper sets it.
wrap() { # real-binary  wrapper-path
  mkdir -p "$(dirname "$2")"; install -m755 "$1" "$2.real"
  printf '#!/bin/sh\nexport CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-0}"\nexec "%s.real" "$@"\n' "$2" > "$2"; chmod 755 "$2"
}
# Withdrawals and B24 (SDK 6.3.1), as on Vast 53579074.
wrap $B/sp1-6.3.1/sp1-gpu-server /root/.sp1/bin/sp1-gpu-server
W=/workspace/b23-withdraw-20261116
mkdir -p $W/jobs /workspace/b23-gpu /workspace/b23-tee-nav
install -m755 $B/wd/proof-host $W/proof-host; install -m644 $B/wd/egress.elf $W/egress.elf
: >> /workspace/b23-gpu/prover.lock
# Deposits (SDK 6.10.0), as on Vast 62235352, with a home of their own so the two server versions do not collide.
D=/workspace/sol-ingress/speed-i2; H=/root/dep-home
mkdir -p $D $H/.sp1/bin $B/dep/gs
cp -a $B/dep/host-live $B/dep/elf $B/dep/prime.bin $B/dep/serve.py $D/
tar -C $B/dep/gs -xzf $B/dep/gpu-server.tar.gz
wrap $B/dep/gs/sp1-gpu-server $H/.sp1/bin/sp1-gpu-server
test "$(/root/.sp1/bin/sp1-gpu-server --version)" = 6.3.1
test "$($H/.sp1/bin/sp1-gpu-server --version)" = 6.10.0
# The Solana withdrawal service's restricted door: its own SSH daemon with a fixed host key, admitting
# only that service's key (and this launcher's self-test key), each bound to the forced command.
if [ -d /content/solexit ]; then
  mkdir -p /root/pfx /run/sshd
  install -m600 /content/solexit/hostkey /root/pfx/hostkey; install -m600 /content/solexit/authorized_keys /root/pfx/authorized_keys
  install -m755 /content/solexit/pf-sol-exit-prove /usr/local/bin/pf-sol-exit-prove
  chown -R root:root /root/pfx
  [ -f /root/pfx/sshd.pid ] && kill "$(cat /root/pfx/sshd.pid)" 2>/dev/null || true
  /usr/sbin/sshd -p 2223 -o ListenAddress=127.0.0.1 -h /root/pfx/hostkey -o AuthorizedKeysFile=/root/pfx/authorized_keys \
    -o PasswordAuthentication=no -o PidFile=/root/pfx/sshd.pid -E /root/pfx/sshd.log
fi
# The circuit files, fetched in parallel and checked, so that no proof downloads them.
python3 /content/fetch-circuits.py $B/dep/circuits.sha256
nvidia-smi --query-gpu=name,memory.total --format=csv,noheader
echo PROVISIONED

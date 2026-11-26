#!/bin/bash
# Runs ON a freshly rented Vast machine: sets up and starts the warm Solana deposit prover only.
set -euo pipefail
B=/content/bundle/dep; D=/workspace/sol-ingress/speed-i2; H=/root/dep-home
command -v python3 >/dev/null || { apt-get update -qq && apt-get install -y -qq python3 >/dev/null; }
rt=$(dirname "$(find /usr/local /usr/lib -name 'libcudart.so.12' 2>/dev/null | head -1)")
[ -n "$rt" ] && [ "$rt" != . ] || { echo "no libcudart.so.12 on this machine"; exit 1; }
echo "$rt" > /etc/ld.so.conf.d/pft-prover.conf && ldconfig
( cd $B && sha256sum -c --quiet prover-inputs.sha256 ) || true
mkdir -p $D $H/.sp1/bin $B/gs
cp -a $B/host-live $B/elf $B/prime.bin $B/serve.py $D/
tar -C $B/gs -xzf $B/gpu-server.tar.gz
install -m755 $B/gs/sp1-gpu-server $H/.sp1/bin/sp1-gpu-server.real
printf '#!/bin/sh\nexport CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-0}"\nexec "%s.real" "$@"\n' $H/.sp1/bin/sp1-gpu-server > $H/.sp1/bin/sp1-gpu-server; chmod 755 $H/.sp1/bin/sp1-gpu-server
test "$($H/.sp1/bin/sp1-gpu-server --version)" = 6.10.0
python3 /content/fetch-circuits.py $B/circuits.sha256
ln -sfn /root/.sp1/circuits $H/.sp1/circuits
cd $D; pkill -f "serve.py --lock" 2>/dev/null || true; rm -f prover.sock
HOME=$H RAYON_NUM_THREADS=$(( $(nproc) > 32 ? 32 : $(nproc) )) SP1_WORKER_NUM_CORE_WORKERS=8 SP1_WORKER_CORE_BUFFER_SIZE=8 SP1_WORKER_VERIFY_INTERMEDIATES=false \
  SP1_GROTH16_CIRCUIT_PATH=/root/.sp1/circuits/groth16 nohup setsid python3 serve.py --lock prover.lock --prime prime.bin ./host-live elf $D/prover.sock 1 > serve.log 2>&1 < /dev/null &
sleep 8; until [ -S prover.sock ] || ! pgrep -f "serve.py --lock" >/dev/null; do sleep 3; done
[ -S prover.sock ] && grep -o '"ready_seconds":[0-9.]*' serve.log || { tail -5 serve.log; exit 1; }
nvidia-smi --query-gpu=name,memory.total --format=csv,noheader
echo DEPOSITS-WARM

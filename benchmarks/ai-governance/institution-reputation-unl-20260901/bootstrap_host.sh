#!/bin/bash
# Launch the pinned Qwen3.8 SGLang profile on one H200.
set -euo pipefail
cd /root/institution-reputation

REV=017b9c7af6b5689d5dd426a76e0bc077eb5ca20a
python3 - <<PY
from huggingface_hub import snapshot_download
path = snapshot_download("Qwen/Qwen3.8-27B-FP8", revision="$REV", local_files_only=False)
open("/root/institution-reputation/model_path.txt", "w").write(path)
print("model ready")
PY

nvidia-smi --query-gpu=name,driver_version,uuid --format=csv,noheader > host_identity.txt
python3 --version >> host_identity.txt

nohup python3 -m sglang.launch_server \
  --model-path "$(cat model_path.txt)" \
  --served-model-name Qwen/Qwen3.8-27B-FP8 \
  --host 127.0.0.1 --port 8000 \
  --trust-remote-code --tp 1 \
  --context-length 32768 \
  --mem-fraction-static 0.75 \
  --chunked-prefill-size 4096 \
  --max-running-requests 32 \
  --reasoning-parser qwen3 \
  --enable-deterministic-inference \
  --disable-radix-cache \
  --random-seed 438916795 \
  --enable-metrics \
  --attention-backend triton \
  --linear-attn-backend triton \
  --disable-cuda-graph \
  --disable-overlap-schedule \
  > sglang.log 2>&1 &
echo $! > sglang.pid
for _ in $(seq 1 180); do
  if curl -sf http://127.0.0.1:8000/health >/dev/null 2>&1; then
    echo READY
    exit 0
  fi
  sleep 10
done
echo "server failed to become healthy" >&2
tail -80 sglang.log >&2
exit 1

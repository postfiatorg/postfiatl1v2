#!/bin/sh
set -eu
cd "$(dirname "$0")"
rustup target add x86_64-unknown-linux-musl
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
  cargo build --locked --release --target x86_64-unknown-linux-musl --bin tasknode-l1-validator-signer
python3 - <<'PY'
import hashlib, pathlib, re
binary=pathlib.Path('target/x86_64-unknown-linux-musl/release/tasknode-l1-validator-signer')
digest=hashlib.sha256(binary.read_bytes()).hexdigest()
script=pathlib.Path('l1-validator.sh')
script.write_text(re.sub(r"HELPER_SHA256 = '[^']+'", "HELPER_SHA256 = '"+digest+"'", script.read_text()))
print(digest, binary)
PY

#!/bin/bash
set -eu
cd "$(dirname "$0")/../../tools/pfusdc-sol-capture"
export CARGO_TARGET_DIR="$PWD/../../target/task-l"
export LIBCLANG_PATH="$(dirname "$(find /home/postfiat/.local/share/libclang-18 -name libclang.so -print -quit)")"
export LD_LIBRARY_PATH="$LIBCLANG_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export BINDGEN_EXTRA_CLANG_ARGS="-I$(gcc -print-file-name=include)"
export ROCKSDB_LIB_DIR=/home/postfiat/repos/postfiatl1v2-sol-ingress/tools/pfusdc-sol-ingress-difftest/target/release/build/librocksdb-sys-682181348ecba9d7/out
export ROCKSDB_STATIC=1
cargo +1.99.1 rustc --release -j 8 "$@" -- -C link-arg=-lstdc++

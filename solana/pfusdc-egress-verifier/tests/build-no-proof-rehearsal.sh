#!/usr/bin/env bash
# Build the rehearsal-only verifier image whose Groth16 check accepts any proof, for the
# "test verifier" cases of upgrade_rehearsal.py. NEVER deployable: it is written under a
# fresh scratch directory with a name no deploy script uses, and any output directory inside
# the repository or a deploy path is refused.
#
# Usage: build-no-proof-rehearsal.sh <commit> [output directory outside the repository]
set -euo pipefail
repo="$(git rev-parse --show-toplevel)"
platform="${SBF_PLATFORM_TOOLS:?point SBF_PLATFORM_TOOLS at Anza platform-tools v1.58}"
ref="$(git -C "$repo" rev-parse "${1:?commit}^{commit}")"
out="$(realpath -m "${2:-$(mktemp -d -t no-proof-rehearsal.XXXXXX)}")"
refuse() { echo "refused: $out is $1; the no-proof image must never reach a deploy path" >&2; exit 1; }
case "$out/" in
    "$repo"/*) refuse "inside the repository" ;;
    /opt/postfiat/*|/etc/postfiat/*|/var/lib/postfiat/*) refuse "a host deploy path" ;;
esac
for deploy in "${SOLANA_BRIDGE_IMAGES:-}" "${KIT:-}" "${PAUSE_CHECKOUT:-}"; do
    if [[ -n "$deploy" && "$out/" == "$(realpath -m "$deploy")"/* ]]; then
        refuse "under a deploy directory ($deploy)"
    fi
done
mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git -C "$repo" archive "$ref" solana/pfusdc-egress-verifier | tar -x -C "$work"
groth16="$work/solana/pfusdc-egress-verifier/src/groth16.rs"
sed -i 's/^pub fn verify(values: &\[u8\], proof: &\[u8\]) -> ProgramResult {$/&\n    if true { let _ = (values, proof); return Ok(()); }/' "$groth16"
grep -q 'if true { let _ = (values, proof); return Ok(()); }' "$groth16"
PATH="$platform/rust/bin:$PATH" RUSTC="$platform/rust/bin/rustc" CARGO_TARGET_DIR="$work/target" \
    "$platform/rust/bin/cargo" build --locked --release --target sbpfv3-solana-solana \
    --manifest-path "$work/solana/pfusdc-egress-verifier/Cargo.toml"
image="$out/pfusdc_egress_verifier_NO_PROOF_REHEARSAL_ONLY.so"
"$platform/llvm/bin/llvm-objcopy" --strip-all \
    "$work/target/sbpfv3-solana-solana/release/pfusdc_egress_verifier.so" "$image"
echo "source=$ref image=$image sha256=$(sha256sum "$image" | cut -d' ' -f1)"

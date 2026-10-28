#!/usr/bin/env bash
# CLI-level test of the deploy script: runs `forge script` (not `forge test`,
# which does not apply forge's script-only rules such as the address(this)
# check) on forge's in-process chain with a real Safe treasury, then against a
# local anvil (started here, on a random local port, killed on exit) where a
# real factory creates the Safes whose creation transactions the script reads.
# No public RPC, no key, no broadcast. Isolated: forge runs under `env -i` with FOUNDRY_OFFLINE=true,
# no RPC variable, FOUNDRY_CONFIG unset and a temporary empty HOME (removed on
# exit), so no user environment, no ~/.foundry/foundry.toml and no RPC alias of
# the user reaches it; its output is echoed only through a URL redactor. Not
# isolated, on purpose: the project's own foundry.toml (compiler settings,
# fs_permissions), the forge binary on PATH and the installed solc 0.8.24, which
# is passed with --use because the empty HOME has no svm directory.
set -euo pipefail
cd "$(dirname "$0")/.."

RPC_VARS=(FOUNDRY_ETH_RPC_URL DAPP_ETH_RPC_URL ETH_RPC_URL FOUNDRY_FORK_URL FOUNDRY_CONFIG)
for v in "${RPC_VARS[@]}"; do unset "$v" 2>/dev/null || true; done
for v in "${RPC_VARS[@]}"; do
  if [ -n "${!v+x}" ]; then echo "FAIL: $v could not be unset (readonly?); refusing to run forge" >&2; exit 1; fi
done

FORGE="$(command -v forge)" || { echo "FAIL: forge not on PATH" >&2; exit 1; }
SAFE_PATH="$(dirname "$FORGE"):/usr/bin:/bin"
SOLC=""
for d in "${XDG_DATA_HOME:-$HOME/.local/share}/svm" "$HOME/.svm"; do
  if [ -x "$d/0.8.24/solc-0.8.24" ]; then SOLC="$d/0.8.24/solc-0.8.24"; break; fi
done
[ -n "$SOLC" ] || { echo "FAIL: solc 0.8.24 is not installed (forge build once, online)" >&2; exit 1; }
TMP_HOME="$(mktemp -d)"
trap 'rm -rf "$TMP_HOME"' EXIT

redact() {
  sed -E -e 's#[A-Za-z][A-Za-z0-9+.-]*://[^[:space:]"'"'"'<>]+#<redacted-url>#g' \
    -e 's#\b(localhost|[0-9]{1,3}(\.[0-9]{1,3}){3})(:[0-9]+)?(/[^[:space:]]*)?#<redacted-url>#g'
}

# isolated FORGE_ARGS...: forge with only PATH, the temporary HOME and FOUNDRY_OFFLINE.
isolated() {
  env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" FOUNDRY_OFFLINE=true "$@" 2>&1 | redact
  return "${PIPESTATUS[0]}"
}

# run [VAR=value...]: the script on the in-process chain with only these variables.
run() {
  env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" FOUNDRY_OFFLINE=true \
    PFT_TOKEN_CHAIN_ID=84532 PFT_TOKEN_NAME="Post Fiat Token" PFT_TOKEN_SYMBOL=PFT \
    PFT_TOKEN_TOTAL_SUPPLY=1000000000000 PFT_TOKEN_TOTAL_SUPPLY_PFT=1000000 \
    PFT_TOKEN_TREASURY=0x0000000000000000000000000000000000005AFE \
    PFT_TOKEN_ALLOW_USED_DEPLOYER=1 PFT_TOKEN_ALLOW_EOA_TREASURY=1 "$@" \
    "$FORGE" script test/DeployPFTTokenCli.s.sol:DeployPFTTokenWithFakeSafe --chain-id 84532 --use "$SOLC" 2>&1 | redact
  return "${PIPESTATUS[0]}"
}
# The in-process chain starts the sender at nonce 1, hence PFT_TOKEN_ALLOW_USED_DEPLOYER;
# it has no RPC for the creation transaction, hence the test mode (the Safe checks still run).

out="$(run)" || { printf '%s\n' "$out"; echo "FAIL: the script refused a real Safe" >&2; exit 1; }
grep -q "PFTToken deployed at" <<<"$out"
grep -q "treasury Safe threshold 2" <<<"$out"
grep -q "treasury Safe singleton 0x29fcB43b46531BcA003ddC8FCB67FFE91900C762" <<<"$out"
echo "ok: real Safe proxy of the official singleton accepted"

if out="$(run PFT_TOKEN_TEST_SINGLETON=0x0000000000000000000000000000000000000BAD)"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a wrong singleton" >&2; exit 1
fi
grep -q "UnofficialSafeSingleton" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: Safe proxy with a wrong singleton refused"

if out="$(run PFT_TOKEN_TEST_LOOKALIKE=true)"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a lookalike" >&2; exit 1
fi
grep -q "ProxyCodeMismatch" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: lookalike with an allowed singleton in slot 0 refused"

if out="$(run PFT_TOKEN_TEST_MODULE=0x000000000000000000000000000000000000D0D0)"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a Safe with a module" >&2; exit 1
fi
grep -q "ModulesEnabled(1)" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: real Safe with an enabled module refused"

# Another build (other settings, other output directory) is not the token the
# script deploys. forge itself also follows FOUNDRY_OUT, so the fixture sets it
# for the script only (PFT_TOKEN_TEST_FOUNDRY_OUT) after forge compiled into out/.
OTHER_OUT=cache/deploy-test/out-runs-201
rm -rf "$OTHER_OUT" cache/deploy-test/cache-runs-201
out="$(isolated "$FORGE" build --use "$SOLC" --optimizer-runs 201 --out "$OTHER_OUT" \
  --cache-path cache/deploy-test/cache-runs-201 --force)" || { printf '%s\n' "$out"; exit 1; }
if out="$(run PFT_TOKEN_TEST_FOUNDRY_OUT="$OTHER_OUT")"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted the artifact of another build" >&2; exit 1
fi
grep -q "ArtifactMismatch" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: artifact of another build refused"

# A build whose metadata is another build's: this build's code with the rawMetadata
# of the --optimizer-runs 201 build above.
SPLICED_OUT=cache/deploy-test/out-spliced
rm -rf "$SPLICED_OUT" && mkdir -p "$SPLICED_OUT/PFTToken.sol"
python3 - "$OTHER_OUT/PFTToken.sol/PFTToken.json" "$SPLICED_OUT/PFTToken.sol/PFTToken.json" <<'PY'
import json, sys
artifact = json.load(open("out/PFTToken.sol/PFTToken.json"))
artifact["rawMetadata"] = json.load(open(sys.argv[1]))["rawMetadata"]
json.dump(artifact, open(sys.argv[2], "w"))
PY
if out="$(run PFT_TOKEN_TEST_FOUNDRY_OUT="$SPLICED_OUT")"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a spliced artifact" >&2; exit 1
fi
grep -q "ArtifactMetadataMismatch" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: spliced artifact (this code, another build's metadata) refused"

# This build's artifact with its runtime cut to its last 1,000 bytes (trailer
# kept, still the end of the creation code): refused before the broadcast.
TRUNCATED_OUT=cache/deploy-test/out-truncated
rm -rf "$TRUNCATED_OUT" && mkdir -p "$TRUNCATED_OUT/PFTToken.sol"
python3 - "$TRUNCATED_OUT/PFTToken.sol/PFTToken.json" <<'PY'
import json, sys
artifact = json.load(open("out/PFTToken.sol/PFTToken.json"))
runtime = artifact["deployedBytecode"]["object"]
assert runtime.startswith("0x") and len(runtime) > 2 + 2000
artifact["deployedBytecode"]["object"] = "0x" + runtime[-2000:]
json.dump(artifact, open(sys.argv[1], "w"))
PY
if out="$(run PFT_TOKEN_TEST_FOUNDRY_OUT="$TRUNCATED_OUT")"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a truncated runtime" >&2; exit 1
fi
grep -q "ArtifactMetadataMismatch" <<<"$out" && ! grep -qE "DeployedCodeMismatch|PFTToken deployed at" <<<"$out" \
  || { printf '%s\n' "$out"; exit 1; }
echo "ok: truncated runtime (last 1,000 bytes) refused before the broadcast"

# ---- a local anvil: the creation transaction of a real Safe, read from the chain.
# anvil and cast run under the same env -i, temporary HOME and URL redaction.
CAST="$(dirname "$FORGE")/cast"
ANVIL="$(dirname "$FORGE")/anvil"
PORT=$((20000 + RANDOM % 20000))
RPC="http://127.0.0.1:$PORT"
env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$ANVIL" --chain-id 84532 --port "$PORT" --silent &
ANVIL_PID=$!
trap 'kill "$ANVIL_PID" 2>/dev/null; rm -rf "$TMP_HOME"' EXIT
c() { env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" "$@" --rpc-url "$RPC"; }
for _ in $(seq 50); do c chain-id >/dev/null 2>&1 && break; sleep 0.1; done
FACTORY=0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67
SAFE_L2=0x29fcB43b46531BcA003ddC8FCB67FFE91900C762
c rpc anvil_setCode "$FACTORY" "$(cat test/fixtures/safe/safe-proxy-factory-1.4.1.runtime.hex)" >/dev/null
c rpc anvil_setCode "$SAFE_L2" "$(cat test/fixtures/safe/safe-l2-1.4.1.runtime.hex)" >/dev/null
# anvil's first dev account, unlocked by anvil itself (no key in this file).
FUNDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
DEPLOYER=0x00000000000000000000000000000000DE910E00
c rpc anvil_setBalance "$DEPLOYER" 0x56BC75E2D63100000 >/dev/null
OWNERS="[0x00000000000000000000000000000000000000A1,0x00000000000000000000000000000000000000A2,0x00000000000000000000000000000000000000A3]"
ZERO=0x0000000000000000000000000000000000000000
SIG_SETUP="setup(address[],uint256,address,bytes,address,address,uint256,address)"
SIG_CREATE="createProxyWithNonce(address,bytes,uint256)"

# create_safe SETUP_TO SALT: prints "<proxy> <tx hash>" of a 2-of-3 SafeL2 made by the factory.
create_safe() {
  local init proxy hash
  init="$(env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" calldata "$SIG_SETUP" "$OWNERS" 2 "$1" 0x "$ZERO" "$ZERO" 0 "$ZERO")"
  proxy="$(c call "$FACTORY" "$SIG_CREATE(address)" "$SAFE_L2" "$init" "$2")"
  hash="$(c send --unlocked --from "$FUNDER" "$FACTORY" "$SIG_CREATE" "$SAFE_L2" "$init" "$2" --json \
    | sed -E 's/.*"transactionHash":"(0x[0-9a-f]{64})".*/\1/')"
  [ -n "$proxy" ] && [ "${#hash}" -eq 66 ] || { echo "FAIL: could not create a Safe on the anvil" >&2; return 1; }
  echo "$proxy $hash"
}

# run_anvil CHAIN_ID TREASURY [VAR=value...]: the real script against the anvil,
# with the extra forge arguments in FORGE_ARGS.
FORGE_ARGS=()
run_anvil() {
  local chain="$1" treasury="$2"; shift 2
  env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" FOUNDRY_OFFLINE=true \
    PFT_TOKEN_CHAIN_ID="$chain" PFT_TOKEN_NAME="Post Fiat Token" PFT_TOKEN_SYMBOL=PFT \
    PFT_TOKEN_TOTAL_SUPPLY=1000000000000 PFT_TOKEN_TOTAL_SUPPLY_PFT=1000000 \
    PFT_TOKEN_TREASURY="$treasury" "$@" \
    "$FORGE" script script/DeployPFTToken.s.sol:DeployPFTToken --rpc-url "$RPC" --sender "$DEPLOYER" \
    --use "$SOLC" ${FORGE_ARGS[@]+"${FORGE_ARGS[@]}"} 2>&1 | redact
  return "${PIPESTATUS[0]}"
}

read -r SAFE SAFE_TX <<<"$(create_safe "$ZERO" 0)"
SAFE_RAW="$(c rpc eth_getRawTransactionByHash "$SAFE_TX" | tr -d '"')"
out="$(run_anvil 84532 "$SAFE" PFT_TOKEN_TREASURY_CREATION_TX="$SAFE_TX" PFT_TOKEN_TREASURY_CREATION_RAW_TX="$SAFE_RAW")" \
  || { printf '%s\n' "$out"; echo "FAIL: the script refused a Safe from the factory (raw bytes given)" >&2; exit 1; }
grep -q "treasury Safe factory $FACTORY" <<<"$out" && grep -q "treasury Safe creation tx $SAFE_TX" <<<"$out" \
  || { printf '%s\n' "$out"; exit 1; }
echo "ok: anvil Safe from the canonical factory accepted (PFT_TOKEN_TREASURY_CREATION_RAW_TX)"
out="$(run_anvil 84532 "$SAFE" PFT_TOKEN_TREASURY_CREATION_TX="$SAFE_TX")" \
  || { printf '%s\n' "$out"; echo "FAIL: the script refused a Safe from the factory (eth_getRawTransactionByHash)" >&2; exit 1; }
grep -q "treasury Safe creation tx $SAFE_TX" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: anvil Safe from the canonical factory accepted (eth_getRawTransactionByHash)"

# setup's `to` is delegatecalled with `data`; here the SafeL2 singleton with no data.
read -r BAD BAD_TX <<<"$(create_safe "$SAFE_L2" 1)"
if out="$(run_anvil 84532 "$BAD" PFT_TOKEN_TREASURY_CREATION_TX="$BAD_TX")"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a Safe with a delegating initializer" >&2; exit 1
fi
grep -q "TreasuryInitializerDelegates" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: anvil Safe with a delegating initializer refused"

# The same Safe (a CREATE2 address is the same on every chain) checked on 8453
# with its 84532 creation transaction: the signature binds it to 84532.
c rpc anvil_setChainId 8453 >/dev/null
if out="$(run_anvil 8453 "$SAFE" PFT_TOKEN_TREASURY_CREATION_TX="$SAFE_TX" PFT_TOKEN_TREASURY_CREATION_RAW_TX="$SAFE_RAW")"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a creation transaction signed for another chain" >&2; exit 1
fi
grep -q "TreasuryCreationWrongChain(84532" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: 84532 creation transaction refused on 8453"

# A real deployment of a --no-metadata build, on 8453 (no record yet; 84532's
# would stop the run at RecordExists first) with a Safe created there; forge
# builds into its own FOUNDRY_OUT, which the script reads.
read -r SAFE SAFE_TX <<<"$(create_safe "$ZERO" 2)"
NOMETA_OUT=cache/deploy-test/out-no-metadata
rm -rf "$NOMETA_OUT" cache/deploy-test/cache-no-metadata
FORGE_ARGS=(--no-metadata --cache-path cache/deploy-test/cache-no-metadata)
if out="$(run_anvil 8453 "$SAFE" PFT_TOKEN_TREASURY_CREATION_TX="$SAFE_TX" PFT_TOKEN_REAL_DEPLOYMENT=1 \
  PFT_TOKEN_SOURCE_COMMIT=0123456789abcdef0123456789abcdef01234567 FOUNDRY_OUT="$NOMETA_OUT")"; then
  printf '%s\n' "$out"; echo "FAIL: the script accepted a --no-metadata build for a real deployment" >&2; exit 1
fi
grep -q "MetadataHashDisabled" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: --no-metadata build refused for a real deployment"

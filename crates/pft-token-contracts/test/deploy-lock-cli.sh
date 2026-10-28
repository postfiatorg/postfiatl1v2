#!/usr/bin/env bash
# CLI-level test of the lock deploy script: a preflight of the OpenZeppelin
# submodule, then `forge script` against a local anvil (chain id 84532, a
# random local port, killed on exit) holding the recorded Base Sepolia test
# token, rebuilt at its recorded address by its recorded deployer, another
# token and a stand-in verifier. No public RPC, no key (anvil's unlocked
# accounts only). Isolated as test/deploy-script-cli.sh: forge, cast and anvil
# run under `env -i` with FOUNDRY_OFFLINE=true, no RPC variable, FOUNDRY_CONFIG
# unset and a temporary empty HOME (removed on exit); output goes through a URL
# redactor. Not isolated, on purpose: the project's foundry.toml, the forge
# binary on PATH and the installed solc 0.8.24 (passed with --use).
set -euo pipefail
cd "$(dirname "$0")/.."

# ---- preflight: the submodule is the pinned commit, its nested submodules uninitialised.
EXPECTED_OZ="$(sed -nE 's/.*OPENZEPPELIN_COMMIT = "([0-9a-f]{40})".*/\1/p' script/DeployPFTLock.s.sol)"
ACTUAL_OZ="$(git -C lib/openzeppelin-contracts rev-parse HEAD)"
if [ -z "$EXPECTED_OZ" ] || [ "$ACTUAL_OZ" != "$EXPECTED_OZ" ]; then
  echo "FAIL: lib/openzeppelin-contracts is at $ACTUAL_OZ, OPENZEPPELIN_COMMIT is ${EXPECTED_OZ:-unreadable}" >&2
  exit 1
fi
if git -C lib/openzeppelin-contracts submodule status | grep -qv '^-'; then
  echo "FAIL: OpenZeppelin's nested submodules are initialised (init it without --recursive)" >&2
  exit 1
fi
echo "ok: preflight lib/openzeppelin-contracts at OPENZEPPELIN_COMMIT $EXPECTED_OZ"

RPC_VARS=(FOUNDRY_ETH_RPC_URL DAPP_ETH_RPC_URL ETH_RPC_URL FOUNDRY_FORK_URL FOUNDRY_CONFIG)
for v in "${RPC_VARS[@]}"; do unset "$v" 2>/dev/null || true; done
for v in "${RPC_VARS[@]}"; do
  if [ -n "${!v+x}" ]; then echo "FAIL: $v could not be unset (readonly?); refusing to run forge" >&2; exit 1; fi
done

FORGE="$(command -v forge)" || { echo "FAIL: forge not on PATH" >&2; exit 1; }
CAST="$(dirname "$FORGE")/cast"
ANVIL="$(dirname "$FORGE")/anvil"
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

out="$(env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" FOUNDRY_OFFLINE=true "$FORGE" build --use "$SOLC" 2>&1 | redact)" \
  || { printf '%s\n' "$out"; exit 1; }

PORT=$((20000 + RANDOM % 20000))
RPC="http://127.0.0.1:$PORT"
env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$ANVIL" --chain-id 84532 --port "$PORT" --silent &
ANVIL_PID=$!
trap 'kill "$ANVIL_PID" 2>/dev/null; rm -rf "$TMP_HOME"' EXIT
c() { env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" "$@" --rpc-url "$RPC"; }
for _ in $(seq 50); do c chain-id >/dev/null 2>&1 && break; sleep 0.1; done
artifact() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]]["object"])' "$@"; }
codehash() { env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" keccak "$(c code "$1")"; }

# The recorded test token: its recorded deployer at nonce 0 with the recorded
# constructor arguments (DeployPFTLock.t.sol's testDryRunAgainstTheRecordedBaseSepoliaTestToken).
TEST_TOKEN=0x18fE4ca51A0cE042688D5A54917319d03B527D92
TEST_TOKEN_HASH=0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df
TEST_TOKEN_DEPLOYER=0xDEe78dB2bC6f5EFCcAB7A046997E5c413C26316D
ARGS="$(env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" abi-encode "constructor(string,string,uint256,address)" \
  "Post Fiat Token" PFT 100000000000000000 0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB)"
TOKEN_INIT="$(artifact out/PFTToken.sol/PFTToken.json bytecode)${ARGS#0x}"
# create FROM: deploys PFTToken from an unlocked FROM and prints its address.
create() {
  env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" send --rpc-url "$RPC" --unlocked --from "$1" --json \
    --create "$TOKEN_INIT" \
    | sed -E 's/.*"contractAddress":"(0x[0-9a-fA-F]{40})".*/\1/'
}
c rpc anvil_impersonateAccount "$TEST_TOKEN_DEPLOYER" >/dev/null
c rpc anvil_setBalance "$TEST_TOKEN_DEPLOYER" 0x56BC75E2D63100000 >/dev/null
made="$(create "$TEST_TOKEN_DEPLOYER")"
[ "$(tr 'A-F' 'a-f' <<<"$made")" = "$(tr 'A-F' 'a-f' <<<"$TEST_TOKEN")" ] && [ "$(codehash "$TEST_TOKEN")" = "$TEST_TOKEN_HASH" ] \
  || { echo "FAIL: the recorded test token was not rebuilt at its address with its code hash" >&2; exit 1; }
# Another PFTToken, from anvil's first dev account (unlocked by anvil itself).
OTHER_TOKEN="$(create 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266)"
OTHER_HASH="$(codehash "$OTHER_TOKEN")"
# The stand-in verifier: the tests' MockLockVerifier, whose code hash is not pinned.
VERIFIER=0x0000000000000000000000000000000000005715
c rpc anvil_setCode "$VERIFIER" "$(artifact out/DeployPFTLock.t.sol/MockLockVerifier.json deployedBytecode)" >/dev/null
UNPINNED=0x00000000000000000000000000000000000000000000000000000000000000ff
# anvil's second dev account, at nonce 0, unlocked by anvil itself.
DEPLOYER=0x70997970C51812dc3A010C7d01b50e0d17dc79C8

# run_lock [VAR=value...]: the script against the anvil; extra forge arguments in FORGE_ARGS.
FORGE_ARGS=()
run_lock() {
  env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" FOUNDRY_OFFLINE=true \
    PFT_LOCK_CHAIN_ID=84532 PFT_LOCK_TOKEN="$TEST_TOKEN" PFT_LOCK_TOKEN_CODEHASH="$TEST_TOKEN_HASH" \
    PFT_LOCK_VERIFIER="$VERIFIER" PFT_LOCK_VERIFIER_CODEHASH="$UNPINNED" \
    PFT_LOCK_MAX_LOCK_AMOUNT=2000000000000000 PFT_LOCK_MAX_LOCK_AMOUNT_PFT=2000000000 "$@" \
    "$FORGE" script script/DeployPFTLock.s.sol:DeployPFTLock --rpc-url "$RPC" --sender "$DEPLOYER" --unlocked \
    --use "$SOLC" ${FORGE_ARGS[@]+"${FORGE_ARGS[@]}"} 2>&1 | redact
  return "${PIPESTATUS[0]}"
}

if out="$(run_lock)"; then
  printf '%s\n' "$out"; echo "FAIL: the dry run accepted an unpinned verifier without the switch" >&2; exit 1
fi
grep -q "VerifierCodeHashMismatch" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: dry run refuses an unpinned verifier without PFT_LOCK_ALLOW_TEST_VERIFIER"

if out="$(run_lock PFT_LOCK_ALLOW_TEST_VERIFIER=1 PFT_LOCK_REAL_DEPLOYMENT=1 \
  PFT_LOCK_SOURCE_COMMIT=0123456789abcdef0123456789abcdef01234567)"; then
  printf '%s\n' "$out"; echo "FAIL: the dry run accepted the verifier switch with the real flag" >&2; exit 1
fi
grep -q 'RealDeploymentWithTestSwitch("PFT_LOCK_ALLOW_TEST_VERIFIER")' <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: dry run refuses PFT_LOCK_ALLOW_TEST_VERIFIER with PFT_LOCK_REAL_DEPLOYMENT=1"

if out="$(run_lock PFT_LOCK_ALLOW_TEST_VERIFIER=1 PFT_LOCK_TOKEN="$OTHER_TOKEN" PFT_LOCK_TOKEN_CODEHASH="$OTHER_HASH")"; then
  printf '%s\n' "$out"; echo "FAIL: the dry run accepted another token without PFT_LOCK_ALLOW_TEST_TOKEN" >&2; exit 1
fi
grep -q "TokenNotTheTestToken" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: dry run refuses a token other than the recorded test token without PFT_LOCK_ALLOW_TEST_TOKEN"

out="$(run_lock PFT_LOCK_ALLOW_TEST_VERIFIER=1 PFT_LOCK_ALLOW_TEST_TOKEN=1 PFT_LOCK_TOKEN="$OTHER_TOKEN" \
  PFT_LOCK_TOKEN_CODEHASH="$OTHER_HASH")" || { printf '%s\n' "$out"; echo "FAIL: the test-token switch" >&2; exit 1; }
grep -q "PFTLockV1 deployed at" <<<"$out" || { printf '%s\n' "$out"; exit 1; }
echo "ok: dry run admits the other token with PFT_LOCK_ALLOW_TEST_TOKEN=1"

RECORD=cache/deploy-test/pft-lock-base-84532/deployment.json
rm -f "$RECORD"
FORGE_ARGS=(--broadcast)
out="$(run_lock PFT_LOCK_ALLOW_TEST_VERIFIER=1)" \
  || { printf '%s\n' "$out"; echo "FAIL: the broadcast with the switch and a stand-in verifier" >&2; exit 1; }
grep -q "WARNING: test-only PFT_LOCK_ALLOW_TEST_VERIFIER" <<<"$out" && [ -f "$RECORD" ] \
  || { printf '%s\n' "$out"; echo "FAIL: no test record" >&2; exit 1; }
echo "ok: broadcast with PFT_LOCK_ALLOW_TEST_VERIFIER and a stand-in verifier deploys and writes the test record"

field() { python3 -c 'import json,sys; v=json.load(open(sys.argv[1]))[sys.argv[2]]; print(str(v).lower() if isinstance(v,bool) else v)' "$RECORD" "$1"; }
LOCK="$(field lock)"
RUNTIME_HASH="$(env -i PATH="$SAFE_PATH" HOME="$TMP_HOME" "$CAST" keccak "$(artifact out/PFTLockV1.sol/PFTLockV1.json deployedBytecode)")"
[ "$(field mode)" = test ] && [ "$(field token)" = "$TEST_TOKEN" ] \
  && [ "$(field solc_version)" = "0.8.24+commit.e11b9ed9" ] && [ "$(field optimizer)" = true ] \
  && [ "$(field optimizer_runs)" = 200 ] && [ "$(field evm_version)" = cancun ] && [ "$(field bytecode_hash)" = ipfs ] \
  && [ "$(field runtime_code_keccak256)" = "$RUNTIME_HASH" ] \
  && [ "$(field deployed_code_keccak256)" = "$(codehash "$LOCK")" ] \
  || { cat "$RECORD"; echo "FAIL: the record's compiler settings or code hashes" >&2; exit 1; }
echo "ok: record holds solc 0.8.24, optimizer true, 200 runs, cancun, ipfs and both code hashes"

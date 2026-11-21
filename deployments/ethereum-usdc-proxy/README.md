# Switch the Ethereum USDC wallet proxy to public main

These steps are for the bridge operator after merge, during an approved maintenance window. They keep the current tmux supervision, loopback port, job store, locks, token-file reference and RPC configuration. There is no proxy systemd unit to install or edit. The GPU and validator services have separate owners.

The reviewed Ethereum chain-1 route in [profile.json](profile.json) uses epoch 11 on `postfiat-wan-devnet-2`. The GPU stages confirm the deposit, wait for Ethereum finality, capture the state proof, prove, verify, grow the backed cap and claim. The live configuration gives proving 90,000,000 ms and the other stages 2,200,000 ms each. Driver readiness has 120,000 ms. The outer worker has 172,800,000 ms and its readiness probe has 130,000 ms.

1. **Stage the merged release.** Run as `postfiat` on the proxy host, in Bash. Confirm the chosen public commit contains this deployment directory and verifier source. Record the commit. Use a new release directory with an owner-controlled parent.

   ```bash
   set -euo pipefail
   umask 077
   url=https://github.com/postfiatorg/postfiatl1v2.git
   sha=$(git ls-remote "$url" refs/heads/main | awk '{print $1}')
   base=/home/postfiat/var/ethereum-proxy-releases
   rel=$base/$sha
   src=$rel/src
   cfg=$rel/config
   g=/home/postfiat/repos/b23-coord/evidence/ingress/g2-service
   node=/home/postfiat/.nvm/versions/node/v24.22.1/bin/node
   export PATH="$(dirname "$node"):$PATH"
   install -d -m 700 "$base"
   mkdir -m 700 "$rel"
   git clone --branch main --single-branch "$url" "$src"
   test "$(git -C "$src" rev-parse HEAD)" = "$sha"
   (cd "$src/wallet-proxy" && npm ci --omit=dev --ignore-scripts)
   (cd "$src" && CARGO_TARGET_DIR="$rel/target" cargo build --locked --release -p pftl-ingress-groth16-verify)
   verify=$rel/target/release/pftl-ingress-groth16-verify
   ```

2. **Check the binaries and assemble the descriptors offline.** The service user needs the existing Python 3.12 environment with Web3. Keep the absolute checkout and config paths stable. The preparer validates the old descriptors, moves the code paths, preserves the existing stage and guest hashes, and pins the reviewed adapter, rebuilt verifier and resulting driver config. It creates a new directory. A failed preparation leaves an unpublished directory for inspection.

   ```bash
   /usr/bin/python3.12 -c 'import web3'
   "$node" "$src/deployments/ethereum-usdc-proxy/prepare.js" "$g" "$cfg" "$verify"
   "$node" "$src/wallet-proxy/eth-fast-lane-driver.js" validate-config --config "$cfg/driver.json" |
     jq -e '.ok and (.stages | length == 7)' >/dev/null
   q=$g/gpu-qualification
   vkey=$(jq -er '.[0].program_vkey' "$cfg/routes.json")
   "$verify" "$q/proof-calldata.bin" "$q/proof-calldata.bin.public-values.bin" "$vkey" |
     jq -e --arg v "$vkey" '.verified and .program_vkey == $v and .proof_bytes == 356' >/dev/null
   ```

   For the November 21 release, use the following overrides. Verify the staged node against the release manifest. Obtain confirmation from the fleet operator that the same node and topology are installed at the named paths on the relay and all possible proposers. If the fleet release has since changed, qualify its CLI and update these pins first. The local binary is for inspection. The bridge stage runs the relay CLI on the validator hosts.

   ```bash
   r=main-r64-20261121
   digest=95d5b237c991f57a9ea122e110f6a2d0c017bf9ca0a85315b2359ecba3b017d1
   test "$(sha256sum "/home/postfiat/var/$r/bin/postfiat-node" | cut -d' ' -f1)" = "$digest"
   man=/home/postfiat/var/$r/stage/rootfs/etc/postfiat/releases/$r/deployment-manifest.json
   jq -e --arg h "$digest" '.chain_id == "postfiat-wan-devnet-2" and .binary_sha256 == $h and .git_revision == "674dbb18a29931d9ae749d28270e3c6cc508795a"' "$man" >/dev/null
   jq -n --arg routes "$cfg/routes.json" --arg verifier "$verify" \
     --arg vh "$(sha256sum "$verify" | cut -d' ' -f1)" --arg r "$r" --arg h "$digest" \
     '{TRUSTLESS_BRIDGE_ROUTES_JSON_FILE:$routes,B23_INGRESS_NODE_VERIFY_BIN:$verifier,
       B23_INGRESS_NODE_VERIFY_SHA256:$vh,PFTL_NODE_BIN:("/opt/postfiat/releases/"+$r+"/postfiat-node"),
       PFTL_TOPOLOGY:("/etc/postfiat/releases/"+$r+"/topology.json"),B23_PFTL_NODE_SHA256:$h}' \
     > "$cfg/release.json"
   ```

3. **Close admission and drain the old owner.** Put `POST /api/bridge/jobs` into maintenance at every submitting entry point, including local clients. Keep readiness reads and the old worker running. Confirm that the operator's recoverable job-store snapshot is available and that the old config, checkout and launcher remain at their original paths. Enumerate every job under `$g/jobs/jobs`. For each `job.json`, require a matching, validated terminal worker state and no active worker. A missing state, retry, or uncertain transaction outcome holds the cutover. Reconcile those jobs with the bridge operator. For a quick census:

   ```bash
   for j in "$g"/jobs/jobs/*/job.json; do
     test -e "$j" || continue
     jq -e '(.status == "accepted" or .status == "failed") and .retryable == false' \
       "${j%/job.json}/worker-state.json" >/dev/null
   done
   ```

   The driver binds each checkpoint to the SHA-256 of the entire driver descriptor. Even a path-only edit changes that hash. Keep job IDs, checkpoints and completed receipts intact. The new server starts its watchdog and resumes jobs on startup. Starting a second server against this store would give it an opportunity to act before traffic switches.

4. **Hand over the listener.** Confirm a single listener on `127.0.0.1:18082`, its cwd, and its parent in the expected tmux pane. Check for surviving workers or remote rounds from the old owner. Once drained, stop only the verified Node PID. Keep the old launch command available. This operator command assumes the original single-pane tmux layout.

   ```bash
   oldcwd=/home/postfiat/repos/postfiatl1v2-b23-ingress
   pane=$(tmux display-message -p -t b23-bridge-proxy '#{pane_pid}')
   pid=$(ss -Hlntp 'sport = :18082' | sed -n 's/.*pid=\([0-9]*\),.*/\1/p')
   test "$(readlink "/proc/$pid/cwd")" = "$oldcwd"
   test "$(ps -o ppid= -p "$pid" | tr -d ' ')" = "$pane"
   kill -TERM "$pid"
   for i in {1..30}; do test ! -d "/proc/$pid" && break; sleep 1; done
   test ! -d "/proc/$pid"
   test -z "$(ss -Hlnt 'sport = :18082')"
   started=$(date +%s)
   tmux new-session -d -s ethereum-usdc-main -c "$src" \
     "exec /usr/bin/python3.12 '$src/deployments/ethereum-usdc-proxy/start.py' '$g/runtime-config.json' '$cfg/release.json' '$node' >>'$rel/proxy.log' 2>&1"
   ```

5. **Verify before reopening admission.** Check the new PID, cwd and executable. Confirm the existing wallet read path through its usual upstream. Allow for the 130-second route readiness timeout and wait for a check started by the new process to complete. Require the configured profile, authenticated beacon anchor and healthy prover. The old process may have left a valid warm cache. A health response alone cannot distinguish that cache from a new probe.

   ```bash
   newpid=$(ss -Hlntp 'sport = :18082' | sed -n 's/.*pid=\([0-9]*\),.*/\1/p')
   test "$(readlink "/proc/$newpid/cwd")" = "$src"
   test "$(readlink "/proc/$newpid/exe")" = "$node"
   curl -fsS http://127.0.0.1:18082/healthz | jq -e '.status == "ok"' >/dev/null
   profile=$(jq -er '.[0].route_profile_hash' "$cfg/routes.json")
   curl -fsS 'http://127.0.0.1:18082/api/bridge/readiness?route=ethereum-mainnet-usdc-v1' |
     jq -e --arg p "$profile" --argjson t "$started" \
       '.ready == true and .readiness_checked_at_unix > $t and .route_profile_hash == $p and
        .beacon_anchor_authenticated == true and .prover_healthy == true' >/dev/null
   ```

   Repeat the reads during warmup. Reopen submissions only after the checks pass. Record the new PID, main SHA, verifier and node hashes. Use normal authorized deposits for subsequent receipt reconciliation.

6. **Roll back if necessary.** Keep admission closed or close it again. Drain and reconcile any job already admitted by the new release under its own descriptor, including remote submissions. Verify no worker remains. If a job cannot drain, keep the ingress closed and reconcile it before changing owners. Restoring a stale job store can replay already submitted work. Recheck that the port owner is the new Node process and that its cwd is `$src`. Send TERM to that PID, wait for exit and a free port as in step 4. Restore the original launcher with its original runtime, route and driver files:

   ```bash
   started=$(date +%s)
   tmux new-session -d -s b23-bridge-proxy-rollback -c /home/postfiat/repos/b23-coord \
     "exec /usr/bin/python3 /home/postfiat/repos/b23-coord/evidence/ingress/g2-start-proxy.py >>'$g/proxy.log' 2>&1"
   ```

   Require the old cwd, the original executable, fresh readiness and the original route profile using step 5. Confirm the durable job inventory again, then reopen admission. The rollback restores the old relay-node pin as part of the old runtime. Confirm the fleet still supports that pin before using rollback.

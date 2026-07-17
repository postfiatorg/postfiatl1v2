# Signed validator snapshot recovery

Status: controlled-testnet operator procedure

This procedure replaces manual validator-directory copying. It preserves the
chain state and mempool policy while excluding validator keys, faucet private
keys, leases, sockets, logs, RPC spool files, and the certified-send outbox.

## Safety gates

- Stop and fence the validator being replaced before assigning its signer.
- Keep the failed disk or image for forensics; do not repair the only copy.
- Never copy `validator_keys.json`, `faucet_key.json`, deployment private keys,
  or another validator's signing identity into the snapshot.
- Import into a new empty directory. Do not overlay a live data directory.
- The replacement remains RPC/read-only until chain, genesis, protocol,
  height, tip, state root, and signed publisher identity all verify.

## One-time publisher key

Use a dedicated snapshot publisher key. It is not a validator key.

```text
postfiat-node snapshot-publisher-key-export \
  --publisher-key-file /secure/snapshot-publisher.private.json \
  --public-key-file /etc/postfiat/snapshot-publisher.public.json
```

Distribute the public file through the pinned deployment configuration. Keep
the private file outside validator data directories and snapshots.

## Export

Quiesce writes or export from a verified recovery copy, then run:

```text
postfiat-node snapshot-export-signed \
  --data-dir /var/lib/postfiat/validator-healthy \
  --snapshot-dir /srv/postfiat/snapshots/height-N \
  --publisher-key-file /secure/snapshot-publisher.private.json
```

The command verifies governance, bridge, shielded state, mempool, and block
history before export. The signed manifest binds:

- chain ID, genesis hash, protocol version, source node, state root, height,
  tip hash, and last certificate ID;
- the source build revision/profile;
- the explicit `preserve-verified-empty-or-pending` mempool policy;
- every replicated file's byte length and content hash; and
- `signer_material_included=false`.

Archive the complete snapshot directory by content hash. Do not add excluded
runtime or key files afterward.

## Import and replacement

Provision a fresh directory and the replacement validator's own isolated
signer. Then run:

```text
postfiat-node snapshot-import-signed \
  --data-dir /var/lib/postfiat/validator-replacement \
  --snapshot-dir /srv/postfiat/snapshots/height-N \
  --trusted-publisher-key-file /etc/postfiat/snapshot-publisher.public.json \
  --node-id validator-0
```

Import verifies the publisher signature before reading state, verifies every
file hash and byte length, writes into the new directory atomically, replaces
only the public node ID, then re-verifies chain/genesis/protocol/root/height/
tip plus governance, bridge, shielded, mempool, and block history.

Install the replacement validator's signer separately. Confirm the old host is
fenced and that exactly one signer can answer before enabling transport.

## Cold and warm restart drill

Record monotonic time for each drill:

1. `time-to-first-read`: process start until local `status` succeeds.
2. `time-to-ready`: process start until both circuit verifiers are warm, the
   deployment manifest is loaded, RPC health is green, and the transport ready
   marker exists.
3. Run a warm restart of the imported copy without changing files.
4. Run a cold restart after dropping only operating-system page cache on the
   disposable recovery host; do not mutate chain state.
5. Require the identical height, tip, and root after both restarts.

Store timings with the snapshot and deployment-manifest hashes. A test result
without those identities is not comparable evidence.

## Rollback

If signature, content hash, domain identity, state root, or signer uniqueness
fails, keep transport disabled, stop the replacement, preserve its logs and
directory, and return traffic to the still-fenced known-good validator. Never
edit the signed snapshot to make import pass; create a new signed export after
the source problem is understood.

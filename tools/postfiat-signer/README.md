# PostFiat constrained signer

`postfiat_signer.py` is the provider-neutral signer boundary used by bridge
relays. It accepts bounded JSON over an owner-only Unix socket, checks the
chain, route digest, target contract, function selector, transaction kind,
native value, total fee, rolling value budget, and durable idempotency key,
then signs with an encrypted Ethereum keystore.

The signer retains at most 4,096 durable idempotency records and fails closed
before signing a new request when that capacity is reached. State rotation is
therefore an explicit, reviewed operator action; the service never evicts a
completed key and silently permits it to sign again.

The service never accepts an RPC URL or policy override from a request. A
remote caller therefore cannot redirect signing or substitute a deployment.

Create an isolated user environment and an owner-only runtime policy:

```bash
python3 -m venv /home/postfiat/.local/share/postfiat-constrained-signer/venv
/home/postfiat/.local/share/postfiat-constrained-signer/venv/bin/pip install \
  -r tools/postfiat-signer/requirements.lock
install -d -m 0700 /home/postfiat/.config/postfiat-constrained-signer
install -m 0600 deployments/a666-export-relay-mainnet-20260731/signer-policy.example.json \
  /home/postfiat/.config/postfiat-constrained-signer/a666-signer.json
```

Create a dedicated encrypted relay key from an owner-only passphrase file. The
command prints only the public address and refuses to replace an existing key:

```bash
/home/postfiat/.local/share/postfiat-constrained-signer/venv/bin/python \
  tools/postfiat-signer/postfiat_signer.py \
  create-keystore \
  --keystore /home/postfiat/.local/state/postfiat-constrained-signer/a666-operator.keystore.json \
  --passphrase-file /home/postfiat/.local/state/postfiat-constrained-signer/a666-keystore.passphrase
```

Unlock without placing the passphrase in process arguments:

```bash
/home/postfiat/.local/share/postfiat-constrained-signer/venv/bin/python \
  tools/postfiat-signer/postfiat_signer.py unlock \
  --socket /run/user/1000/postfiat-constrained-signer/a666-signer.sock \
  --passphrase-file /home/postfiat/.local/state/postfiat-constrained-signer/a666-keystore.passphrase
```

The example policy's `keystore_path` is a placeholder for a Web3 Secret
Storage JSON keystore. The policy, state, passphrase file (if used), and
keystore must be owned by the service user and mode `0600`; the socket is
created mode `0600`. Requests and receipts never contain private-key material.

The wire schemas are:

- `postfiat.constrained_signer.request.v1`
- `postfiat.constrained_signer.response.v1`

The implementation is licensed under MIT or Apache-2.0 with the rest of the
PostFiat L1 repository.

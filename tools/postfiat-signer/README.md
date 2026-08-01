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

Create an isolated environment and an owner-only runtime policy:

```bash
python3 -m venv /opt/postfiat-signer/venv
/opt/postfiat-signer/venv/bin/pip install -r tools/postfiat-signer/requirements.lock
install -m 0600 deployments/a666-export-relay-mainnet-20260731/signer-policy.example.json \
  /etc/postfiat/a666-signer.json
/opt/postfiat-signer/venv/bin/python tools/postfiat-signer/postfiat_signer.py \
  daemon --config /etc/postfiat/a666-signer.json
```

Unlock interactively without placing the passphrase in process arguments:

```bash
/opt/postfiat-signer/venv/bin/python tools/postfiat-signer/postfiat_signer.py \
  unlock --socket /run/postfiat/a666-signer.sock
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

# Fixture: every doctor command here is valid

Prose is not a command, so this bare unit outside a fence is ignored:
scripts/testnet-validator-doctor --data-dir /x --validator-service postfiat-validator-0.service

Neither is inline code: `scripts/testnet-validator-doctor --rpc-service postfiat-rpc-0.service`.

```bash
scripts/testnet-validator-doctor \
  --data-dir /var/lib/postfiat/validator-0 \
  --validator-service validator-0=postfiat-validator-0.service \
  --rpc-service validator-0=postfiat-validator-0-rpc.service
```

````markdown
A longer fence: the inner three-backtick line does not close it.
```
scripts/testnet-validator-doctor --data-root /var/lib/postfiat --validator-count 6 \
  --validator-service-template 'postfiat-{node_id}.service' \
  --rpc-service-template 'postfiat-{node_id}-rpc.service'
````

~~~sh
RUN_ID=x scripts/testnet-validator-doctor --data-dir /d --rpc-service ' validator-5 = postfiat-validator-5-rpc.service '
~~~

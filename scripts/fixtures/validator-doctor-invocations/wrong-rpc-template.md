# Fixture: an RPC template that expands to unit names no host ships

```bash
scripts/testnet-validator-doctor \
  --data-root /var/lib/postfiat \
  --validator-count 5 \
  --rpc-service-template 'postfiat-rpc-{index}.service'
```

# Superseded partial fork attempt

This directory is an incomplete first attempt. Deployment and AMM transactions
ran, but evidence collection stopped when the wallet collector lacked its
`ws` dependency. The Anvil process was terminated, so the addresses and
transactions are ephemeral.

Do not use this directory as passing evidence. The clean fresh rerun is:

```text
../controlled-mainnet-fork-rerun/
```

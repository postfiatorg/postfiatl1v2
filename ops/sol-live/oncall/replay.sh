#!/bin/bash
# replay <binary> <store> <out>
docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges -u "$(id -u):$(id -g)" \
  -v "$2:/var/lib/postfiat/validator-3" \
  -v "/home/postfiat/var/sol-exit/replay/source/faucet_account.json:/var/lib/postfiat/validator-3/faucet_account.json:ro" \
  -v "/home/postfiat/var/sol-exit/replay/source/.integrity.key:/var/lib/postfiat/validator-3/.integrity.key:ro" \
  -v "$1:/opt/node:ro" ubuntu:24.04 /opt/node verify-state --data-dir /var/lib/postfiat/validator-3 > "$3" 2> "$3.err"; echo "EXIT=$?" >> "$3.err"

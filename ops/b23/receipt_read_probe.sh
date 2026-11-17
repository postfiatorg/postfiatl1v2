#!/bin/bash
# INSIDE a network-less container: boot rpc-serve on a copied data dir, snapshot file
# hashes, run pftl_uniswap_receipt reads, snapshot again, report any changed file.
D=/var/lib/postfiat/validator-3; B=/opt/rel/postfiat-node; O=/out; mkdir -p $D/readiness $O
rm -f $D/readiness/rpc.ready.json
$B rpc-serve --unsafe-devnet-json-storage --data-dir $D --spool-dir $D/runtime/rpc-spool --ready-file $D/readiness/rpc.ready.json \
  --bind-host 127.0.0.1 --port 27653 --max-requests 1000 --timeout-ms 30000 --child-timeout-ms 30000 > $O/rpc.stdout 2> $O/rpc.stderr &
RP=$!
for i in $(seq 1 120); do [ -f $D/readiness/rpc.ready.json ] && break; sleep 1; done
q() { exec 3<>/dev/tcp/127.0.0.1/27653; printf '%s\n' "{\"version\":\"postfiat-local-rpc-v1\",\"id\":\"$1\",\"method\":\"$2\",\"params\":$3}" >&3; timeout 60 head -n 1 <&3 > $O/q-$1.json; exec 3<&-; }
q warm server_info '{}'; sleep 2
snap() { find $D -type f ! -path "*/runtime/rpc-spool/*" ! -path "*/readiness/*" -printf '%P %s %T@\n' | sort > $O/$1.txt; }
snap before
q missing pftl_uniswap_receipt '{"tx_id":"'$(printf 'ab%.0s' $(seq 1 48))'"}'
q shortid pftl_uniswap_receipt '{"tx_id":"00"}'
q rcpt1209 pftl_uniswap_receipt '{"tx_id":"8d5f3cb096606375037064e3ece44fcabd6487ea50e7255f73dcfbf104c20a6b9cf87b7598e8b6be64a473d643b03c45"}'
sleep 2; snap after
diff $O/before.txt $O/after.txt > $O/diff.txt; echo "changed_files=$(grep -c '^[<>]' $O/diff.txt)" > $O/result.txt
kill $RP; wait 2>/dev/null

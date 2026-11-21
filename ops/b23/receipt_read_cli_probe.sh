D=/var/lib/postfiat/validator-3; B=/opt/rel/postfiat-node; O=/out
snap() { find $D -type f -printf '%P %s %T@\n' | sort > $O/cli-$1.txt; }
snap before
$B rpc --method pftl_uniswap_receipt --data-dir $D --tx-id $(printf 'ab%.0s' $(seq 1 48)) > $O/cli-missing.json 2>&1; echo "missing rc=$?" > $O/cli-rc.txt
$B rpc --method pftl_uniswap_receipt --data-dir $D --tx-id 8d5f3cb096606375037064e3ece44fcabd6487ea50e7255f73dcfbf104c20a6b9cf87b7598e8b6be64a473d643b03c45 > $O/cli-1209.json 2>&1; echo "1209 rc=$?" >> $O/cli-rc.txt
snap after
diff $O/cli-before.txt $O/cli-after.txt > $O/cli-diff.txt; echo "changed_files=$(grep -c '^[<>]' $O/cli-diff.txt)" >> $O/cli-rc.txt

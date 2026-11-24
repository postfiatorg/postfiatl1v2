#!/bin/bash
# Stream the withdrawal proving program and its SP1 home from the single-GPU machine to the four-GPU machine.
# Nothing on the four-GPU machine's deposit prover is touched: everything lands under /workspace/exit.
set -euo pipefail
cd ~/var/vast-rebuild
OLD="ssh -oBatchMode=yes -oConnectTimeout=10 -p 20628 root@ssh8.vast.ai"
./remote prover 'mkdir -p /workspace/exit/home /workspace/exit/b23-withdraw-20261116'
$OLD 'cd /workspace/b23-withdraw-20261116 && tar c proof-host egress.elf exit-witness.json' | ./remote prover 'tar x -C /workspace/exit/b23-withdraw-20261116'
$OLD 'cd /root && tar c .sp1' | ./remote prover 'tar x -C /workspace/exit/home'
$OLD 'cd /workspace/b23-withdraw-20261116 && sha256sum proof-host egress.elf; du -sb /root/.sp1 | cut -f1'
./remote prover 'cd /workspace/exit/b23-withdraw-20261116 && sha256sum proof-host egress.elf; du -sb /workspace/exit/home/.sp1 | cut -f1; ls /workspace/exit/home/.sp1 /workspace/exit/home/.sp1/bin'
echo COPY-DONE

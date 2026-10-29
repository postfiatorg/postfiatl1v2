#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
state=${L1_REGISTRY_STATE_DIR:-/home/postfiat/var/track-e-20261028}
mkdir -p "$state"
# The service owns the flock for this entire collection and publication.
python3 "$root/registry.py" --state "$state/observer.json" --output "$state/registry.json"
scp -q "$state/registry.json" pfrpc@100.85.116.26:/mnt/HC_Volume_101713660/pfrpc/ga-research-socket/l1-registry/.registry.next.json
ssh -o BatchMode=yes pfrpc@100.85.116.26 'chmod 644 /mnt/HC_Volume_101713660/pfrpc/ga-research-socket/l1-registry/.registry.next.json && mv /mnt/HC_Volume_101713660/pfrpc/ga-research-socket/l1-registry/.registry.next.json /mnt/HC_Volume_101713660/pfrpc/ga-research-socket/l1-registry/registry.json'

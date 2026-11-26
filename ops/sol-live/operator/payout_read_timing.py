"""Read-only timing of the payout steps for an already-paid burn.

  payout_read_timing.py [BURN_PREFIX [WORK_DIR [force]]]
"""
import json
import sys
import time
from pathlib import Path

from exit_payout import *  # noqa: F401,F403
from exit_payout import ASSET, BlockReads, chain_rules, covered_buckets, egress_witness, light, prior_before
from exit_payout import rpc, signature, validate_burn

T = [time.time()]


def lap(name):
    n = time.time()
    print(f"{n - T[0]:7.1f}s  {name}", flush=True)
    T[0] = n


def pick(status, prefix, rules):
    """The burn to time (the newest, or the one whose id starts with `prefix`), checked as the
    payout service checks it: against the buckets the chain covers."""
    reds = status["redemptions"]
    red = reds[-1] if not prefix else next(r for r in reds if r["burn_tx_id"].startswith(prefix))
    owner = validate_burn(red, red["burn_tx_id"], covered_buckets(status["buckets"], rules["covered"]))
    return red, owner


def main(argv):
    from exit_tag8 import setup_cluster

    c = setup_cluster("stage")
    lap("cluster")
    status = rpc(0, "vault_bridge_status", asset_id=ASSET)
    lap("vault_bridge_status")
    red, _ = pick(status, argv[1] if len(argv) > 1 else None, chain_rules())
    burn, h = red["burn_tx_id"], red["created_at_height"]
    print("burn", burn[:12], "height", h)
    signature(c, burn, None)
    lap("signature lookup")
    latest, prior = light(c)
    lap("light")
    fetch = BlockReads()
    if len(argv) > 3 or latest >= h:
        prior = prior_before(c, h, fetch)
        lap("prior_before")
    work = Path(argv[2] if len(argv) > 2 else "work")
    work.mkdir(exist_ok=True)
    w = egress_witness(c, red, prior, h, work, fetch)
    lap("egress_witness")
    (work / "witness.json").write_text(json.dumps(w, separators=(",", ":")))
    print("witness bytes", (work / "witness.json").stat().st_size)


if __name__ == "__main__":
    main(sys.argv)

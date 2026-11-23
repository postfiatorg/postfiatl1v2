"""Fund the operational fee payer once. Preserve the journal across releases."""
import argparse
import base64
import fcntl
import os
from pathlib import Path

from solders.hash import Hash
from solders.system_program import transfer
from solders.transaction import Transaction
from exit_setup import (CONFIG, DIRECTORY, ExitCluster, FEE_PAYER, MAINNET_PAYER,
                        check_payout_authorities, prove_roles, read, require, save)

JOURNAL = Path.home() / "var/sol-exit/funding-attempt.json"


def fund(c, path):
    with path.with_suffix(".lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if path.exists():
            attempt = read(path)
            sig = attempt["signature"]
            # Query history after the finalized expiry read, including on ambiguous sends.
            height = c.call("getBlockHeight", c.level)
            status = c.call("getSignatureStatuses", [sig], {"searchTransactionHistory": True})["value"][0]
            if status and status["confirmationStatus"] == "finalized" and status["err"] is None:
                return sig
            require(height > attempt["lastValidBlockHeight"] and
                    (status is None or status["confirmationStatus"] == "finalized"),
                    "Funding transaction pending. Reconcile again later.")
        require(c.call("getBalance", str(FEE_PAYER), c.level)["value"] == 0,
                "Existing recipient balance. Reconcile the prior transfer.")
        recent = c.call("getLatestBlockhash", c.level)["value"]
        ix = transfer(dict(from_pubkey=c.owner, to_pubkey=FEE_PAYER, lamports=100_000_000))
        tx = Transaction.new_signed_with_payer([ix], c.owner, [c.payer], Hash.from_string(recent["blockhash"]))
        sig = str(tx.signatures[0])
        save(path, dict(signature=sig, blockhash=recent["blockhash"],
                        lastValidBlockHeight=recent["lastValidBlockHeight"]))
        fd = os.open(path.parent, os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
        result = c.call("sendTransaction", base64.b64encode(bytes(tx)).decode(),
                        dict(encoding="base64", preflightCommitment="finalized", maxRetries=0))
        require(sig == result, "Funding signature differs from journal.")
        c.confirm([("exit.fund", sig)])
        return sig


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", required=True, action="store_true")
    parser.parse_args()
    os.umask(0o077)
    prove_roles(read(CONFIG))
    cluster = ExitCluster(MAINNET_PAYER)
    check_payout_authorities(cluster)
    cluster.unlock(DIRECTORY / "mainnet-payer.json")
    print(fund(cluster, JOURNAL))

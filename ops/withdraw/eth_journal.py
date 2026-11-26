"""Durable local evidence for the checkpoint and payout sender."""
import json
import os
import re
import tempfile
from pathlib import Path


def intent_path(result):
    return result.with_suffix('.intent.json')


def sync_directory(path):
    directory = os.open(path, os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def ensure_directory(path):
    if not path.exists():
        ensure_directory(path.parent)
        path.mkdir(exist_ok=True)
        sync_directory(path.parent)


def durable_json(path, value):
    # The transaction must not be broadcast until the file and directory entry survive a restart.
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as tmp:
        name = Path(tmp.name)
        try:
            json.dump(value, tmp, indent=1)
            tmp.flush()
            os.fsync(tmp.fileno())
            name.replace(path)
            sync_directory(path.parent)
        finally:
            name.unlink(missing_ok=True)


class ReconciliationRequired(RuntimeError):
    pass


def read_eth_result(mode, out):
    """Only a mined, successful receipt for the requested action confirms the job."""
    try:
        receipt = json.loads(out.read_text())
    except (OSError, ValueError) as error:
        raise ReconciliationRequired(f'Cannot read Ethereum {mode} receipt in {out.name}; reconcile before retry.') from error
    if (not isinstance(receipt, dict) or receipt.get('mode') != mode
            or type(receipt.get('status')) is not int or receipt['status'] != 1
            or not isinstance(receipt.get('tx'), str)
            or not re.fullmatch(r'(?:0x)?[a-fA-F0-9]{64}', receipt['tx'])
            or type(receipt.get('block')) is not int or receipt['block'] < 0):
        raise ReconciliationRequired(f'No successful Ethereum {mode} receipt in {out.name}; reconcile before retry.')
    journal = intent_path(out)
    if journal.exists():
        try:
            intent = json.loads(journal.read_text())
            if intent['mode'] != mode or intent['tx'].removeprefix('0x').lower() != receipt['tx'].removeprefix('0x').lower():
                raise ValueError('different transaction')
        except (AttributeError, KeyError, OSError, TypeError, ValueError) as error:
            raise ReconciliationRequired(f'Signed transaction differs from {out.name}; reconcile before retry.') from error
    return receipt

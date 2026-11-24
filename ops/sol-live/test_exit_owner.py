"""The payout service refuses destination owners that are not plain wallets."""

import unittest

from exit_payout import BUCKET, PREFIX, check_owner, validate_burn
from bridge import SYSTEM, TOKEN
from solders.pubkey import Pubkey

WALLET = Pubkey.from_string("Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8")
CUSTODY = Pubkey.from_string("AhmUaCijypvcbYMDxeXVbUPqjPX1tiHfLH6NWkYnQTCi")


class Accounts:
    def __init__(self, account):
        self.value = account

    def account(self, key):
        return self.value


def row(key):
    return dict(
        burn_tx_id="ab" * 48,
        bucket_id=BUCKET,
        amount_atoms=20_000,
        destination_ref=PREFIX + bytes(key).hex(),
    )


class OwnerTest(unittest.TestCase):
    def test_off_curve_owner_is_refused(self):
        self.assertEqual(validate_burn(row(WALLET), "ab" * 48, {BUCKET: {}}), WALLET)
        with self.assertRaises(ValueError):
            validate_burn(row(CUSTODY), "ab" * 48, {BUCKET: {}})

    def test_only_absent_or_system_owned_wallets_are_paid(self):
        for account in (None, dict(owner=str(SYSTEM), executable=False, data=b"")):
            check_owner(Accounts(account), WALLET)
        for account in (
            dict(owner=str(TOKEN), executable=False, data=bytes(165)),
            dict(owner=str(SYSTEM), executable=True, data=b""),
            dict(owner="BPFLoaderUpgradeab1e11111111111111111111111", executable=True, data=b""),
        ):
            with self.assertRaises(ValueError):
                check_owner(Accounts(account), WALLET)


if __name__ == "__main__":
    unittest.main()

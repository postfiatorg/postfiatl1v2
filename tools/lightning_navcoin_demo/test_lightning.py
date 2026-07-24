from __future__ import annotations

import hashlib
from pathlib import Path
import unittest

from tools.lightning_navcoin_demo.coordinator.protocol import SecretPreimage
from tools.lightning_navcoin_demo.lightning import (
    DirectLncliGrpc,
    LightningTransportError,
)


SECRET = SecretPreimage(bytes(range(32)))
HASH = hashlib.sha256(bytes(range(32))).hexdigest()
PAYREQ = "lnbcrt1synthetic"
DECODED = {
    "destination": "02" + "11" * 32,
    "payment_hash": HASH,
    "num_msat": "100000",
    "timestamp": "1700000000",
    "expiry": "900",
    "cltv_expiry": "144",
    "features": {},
}


class FakeExecutor:
    def __init__(self) -> None:
        self.calls: list[tuple[str, tuple[str, ...]]] = []
        self.wrong_hash = False
        self.failed = False

    def __call__(self, node, arguments, timeout):
        del timeout
        self.calls.append((node, tuple(arguments)))
        if arguments[0] == "decodepayreq":
            return DECODED
        if arguments[0] == "addinvoice":
            if "--amp" in arguments:
                return {
                    "r_hash": HASH,
                    "payment_request": "lnbcrt1amp",
                    "add_index": "8",
                    "payment_addr": "33" * 32,
                }
            return {
                "r_hash": "ff" * 32 if self.wrong_hash else HASH,
                "payment_request": PAYREQ,
                "add_index": "7",
                "payment_addr": "22" * 32,
            }
        if arguments[0] == "payinvoice":
            if self.failed:
                return {
                    "payment_hash": HASH,
                    "value_msat": "0",
                    "fee_msat": "0",
                    "payment_preimage": "0" * 64,
                    "status": "FAILED",
                    "failure_reason": "FAILURE_REASON_NO_ROUTE",
                    "htlcs": [],
                }
            return {
                "payment_hash": HASH,
                "value_msat": "100000",
                "fee_msat": "0",
                "payment_preimage": SECRET.protocol_hex(),
                "status": "SUCCEEDED",
                "failure_reason": "FAILURE_REASON_NONE",
                "htlcs": [
                    {
                        "route": {
                            "hops": [
                                {"expiry": 400, "preimage": SECRET.protocol_hex()}
                            ]
                        },
                        "preimage": SECRET.protocol_hex(),
                    }
                ],
            }
        if arguments[0] == "lookupinvoice":
            return {
                "r_hash": HASH,
                "settled": True,
                "r_preimage": SECRET.protocol_hex(),
            }
        if arguments[0] == "getinfo":
            return {
                "chains": [{"chain": "bitcoin", "network": "regtest"}],
                "synced_to_chain": True,
            }
        raise AssertionError(arguments)


class DirectLncliGrpcTests(unittest.TestCase):
    def setUp(self) -> None:
        self.executor = FakeExecutor()
        self.client = DirectLncliGrpc(
            Path("/nonexistent"),
            executor=self.executor,
        )

    def test_invoice_and_payment_cross_check_and_redact(self) -> None:
        invoice = self.client.add_invoice(
            "coordinator",
            SECRET,
            amount_msat=100_000,
            memo="synthetic",
        )
        self.assertEqual(invoice.payment_hash, HASH)
        payment = self.client.pay_invoice("user", invoice.payment_request)
        self.assertEqual(payment.status, "SUCCEEDED")
        self.assertEqual(payment.payer_htlc_expiries, (400,))
        self.assertNotIn(SECRET.protocol_hex(), repr(payment))
        self.assertEqual(
            payment.public_response["payment_preimage"], "<redacted>"
        )
        self.assertEqual(
            payment.public_response["htlcs"][0]["preimage"], "<redacted>"
        )

    def test_receiver_generated_invoice_does_not_export_a_secret(self) -> None:
        invoice = self.client.add_invoice_generated(
            "user",
            amount_msat=100_000,
            memo="receiver-controlled",
        )
        self.assertEqual(invoice.payment_hash, HASH)
        add_call = next(
            arguments
            for _, arguments in self.executor.calls
            if arguments[0] == "addinvoice"
        )
        self.assertFalse(
            any(argument.startswith("--preimage=") for argument in add_call)
        )

    def test_wrong_invoice_hash_and_route_failure_fail_closed(self) -> None:
        self.executor.wrong_hash = True
        with self.assertRaisesRegex(LightningTransportError, "differs"):
            self.client.add_invoice(
                "coordinator",
                SECRET,
                amount_msat=100_000,
                memo="synthetic",
            )
        self.executor.wrong_hash = False
        self.executor.failed = True
        payment = self.client.pay_invoice("user", PAYREQ)
        self.assertEqual(payment.status, "FAILED")
        self.assertIsNone(payment.payment_preimage)

    def test_lookup_redacts_preimage(self) -> None:
        result = self.client.lookup_invoice("coordinator", HASH)
        self.assertEqual(result["r_preimage"], "<redacted>")

    def test_amp_test_invoice_is_created_but_never_interpreted_as_ordinary(self) -> None:
        request = self.client.add_amp_invoice_for_rejection_test(
            "coordinator", amount_msat=3_000
        )
        self.assertEqual(request, "lnbcrt1amp")
        self.assertIn("--amp", self.executor.calls[-1][1])


if __name__ == "__main__":
    unittest.main()

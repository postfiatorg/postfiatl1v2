from __future__ import annotations

from types import SimpleNamespace
import unittest

from tools.lightning_navcoin_demo.coordinator.lnd_grpc import (
    LightningPaymentError,
    LndGrpcAdapter,
    LndGrpcError,
    LndRequestFactories,
)
from tools.lightning_navcoin_demo.coordinator.protocol import (
    SecretPreimage,
    payment_hash,
)


def message(**fields: object) -> SimpleNamespace:
    return SimpleNamespace(**fields)


class FakeLightningStub:
    def __init__(self, secret: SecretPreimage) -> None:
        self.secret = secret
        self.calls: list[tuple[str, object, float]] = []

    def AddInvoice(self, request: object, *, timeout: float) -> SimpleNamespace:
        self.calls.append(("AddInvoice", request, timeout))
        self.assert_non_amp(request)
        return message(
            r_hash=payment_hash(self.secret),
            payment_request="lnbcrt-direct-grpc-test",
            add_index=7,
            payment_addr=b"\x44" * 32,
        )

    def DecodePayReq(self, request: object, *, timeout: float) -> SimpleNamespace:
        self.calls.append(("DecodePayReq", request, timeout))
        return message(
            payment_hash=payment_hash(self.secret).hex(),
            num_msat="2500000",
            destination="02" + "55" * 32,
            timestamp="1700000000",
            expiry="900",
            cltv_expiry="144",
            features={},
            is_amp=False,
        )

    def LookupInvoice(self, request: object, *, timeout: float) -> SimpleNamespace:
        self.calls.append(("LookupInvoice", request, timeout))
        return message(
            r_hash=payment_hash(self.secret),
            settled=True,
            state="SETTLED",
            amt_paid_msat="2500000",
            add_index=7,
            settle_index=9,
            is_amp=False,
        )

    @staticmethod
    def assert_non_amp(request: object) -> None:
        if getattr(request, "is_amp") is not False:
            raise AssertionError("adapter did not explicitly disable AMP")


class FakeRouterStub:
    def __init__(self, lightning: FakeLightningStub) -> None:
        self.lightning = lightning
        self.calls: list[tuple[str, object, float]] = []
        self.payment_error = ""
        self.wrong_preimage = False

    def SendPaymentV2(self, request: object, *, timeout: float) -> list[SimpleNamespace]:
        self.calls.append(("SendPaymentV2", request, timeout))
        if self.payment_error:
            return [
                message(
                    status="FAILED",
                    failure_reason=self.payment_error,
                    payment_hash=payment_hash(self.lightning.secret).hex(),
                )
            ]
        preimage = (
            bytes(reversed(range(32)))
            if self.wrong_preimage
            else self.lightning.secret.reveal_for_protocol()
        )
        return [
            message(
                status="IN_FLIGHT",
                payment_hash=payment_hash(self.lightning.secret).hex(),
            ),
            message(
                status="SUCCEEDED",
                payment_hash=payment_hash(self.lightning.secret).hex(),
                payment_preimage=preimage.hex(),
                fee_sat="2",
                htlcs=[
                    message(route=message(total_time_lock="501")),
                    message(route=message(total_time_lock="503")),
                ],
            ),
        ]

class LndGrpcAdapterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.secret = SecretPreimage(bytes(range(32)))
        self.stub = FakeLightningStub(self.secret)
        self.router_stub = FakeRouterStub(self.stub)
        factories = LndRequestFactories(
            invoice=message,
            pay_req_string=message,
            payment_hash=message,
            send_payment_request=message,
        )
        self.adapter = LndGrpcAdapter(
            self.stub,
            self.router_stub,
            factories,
            network="regtest",
            rpc_timeout_seconds=5,
        )

    def test_runtime_operations_call_direct_grpc_stub(self) -> None:
        created = self.adapter.add_invoice(
            self.secret,
            amount_msat=2_500_000,
            expiry_seconds=900,
            min_final_cltv_delta=144,
        )
        self.assertEqual(created.payment_hash, payment_hash(self.secret))
        self.assertEqual(created.facts.amount_msat, 2_500_000)
        status = self.adapter.lookup_invoice(created.payment_hash)
        self.assertTrue(status.settled)
        settled = self.adapter.send_payment(
            created.payment_request,
            fee_limit_msat=1000,
            max_total_cltv_delta=288,
            timeout_seconds=30,
        )
        self.assertEqual(settled.payment_hash, payment_hash(self.secret))
        self.assertEqual(
            settled.payment_preimage.reveal_for_protocol(),
            self.secret.reveal_for_protocol(),
        )
        self.assertEqual(settled.fee_sat, 2)
        self.assertEqual(settled.payer_htlc_expiries, (501, 503))
        send_request = self.router_stub.calls[-1][1]
        self.assertEqual(send_request.cltv_limit, 288)
        self.assertEqual(send_request.max_parts, 1)
        self.assertFalse(send_request.amp)
        self.assertEqual(
            [call[0] for call in self.stub.calls]
            + [call[0] for call in self.router_stub.calls],
            [
                "AddInvoice",
                "DecodePayReq",
                "LookupInvoice",
                "DecodePayReq",
                "SendPaymentV2",
            ],
        )

    def test_payment_preimage_is_redacted_from_repr(self) -> None:
        settled = self.adapter.send_payment(
            "lnbcrt-direct-grpc-test",
            fee_limit_msat=1000,
            max_total_cltv_delta=288,
            timeout_seconds=30,
        )
        self.assertNotIn(self.secret.protocol_hex(), repr(settled))
        self.assertIn("<redacted>", repr(settled))

    def test_route_failure_and_wrong_preimage_fail_closed(self) -> None:
        self.router_stub.payment_error = "unable to find a path"
        with self.assertRaises(LightningPaymentError):
            self.adapter.send_payment(
                "lnbcrt-direct-grpc-test",
                fee_limit_msat=1000,
                max_total_cltv_delta=288,
                timeout_seconds=30,
            )
        self.router_stub.payment_error = ""
        self.router_stub.wrong_preimage = True
        with self.assertRaises(LndGrpcError):
            self.adapter.send_payment(
                "lnbcrt-direct-grpc-test",
                fee_limit_msat=1000,
                max_total_cltv_delta=288,
                timeout_seconds=30,
            )

    def test_add_invoice_wrong_hash_rejects(self) -> None:
        original = self.stub.AddInvoice

        def wrong_hash(request: object, *, timeout: float) -> SimpleNamespace:
            response = original(request, timeout=timeout)
            response.r_hash = bytes(32)
            return response

        self.stub.AddInvoice = wrong_hash  # type: ignore[method-assign]
        with self.assertRaises(LndGrpcError):
            self.adapter.add_invoice(
                self.secret,
                amount_msat=2_500_000,
                expiry_seconds=900,
                min_final_cltv_delta=144,
            )

    def test_amp_decode_rejects_before_payment(self) -> None:
        original = self.stub.DecodePayReq

        def amp_decode(request: object, *, timeout: float) -> SimpleNamespace:
            response = original(request, timeout=timeout)
            response.is_amp = True
            return response

        self.stub.DecodePayReq = amp_decode  # type: ignore[method-assign]
        with self.assertRaises(LightningPaymentError):
            self.adapter.send_payment(
                "lnbcrt-direct-grpc-test",
                fee_limit_msat=1000,
                max_total_cltv_delta=288,
                timeout_seconds=30,
            )

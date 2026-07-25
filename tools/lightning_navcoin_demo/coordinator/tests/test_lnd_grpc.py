from __future__ import annotations

from types import SimpleNamespace
import unittest

from tools.lightning_navcoin_demo.coordinator.lnd_grpc import (
    LightningPaymentError,
    LndGrpcAdapter,
    LndGrpcError,
    LndInvoiceNotFound,
    LndRequestFactories,
    PaymentReconciliationStatus,
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
        self.identity_pubkey = "02" + "11" * 32
        self.synced_to_chain = True
        self.synced_to_graph = True
        self.chain_network = "regtest"
        self.version = "0.20.1-beta commit=v0.20.1-beta"
        self.commit_hash = "848b72ce96eb68fa90fd4336523ca4c59bddcd4c"
        self.channels = [
            message(
                active=True,
                local_balance="4000",
                remote_balance="6000",
                local_chan_reserve_sat="1000",
                remote_chan_reserve_sat="2000",
            ),
            message(active=False, local_balance="8000", remote_balance="9000"),
        ]

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
            value_msat="2500000",
            add_index=7,
            settle_index=9,
            is_amp=False,
            payment_request="lnbcrt-direct-grpc-test",
            payment_addr=b"\x44" * 32,
        )

    def GetInfo(self, request: object, *, timeout: float) -> SimpleNamespace:
        self.calls.append(("GetInfo", request, timeout))
        return message(
            identity_pubkey=self.identity_pubkey,
            alias="coordinator",
            block_height="850000",
            synced_to_chain=self.synced_to_chain,
            synced_to_graph=self.synced_to_graph,
            version=self.version,
            commit_hash=self.commit_hash,
            chains=[
                message(chain="bitcoin", network=self.chain_network),
            ],
        )

    def ListChannels(
        self, request: object, *, timeout: float
    ) -> SimpleNamespace:
        self.calls.append(("ListChannels", request, timeout))
        return message(channels=self.channels)

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
        self.track_transport_error = False
        self.track_status: int | str = "SUCCEEDED"
        self.track_value_msat = "2500000"

    def SendPaymentV2(
        self, request: object, *, timeout: float
    ) -> list[SimpleNamespace]:
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
                value_msat="2500000",
                fee_sat="2",
                fee_msat="2501",
                htlcs=[
                    message(route=message(total_time_lock="501")),
                    message(route=message(total_time_lock="503")),
                ],
            ),
        ]

    def TrackPaymentV2(
        self, request: object, *, timeout: float
    ) -> list[SimpleNamespace]:
        self.calls.append(("TrackPaymentV2", request, timeout))
        if self.track_transport_error:
            raise OSError("simulated transport loss")
        if self.track_status in (3, "FAILED"):
            return [
                message(
                    status=self.track_status,
                    failure_reason="FAILURE_REASON_NO_ROUTE",
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
                status=self.track_status,
                payment_hash=payment_hash(self.lightning.secret).hex(),
                payment_preimage=preimage.hex(),
                value_msat=self.track_value_msat,
                fee_sat="2",
                fee_msat="2501",
                htlcs=[message(route=message(total_time_lock="503"))],
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
            get_info_request=message,
            list_channels_request=message,
            track_payment_request=message,
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
        self.assertEqual(settled.fee_msat, 2501)
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

    def test_node_and_active_liquidity_preflight(self) -> None:
        preflight = self.adapter.preflight_node(
            expected_identity_pubkey=self.stub.identity_pubkey,
            min_active_channels=1,
            min_inbound_msat=4_000_000,
            min_outbound_msat=3_000_000,
        )
        self.assertEqual(preflight.node.network, "regtest")
        self.assertEqual(preflight.node.block_height, 850000)
        self.assertEqual(preflight.liquidity.total_channels, 2)
        self.assertEqual(preflight.liquidity.active_channels, 1)
        self.assertEqual(preflight.liquidity.inactive_channels, 1)
        self.assertEqual(preflight.liquidity.unconfirmed_active_channels, 0)
        self.assertEqual(preflight.liquidity.inbound_msat, 4_000_000)
        self.assertEqual(preflight.liquidity.outbound_msat, 3_000_000)
        list_request = self.stub.calls[-1][1]
        self.assertFalse(list_request.active_only)
        self.assertFalse(list_request.inactive_only)

    def test_unconfirmed_zero_conf_channel_contributes_no_liquidity(self) -> None:
        self.stub.channels = [
            message(
                active=True,
                zero_conf=True,
                zero_conf_confirmed_scid="0",
                local_balance="4000",
                remote_balance="6000",
                local_chan_reserve_sat="1000",
                remote_chan_reserve_sat="2000",
            ),
        ]
        liquidity = self.adapter.list_active_liquidity()
        self.assertEqual(liquidity.total_channels, 1)
        self.assertEqual(liquidity.active_channels, 0)
        self.assertEqual(liquidity.inactive_channels, 0)
        self.assertEqual(liquidity.unconfirmed_active_channels, 1)
        self.assertEqual(liquidity.inbound_msat, 0)
        self.assertEqual(liquidity.outbound_msat, 0)
        with self.assertRaisesRegex(LndGrpcError, "active channels"):
            self.adapter.preflight_node(
                expected_identity_pubkey=self.stub.identity_pubkey,
                min_active_channels=1,
                min_inbound_msat=1,
            )

    def test_confirmed_zero_conf_channel_is_counted_after_confirmed_scid(self) -> None:
        self.stub.channels[0].zero_conf = True
        self.stub.channels[0].zero_conf_confirmed_scid = "742100x1x0"
        with self.assertRaisesRegex(LndGrpcError, "unsigned integer"):
            self.adapter.list_active_liquidity()
        self.stub.channels[0].zero_conf_confirmed_scid = "810123456789"
        liquidity = self.adapter.list_active_liquidity()
        self.assertEqual(liquidity.active_channels, 1)
        self.assertEqual(liquidity.unconfirmed_active_channels, 0)

    def test_node_preflight_rejects_identity_network_sync_and_liquidity(
        self,
    ) -> None:
        with self.assertRaisesRegex(LndGrpcError, "identity pubkey"):
            self.adapter.preflight_node(
                expected_identity_pubkey="03" + "22" * 32
            )
        self.stub.chain_network = "mainnet"
        with self.assertRaisesRegex(LndGrpcError, "chain/network"):
            self.adapter.preflight_node(
                expected_identity_pubkey=self.stub.identity_pubkey
            )
        self.stub.chain_network = "regtest"
        self.stub.synced_to_graph = False
        with self.assertRaisesRegex(LndGrpcError, "not fully"):
            self.adapter.preflight_node(
                expected_identity_pubkey=self.stub.identity_pubkey
            )
        self.stub.synced_to_graph = True
        with self.assertRaisesRegex(LndGrpcError, "inbound"):
            self.adapter.preflight_node(
                expected_identity_pubkey=self.stub.identity_pubkey,
                min_inbound_msat=4_000_001,
            )

    def test_mainnet_get_info_maps_bitcoin_to_lnd_mainnet(self) -> None:
        self.stub.chain_network = "mainnet"
        adapter = LndGrpcAdapter(
            self.stub,
            self.router_stub,
            self.adapter._messages,
            network="bitcoin",
            rpc_timeout_seconds=5,
        )
        info = adapter.get_info(
            expected_identity_pubkey=self.stub.identity_pubkey
        )
        self.assertEqual(info.network, "bitcoin")

    def test_reviewed_lnd_version_and_commit_are_exact_pins(self) -> None:
        adapter = LndGrpcAdapter(
            self.stub,
            self.router_stub,
            self.adapter._messages,
            network="regtest",
            rpc_timeout_seconds=5,
            expected_version="0.20.1-beta commit=v0.20.1-beta",
            expected_commit_hash="848b72ce96eb68fa90fd4336523ca4c59bddcd4c",
        )
        info = adapter.get_info(
            expected_identity_pubkey=self.stub.identity_pubkey
        )
        self.assertEqual(
            info.version, "0.20.1-beta commit=v0.20.1-beta"
        )
        self.assertEqual(
            info.commit_hash, "848b72ce96eb68fa90fd4336523ca4c59bddcd4c"
        )
        self.stub.version = "0.20.2-beta commit=v0.20.2-beta"
        with self.assertRaisesRegex(LndGrpcError, "reviewed release"):
            adapter.get_info(
                expected_identity_pubkey=self.stub.identity_pubkey
            )
        self.stub.version = "0.20.1-beta commit=v0.20.1-beta"
        self.stub.commit_hash = "0" * 40
        with self.assertRaisesRegex(LndGrpcError, "reviewed release"):
            adapter.get_info(
                expected_identity_pubkey=self.stub.identity_pubkey
            )

    def test_legacy_factories_remain_constructible(self) -> None:
        factories = LndRequestFactories(
            invoice=message,
            pay_req_string=message,
            payment_hash=message,
            send_payment_request=message,
        )
        adapter = LndGrpcAdapter(
            self.stub,
            self.router_stub,
            factories,
            network="regtest",
        )
        self.assertEqual(
            adapter.decode_invoice("lnbcrt-direct-grpc-test").amount_msat,
            2_500_000,
        )
        with self.assertRaisesRegex(LndGrpcError, "factory"):
            adapter.get_info(
                expected_identity_pubkey=self.stub.identity_pubkey
            )

    def test_track_payment_reconciles_success_failure_and_uncertainty(
        self,
    ) -> None:
        digest = payment_hash(self.secret)
        settled = self.adapter.track_payment(
            digest, expected_amount_msat=2_500_000
        )
        self.assertEqual(settled.status, PaymentReconciliationStatus.SETTLED)
        self.assertTrue(settled.terminal)
        self.assertIsNotNone(settled.settled_payment)
        assert settled.settled_payment is not None
        self.assertEqual(settled.settled_payment.amount_msat, 2_500_000)
        self.assertEqual(
            settled.settled_payment.payment_preimage.reveal_for_protocol(),
            self.secret.reveal_for_protocol(),
        )

        self.router_stub.track_status = "FAILED"
        failed = self.adapter.track_payment(digest)
        self.assertEqual(failed.status, PaymentReconciliationStatus.FAILED)
        self.assertTrue(failed.terminal)
        self.assertEqual(failed.failure_reason, "FAILURE_REASON_NO_ROUTE")

        self.router_stub.track_transport_error = True
        uncertain = self.adapter.track_payment(digest)
        self.assertEqual(
            uncertain.status, PaymentReconciliationStatus.UNCERTAIN
        )
        self.assertFalse(uncertain.terminal)

    def test_track_payment_rejects_wrong_preimage_or_amount(self) -> None:
        digest = payment_hash(self.secret)
        self.router_stub.wrong_preimage = True
        with self.assertRaisesRegex(LndGrpcError, "preimage"):
            self.adapter.track_payment(
                digest, expected_amount_msat=2_500_000
            )
        self.router_stub.wrong_preimage = False
        self.router_stub.track_value_msat = "2499999"
        with self.assertRaisesRegex(LndGrpcError, "amount"):
            self.adapter.track_payment(
                digest, expected_amount_msat=2_500_000
            )

    def test_lookup_settled_invoice_checks_exact_binding(self) -> None:
        digest = payment_hash(self.secret)
        status = self.adapter.lookup_invoice(
            digest,
            require_settled=True,
            expected_amount_msat=2_500_000,
            expected_add_index=7,
            expected_settle_index=9,
        )
        self.assertTrue(status.settled)
        with self.assertRaisesRegex(LndGrpcError, "paid amount"):
            original = self.stub.LookupInvoice

            def wrong_paid(
                request: object, *, timeout: float
            ) -> SimpleNamespace:
                response = original(request, timeout=timeout)
                response.amt_paid_msat = "2499999"
                return response

            self.stub.LookupInvoice = wrong_paid  # type: ignore[method-assign]
            self.adapter.lookup_invoice(
                digest,
                require_settled=True,
                expected_amount_msat=2_500_000,
            )

    def test_lookup_open_and_canceled_invoice_are_distinguished(self) -> None:
        original = self.stub.LookupInvoice

        def open_lookup(
            request: object, *, timeout: float
        ) -> SimpleNamespace:
            response = original(request, timeout=timeout)
            response.settled = False
            response.state = "OPEN"
            response.amt_paid_msat = "0"
            response.settle_index = 0
            return response

        self.stub.LookupInvoice = open_lookup  # type: ignore[method-assign]
        opened = self.adapter.lookup_invoice(
            payment_hash(self.secret),
            expected_amount_msat=2_500_000,
        )
        self.assertEqual(opened.state_name, "OPEN")
        self.assertFalse(opened.terminal_unpaid)

        def canceled_lookup(
            request: object, *, timeout: float
        ) -> SimpleNamespace:
            response = open_lookup(request, timeout=timeout)
            response.state = "CANCELED"
            return response

        self.stub.LookupInvoice = canceled_lookup  # type: ignore[method-assign]
        canceled = self.adapter.lookup_invoice(
            payment_hash(self.secret),
            expected_amount_msat=2_500_000,
        )
        self.assertEqual(canceled.state_name, "CANCELED")
        self.assertTrue(canceled.terminal_unpaid)

    def test_recover_created_invoice_and_exact_not_found(self) -> None:
        recovered = self.adapter.recover_created_invoice(
            payment_hash(self.secret),
            expected_amount_msat=2_500_000,
            expected_payee="02" + "55" * 32,
            expected_min_final_cltv_delta=144,
        )
        self.assertEqual(recovered.payment_request, "lnbcrt-direct-grpc-test")
        self.assertEqual(recovered.add_index, 7)

        class NotFoundError(Exception):
            @staticmethod
            def code() -> SimpleNamespace:
                return message(name="NOT_FOUND")

        def not_found(_request: object, *, timeout: float) -> SimpleNamespace:
            del timeout
            raise NotFoundError()

        self.stub.LookupInvoice = not_found  # type: ignore[method-assign]
        with self.assertRaises(LndInvoiceNotFound):
            self.adapter.lookup_invoice(payment_hash(self.secret))

    def test_lookup_invoice_rejects_amp_and_inconsistent_settlement(
        self,
    ) -> None:
        original = self.stub.LookupInvoice

        def bad_lookup(request: object, *, timeout: float) -> SimpleNamespace:
            response = original(request, timeout=timeout)
            response.is_amp = True
            return response

        self.stub.LookupInvoice = bad_lookup  # type: ignore[method-assign]
        with self.assertRaisesRegex(LndGrpcError, "AMP"):
            self.adapter.lookup_invoice(payment_hash(self.secret))

        def inconsistent(
            request: object, *, timeout: float
        ) -> SimpleNamespace:
            response = original(request, timeout=timeout)
            response.state = "OPEN"
            return response

        self.stub.LookupInvoice = inconsistent  # type: ignore[method-assign]
        with self.assertRaisesRegex(LndGrpcError, "inconsistent"):
            self.adapter.lookup_invoice(payment_hash(self.secret))

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

from __future__ import annotations

import json
import unittest

from ..http_api import API_PREFIX, ApiError, LightningNavcoinApi


class FakeFacade:
    def __init__(self) -> None:
        self.calls: list[tuple[str, str, str | None]] = []

    def public_status(self) -> dict[str, object]:
        return {
            "ok": True,
            "mode": "DRY_RUN",
            "trust_class": "CONTROLLED",
            "claim": "non-custodial, conditionally-atomic, COORDINATOR-TRUSTED timing",
            "hold_reasons": ["pftl_handoff_missing"],
        }

    def create_quote(self, request: object) -> dict[str, object]:
        return {
            "ok": True,
            "can_execute": False,
            "request": request,
            "hold_reasons": ["real_value_policy_mode_is_dry_run"],
        }

    def public_swap(self, swap_id: str) -> dict[str, object]:
        return {"ok": True, "swap_id": swap_id, "state": "QUOTED"}

    def observe_user_lock(self, swap_id: str, tx_id: str) -> dict[str, object]:
        self.calls.append(("lock", swap_id, tx_id))
        return {"swap_id": swap_id, "state": "PFTL_LOCK_FINAL"}

    def observe_user_finish(self, swap_id: str, tx_id: str) -> dict[str, object]:
        self.calls.append(("finish", swap_id, tx_id))
        return {"swap_id": swap_id, "state": "PFTL_FINISH_FINAL"}

    def observe_user_cancel(self, swap_id: str, tx_id: str) -> dict[str, object]:
        self.calls.append(("cancel", swap_id, tx_id))
        return {"swap_id": swap_id, "state": "PFTL_CANCEL_FINAL"}


class ApiTests(unittest.TestCase):
    def setUp(self) -> None:
        self.clock_value = 1000.0
        self.token = bytes.fromhex("11" * 32)
        self.facade = FakeFacade()
        self.api = LightningNavcoinApi(
            self.facade,
            session_token=self.token,
            clock=lambda: self.clock_value,
        )
        self.auth = {
            "authorization": f"Bearer {self.token.hex()}",
            "origin": "http://127.0.0.1:5173",
            "x-postfiat-csrf": "44" * 32,
            "x-requested-with": "postfiat-wallet",
        }

    def test_public_status_and_authenticated_quote(self) -> None:
        status, response = self.api.dispatch(
            "GET", f"{API_PREFIX}/status", {}, b"", principal="127.0.0.1"
        )
        self.assertEqual(status, 200)
        self.assertEqual(response["result"]["mode"], "DRY_RUN")
        request = {
            "direction": "lightning_to_pftl",
            "amount_msat": "10000",
            "wallet_address": "pf" + "22" * 20,
            "client_request_id": "33" * 32,
        }
        status, response = self.api.dispatch(
            "POST",
            f"{API_PREFIX}/quotes",
            self.auth,
            json.dumps(request).encode(),
            principal="127.0.0.1",
        )
        self.assertEqual(status, 200)
        self.assertIs(response["result"]["can_execute"], False)
        self.assertEqual(response["result"]["request"]["amount_msat"], 10_000)

    def test_auth_origin_shape_and_secret_responses_fail_closed(self) -> None:
        with self.assertRaisesRegex(ApiError, "session"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/quotes",
                {"origin": "http://127.0.0.1:5173"},
                b"{}",
                principal="127.0.0.2",
            )
        with self.assertRaisesRegex(ApiError, "origin"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/quotes",
                {
                    "authorization": f"Bearer {self.token.hex()}",
                    "origin": "https://evil.test",
                    "x-postfiat-csrf": "44" * 32,
                    "x-requested-with": "postfiat-wallet",
                },
                b"{}",
                principal="127.0.0.3",
            )

        class Unsafe(FakeFacade):
            def public_swap(self, swap_id: str) -> dict[str, object]:
                return {"swap_id": swap_id, "payment_preimage": "00" * 32}

        unsafe = LightningNavcoinApi(
            Unsafe(), session_token=self.token, clock=lambda: self.clock_value
        )
        with self.assertRaisesRegex(ApiError, "secret-bearing"):
            unsafe.dispatch(
                "GET",
                f"{API_PREFIX}/swaps/" + "aa" * 32,
                {"authorization": f"Bearer {self.token.hex()}"},
                b"",
                principal="127.0.0.4",
            )

    def test_swap_get_is_pure_and_receipt_notices_only_observe_typed_actions(self) -> None:
        swap_id = "aa" * 32
        tx_id = "bb" * 48
        status, response = self.api.dispatch(
            "GET",
            f"{API_PREFIX}/swaps/{swap_id}",
            {"authorization": f"Bearer {self.token.hex()}"},
            b"",
            principal="127.0.0.9",
        )
        self.assertEqual(status, 200)
        self.assertEqual(response["result"]["swap_id"], swap_id)
        self.assertEqual(self.facade.calls, [])

        status, response = self.api.dispatch(
            "POST",
            f"{API_PREFIX}/swaps/{swap_id}/pftl-lock",
            self.auth,
            json.dumps({"tx_id": tx_id}).encode(),
            principal="127.0.0.10",
        )
        self.assertEqual(status, 200)
        self.assertEqual(response["result"]["state"], "PFTL_LOCK_FINAL")
        self.assertEqual(self.facade.calls[-1], ("lock", swap_id, tx_id))

        status, response = self.api.dispatch(
            "POST",
            f"{API_PREFIX}/swaps/{swap_id}/pftl-finish",
            self.auth,
            json.dumps({"tx_id": tx_id}).encode(),
            principal="127.0.0.11",
        )
        self.assertEqual(status, 200)
        self.assertEqual(response["result"]["state"], "PFTL_FINISH_FINAL")
        self.assertEqual(self.facade.calls[-1], ("finish", swap_id, tx_id))

        status, response = self.api.dispatch(
            "POST",
            f"{API_PREFIX}/swaps/{swap_id}/pftl-cancel",
            self.auth,
            json.dumps({"tx_id": tx_id}).encode(),
            principal="127.0.0.13",
        )
        self.assertEqual(status, 200)
        self.assertEqual(response["result"]["state"], "PFTL_CANCEL_FINAL")
        self.assertEqual(self.facade.calls[-1], ("cancel", swap_id, tx_id))

        with self.assertRaisesRegex(ApiError, "canonical 48-byte"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/swaps/{swap_id}/pftl-finish",
                self.auth,
                json.dumps({"tx_id": "bb" * 32}).encode(),
                principal="127.0.0.12",
            )

    def test_offramp_invoice_and_duplicate_json_validation(self) -> None:
        base = {
            "direction": "pftl_to_lightning",
            "amount_msat": "10000",
            "wallet_address": "pf" + "22" * 20,
            "client_request_id": "33" * 32,
        }
        with self.assertRaisesRegex(ApiError, "off-ramp"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/quotes",
                self.auth,
                json.dumps(base).encode(),
                principal="127.0.0.5",
            )
        duplicate = (
            '{"direction":"lightning_to_pftl","direction":"pftl_to_lightning",'
            '"amount_msat":"10000","wallet_address":"pf'
            + "22" * 20
            + '","client_request_id":"'
            + "33" * 32
            + '"}'
        ).encode()
        with self.assertRaisesRegex(ApiError, "duplicate JSON"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/quotes",
                self.auth,
                duplicate,
                principal="127.0.0.6",
            )

    def test_quote_requires_browser_headers_and_canonical_decimal_amount(self) -> None:
        request = {
            "direction": "lightning_to_pftl",
            "amount_msat": "10000",
            "wallet_address": "pf" + "22" * 20,
            "client_request_id": "33" * 32,
        }
        missing_csrf = dict(self.auth)
        del missing_csrf["x-postfiat-csrf"]
        with self.assertRaisesRegex(ApiError, "browser request"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/quotes",
                missing_csrf,
                json.dumps(request).encode(),
                principal="127.0.0.7",
            )
        request["amount_msat"] = "010000"
        with self.assertRaisesRegex(ApiError, "canonical positive"):
            self.api.dispatch(
                "POST",
                f"{API_PREFIX}/quotes",
                self.auth,
                json.dumps(request).encode(),
                principal="127.0.0.8",
            )


if __name__ == "__main__":
    unittest.main()

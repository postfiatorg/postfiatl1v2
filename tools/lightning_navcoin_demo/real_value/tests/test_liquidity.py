from __future__ import annotations

import io
import json
import unittest

from ..liquidity import (
    MAGMA_INFO_URLS,
    LiquidityDiscoveryError,
    read_magma_capabilities,
)


def capability() -> dict[str, object]:
    return {
        "min_required_channel_confirmations": 0,
        "min_funding_confirms_within_blocks": 6,
        "supports_zero_channel_reserve": False,
        "max_channel_expiry_blocks": 12_960,
        "min_initial_client_balance_sat": "0",
        "max_initial_client_balance_sat": "0",
        "min_initial_lsp_balance_sat": "500000",
        "max_initial_lsp_balance_sat": "100000000",
        "min_channel_balance_sat": "500000",
        "max_channel_balance_sat": "100000000",
        "uris": ["02" + "11" * 32 + "@lsp.example:9735"],
    }


class FakeResponse:
    def __init__(self, value: object, *, url: str | None = None) -> None:
        self.body = io.BytesIO(json.dumps(value).encode())
        self.url = url or MAGMA_INFO_URLS["recommended"]
        self.headers = {"Content-Length": str(len(self.body.getvalue()))}

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *_exc: object) -> None:
        return None

    def geturl(self) -> str:
        return self.url

    def getcode(self) -> int:
        return 200

    def read(self, size: int) -> bytes:
        return self.body.read(size)


class LiquidityTests(unittest.TestCase):
    def test_read_only_capability_and_provider_funded_inbound(self) -> None:
        observed = []

        def opener(request: object, **kwargs: object) -> FakeResponse:
            observed.append((request, kwargs))
            return FakeResponse(capability())

        result = read_magma_capabilities(opener=opener)
        self.assertTrue(result.supports_provider_funded_inbound(500_000))
        self.assertFalse(result.supports_provider_funded_inbound(499_999))
        self.assertEqual(result.public_evidence()["order_created"], False)
        self.assertEqual(len(observed), 1)
        self.assertEqual(observed[0][0].method, "GET")

    def test_malformed_range_and_redirect_fail_closed(self) -> None:
        malformed = capability()
        malformed["min_initial_lsp_balance_sat"] = "100000001"
        with self.assertRaisesRegex(LiquidityDiscoveryError, "inverted"):
            read_magma_capabilities(
                opener=lambda *_args, **_kwargs: FakeResponse(malformed)
            )
        with self.assertRaisesRegex(LiquidityDiscoveryError, "redirected"):
            read_magma_capabilities(
                opener=lambda *_args, **_kwargs: FakeResponse(
                    capability(), url="https://evil.example/get_info"
                )
            )

    def test_unknown_pool_and_noncanonical_amount_reject(self) -> None:
        with self.assertRaisesRegex(LiquidityDiscoveryError, "unsupported"):
            read_magma_capabilities(pool="random")
        value = capability()
        value["min_initial_lsp_balance_sat"] = "0500000"
        with self.assertRaisesRegex(LiquidityDiscoveryError, "canonical"):
            read_magma_capabilities(
                opener=lambda *_args, **_kwargs: FakeResponse(value)
            )


if __name__ == "__main__":
    unittest.main()

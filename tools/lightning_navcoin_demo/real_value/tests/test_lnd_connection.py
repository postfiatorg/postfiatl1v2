from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from ..lnd_connection import (
    CONNECTION_SCHEMA,
    LndConnectionError,
    MainnetLndConnection,
    _read_pinned_file,
)


def connection_mapping(endpoint: str = "127.0.0.1:11009") -> dict[str, object]:
    return {
        "schema": CONNECTION_SCHEMA,
        "endpoint": endpoint,
        "tls_server_name": "localhost",
        "tls_cert_path": "/tmp/lnd-tls.cert",
        "tls_cert_sha256": "11" * 32,
        "macaroon_path": "/tmp/lnd.macaroon",
        "macaroon_sha256": "22" * 32,
        "macaroon_profile": "LIGHTNING_NAVCOIN_RECEIVE_ONLY_V1",
        "ready_timeout_seconds": 10,
    }


class LndConnectionTests(unittest.TestCase):
    def test_connection_accepts_only_explicit_loopback_grpc(self) -> None:
        parsed = MainnetLndConnection.from_mapping(connection_mapping())
        self.assertEqual(parsed.endpoint, "127.0.0.1:11009")
        for endpoint in (
            "203.0.113.4:11009",
            "lnd.example:11009",
            "user@127.0.0.1:11009",
            "127.0.0.1",
            "https://127.0.0.1:11009",
        ):
            with self.subTest(endpoint=endpoint):
                with self.assertRaisesRegex(LndConnectionError, "loopback"):
                    MainnetLndConnection.from_mapping(
                        connection_mapping(endpoint)
                    )

    def test_connection_rejects_legacy_payment_capable_macaroon_profile(
        self,
    ) -> None:
        mapping = connection_mapping()
        mapping["macaroon_profile"] = "LIGHTNING_NAVCOIN_LEAST_PRIVILEGE_V1"
        with self.assertRaisesRegex(LndConnectionError, "receive-only"):
            MainnetLndConnection.from_mapping(mapping)

    def test_pinned_credentials_reject_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "credential"
            target.write_bytes(b"credential")
            target.chmod(0o600)
            link = root / "credential-link"
            link.symlink_to(target)
            with self.assertRaisesRegex(LndConnectionError, "non-symlink"):
                _read_pinned_file(
                    link,
                    expected_sha256="00" * 32,
                    name="test credential",
                    private=True,
                )


if __name__ == "__main__":
    unittest.main()

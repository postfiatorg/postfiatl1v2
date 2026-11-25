import json
import pytest

from pftl_schwab_tee.nitro_oauth import NitroSchwabTokenProvider
from pftl_schwab_tee.nitro_transport import NitroHttpResponse


class Transport:
    host = "api.schwabapi.com"
    max_response_bytes = 64 * 1024
    def __init__(self):
        self.calls = []
        self.response = {"access_token": "synthetic-access", "refresh_token": "synthetic-rotated",
                         "token_type": "Bearer", "expires_in": 1800}
    def request(self, method, path, **kwargs):
        self.calls.append((method, path, kwargs))
        return NitroHttpResponse(200, {}, json.dumps(self.response).encode())


def credentials():
    return {"schema": "postfiat.yolo.schwab_credentials.v1", "clientId": "synthetic-client",
            "clientSecret": "synthetic-secret", "refreshToken": "synthetic-refresh",
            "accountApplicationIdentitySha256": "ab" * 32}


def test_refresh_is_fixed_route_and_rotation_is_persisted_before_use():
    transport, saved = Transport(), []
    token = NitroSchwabTokenProvider(transport, load_credentials=credentials,
        persist_credentials=saved.append, account_identity="ab" * 32)
    assert token() == "synthetic-access"
    assert saved[0]["refreshToken"] == "synthetic-rotated"
    assert token() == "synthetic-access"
    assert transport.calls[1][2]["body"] == b"grant_type=refresh_token&refresh_token=synthetic-rotated"
    assert all(call[:2] == ("POST", "/v1/oauth/token") for call in transport.calls)


@pytest.mark.parametrize("field,value", [("access_token", "bad\r\ntoken"), ("token_type", "Other"),
    ("expires_in", True), ("expires_in", 0), ("refresh_token", "")])
def test_invalid_refresh_response_cannot_escape_or_persist(field, value):
    transport, saved = Transport(), []
    transport.response[field] = value
    token = NitroSchwabTokenProvider(transport, load_credentials=credentials,
        persist_credentials=saved.append, account_identity="ab" * 32)
    with pytest.raises(ValueError, match="invalid Schwab token response"): token()
    assert not saved


def test_failed_rotation_archive_does_not_release_an_access_token():
    def fail(value): raise OSError("synthetic archive error")
    token = NitroSchwabTokenProvider(Transport(), load_credentials=credentials,
        persist_credentials=fail, account_identity="ab" * 32)
    with pytest.raises(OSError): token()


def test_wrong_identity_and_destination_reject_before_credentials_are_sent():
    transport = Transport()
    token = NitroSchwabTokenProvider(transport, load_credentials=credentials,
        persist_credentials=lambda _: None, account_identity="cd" * 32)
    with pytest.raises(ValueError): token()
    assert not transport.calls
    transport.host = "example.com"
    token.load_credentials = lambda: pytest.fail("must not load credentials")
    with pytest.raises(ValueError): token()


def test_optional_refresh_token_retains_existing_token_per_oauth_protocol():
    transport, saved = Transport(), []
    del transport.response["refresh_token"]
    token = NitroSchwabTokenProvider(transport, load_credentials=credentials,
        persist_credentials=saved.append, account_identity="ab" * 32)
    assert token() == "synthetic-access"
    assert saved[0]["refreshToken"] == "synthetic-refresh"

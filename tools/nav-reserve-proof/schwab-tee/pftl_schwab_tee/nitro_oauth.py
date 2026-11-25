"""Fixed Schwab refresh-token exchange inside enclave-owned TLS."""

import base64
import json
from urllib.parse import urlencode

from .nitro_run import strict


class NitroSchwabTokenProvider:
    def __init__(self, transport, *, load_credentials, persist_credentials, account_identity):
        self.transport = transport
        self.load_credentials = load_credentials
        self.persist_credentials = persist_credentials
        self.account_identity = account_identity
        self.credentials = None

    def __call__(self):
        if self.transport.host != "api.schwabapi.com" or not 0 < self.transport.max_response_bytes <= 64 * 1024:
            raise ValueError("Schwab OAuth requires its fixed bounded TLS endpoint")
        if self.credentials is None:
            value = strict(self.load_credentials(), {"schema", "clientId", "clientSecret",
                "refreshToken", "accountApplicationIdentitySha256"}, "Schwab credential envelope")
            if (value["schema"] != "postfiat.yolo.schwab_credentials.v1"
                    or value["accountApplicationIdentitySha256"] != self.account_identity):
                raise ValueError("Schwab credential identity differs from the epoch")
            for name in ("clientId", "clientSecret", "refreshToken"):
                self._token(value[name])
            if ":" in value["clientId"]:
                raise ValueError("invalid OAuth client identifier")
            self.credentials = dict(value)
        value = self.credentials
        basic = base64.b64encode((value["clientId"] + ":" + value["clientSecret"]).encode()).decode()
        response = self.transport.request("POST", "/v1/oauth/token",
            headers={"Authorization": "Basic " + basic,
                     "Content-Type": "application/x-www-form-urlencoded"},
            body=urlencode({"grant_type": "refresh_token", "refresh_token": value["refreshToken"]}).encode())
        if response.status_code != 200:
            raise RuntimeError(f"Schwab token exchange returned HTTP {response.status_code}")
        if len(response.content) > 64 * 1024:
            raise ValueError("Schwab token response exceeds its bound")
        def object_pairs(pairs):
            result = {}
            for key, item in pairs:
                if key in result: raise ValueError("duplicate token field")
                result[key] = item
            return result
        try:
            result = json.loads(response.content, object_pairs_hook=object_pairs)
            if not isinstance(result, dict): raise ValueError()
            token = self._token(result["access_token"])
            if (not isinstance(result["token_type"], str) or result["token_type"].lower() != "bearer"
                    or type(result["expires_in"]) is not int or result["expires_in"] <= 0):
                raise ValueError()
            # RFC 6749 section 6 permits a refresh response to omit a replacement.
            replacement = self._token(result.get("refresh_token", value["refreshToken"]))
        except (KeyError, TypeError, ValueError, UnicodeDecodeError, RecursionError):
            raise ValueError("invalid Schwab token response") from None
        refreshed = {**value, "refreshToken": replacement}
        # Persist rotation encrypted before making an access token usable.
        self.persist_credentials(refreshed)
        self.credentials = refreshed
        return token

    @staticmethod
    def _token(value):
        if (not isinstance(value, str) or not 0 < len(value) <= 16 * 1024
                or any(ord(char) < 33 or ord(char) > 126 for char in value)):
            raise ValueError("invalid credential/token encoding")
        return value

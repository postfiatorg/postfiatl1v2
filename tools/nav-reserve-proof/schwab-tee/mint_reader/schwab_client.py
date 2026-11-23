"""Read-only Schwab Trader API client for the B24 mint reader.

Same token handling as the broker client it replaces: reuse the stored access
token for 25 minutes, otherwise refresh with the stored refresh token and write
the refreshed token file atomically with owner-only permissions. The token and
app files stay where the existing Schwab login keeps them on the reading host;
the refresh token rotates, so every reader must share that one file.
"""

from __future__ import annotations

import base64
import json
import os
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

SCHWAB_TOKEN_PATH = Path.home() / ".config" / "navstrategies" / "schwab_tokens.json"
SCHWAB_CREDENTIALS_PATH = Path.home() / "repos" / "schwab.txt"
SCHWAB_API_ROOT = "https://api.schwabapi.com"
TOKEN_REUSE = timedelta(minutes=25)
TIMEOUT_SECONDS = 20


def _read_key_value_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def _atomic_private_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(f"{path.suffix}.tmp")
    temporary.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    os.chmod(temporary, 0o600)
    temporary.replace(path)
    os.chmod(path, 0o600)


def _obtained(token: dict[str, Any]) -> datetime | None:
    value = token.get("obtained_at_utc")
    if not isinstance(value, str):
        return None
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)


class SchwabClient:
    def __init__(self, token_path: Path = SCHWAB_TOKEN_PATH, credentials_path: Path = SCHWAB_CREDENTIALS_PATH):
        self.token_path = token_path
        self.credentials_path = credentials_path

    def _access_token(self, *, force_refresh: bool = False) -> str:
        token = json.loads(self.token_path.read_text(encoding="utf-8"))
        obtained = _obtained(token)
        stale = obtained is None or datetime.now(timezone.utc) >= obtained + TOKEN_REUSE
        if not force_refresh and not stale and token.get("access_token"):
            return str(token["access_token"])
        credentials = _read_key_value_file(self.credentials_path)
        basic = base64.b64encode(f"{credentials['client_id']}:{credentials['client_secret']}".encode()).decode()
        request = urllib.request.Request(
            f"{SCHWAB_API_ROOT}/v1/oauth/token",
            data=urllib.parse.urlencode({"grant_type": "refresh_token", "refresh_token": token["refresh_token"]}).encode(),
            headers={"Authorization": f"Basic {basic}", "Content-Type": "application/x-www-form-urlencoded"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
            refreshed = json.loads(response.read())
        refreshed["obtained_at_utc"] = datetime.now(timezone.utc).isoformat()
        _atomic_private_json(self.token_path, refreshed)
        return str(refreshed["access_token"])

    def request(self, path: str, *, params: dict[str, Any] | None = None) -> Any:
        query = ("?" + urllib.parse.urlencode(params)) if params else ""
        for attempt in range(2):
            token = self._access_token(force_refresh=bool(attempt))
            request = urllib.request.Request(f"{SCHWAB_API_ROOT}{path}{query}", headers={"Authorization": f"Bearer {token}"})
            try:
                with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
                    return json.loads(response.read())
            except urllib.error.HTTPError as error:
                if error.code not in {400, 401} or attempt:
                    raise
        raise RuntimeError("Schwab request failed")
